//! **`terminal_supervisor`: holds the line editor, and replaces it live** (milestone 23 (a
//! capability-routed component OS with live replacement); calef's ruling of 2026-09-27, option A of
//! `design/roadmap/0661-a-terminal-supervisor-holds-the-line-editor.md`).
//!
//! `system_initializer` used to build `line_editor` itself and then give away every terminal
//! capability it held, so nothing on a running system could replace the terminal. Now it builds
//! this instead and hands it the objects a `line_editor` is made of, plus a budget for two
//! instances. This program builds `line_editor` from the declaration in `line_editor::component`,
//! keeps its endowment, its control endpoint and its handoff page, and runs the swap of DECISIONS
//! §209 (state handoff is an opaque blob over a granted frame, and it is optional) when asked:
//!
//! ```text
//!   1 BUILT      lay the replacement out, wired from the same declaration, not started.
//!   2 QUIESCED   CALL OPERATION_QUIESCE on the terminal endpoint. Its FIFO drains everything ahead; a
//!                parked reader is answered FLAG_RETRY and asks again; the blob is written.
//!   3 ABSORBING  start the replacement with START_ABSORB. It reads the blob before it serves.
//!   4 COMMIT     NOTE_ABSORBED: tell the incumbent CTL_QUIT and collect it.
//!     or UNDO    NOTE_REFUSED: collect the refuser and tell the incumbent CTL_RESUME.
//! ```
//!
//! It is `swapper`'s shape, an unprivileged operator holding exactly the objects it swaps, pointed
//! at a component a person uses. The two control endpoints alternate, so a word meant for the
//! incumbent can never reach its replacement.
//!
//! # BUGS
//!
//! **Nothing on a real boot asks it to swap.** The trigger is milestone 198 (a package manager, and the trivial install)'s installer activating a
//! new `line_editor` build, and that message is not built; `system_initializer` passes no swap
//! endpoint, so the supervisor builds the terminal and then waits on its supervision endpoint. The
//! kernel's guest test holds the swap endpoint and is the only thing that swaps today.
//!
//! **A replacement is the same image.** The supervisor was handed one `line_editor` image, and the
//! request carries none. A new build arriving with the trigger is the trigger's design, above.
//!
//! **A `line_editor` that faults is not restarted.** Its death is collected at the next swap, and
//! until then the terminal is gone. Restarting it fresh is kill-and-replace, which this program
//! could do, and is not built because nothing has asked for it.
//!
//! Name: ratified 2026-10-06 (calef, "Ratify" it, in conversation). Minted
//! provisionally 2026-09-27 by the lane for milestone 23.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use component_plan::Provisions;
use line_editor::component::{self, supervisor as s};
use line_editor::proto;
use supervision_protocol::{ChildEndowment, Retention};
use user_mode_runtime::{cap_delete, receive, receive_fault, receive_request, reply, send};

const MODE_DISPLAY: u64 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, elf_len: u64, swap: u64) -> ! {
    // SAFETY: the builder copied the `line_editor` image to ELF_VA, `elf_len` bytes, before
    // starting us, and nothing writes it afterwards.
    let bytes = unsafe { core::slice::from_raw_parts(s::ELF_VA as *const u8, elf_len as usize) };
    let Ok(image) = elf::Elf::parse(bytes) else {
        fail()
    };

    let faultep = obj(abi::objtype::RENDEZVOUS);
    let controls = [obj(abi::objtype::RENDEZVOUS), obj(abi::objtype::RENDEZVOUS)];
    let notify = obj(abi::objtype::RENDEZVOUS);
    let handoff =
        match user_mode_runtime::retype_page_frame_run(s::BUDGET, component::HANDOFF_PAGES) {
            f if f >= 0 => f as u64,
            _ => fail(),
        };

    let decl = if mode == MODE_DISPLAY {
        &component::DISPLAY
    } else {
        &component::CONSOLE
    };
    let control_in_child = component_plan::slot_of(decl, "control");

    // **Two bays, one per instance, split once and reused.** A child's region comes home to the
    // region it was split from, and only un-bumps that region's watermark when it is on top. Built
    // straight from the budget, the incumbent's region is always under its replacement's when it
    // dies, so every swap would leave a hole the size of an instance: the second swap failed for
    // want of budget, which is how this was found. Each instance is split from its own bay, where
    // it is the only child and so always on top.
    let bays = [bay(decl), bay(decl)];

    let mut sup = Supervisor {
        image,
        decl,
        mode,
        control_in_child,
        faultep,
        controls,
        notify,
        handoff,
        bays,
        current: 0,
        started: 0,
    };
    let plan = sup.plan_for(sup.controls[sup.current]);
    launch(
        &sup.image,
        &plan,
        sup.bays[sup.current],
        faultep,
        [mode, control_in_child, proto::START_HANDOFF],
    );
    sup.started += 1;

    if swap == 0 {
        // No trigger on this boot (see BUGS). Hold everything and wait, so the objects stay ours.
        loop {
            let _ = receive(faultep);
        }
    }

    loop {
        let req = receive_request(swap);
        let verb = req.w0;
        // Only a CALL is answered; a delegation is deleted (milestone 706 (a `CALL` server can tell
        // a Reply from a delegation)).
        let Some(caller) = req.delivered.into_reply() else {
            continue;
        };
        if verb == s::STOP {
            sup.stop(caller)
        }
        if verb != s::SWAP {
            reply(caller, proto::BAD_REQUEST, 0);
            continue;
        }
        sup.swap(caller);
    }
}

