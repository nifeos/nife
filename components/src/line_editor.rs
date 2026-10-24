//! **`line_editor`: the line discipline as a userspace component** (milestone 28).
//!
//! The layer Unix builds into the kernel as the tty line discipline is here a process. It sits
//! between the raw input driver and the application, and between the application and its output
//! sink, on plain endpoints:
//!
//! ```text
//!   MODE_CONSOLE (the plain-console boot):
//!   input driver ──OPERATION_BYTES──►┌─────────────┐──text──► console server ──► UART
//!                                    │ line_editor │
//!        application ◄─lines─────────└─────────────┘◄──OPERATION_WRITE / OPERATION_READLINE── application
//!
//!   MODE_DISPLAY (a `graphical_terminal` session, built when the user launches it from the prompt):
//!   keyboard_driver (direct) ──OPERATION_BYTES──►┌─────────────┐──OPERATION_WRITE──► display_terminal
//!                                                │ line_editor │
//!        application ◄─lines─────────────────────└─────────────┘◄──OPERATION_WRITE / OPERATION_READLINE── application
//! ```
//!
//! Nobody in that picture knows what they are talking to: rendezvous-only naming
//! (notes/ipc-naming.md) makes the discipline swappable. The output side is chosen at spawn by
//! `mode` ([`MODE_CONSOLE`]/[`MODE_DISPLAY`]) and nowhere else: `Con::put` writes the same page
//! either way, and the wire shape differs only in [`Con::flush`].
//!
//! The IPC protocol is the terminal contract, notes/terminal-contract.md; the framing constants
//! are `line_editor::proto`. Every request served here is a `CALL`, served through `RECEIVE_CAP`,
//! answered through the kernel's one-shot Reply capability (DECISIONS §12), which makes the server
//! deadlock-free: a READLINE with no line ready is *held* (the Reply parked in a slot) while the
//! server keeps serving. The editing lives in the host-tested `line_editor`
//! crate; this file is wiring.
//!
//! Its whole authority: the terminal endpoint (slot 0, RECEIVE), the output sink's request endpoint
//! (slot 1: `CONREQ`/`SEND` in [`MODE_CONSOLE`], `display_terminal`'s served endpoint/`CALL` in
//! [`MODE_DISPLAY`]), the console server's reply endpoint (slot 2, [`MODE_CONSOLE`] only), the
//! sink's output page (write), the client's output page (read) and input page (write). No device.
//!
//! **Plus, when a supervisor that can replace it built it, a control endpoint** (milestone 23 (a
//! capability-routed component OS with live replacement)) in the slot named by the second start
//! argument (`0` for none). With one, `OPERATION_QUIESCE` on a `proto::SUPERVISOR_BADGE` copy answers
//! parked reads `FLAG_RETRY`, replies, and waits for `CTL_RESUME` or `CTL_QUIT`; otherwise, refused.
//!

//! Name: ratified 2026-07-30 (calef, DECISIONS §39, landed by milestone 46) for the word and again
//! 2026-08-01 (milestone 63) for the spelling, replacing `termd` and then `lineedit`. Refused
//! `termd` (the `-d` claim) and `linedisc`, the correct Unix term of art, which is the second half
//! of §39: calef did not recognise the phrase, and he built this system, which is evidence about
//! the name rather than about him.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::{Event, LINE_MAX, LineDisc, PROMPT_MAX, RawQueue, Sink, proto};
use user_mode_runtime::{Reply, call, receive, receive_request, reply, send};

