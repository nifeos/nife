//! **`terminal_supervisor`: holds the terminal's components, and replaces them live** (milestone
//! 23 (a capability-routed component OS with live replacement); calef's ruling of 2026-09-27,
//! option A of `design/roadmap/0661-a-terminal-supervisor-holds-the-line-editor.md`, and his
//! rulings of 2026-10-19 (UTC) on `display_terminal`).
//!
//! A component's spawner supervises its swaps (calef, 2026-10-19). `system_initializer` builds this
//! rather than `line_editor`, and hands it the objects a `line_editor` is made of plus a budget for
//! two instances. In display mode a spawner may also hand it `display_terminal`'s image and the
//! display endpoint and scanout, and then it builds and holds `display_terminal` too. `SWAP`'s
//! second word names the child. It runs the swap of DECISIONS §209 (state handoff is an opaque blob
//! over a granted frame, and it is optional):
//!
//! ```text
//!   1 BUILT      lay the replacement out, wired like the incumbent, not started.
//!   2 QUIESCED   OPERATION_QUIESCE, answered within QUIESCE_BOUND_MS. The endpoint's FIFO drains
//!                everything ahead; a parked reader is answered FLAG_RETRY; the blob is written.
//!   3 ABSORBING  start the replacement with START_ABSORB. It reads the blob before it serves.
//!   4 COMMIT     NOTE_ABSORBED: tell the incumbent CTL_QUIT and collect it.
//!     or UNDO    NOTE_NOT_ABSORBED: collect the refuser and tell the incumbent CTL_RESUME.
//!   2' HUNG      no answer: start the replacement empty, destroy the incumbent's region (its
//!                callers are answered Gone), and reply REPLACED_HUNG.
//! ```
//!
//! `line_editor`'s budget holds two bays and a swap moves it between them. `display_terminal`'s
//! holds one: its swap is lent the machine's swap reserve (`supervisor::SWAP_RESERVE_PAGES`) with
//! `LEND`, and moves into it and home again, answering `NEEDS_RESERVE` when nothing was lent. A
//! `CALL` takes no deadline, so a stand-in built from this program's own image makes each quiesce
//! and signals a notification the supervisor's timer also signals (notes/hung-component.md).
//!
//! # BUGS
//!
//! Nothing on a real boot asks it to swap or holds the reserve: milestone 694 (the installer asks
//! the terminal to swap). A session's `display_terminal` is not supervised. A hung child's state is
//! lost; a slow one is convicted as hung; one that faults is not restarted. A `display_terminal`
//! that refuses its own blob on the move home serves from the reserve, answered `IN_RESERVE`.
//! notes/interactive-stack-swap.md has each in full.
//!
//! Name: ratified 2026-10-06 (calef, "Ratify" it, in conversation). Minted
//! provisionally 2026-09-27 by the lane for milestone 23.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use capability_plan::Provisions;
use line_editor::component::{self, supervisor as s};
use line_editor::proto;
use supervision_protocol::{Child as Built, ChildEndowment, Retention};
use user_mode_runtime::{
    Delivered, cap_delete as delete_capability, cntfrq, notification_poll, notification_signal,
    notification_wait, now, receive, receive_fault, receive_request, reply, send, send_capability,
    timer_arm,
};
use video_terminal::component as display;

const MODE_DISPLAY: u64 = 1;
/// The first start argument that makes this image a stand-in rather than a supervisor. Clear of
/// `line_editor`'s own sink modes, `0` and `1`, which share the argument.
const MODE_STAND_IN: u64 = 2;

/// A stand-in's slots: the terminal endpoint it quiesces, and the notification it answers on.
const STAND_IN_TERMINAL: u64 = 0;
const STAND_IN_ANSWER: u64 = 1;
/// A stand-in makes three syscalls from `_start`'s own frame.
const STAND_IN_STACK_PAGES: u64 = 4;
/// The bits of the quiesce's notification: the timer's deadline, and the stand-in's two answers.
const DEADLINE: u64 = 1 << 0;
const ANSWERED_QUIESCED: u64 = 1 << 1;
const ANSWERED_OTHERWISE: u64 = 1 << 2;