/// Everything the swap loop works with: the image and declaration every instance is built from,
/// the objects this supervisor made, and which bay holds the live instance.
struct Supervisor<'a> {
    image: elf::Elf<'a>,
    decl: &'static component_plan::Requirements,
    mode: u64,
    control_in_child: u64,
    faultep: u64,
    controls: [u64; 2],
    notify: u64,
    handoff: u64,
    bays: [u64; 2],
    /// Which bay, and which control endpoint, belong to the live instance.
    current: usize,
    /// How many instances have been started, reported back on `SWAPPED` and `STOPPED`.
    started: u64,
}

impl Supervisor<'_> {
    /// The plan for an instance whose control endpoint is `control`.
    fn plan_for(&self, control: u64) -> component_plan::Plan {
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
            (component_plan::STATE_HANDOFF_ROLE, self.handoff),
        ];
        let Ok(plan) = component_plan::plan(self.decl, &Provisions { held: &held }) else {
            fail()
        };
        plan
    }

    /// `STOP`: quiesce and retire the live instance, give both bays back, and exit.
    fn stop(&self, caller: user_mode_runtime::Reply) -> ! {
        let (q, _) =
            user_mode_runtime::call(s::TERMINAL, proto::req(proto::OPERATION_QUIESCE, 0), 0);
        if q != proto::QUIESCED {
            fail()
        }
        send(self.controls[self.current], proto::CTL_QUIT, 0, 0);
        collect(self.faultep);
        // Both bays are empty now, and a budget with children cannot be reclaimed by whoever
        // owns it, so give them back before going.
        for b in self.bays {
            if !supervision_protocol::memory_region_destroy(b) {
                fail()
            }
        }
        reply(caller, s::STOPPED, self.started);
        user_mode_runtime::exit()
    }

    /// `SWAP`: build a replacement in the other bay, quiesce the incumbent, start the replacement
    /// absorbing its state, and commit or roll back on what the replacement says.
    fn swap(&mut self, caller: user_mode_runtime::Reply) {
        let next = 1 - self.current;
        let plan = self.plan_for(self.controls[next]);
        let Ok(region) = supervision_protocol::memory_region_split(self.bays[next], plan.pages())
        else {
            fail()
        };
        let Ok((child, aspace)) = supervision_protocol::build_child_space(
            s::BUDGET,
            region,
            &self.image,
            &endowment(&plan, self.faultep),
        ) else {
            fail()
        };

        let (q, _) =
            user_mode_runtime::call(s::TERMINAL, proto::req(proto::OPERATION_QUIESCE, 0), 0);
        if q != proto::QUIESCED {
            fail()
        }
        if supervision_protocol::configure_child(child.tcb, aspace, self.image.entry()).is_err()
            || !supervision_protocol::start_child(
                child,
                self.mode,
                self.control_in_child,
                proto::START_HANDOFF | proto::START_ABSORB,
            )
        {
            fail()
        }
        cap_delete(region);
        self.started += 1;

        let (note, why, _) = receive(self.notify);
        if note == proto::NOTE_ABSORBED {
            send(self.controls[self.current], proto::CTL_QUIT, 0, 0);
            collect(self.faultep);
            self.current = next;
            reply(caller, s::SWAPPED, self.started);
        } else {
            collect(self.faultep);
            send(self.controls[self.current], proto::CTL_RESUME, 0, 0);
            reply(caller, s::ROLLED_BACK, why);
        }
    }
}

fn endowment<'a>(plan: &'a component_plan::Plan, faultep: u64) -> ChildEndowment<'a> {
    ChildEndowment {
        caps: plan.caps(),
        maps: plan.maps(),
        blobs: &[],
        fault: Some(faultep),
        stack_pages: component::STACK_PAGES,
        ..ChildEndowment::new(Retention::Nothing)
    }
}

fn launch(image: &elf::Elf, plan: &component_plan::Plan, bay: u64, faultep: u64, args: [u64; 3]) {
    let Ok(region) = supervision_protocol::memory_region_split(bay, plan.pages()) else {
        fail()
    };
    let Ok(child) =
        supervision_protocol::build_child(s::BUDGET, region, image, &endowment(plan, faultep))
    else {
        fail()
    };
    if !supervision_protocol::start_child(child, args[0], args[1], args[2]) {
        fail()
    }
    cap_delete(region);
}

/// Wait for one death and collect it, so the instance's region comes home to the budget.
fn collect(faultep: u64) {
    let (_event, tid, _pc, _addr, _, _) = receive_fault(faultep);
    if user_mode_runtime::reap(faultep, tid) != 0 {
        fail()
    }
}

fn bay(decl: &component_plan::Requirements) -> u64 {
    match supervision_protocol::memory_region_split(s::BUDGET, decl.pages) {
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