/// The terminal endpoint (slot 0): clients CALL requests here; we serve it with `RECEIVE_CAP`. Its
/// clients differ by [`MODE_CONSOLE`]/[`MODE_DISPLAY`] (`input` or `keyboard_driver`, directly, for the
/// keystroke half; `swish` either way), but this server never has to know which: an `OPERATION_BYTES`
/// CALL looks the same regardless of who is holding the other end (notes/ipc-naming.md). The one
/// distinction it does draw is the kernel's, not a name: a badged copy is served only the raw half
/// (`proto::RAW_ONLY_BADGE`, milestone 709 (a graphical terminal session on the no-keyboard arm
/// holds only the raw half of the boot discipline)).
const TERM: u64 = 0;
/// The output sink's request endpoint (slot 1): [`MODE_CONSOLE`] SENDs a byte count here
/// ([`CONREQ`]'s own doc); [`MODE_DISPLAY`] CALLs it with `OPERATION_WRITE` (`display_terminal`'s own
/// served endpoint). One slot, two meanings, chosen by `mode` at spawn -- the same shape
/// `display_terminal`'s own `PRESENT` slot and `keyboard_driver`'s own `OUT` slot already use.
const CONREQ: u64 = 1;
/// The console server's reply endpoint (slot 2): we RECEIVE its ack here. The console speaks the
/// pre-§12 (Call/Reply IPC) two-endpoint protocol (it serves with plain RECEIVE, which cannot answer a CALL), so
/// this hop is SEND+RECEIVE, not CALL. Safe with one console client, and `line_editor` is that client.
/// **[`MODE_CONSOLE`] only**: [`MODE_DISPLAY`] prints through one `CALL` on [`CONREQ`] and needs no
/// second endpoint, so nothing is granted here in that mode and this slot is simply never read.
const CONREP: u64 = 2;

/// `mode`: the pre-milestone-177 wiring, unchanged. `Con::flush` speaks the console's bespoke
/// two-endpoint protocol over [`CONREQ`]/[`CONREP`]. Named for symmetry with [`MODE_DISPLAY`]
/// rather than matched by value: it is also `Con::flush`'s fallback for any `mode` this process
/// was never told to expect, which is the safer of the two directions to default in (a refused
/// `SEND` is silent; a `CALL` to an endpoint that does not speak that contract would hang this
/// server forever).
#[allow(dead_code)]
const MODE_CONSOLE: u64 = 0;
/// `mode`: the wiring a `graphical_terminal` session uses (built by `system_initializer` when the user
/// launches it from the prompt; no boot builds it, milestone 632 (graphics on demand)). `Con::flush` speaks
/// `display_terminal`'s `OPERATION_WRITE`/one-`CALL` contract over [`CONREQ`], which in this mode holds
/// `display_terminal`'s own served endpoint instead of the console's request endpoint.
const MODE_DISPLAY: u64 = 1;

/// The output sink's shared page, mapped read/write: we fill it, the sink prints it. In
/// [`MODE_CONSOLE`] the same frame the console server reads at its own `SHARED_VA`; in
/// [`MODE_DISPLAY`] the same frame `display_terminal` reads at its own `OUT_VA`. Must match the
/// wiring (the progenitor, which builds `display_terminal` too since milestone 600 (provisional)); one address
/// either way, since the two modes never coexist in one process.
const CONOUT_VA: u64 = address_space_map::pair_page(0x0060_0000);
/// The client's output page, mapped read-only: `OPERATION_WRITE` text and `OPERATION_READLINE` prompts arrive
/// here, written by the client at its own VA for this frame.
const APP_OUT_VA: u64 = address_space_map::pair_page(0x0080_0000);
/// The client's input page, mapped read/write: completed lines are delivered here for the
/// client to read at its own VA for this frame.
const APP_IN_VA: u64 = address_space_map::pair_page(0x0090_0000);

const PAGE: usize = 4096;

/// Where a supervisor maps the handoff page when it starts us with `START_HANDOFF`. Declared in
/// `line_editor::component`, beside the capability half of the contract.
const HANDOFF_VA: u64 = line_editor::component::HANDOFF_VA;
const HANDOFF_BYTES: usize = PAGE * line_editor::component::HANDOFF_PAGES as usize;

/// How many completed lines the type-ahead queue holds. A user typing while the application is
/// busy loses nothing until this overflows, and then the newest line is dropped with a bell,
/// which is what a real tty's flooded input queue does too.
const QUEUE: usize = 4;