/// The reserve is exactly what a `display_terminal` swap takes, the largest swap there is.
const _: () = assert!(display::SWAP_PAGES == s::SWAP_RESERVE_PAGES);

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, lengths: u64, swap: u64) -> ! {
    if mode == MODE_STAND_IN {
        stand_in()
    }
    supervise(mode, lengths, swap)
}

/// **The stand-in: one `CALL` the supervisor cannot make itself**, because it has to be able to
/// stop waiting for the answer. Its whole authority is `WRITE` on the endpoint it quiesces and on
/// the supervisor's notification. A hung incumbent leaves it blocked here until the supervisor
/// destroys its region, which finishes it in place and unlinks its request from the endpoint's
/// queue (milestone 133 (ending a permanently blocked thread, and deciding who may)).
fn stand_in() -> ! {
    let (q, _) = user_mode_runtime::call(
        STAND_IN_TERMINAL,
        proto::req(proto::OPERATION_QUIESCE, 0),
        0,
    );
    let answer = if q == proto::QUIESCED {
        ANSWERED_QUIESCED
    } else {
        ANSWERED_OTHERWISE
    };
    let _ = notification_signal(STAND_IN_ANSWER, answer);
    user_mode_runtime::exit()
}

/// An image the spawner copied into this address space at `va`, `len` bytes long.
fn image_at(va: u64, len: u64) -> Option<elf::Elf<'static>> {
    // SAFETY: the builder copied the image to `va`, at the length it packed, before starting us,
    // and nothing writes it afterwards. Each image has its own window below the scratch window
    // (`supervisor::ELF_VA` and its neighbors).
    let bytes = unsafe { core::slice::from_raw_parts(va as *const u8, len as usize) };
    elf::Elf::parse(bytes).ok()
}

#[inline(never)]
fn supervise(mode: u64, lengths: u64, swap: u64) -> ! {
    let (editor_len, own_len, display_len) = s::unpack_image_lengths(lengths);
    let Some(editor_image) = image_at(s::ELF_VA, editor_len) else {
        fail()
    };
    // Only a supervisor that can be asked to swap ever quiesces, so only its spawner copies our own
    // image (`supervisor::OWN_IMAGE_ADDRESS`). One that passed a swap endpoint without it is
    // refused here, at start, rather than at the first swap.
    let own = (own_len != 0)
        .then(|| image_at(s::OWN_IMAGE_ADDRESS, own_len))
        .flatten();
    let deadline = match (swap, own) {
        (0, _) => None,
        (_, Some(own)) => Some(Deadline {
            own,
            wake: obj(abi::objtype::NOTIFICATION),
            timer: obj(abi::objtype::TIMER),
            bay: bay(s::STAND_IN_PAGES),
        }),
        (_, None) => fail(),
    };
    let sup = Supervisor { deadline };

    let decl = if mode == MODE_DISPLAY {
        &component::DISPLAY
    } else {
        &component::CONSOLE
    };
    let handoff =
        match user_mode_runtime::retype_page_frame_run(s::BUDGET, component::HANDOFF_PAGES) {
            f if f >= 0 => f as u64,
            _ => fail(),
        };

    // `display_terminal` first, so the sink `line_editor` prints through is being served before
    // `line_editor` exists.
    let mut display = match display_len {
        0 => None,
        _ if mode != MODE_DISPLAY => fail(),
        len => {
            let Some(image) = image_at(s::DISPLAY_IMAGE_ADDRESS, len) else {
                fail()
            };
            let mut d = Child::new(image, Kind::DisplayTerminal, s::SINK, display::BAY_PAGES);
            d.launch_first();
            Some(d)
        }
    };
    let mut editor = Child::new(
        editor_image,
        Kind::LineEditor {
            decl,
            mode,
            control_in_child: capability_plan::slot_of(decl, "control"),
            handoff,
        },
        s::TERMINAL,
        decl.pages,
    );
    editor.launch_first();

    if swap == 0 {
        // No trigger on this boot (see BUGS). Hold everything and wait, so the objects stay ours.
        loop {
            let _ = receive(editor.faultep);
        }
    }

    // The swap reserve, while a lender has lent it and the swap it was lent for has not ended.
    let mut lent: Option<u64> = None;
    loop {
        let req = receive_request(swap);
        let caller = match req.delivered {
            Delivered::Reply(caller) => caller,
            Delivered::Delegation(reserve) if req.w0 == s::LEND => {
                if let Some(stale) = lent.replace(reserve) {
                    delete_capability(stale);
                }
                continue;
            }
            // Anything else is not ours to keep (milestone 706 (a `CALL` server can tell a Reply
            // from a delegation)).
            other => {
                let _ = other.into_reply();
                continue;
            }
        };
        if req.w0 == s::STOP {
            sup.stop(caller, &mut editor, display.as_mut(), lent)
        }
        let (r0, r1) = match (req.w0, req.w1, display.as_mut()) {
            (s::SWAP, s::CHILD_LINE_EDITOR, _) => editor.swap_in_bays(&sup),
            (s::SWAP, s::CHILD_DISPLAY_TERMINAL, Some(d)) => {
                d.swap_through_reserve(&sup, &mut lent)
            }
            _ => (proto::BAD_REQUEST, 0),
        };
        reply(caller, r0, r1);
    }
}