/// **These three live in `.bss`, not on the stack.** A user process here gets one 4 KiB page of
/// stack (`kernel/src/user.rs`'s `USER_STACK_VA`), and `LineDisc` alone is a couple of KiB before
/// `LineQueue` (four buffered lines) and `RawQueue` (milestone 169) are added on top of it; as
/// stack locals in `_start`'s own frame that overflowed the page (found by
/// `kernel::user::raw_mode_tests` faulting at `_start` with `sp` just past the mapped stack, the
/// exact symptom `display_terminal.rs`'s own `TERMINAL` comment already names). `LineDisc::new`,
/// `LineQueue::new` and `RawQueue::new` are all `const` precisely so this is possible; see
/// `display_terminal.rs`'s `TERMINAL` for the same fix, one program over.
static mut DISC: LineDisc = LineDisc::new();
static mut LINE_QUEUE: LineQueue = LineQueue::new();
static mut RAW_QUEUE: RawQueue = RawQueue::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, control: u64, start: u64) -> ! {
    // A raw pointer first, then one dereference: taking `&mut DISC` directly is what
    // `static_mut_refs` exists to refuse. This process has exactly one thread and cannot be given a
    // second (it is built without its own process; milestone 812 (`std::thread::spawn` runs real
    // threads in one address space)), so each pointer below is the only route to its static and
    // there is no aliasing question, the same reasoning `display_terminal.rs`'s `term()` documents.
    let disc_p = &raw mut DISC;
    // SAFETY: see above.
    let disc = unsafe { &mut *disc_p };
    let queue_p = &raw mut LINE_QUEUE;
    // SAFETY: see above.
    let queue = unsafe { &mut *queue_p };
    let raw_queue_p = &raw mut RAW_QUEUE;
    // SAFETY: see above.
    let raw_queue = unsafe { &mut *raw_queue_p };
    let mut term = Terminal {
        disc,
        queue,
        raw_queue,
        pending_stdin: None,
        con: Con { used: 0, mode },
        pending: None,
        intr_count: 0,
        suspend_count: 0,
        raw_mode: false,
        pending_raw: None,
        resuming_line: false,
        control,
        start,
    };

    // A replacement reads its predecessor's state before it serves anything, per DECISIONS §209 (state
    // handoff is an opaque blob over a granted frame, and it is optional). An
    // instance that served first and checked later would already have answered with a blank line.
    if start & proto::START_ABSORB != 0 {
        term.absorb();
    }

    loop {
        let req = receive_request(TERM);
        let (w0, w1, badge) = (req.w0, req.w1, req.badge);
        let Some(slot) = req.delivered.into_reply() else {
            // A plain SEND or a SEND_CAP slipped in; the contract says CALL. With no Reply there is
            // nobody to answer, so the only honest move is to drop it, and a delegated capability
            // is deleted with it (milestone 706 (a `CALL` server can tell a Reply from a
            // delegation)).
            continue;
        };
        // **The badge decides what a holder may ask**, and `proto::admits` is the table, with why
        // each badge narrows. An allowlist, so a request added to the contract later is refused on
        // every badge until somebody decides its holder may send it.
        let allowed = proto::admits(badge, proto::operation(w0));
        if !allowed {
            reply(slot, proto::BAD_REQUEST, 0);
            continue;
        }
        term.serve(w0, w1, slot);
    }
}

/// Everything the serve loop carries between requests: the three `.bss` statics, the output sink,
/// both parked readers, and the mode and counters a quiesce hands to a replacement.
struct Terminal {
    disc: &'static mut LineDisc,
    queue: &'static mut LineQueue,
    raw_queue: &'static mut RawQueue,
    /// A parked `OPERATION_READ_LINE_BYTES`, answered when a line is complete.
    pending_stdin: Option<Reply>,
    con: Con,
    // A parked READLINE: the slot holding the caller's one-shot Reply capability. The caller
    // stays blocked (that is CALL's contract) while we serve everyone else.
    pending: Option<Reply>,
    // How many ^C we have seen since boot. The shell reads this (OPERATION_INTRCOUNT) to sense interrupts
    // while a foreground job runs and no read is parked to fail (DECISIONS §24). A
    // monotonic counter, so the shell learns of a ^C by the count advancing, never missing one.
    intr_count: u64,
    // How many ^Z, the same way, for OPERATION_SUSPEND_COUNT (milestone 48 (job control)).
    suspend_count: u64,
    // Raw mode (milestone 169 (`kilo`, the smallest real text editor, as the forcing function for raw terminal input); OPERATION_RAWMODE): while true, OPERATION_BYTES bypasses `disc` entirely and
    // lands in `raw_queue` instead, with no echo and no interpretation. `pending_raw` is
    // OPERATION_READRAW's own parked reply capability, the raw-mode twin of `pending`.
    raw_mode: bool,
    pending_raw: Option<Reply>,
    // Set when a quiesce answered a parked OPERATION_READLINE with FLAG_RETRY and this instance then
    // resumed: the reader's re-issued read must not repaint a prompt the screen already shows.
    resuming_line: bool,
    /// The control endpoint's slot, `0` when no supervisor can replace this instance.
    control: u64,
    /// The start flags (`proto::START_ABSORB`, `proto::START_HANDOFF`).
    start: u64,
}

impl Terminal {
    /// Read a predecessor's handoff blob and tell the supervisor whether it took; exits on a refusal.
    fn absorb(&mut self) {
        let notify = self.control + 1;
        // SAFETY: a supervisor passes START_ABSORB only alongside START_HANDOFF, having mapped the
        // handoff page read/write at HANDOFF_VA before starting us.
        let page = unsafe { core::slice::from_raw_parts(HANDOFF_VA as *const u8, HANDOFF_BYTES) };
        let absorbed = line_editor::handoff::Reader::new(page).map_err(|e| match e {
            line_editor::handoff::Refusal::NoBlob => 0,
            line_editor::handoff::Refusal::Malformed => 1,
            line_editor::handoff::Refusal::Layout(l) => l as u64,
        });
        let ok = match absorbed {
            Ok(mut r) => restore(&mut r, self.disc, self.queue, self.raw_queue)
                .map(|(raw, intr, resuming)| {
                    self.raw_mode = raw;
                    self.intr_count = intr;
                    self.resuming_line = resuming;
                })
                .ok_or(1),
            Err(why) => Err(why),
        };
        match ok {
            Ok(()) => {
                send(notify, proto::NOTE_ABSORBED, 0, 0);
            }
            Err(why) => {
                send(notify, proto::NOTE_NOT_ABSORBED, why, 0);
                user_mode_runtime::exit()
            }
        }
    }

    /// Answer one CALL by its operation. Per keystroke, so inlined into the serve loop.
    #[inline]
    fn serve(&mut self, w0: u64, w1: u64, slot: Reply) {
        match proto::operation(w0) {
            proto::OPERATION_BYTES if self.raw_mode => self.bytes_raw(w0, w1, slot),
            proto::OPERATION_BYTES => self.bytes_cooked(w0, w1, slot),
            proto::OPERATION_RAWMODE => self.set_raw_mode(w0, slot),
            proto::OPERATION_READRAW if !self.raw_mode => {
                reply(slot, proto::BAD_REQUEST, 0);
            }
            proto::OPERATION_READRAW => {
                if self.pending_raw.is_some() {
                    reply(slot, proto::BAD_REQUEST, 0);
                    return;
                }
                self.pending_raw = Some(slot);
                deliver_raw(self.raw_queue, &mut self.pending_raw);
            }
            proto::OPERATION_READLINE if self.raw_mode => {
                reply(slot, proto::BAD_REQUEST, 0);
            }
            proto::OPERATION_READ_LINE_BYTES if self.raw_mode || self.pending_stdin.is_some() => {
                reply(slot, proto::BAD_REQUEST, 0);
            }
            proto::OPERATION_READ_LINE_BYTES => {
                self.pending_stdin = Some(slot);
                self.deliver_stdin();
            }
            proto::OPERATION_WRITE => self.write(w0, slot),
            proto::OPERATION_READLINE => self.read_line(w0, slot),
            proto::OPERATION_PRINT => {
                // **A second writer, with no second page** (DECISIONS §67, notes/sink-protocol.md).
                // The bytes are in the request's own words, so this client needs no frame mapped
                // here and no frame of its own; `OPERATION_WRITE` above reads the *one* page the progenitor maps in,
                // and a second page-based client would need this contract to grow a page index.
                //
                // Through the same `expand_output` as `OPERATION_WRITE`, so a newline from a sink adapter
                // becomes a carriage return and a newline exactly as one from the shell does. Two
                // writers, one terminal, one set of manners.
                let len = proto::len(w0).min(8);
                let bytes = w1.to_le_bytes();
                line_editor::expand_output(&bytes[..len], &mut self.con);
                self.con.flush();
                reply(slot, len as u64, 0);
            }
            proto::OPERATION_QUIESCE if self.control == 0 => {
                // Only a `proto::SUPERVISOR_BADGE` copy reaches here. With no control endpoint
                // nobody could tell us to resume, so honoring it would leave a dead terminal.
                reply(slot, proto::BAD_REQUEST, 0);
            }
            proto::OPERATION_QUIESCE => self.quiesce(slot),
            proto::OPERATION_INTRCOUNT => {
                // The shell's ^C sensor: reply immediately with the running count. Never blocks, so
                // the shell can busy-poll it while watching a foreground job (DECISIONS §24).
                reply(slot, self.intr_count, 0);
            }
            proto::OPERATION_SUSPEND_COUNT => {
                reply(slot, self.suspend_count, 0);
            }
            _ => {
                reply(slot, proto::BAD_REQUEST, 0);
            }
        }
    }