/// What every quiesce is made with: `Some` whenever there is a swap endpoint, which is the only way
/// a quiesce is reached.
struct Supervisor<'a> {
    deadline: Option<Deadline<'a>>,
}

/// Which of the two programs a [`Child`] is, and what only that one needs.
enum Kind {
    /// Built from its `capability_plan` declaration, with its one-page handoff mapped at build in
    /// every instance.
    LineEditor {
        decl: &'static capability_plan::Requirements,
        mode: u64,
        control_in_child: u64,
        handoff: u64,
    },
    /// Built at `video_terminal::component`'s slots, with a map budget per instance and a handoff
    /// run that exists only during a swap.
    DisplayTerminal,
}

/// **One supervised component**: its image, the objects this supervisor made for it, and which
/// place holds the live instance.
struct Child<'a> {
    image: elf::Elf<'a>,
    kind: Kind,
    /// The endpoint it serves, which a quiesce is sent on.
    served: u64,
    /// Its own supervision endpoint, so a death collected is always this child's.
    faultep: u64,
    controls: [u64; 2],
    /// Where its instances answer the supervisor: `NOTE_*`, and `display_terminal`'s `TERM_UP`.
    notify: u64,
    /// The regions instances are split from. `line_editor`: two bays in the budget.
    /// `display_terminal`: its bay, and the lent reserve while a swap holds it.
    places: [u64; 2],
    /// The region of the instance in each place, `0` for none. Kept, unlike any other child's,
    /// because destroying it is the one way to end an instance that will never cooperate.
    instances: [u64; 2],
    /// `display_terminal`: each instance's map budget, split from its place beside it.
    budgets: [u64; 2],
    /// `display_terminal`: the handoff run retyped from the reserve, `0` outside a swap.
    run: u64,
    /// `display_terminal`: the instance in each place maps [`Self::run`], so it is told
    /// `CTL_HANDOFF` without a capability.
    maps_run: [bool; 2],
    /// Which place, and which control endpoint, belong to the live instance.
    current: usize,
    /// How many instances have been started, reported back on `SWAPPED`.
    started: u64,
}

/// What one move from a place to the other found.
enum Hop {
    /// The replacement absorbed the state and serves; the incumbent is gone.
    Swapped,
    /// The replacement refused or the blob could not be written, and the incumbent serves on.
    RolledBack(u64),
    /// The incumbent did not answer the quiesce in time, waited this many milliseconds; the
    /// replacement serves empty.
    Hung(u64),
}

/// An instance laid out and not yet started.
struct Laid {
    child: Built,
    aspace: u64,
    region: u64,
    budget: u64,
}

impl<'a> Child<'a> {
    fn new(image: elf::Elf<'a>, kind: Kind, served: u64, bay_pages: u64) -> Self {
        let second_bay = match kind {
            Kind::LineEditor { .. } => bay(bay_pages),
            Kind::DisplayTerminal => 0,
        };
        Child {
            image,
            kind,
            served,
            faultep: obj(abi::objtype::RENDEZVOUS),
            controls: [obj(abi::objtype::RENDEZVOUS), obj(abi::objtype::RENDEZVOUS)],
            notify: obj(abi::objtype::RENDEZVOUS),
            // **Bays, one per instance, split once and reused.** A child's region comes home to
            // the region it was split from, and only un-bumps that region's watermark when it is
            // on top. Built straight from the budget, the incumbent's region is always under its
            // replacement's when it dies, so every swap would leave a hole the size of an
            // instance: the second swap failed for want of budget, which is how this was found.
            places: [bay(bay_pages), second_bay],
            instances: [0, 0],
            budgets: [0, 0],
            run: 0,
            maps_run: [false, false],
            current: 0,
            started: 0,
        }
    }

    /// Lay an instance out in place `p`, wired to that place's control endpoint, not started.
    /// `None` when the place cannot hold it, which for the reserve is a lender's mistake.
    fn lay_out(&self, p: usize) -> Option<Laid> {
        let place = self.places[p];
        let budget = match self.kind {
            Kind::LineEditor { .. } => 0,
            Kind::DisplayTerminal => {
                supervision_protocol::memory_region_split(place, display::MAP_BUDGET_PAGES).ok()?
            }
        };
        let laid = match &self.kind {
            Kind::LineEditor { .. } => {
                let plan = self.plan_for(self.controls[p]);
                let region = supervision_protocol::memory_region_split(place, plan.pages()).ok()?;
                let endow = ChildEndowment {
                    caps: plan.caps(),
                    maps: plan.maps(),
                    blobs: &[],
                    fault: Some(self.faultep),
                    stack_pages: component::STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                };
                supervision_protocol::build_child_space(s::BUDGET, region, &self.image, &endow)
                    .ok()
                    .map(|(child, aspace)| (child, aspace, region))
            }
            Kind::DisplayTerminal => {
                let rw = abi::rights::READ | abi::rights::WRITE;
                // `video_terminal::component`'s slots, in order.
                let capabilities = [
                    (self.notify, abi::rights::WRITE),
                    (s::PRESENT, abi::rights::WRITE),
                    (self.served, abi::rights::READ),
                    (budget, abi::rights::SPEND_ONLY),
                    (s::SURFACE, rw),
                    (s::SINK_PAGE, rw),
                    (self.controls[p], abi::rights::READ),
                    (self.notify, abi::rights::WRITE),
                ];
                let endow = ChildEndowment {
                    caps: &capabilities,
                    fault: Some(self.faultep),
                    ..ChildEndowment::new(Retention::Nothing)
                };
                supervision_protocol::memory_region_split(place, display::INSTANCE_PAGES)
                    .ok()
                    .and_then(|region| {
                        supervision_protocol::build_child_space(
                            s::BUDGET,
                            region,
                            &self.image,
                            &endow,
                        )
                        .ok()
                        .map(|(child, aspace)| (child, aspace, region))
                    })
            }
        };
        match laid {
            Some((child, aspace, region)) => Some(Laid {
                child,
                aspace,
                region,
                budget,
            }),
            None => {
                // Whatever was split is given back; a region that never held a thread destroys.
                give_back(budget);
                None
            }
        }
    }