    /// `OPERATION_BYTES` in raw mode. Per keystroke, so inlined.
    #[inline]
    fn bytes_raw(&mut self, w0: u64, w1: u64, slot: Reply) {
        // Raw mode: no echo, no interpretation, straight into the raw queue. A burst past
        // RAW_QUEUE_MAX is dropped audibly, the same overflow contract `disc.feed` uses.
        let n = proto::len(w0).min(8);
        let bytes = w1.to_le_bytes();
        if self.raw_queue.push(&bytes[..n]) < n {
            self.con.put(&[0x07]);
            self.con.flush();
        }
        reply(slot, 0, 0);
        deliver_raw(self.raw_queue, &mut self.pending_raw);
    }

    /// `OPERATION_BYTES` through the line discipline. Per keystroke, so inlined.
    #[inline]
    fn bytes_cooked(&mut self, w0: u64, w1: u64, slot: Reply) {
        let n = proto::len(w0).min(8);
        for i in 0..n {
            let b = (w1 >> (8 * i)) as u8;
            match self.disc.feed(b, &mut self.con) {
                Event::Line => self.queue.push(self.disc.line(), 0, &mut self.con),
                Event::Eof => self.queue.push(&[], proto::FLAG_EOF, &mut self.con),
                Event::Interrupt => {
                    // ^C: the discipline discarded the edit line. We discard the type-ahead
                    // and fail a pending read (case 1, the shell is at the prompt). And we
                    // bump the interrupt count, which the shell polls (OPERATION_INTRCOUNT) when a
                    // foreground job is running and no read is parked (case 2,
                    // DECISIONS §24). One ^C, both effects; whichever the shell is watching.
                    self.intr_count = self.intr_count.wrapping_add(1);
                    self.queue.clear();
                    if let Some(p) = self.pending.take() {
                        reply(p, 0, proto::FLAG_INTERRUPTED);
                    }
                    self.end_stdin();
                }
                // ^Z: only counted. The shell polls the count while it watches a foreground job
                // and suspends the job; a pending read is not failed, since nothing was asked.
                Event::Suspend => {
                    self.suspend_count = self.suspend_count.wrapping_add(1);
                    self.queue.clear();
                }
                // Tab is the application's (§227 (how Tab reaches the shell)): a
                // client that wants completion runs the engine itself over raw mode.
                Event::None | Event::Tab => {}
            }
        }
        self.con.flush();
        reply(slot, 0, 0);
        deliver(self.queue, &mut self.pending);
        self.deliver_stdin();
    }

    /// `OPERATION_RAWMODE`: switch modes, abandoning the one being left.
    fn set_raw_mode(&mut self, w0: u64, slot: Reply) {
        let on = proto::len(w0) != 0;
        // Abandon whichever mode's in-progress line we're leaving: the edit buffer (and
        // any parked OPERATION_READLINE, which raw mode would otherwise never complete) going
        // into raw mode, the unread raw queue (and any parked OPERATION_READRAW) coming out of
        // it. A session must never resume half a line typed under the mode it just left.
        if on {
            self.disc.abandon();
            self.queue.clear();
            if let Some(p) = self.pending.take() {
                reply(p, proto::BAD_REQUEST, 0);
            }
            self.end_stdin();
        } else {
            *self.raw_queue = RawQueue::new();
            if let Some(p) = self.pending_raw.take() {
                reply(p, proto::BAD_REQUEST, 0);
            }
        }
        self.raw_mode = on;
        reply(slot, 0, 0);
    }