    /// The plan for a `line_editor` whose control endpoint is `control`.
    fn plan_for(&self, control: u64) -> capability_plan::Plan {
        let Kind::LineEditor { decl, handoff, .. } = self.kind else {
            fail()
        };
        // The sink's reply endpoint is routed whether or not the declaration names it: a provision
        // nothing asks for is not an error, and one table serves both modes.
        let held: [(&'static str, u64); 9] = [
            ("terminal", s::TERMINAL),
            ("sink", s::SINK),
            ("sink_reply", s::SINK_REPLY),
            ("control", control),
            ("supervisor", self.notify),
            ("sink_page", s::SINK_PAGE),
            ("client_out", s::CLIENT_OUT),
            ("client_in", s::CLIENT_IN),
            (capability_plan::STATE_HANDOFF_ROLE, handoff),
        ];
        let Ok(plan) = capability_plan::plan(decl, &Provisions { held: &held }) else {
            fail()
        };
        plan
    }

    /// Start what [`Child::lay_out`] laid out in place `p`, absorbing the incumbent's state or not.
    fn start(&mut self, p: usize, laid: &Laid, absorb: bool) {
        let absorb = if absorb { proto::START_ABSORB } else { 0 };
        let (a0, a1, a2) = match self.kind {
            Kind::LineEditor {
                mode,
                control_in_child,
                ..
            } => (mode, control_in_child, proto::START_HANDOFF | absorb),
            Kind::DisplayTerminal => (
                video_terminal::status::MODE_DISPLAY,
                display::CONTROL,
                absorb,
            ),
        };
        if supervision_protocol::configure_child(laid.child.tcb, laid.aspace, self.image.entry())
            .is_err()
            || !supervision_protocol::start_child(laid.child, a0, a1, a2)
        {
            fail()
        }
        self.instances[p] = laid.region;
        self.budgets[p] = laid.budget;
        self.started += 1;
    }

    /// Build and start the first instance, in place 0.
    fn launch_first(&mut self) {
        let Some(laid) = self.lay_out(0) else { fail() };
        self.start(0, &laid, false);
        self.wait_up();
    }

    /// `display_terminal` says `TERM_UP` once it has painted, a blocking `SEND`; take it.
    fn wait_up(&self) {
        if matches!(self.kind, Kind::DisplayTerminal)
            && receive(self.notify).0 != video_terminal::status::TERM_UP
        {
            fail()
        }
    }

    /// Collect the instance in place `p`, which has been told to quit or has refused, and give
    /// its place back.
    fn collect(&mut self, p: usize) {
        collect(self.faultep, self.instances[p]);
        self.instances[p] = 0;
        give_back(self.budgets[p]);
        self.budgets[p] = 0;
        self.maps_run[p] = false;
    }

    /// End the instance in place `p` whatever it is doing.
    fn destroy(&mut self, p: usize) {
        give_back(self.instances[p]);
        self.instances[p] = 0;
        give_back(self.budgets[p]);
        self.budgets[p] = 0;
        self.maps_run[p] = false;
    }

    /// `display_terminal`: tell the instance in place `p` `CTL_HANDOFF`, with the run unless it
    /// already maps it. `true` once it is mapped there.
    fn hand_run(&mut self, p: usize) -> bool {
        let sent = if self.maps_run[p] {
            send(self.controls[p], proto::CTL_HANDOFF, 0, 0)
        } else {
            send_capability(
                self.controls[p],
                self.run,
                abi::rights::READ | abi::rights::WRITE,
                proto::CTL_HANDOFF,
            )
        };
        self.maps_run[p] = sent >= 0;
        self.maps_run[p]
    }

    /// **Move the live instance to the other place**: steps 1 to 4 of the module's table, or 2'.
    fn hop(&mut self, sup: &Supervisor) -> Option<Hop> {
        let (cur, next) = (self.current, 1 - self.current);
        let laid = self.lay_out(next)?;
        let display = matches!(self.kind, Kind::DisplayTerminal);

        let waited_ms = match sup.quiesce(self.served, self.faultep) {
            Quiesce::Answered => None,
            Quiesce::Unanswered { waited_ms } => Some(waited_ms),
        };
        if let Some(waited_ms) = waited_ms {
            // A hung incumbent wrote no blob, so the replacement starts empty. Started before the
            // incumbent is destroyed, so a caller freed below finds a server when it asks again
            // rather than parking in the down window. `DESTROY` finishes the blocked incumbent
            // where it sits (milestone 133) and answers every caller it held a reply capability
            // for with `Gone` (milestone 254 (a caller stranded by a server that died)). A
            // finished thread sends no death message, so there is nothing to collect.
            self.start(next, &laid, false);
            self.destroy(cur);
            self.current = next;
            self.wait_up();
            return Some(Hop::Hung(waited_ms));
        }

        if display {
            // The incumbent writes its blob into the run now, since the run did not exist when it
            // was built.
            let written = self.hand_run(cur) && receive(self.notify).0 == proto::NOTE_WRITTEN;
            if !written {
                // The replacement was laid out and never started: its region destroys whole.
                delete_capability(laid.child.tcb);
                delete_capability(laid.aspace);
                give_back(laid.region);
                give_back(laid.budget);
                send(self.controls[cur], proto::CTL_RESUME, 0, 0);
                return Some(Hop::RolledBack(0));
            }
        }
        self.start(next, &laid, true);
        if display && !self.hand_run(next) {
            fail()
        }
        let (note, why, _) = receive(self.notify);
        if note != proto::NOTE_ABSORBED {
            self.collect(next);
            send(self.controls[cur], proto::CTL_RESUME, 0, 0);
            return Some(Hop::RolledBack(why));
        }
        send(self.controls[cur], proto::CTL_QUIT, 0, 0);
        self.collect(cur);
        self.current = next;
        self.wait_up();
        Some(Hop::Swapped)
    }

    /// `SWAP` of `line_editor`: one move, from one bay to the other.
    fn swap_in_bays(&mut self, sup: &Supervisor) -> (u64, u64) {
        match self.hop(sup) {
            Some(Hop::Swapped) => (s::SWAPPED, self.started),
            Some(Hop::RolledBack(why)) => (s::ROLLED_BACK, why),
            Some(Hop::Hung(waited)) => (s::REPLACED_HUNG, waited),
            None => fail(),
        }
    }

    /// **`SWAP` of `display_terminal`: into the lent reserve, then home to the bay**, so the
    /// reserve is free again when the answer goes back. `NEEDS_RESERVE`, retryable, when nothing
    /// was lent or what was lent cannot hold an instance and its handoff.
    fn swap_through_reserve(&mut self, sup: &Supervisor, lent: &mut Option<u64>) -> (u64, u64) {
        let Some(reserve) = *lent else {
            return (s::NEEDS_RESERVE, 0);
        };
        if self.run == 0 {
            // The run first, so it sits at the reserve's base.
            let run = user_mode_runtime::retype_page_frame_run(
                reserve,
                video_terminal::handoff::HANDOFF_PAGES,
            );
            if run < 0 {
                return (s::NEEDS_RESERVE, 1);
            }
            self.places[1] = reserve;
            self.run = run as u64;
        }
        let out = if self.current == 0 {
            match self.hop(sup) {
                None => (s::NEEDS_RESERVE, 2),
                Some(Hop::RolledBack(why)) => (s::ROLLED_BACK, why),
                Some(Hop::Swapped) => (s::SWAPPED, 0),
                Some(Hop::Hung(waited)) => (s::REPLACED_HUNG, waited),
            }
        } else {
            (s::SWAPPED, 0)
        };
        if self.current == 1 {
            // Home, with the image now running, which reads its own blob.
            match self.hop(sup) {
                Some(Hop::Swapped | Hop::Hung(_)) => {}
                Some(Hop::RolledBack(why)) => return (s::IN_RESERVE, why),
                None => fail(),
            }
        }
        delete_capability(self.run);
        self.run = 0;
        self.places[1] = 0;
        self.maps_run = [false, false];
        delete_capability(reserve);
        *lent = None;
        match out {
            (s::SWAPPED, _) => (s::SWAPPED, self.started),
            other => other,
        }
    }

    /// Retire the live instance (quiesce it, tell it to quit, collect it) and give its places
    /// back.
    fn retire(&mut self, sup: &Supervisor) {
        let cur = self.current;
        match sup.quiesce(self.served, self.faultep) {
            Quiesce::Answered => {
                send(self.controls[cur], proto::CTL_QUIT, 0, 0);
                self.collect(cur);
            }
            Quiesce::Unanswered { .. } => self.destroy(cur),
        }
        // Both places are empty now, and a budget with children cannot be reclaimed by whoever
        // owns it, so give the bays back before going. The reserve is its lender's.
        for (p, place) in self.places.into_iter().enumerate() {
            if p == 0 || matches!(self.kind, Kind::LineEditor { .. }) {
                end_region(place);
            }
        }
    }
}

impl Supervisor<'_> {
    /// `STOP`: retire every child, give the bays back, and exit.
    fn stop(
        &self,
        caller: user_mode_runtime::Reply,
        editor: &mut Child,
        display: Option<&mut Child>,
        lent: Option<u64>,
    ) -> ! {
        // `line_editor` first: it prints through `display_terminal`.
        editor.retire(self);
        if let Some(d) = display {
            d.retire(self);
        }
        if let Some(reserve) = lent {
            delete_capability(reserve);
        }
        if let Some(d) = &self.deadline {
            end_region(d.bay);
        }
        reply(caller, s::STOPPED, editor.started);
        user_mode_runtime::exit()
    }

    /// **`OPERATION_QUIESCE` on `served`, with a deadline a `CALL` cannot have.** Builds a
    /// stand-in, starts it to make the call, arms the timer, and waits on the notification both of
    /// them signal. The stand-in is built before the clock starts, so the bound measures the
    /// incumbent and not a build.
    ///
    /// **The bound, `QUIESCE_BOUND_MS` (250 ms), is about thirty times the slowest healthy answer
    /// measured.** Timed from the stand-in's start to its answer, on 2026-10-19 (UTC), eleven swaps
    /// per run under `script/test`, on a 16 GB Apple Silicon host with a load average of 5 to 6
    /// from other lanes' builds: aarch64 2.0 to 2.8 ms over three runs, riscv64 3.6 to 7.8 ms over
    /// three, `x86_64` 1.9 to 2.6 ms over its two legs of one run. It covers a parked reader
    /// answered `FLAG_RETRY` and the blob written, which `swapper`'s fixture does not have, so it
    /// is this component's bound and not a copy of `swapper`'s. It is wall time, since
    /// `script/test` runs TCG without `-icount`, and the margin is for a loaded host. The deadline
    /// fires at the first 10 ms tick at or after it; the hung test prints the real wait.
    fn quiesce(&self, served: u64, faultep: u64) -> Quiesce {
        let Some(d) = &self.deadline else { fail() };
        let Ok(region) = supervision_protocol::memory_region_split(d.bay, s::STAND_IN_PAGES) else {
            fail()
        };
        let granted = [(served, abi::rights::WRITE), (d.wake, abi::rights::WRITE)];
        let Ok(child) = supervision_protocol::build_child(
            s::BUDGET,
            region,
            &d.own,
            &ChildEndowment {
                caps: &granted,
                fault: Some(faultep),
                stack_pages: STAND_IN_STACK_PAGES,
                ..ChildEndowment::new(Retention::Nothing)
            },
        ) else {
            fail()
        };

        let bound = cntfrq() * s::QUIESCE_BOUND_MS / 1000;
        let started = now();
        if !supervision_protocol::start_child(child, MODE_STAND_IN, 0, 0)
            || timer_arm(d.timer, started + bound, d.wake, DEADLINE) < 0
        {
            fail()
        }
        let word = notification_wait(d.wake);
        if word <= 0 {
            fail()
        }
        let word = word as u64;
        if word & (ANSWERED_QUIESCED | ANSWERED_OTHERWISE) != 0 {
            // `0` means the deadline had already passed and signaled, after the word just taken.
            // Clear it now, or the next quiesce's wait would end on it at once and convict a
            // healthy incumbent.
            if user_mode_runtime::timer_cancel(d.timer) == 0 {
                let _ = notification_poll(d.wake);
            }
            collect(faultep, region); // the stand-in, which exits after its answer
            if word & ANSWERED_QUIESCED == 0 {
                fail()
            }
            return Quiesce::Answered;
        }
        let waited = now() - started;
        // First, because the stand-in's request is still queued on the served endpoint, and the
        // replacement about to serve it would otherwise be quiesced by it. Then clear an answer the
        // stand-in managed to signal before it was finished, which the next quiesce would read.
        give_back(region);
        let _ = notification_poll(d.wake);
        Quiesce::Unanswered {
            waited_ms: waited * 1000 / cntfrq(),
        }
    }
}