    /// `OPERATION_WRITE`: copy the client's page out through newline expansion.
    fn write(&mut self, w0: u64, slot: Reply) {
        let len = proto::len(w0).min(PAGE);
        // Stream the client's text through newline expansion into the console page, in
        // small chunks so the expansion never needs a buffer the size of its input.
        let mut done = 0;
        let mut chunk = [0u8; 256];
        while done < len {
            let n = (len - done).min(chunk.len());
            copy_in(APP_OUT_VA, done, &mut chunk[..n]);
            line_editor::expand_output(&chunk[..n], &mut self.con);
            done += n;
        }
        self.con.flush();
        reply(slot, len as u64, 0);
    }

    /// `OPERATION_READLINE` in cooked mode: paint the prompt and park the reader.
    fn read_line(&mut self, w0: u64, slot: Reply) {
        if self.pending.is_some() {
            // One outstanding read per terminal (the contract). A second caller while
            // one is parked is a protocol violation; refuse it loudly.
            reply(slot, proto::BAD_REQUEST, 0);
            return;
        }
        let plen = proto::len(w0).min(PROMPT_MAX);
        let mut prompt = [0u8; PROMPT_MAX];
        copy_in(APP_OUT_VA, 0, &mut prompt[..plen]);
        if core::mem::replace(&mut self.resuming_line, false) {
            self.disc.resume_line(&prompt[..plen]);
        } else {
            self.disc.start_line(&prompt[..plen], &mut self.con);
            self.con.flush();
        }
        self.pending = Some(slot);
        deliver(self.queue, &mut self.pending);
    }

    /// `OPERATION_QUIESCE` with a control endpoint: release parked readers, write the handoff
    /// blob, reply, and wait on the control endpoint; exits unless told to resume.
    fn quiesce(&mut self, slot: Reply) {
        // A reply capability cannot leave this process, so a parked reader is answered
        // now or never. It asks again, and whoever receives next answers it.
        if let Some(p) = self.pending.take() {
            reply(p, 0, proto::FLAG_RETRY);
            self.resuming_line = true;
        }
        if let Some(p) = self.pending_raw.take() {
            reply(p, 0, proto::FLAG_RETRY);
        }
        // A stdin reader is not in the handoff blob: it reads end of input instead.
        self.end_stdin();
        if self.start & proto::START_HANDOFF != 0 {
            // Written before the reply, so the supervisor's QUIESCED is also the receipt
            // that the page holds this instance's final state.
            // SAFETY: START_HANDOFF means the page is mapped read/write at HANDOFF_VA.
            let page =
                unsafe { core::slice::from_raw_parts_mut(HANDOFF_VA as *mut u8, HANDOFF_BYTES) };
            let mut w = line_editor::handoff::Writer::new(page);
            self.disc.save(&mut w);
            self.queue.save(&mut w);
            self.raw_queue.save(&mut w);
            w.u32(self.raw_mode as u32);
            w.u32(self.intr_count as u32);
            w.u32(self.resuming_line as u32);
            if w.finish().is_none() {
                // Cannot happen at these sizes (the blob is under 4 KiB); if it did, the
                // replacement's reader refuses a truncated blob and the swap rolls back.
                page[..4].copy_from_slice(&[0; 4]);
            }
        }
        reply(slot, proto::QUIESCED, 0);
        // Stopped receiving on TERM: anything that arrives now parks on its sender queue.
        let (what, _, _) = receive(self.control);
        if what != proto::CTL_RESUME {
            user_mode_runtime::exit()
        }
    }
}

impl Terminal {
    /// **Answer a parked stdin read** from the line it is draining, or take the next complete line
    /// for it when the shell is not waiting for one: the shell's own read comes first, since a
    /// line typed while it reads is a command.
    fn deliver_stdin(&mut self) {
        if self.pending_stdin.is_none() {
            return;
        }
        if self.queue.stdin.is_drained() {
            if self.pending.is_some() || self.queue.count == 0 {
                return;
            }
            let flags = self.queue.pop_front_into_stdin();
            if flags & proto::FLAG_EOF != 0 {
                self.end_stdin();
                return;
            }
        }
        let (n, packed) = self.queue.stdin.take8();
        if let Some(p) = self.pending_stdin.take() {
            reply(p, n as u64, packed);
        }
    }