/// What a bounded quiesce is made with: this program's own image, which every stand-in is built
/// from, and the notification and timer the wait ends on. Retyped from the budget, so a spawner
/// grants nothing for them.
struct Deadline<'a> {
    own: elf::Elf<'a>,
    wake: u64,
    timer: u64,
    /// **Every stand-in is built in this one bay**, for the reason the children's bays exist. A
    /// stand-in split straight from the budget is under whatever the build retyped from the budget
    /// while it lived (a scratch page table, the first time the loader's cursor reaches a new 2 MiB),
    /// so when it dies it leaves a hole the size of a stand-in. On riscv64 the second display swap
    /// did that, and `STOP` then found no budget for its own stand-in, which is how this was found.
    bay: u64,
}

/// What a bounded quiesce found.
enum Quiesce {
    /// The incumbent answered within the bound, and is drained and waiting for its control word.
    Answered,
    /// The bound passed with no answer.
    Unanswered { waited_ms: u64 },
}

/// **Destroy a region, whatever its resident is doing** (`swapper`'s helper of the same name). A
/// `Blocked` resident is finished in place on the first try. A `Ready` or `Running` one is marked
/// killed and the `DESTROY` refused; it dies at its next preemption, and a retry then finds a
/// corpse and reclaims. `0`, a region never split, is nothing to do.
fn end_region(region: u64) {
    if region == 0 {
        return;
    }
    for _ in 0..1000 {
        if supervision_protocol::memory_region_destroy(region) {
            return;
        }
        user_mode_runtime::yield_now();
    }
    fail()
}

/// [`end_region`], then drop the capability: a supervisor that swaps for the life of a session
/// cannot leak a slot per swap from a 64-slot table.
fn give_back(region: u64) {
    if region != 0 {
        end_region(region);
        delete_capability(region);
    }
}

/// Wait for one death and collect it, so the child's region comes home to the budget, then drop
/// the region capability that was kept for it.
fn collect(faultep: u64, region: u64) {
    let (_event, tid, _pc, _addr, _, _) = receive_fault(faultep);
    if user_mode_runtime::reap(faultep, tid) != 0 {
        fail()
    }
    delete_capability(region);
}

fn bay(pages: u64) -> u64 {
    match supervision_protocol::memory_region_split(s::BUDGET, pages) {
        Ok(slot) => slot,
        Err(_) => fail(),
    }
}

fn obj(objtype: u64) -> u64 {
    match supervision_protocol::retype_obj_from(s::BUDGET, objtype) {
        Ok(slot) => slot,
        Err(()) => fail(),
    }
}

fn fail() -> ! {
    user_mode_runtime::trap()
}

user_mode_runtime::panic_handler!();