    /// End a stdin reader's input: answer a parked read with count 0, and drop the rest of the line.
    fn end_stdin(&mut self) {
        self.queue.stdin = StdinLine::new();
        if let Some(p) = self.pending_stdin.take() {
            reply(p, 0, 0);
        }
    }
}

/// **The line a stdin reader is draining**, with its `\n`, and how far it has read.
struct StdinLine {
    bytes: [u8; LINE_MAX + 1],
    len: usize,
    at: usize,
}

impl StdinLine {
    const fn new() -> Self {
        StdinLine {
            bytes: [0; LINE_MAX + 1],
            len: 0,
            at: 0,
        }
    }

    fn is_drained(&self) -> bool {
        self.at == self.len
    }

    fn fill(&mut self, line: &[u8]) {
        let n = line.len().min(LINE_MAX);
        self.bytes[..n].copy_from_slice(&line[..n]);
        self.bytes[n] = b'\n';
        self.len = n + 1;
        self.at = 0;
    }

    /// Up to eight bytes, packed little-endian, as `OPERATION_READRAW` packs them.
    fn take8(&mut self) -> (usize, u64) {
        let n = (self.len - self.at).min(8);
        let mut word = [0u8; 8];
        word[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
        self.at += n;
        (n, u64::from_le_bytes(word))
    }
}

/// If a read is parked and a line is queued, marry them: copy the line into the client's input
/// page and wake the caller through its reply capability.
fn deliver(queue: &mut LineQueue, pending: &mut Option<Reply>) {
    if pending.is_none() || queue.count == 0 {
        return;
    }
    let (len, flags) = queue.pop_front_into(APP_IN_VA);
    let p = pending.take().unwrap();
    reply(p, len as u64, flags);
}

/// Raw mode's twin of [`deliver`]: if an `OPERATION_READRAW` is parked and a byte is queued, pop up to
/// eight and reply register-only, exactly [`proto::OPERATION_READRAW`]'s reply shape. No page: the bytes
/// ride in `r1`, the same register-only path [`proto::OPERATION_BYTES`] arrived on.
fn deliver_raw(raw_queue: &mut RawQueue, pending_raw: &mut Option<Reply>) {
    if pending_raw.is_none() {
        return;
    }
    let Some((n, packed)) = raw_queue.pop8() else {
        return;
    };
    let p = pending_raw.take().unwrap();
    reply(p, n as u64, packed);
}

/// The output sink channel: a page we fill and a message that says how much. Batches everything
/// the engine emits between flushes, so one keystroke's echo (or one ^L repaint) is one IPC
/// round trip, not one per escape sequence. `put` (below) is identical in both modes: it always
/// writes to [`CONOUT_VA`], and which physical frame backs that address is the wiring's business,
/// not this struct's. Only `flush`'s wire shape differs, by `mode`.
struct Con {
    used: usize,
    mode: u64,
}

impl Con {
    fn flush(&mut self) {
        if self.used == 0 {
            return;
        }
        match self.mode {
            MODE_DISPLAY => {
                // One CALL, `display_terminal`'s own `OPERATION_WRITE` contract: `CONREQ` holds its
                // served endpoint in this mode, not the console's request endpoint. The reply's
                // byte count is not re-checked here for the same reason the console's ack
                // content already wasn't: a short write from a terminal that never refuses one is
                // not a case this server has ever had to handle, in either mode.
                call(
                    CONREQ,
                    proto::req(proto::OPERATION_WRITE, self.used as u64),
                    0,
                );
            }
            _ => {
                // MODE_CONSOLE, and the default for any value this process was never told to
                // expect: the pre-milestone-177 wiring, which is also the safer of the two to
                // fall back on (a refused SEND is silent; a CALL to an endpoint that does not
                // speak this contract would hang this server forever).
                send(CONREQ, self.used as u64, 0, 0);
                receive(CONREP); // the ack means the page is ours to refill
            }
        }
        self.used = 0;
    }
}

impl Sink for Con {
    fn put(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let space = PAGE - self.used;
            let n = bytes.len().min(space);
            for (i, &b) in bytes[..n].iter().enumerate() {
                // SAFETY: the console page is mapped read/write at CONOUT_VA.
                unsafe { core::ptr::write_volatile((CONOUT_VA as *mut u8).add(self.used + i), b) };
            }
            self.used += n;
            bytes = &bytes[n..];
            if self.used == PAGE {
                self.flush();
            }
        }
    }
}

/// Read `dst.len()` bytes from the page at `base`, starting at `offset`.
fn copy_in(base: u64, offset: usize, dst: &mut [u8]) {
    for (i, b) in dst.iter_mut().enumerate() {
        // SAFETY: the client pages are mapped (read-only is enough to read) and offset+len is
        // bounded by PAGE by every caller.
        *b = unsafe { core::ptr::read_volatile((base as *const u8).add(offset + i)) };
    }
}

/// The type-ahead queue: completed lines waiting for a reader, FIFO, fixed storage.
struct LineQueue {
    lines: [([u8; LINE_MAX], usize, u64); QUEUE],
    head: usize,
    count: usize,
    /// The line a [`proto::STDIN_BADGE`] reader is draining, `\n` included: taken from the front of
    /// `lines`, so it lives beside them, in the same static.
    stdin: StdinLine,
}

impl LineQueue {
    const fn new() -> Self {
        LineQueue {
            lines: [([0; LINE_MAX], 0, 0); QUEUE],
            head: 0,
            count: 0,
            stdin: StdinLine::new(),
        }
    }

    fn push(&mut self, line: &[u8], flags: u64, con: &mut Con) {
        if self.count == QUEUE {
            con.put(&[0x07]); // full: drop the newest, audibly
            return;
        }
        let slot = (self.head + self.count) % QUEUE;
        let n = line.len().min(LINE_MAX);
        self.lines[slot].0[..n].copy_from_slice(&line[..n]);
        self.lines[slot].1 = n;
        self.lines[slot].2 = flags;
        self.count += 1;
    }

    fn clear(&mut self) {
        self.count = 0;
    }

    /// The front line moved to the stdin reader's line, then dropped from the queue; its flags.
    fn pop_front_into_stdin(&mut self) -> u64 {
        let (line, len, flags) = &self.lines[self.head];
        self.stdin.fill(&line[..*len]);
        let flags = *flags;
        self.head = (self.head + 1) % QUEUE;
        self.count -= 1;
        flags
    }

    /// Pop the oldest line into the page at `base`; returns (length, flags).
    fn pop_front_into(&mut self, base: u64) -> (usize, u64) {
        let (line, len, flags) = &self.lines[self.head];
        for (i, &b) in line[..*len].iter().enumerate() {
            // SAFETY: the client input page is mapped read/write at `base`.
            unsafe { core::ptr::write_volatile((base as *mut u8).add(i), b) };
        }
        let out = (*len, *flags);
        self.head = (self.head + 1) % QUEUE;
        self.count -= 1;
        out
    }
}

impl LineQueue {
    fn save(&self, w: &mut line_editor::handoff::Writer<'_>) {
        w.u32(self.count as u32);
        for i in 0..self.count {
            let (line, len, flags) = &self.lines[(self.head + i) % QUEUE];
            w.field(&line[..*len]);
            w.u32(*flags as u32);
        }
    }

    fn restore(&mut self, r: &mut line_editor::handoff::Reader<'_>) -> Option<()> {
        *self = LineQueue::new();
        let count = r.u32()? as usize;
        if count > QUEUE {
            return None;
        }
        for line in self.lines.iter_mut().take(count) {
            line.1 = r.field(&mut line.0)?;
            line.2 = u64::from(r.u32()?);
        }
        self.count = count;
        Some(())
    }
}

/// Read a whole handoff blob, in the order the quiesce path writes it. `None` on anything short
/// or malformed, and then the caller refuses the swap rather than serve half a state.
fn restore(
    r: &mut line_editor::handoff::Reader<'_>,
    disc: &mut LineDisc,
    queue: &mut LineQueue,
    raw_queue: &mut RawQueue,
) -> Option<(bool, u64, bool)> {
    disc.restore(r)?;
    queue.restore(r)?;
    raw_queue.restore(r)?;
    let raw = r.u32()? != 0;
    let intr = u64::from(r.u32()?);
    let resuming = r.u32()? != 0;
    Some((raw, intr, resuming))
}

user_mode_runtime::panic_handler!();
