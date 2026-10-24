// Moved out of `lib.rs` unchanged by milestone 878 (the line editor's inline modules move to
// their own files). Its docs stay on its `pub mod` line in `lib.rs`, where they always were.

/// Request opcode, in bits 63:56 of the first `CALL` word. The low 32 bits carry a length
/// or count; bits 55:32 are reserved and must be zero.
pub const OPERATION_SHIFT: u32 = 56;
/// Application → terminal: print `len` bytes from the client's output page. Reply when the
/// bytes are on the wire: r0 = bytes consumed.
pub const OPERATION_WRITE: u64 = 1;
/// Application → terminal: read one line. The low bits carry a prompt length (0 for none);
/// the prompt bytes are in the client's output page. The reply comes when a line is ready:
/// r0 = line length (the bytes are in the client's input page), r1 = [`FLAG_EOF`] /
/// [`FLAG_INTERRUPTED`] or 0.
pub const OPERATION_READLINE: u64 = 2;
/// Input driver → terminal: `count` (1..=8) raw wire bytes, packed little-endian in the
/// second word. Replied immediately (r0 = 0); the rendezvous is the flow control.
pub const OPERATION_BYTES: u64 = 3;
/// Application → terminal: how many `^C` has the terminal seen since boot? Replied immediately
/// (r0 = the running count). This is the shell's `^C` sensor while a foreground job runs and no
/// read is parked to fail: the shell polls this and drives its two-tier interrupt escalation
/// from the count's advance (DECISIONS §24). Deliberately a poll, not a delivered
/// signal: there is no non-blocking receive, so a busy-poll with `yield` is how the shell watches
/// two things (the job and `^C`) at once until the blocking notification primitive arrives.
pub const OPERATION_INTRCOUNT: u64 = 4;
/// Application → terminal: how many `^Z` has the terminal seen since boot? Replied immediately
/// (r0 = the running count). [`OPERATION_INTRCOUNT`]'s twin for job control (milestone 48 (job
/// control)): the shell polls it while it watches a foreground job, and suspends the job when the
/// count advances. Only the line discipline counts, so a `^Z` in raw mode is a byte to the reader.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
pub const OPERATION_SUSPEND_COUNT: u64 = 9;
/// Adapter → terminal: print `len` (1..=8) bytes carried **in registers**, packed
/// little-endian in the second word. Replied when the bytes are on the wire: r0 = bytes
/// consumed, exactly as [`OPERATION_WRITE`] answers.
///
/// Eight, not sixteen, and that is the request shape rather than a choice: a served request
/// here arrives through `receive_cap`, which hands the server the reply capability and **two** data
/// words. [`OPERATION_BYTES`] carries eight for the same reason, from the other direction. So a
/// sixteen-byte `byte_sink_contract` message is two of these, which is the honest cost of a second
/// writer that needs no page.
///
/// # Why this exists when `OPERATION_WRITE` already prints
///
/// [`OPERATION_WRITE`] reads from **the client's output page**, and there is one of those: init maps a
/// single frame into the terminal read-only and into the shell read/write. A second printing
/// client would need a second frame, and the terminal would have to be told which one a request
/// meant, which is a protocol change with a page-index in it. `filesystem_contract` has the same shape and
/// the same problem (DECISIONS §55: two client processes staging into one page do not compose,
/// and that is why the file behind a `>` is the shell itself).
///
/// Register-only sidesteps it entirely, and the sixteen bytes are not a compromise: they are the
/// three-word fastpath and the exact payload of a `byte_sink_contract` message, so an adapter that turns
/// the sink contract into terminal output is one unpack and one `CALL` with **no page at all**.
/// [`OPERATION_BYTES`] already proved the shape in the other direction, for the input driver.
///
/// So the terminal gets a second *writer* without a second page and without a second client
/// convention, which is what a sink adapter needed (notes/sink-protocol.md).
pub const OPERATION_PRINT: u64 = 5;

/// Application → terminal: switch the terminal between line-discipline and raw mode
/// (milestone 169). `len` is 1 to enter raw mode, 0 to leave it. Replied immediately, r0 = 0.
///
/// **Raw mode is the primitive a screen editor needs that the line discipline does not give
/// it** (design/roadmap/0169-kilo-editor.md): DECISIONS §21 says a program "never sees a
/// keystroke, an escape sequence, or an echo", which is exactly wrong for `kilo`, which needs
/// all three. While raw mode is on, [`OPERATION_BYTES`] bypasses [`super::LineDisc`] entirely: no echo, no
/// editing, no line assembly. A keystroke reaches the application exactly as it arrived, one
/// [`OPERATION_READRAW`] reply per burst the driver delivered.
///
/// `OPERATION_READLINE` is refused (`BAD_REQUEST`) while raw mode is on, and `OPERATION_READRAW` is refused
/// while it is off: the two input models do not mix on one terminal at once, and a client that
/// tries gets a fast, loud refusal rather than a read that silently never completes.
///
/// Switching in either direction **abandons whatever line was in progress**: the line
/// discipline's edit buffer going in, raw mode's queued-but-unread bytes coming out. That is
/// the same discard `^C` already does to the edit buffer, and it exists for the same reason:
/// a session must never resume half a line typed under the mode it just left. History and the
/// kill buffer are untouched, because neither belongs to the in-progress line.
pub const OPERATION_RAWMODE: u64 = 6;

/// Application → terminal: read raw bytes, no line discipline (milestone 169). Valid only
/// while raw mode ([`OPERATION_RAWMODE`]) is on; refused with `BAD_REQUEST` otherwise. Replied when
/// at least one byte is available, never held back to fill more (raw mode's whole point is
/// keystroke-at-a-time delivery, not batching): r0 = byte count (1..=8), r1 = the bytes packed
/// little-endian, the same register-only shape [`OPERATION_BYTES`] and [`OPERATION_PRINT`] already use, so
/// this needs no page either. **At most one read may be outstanding**, exactly like
/// [`OPERATION_READLINE`]; a second one while one is parked is refused with `BAD_REQUEST`.
pub const OPERATION_READRAW: u64 = 7;

/// **The badge on a copy of the terminal endpoint that reads keystrokes and nothing else**
/// (milestone 709 (a graphical terminal session on the no-keyboard arm holds only the raw half
/// of the boot discipline), fatal risk 7). The terminal serves [`OPERATION_RAWMODE`] and
/// [`OPERATION_READRAW`] on a capability carrying **this badge** and refuses every other
/// request on it with [`BAD_REQUEST`]: no [`OPERATION_BYTES`], so its holder cannot type a line
/// somebody else reads; no [`OPERATION_READLINE`], so it cannot take one; no [`OPERATION_PRINT`] or
/// [`OPERATION_WRITE`]; no [`OPERATION_QUIESCE`]. An unbadged capability is served everything but
/// [`OPERATION_READ_LINE_BYTES`] and [`OPERATION_QUIESCE`], which is every holder the boot wires
/// (the input driver, the shell, `login`). [`admits`] is the whole table.
///
/// The spawn service mints this one (`abi::rendezvous::BADGE`) for a `graphical_terminal`
/// session with no keyboard, whose keystrokes come from the boot's own discipline. The badge is
/// the kernel's word on every receive (§230 (badged endpoint capabilities)) and a badged copy
/// cannot be re-badged, so a holder cannot widen it back to the unbadged surface. The terminal
/// serves a badge by its value: this one, [`STDIN_BADGE`], [`SUPERVISOR_BADGE`], and nothing on
/// any other.
///
/// Name: provisional (the lane for milestone 709, 2026-10-03).
pub const RAW_ONLY_BADGE: u64 = 1;

/// **The badge on a copy of the terminal endpoint that is a program's standard input**
/// (milestone 892 (a std program at the prompt holds the network and a resolver); calef,
/// 2026-10-19 (UTC), on #2405: "a"). The terminal serves [`OPERATION_READ_LINE_BYTES`] on it and refuses everything else
/// with [`BAD_REQUEST`], so its holder reads the lines the owner types and can neither type one,
/// print, nor switch modes. The spawn service mints it for a program whose manifest sets
/// `grant_plan::Manifest::console_input`, which only `grant_plan`'s table can, and std reads it at
/// `std_runtime_contract::STDIN_SLOT`.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const STDIN_BADGE: u64 = 2;

/// Application → terminal, on a [`STDIN_BADGE`] copy only: **up to eight bytes of the line being
/// typed** (milestone 892, ruled with [`STDIN_BADGE`]). The terminal's own line discipline echoes
/// and edits the line; once it is complete, the reply is r0 = count (1..=8) and r1 = the bytes,
/// packed little-endian as [`OPERATION_READRAW`] packs them. Later calls drain the rest of the line,
/// its `\n` included, before the next line is taken. r0 = 0 is end of input: `^D`, a `^C`, a
/// quiesce, or raw mode, none of which a reader can be made to wait through. Refused in raw mode,
/// and while another such read is parked. No page is involved.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const OPERATION_READ_LINE_BYTES: u64 = 10;

/// Supervisor → terminal: **stop serving so a replacement can take over** (milestone 23 (a capability-routed component OS with live replacement), the
/// `line_editor` swap; ruled by calef 2026-09-26, "1a"). It rides the served endpoint, so its
/// FIFO does the draining: every request queued ahead of it is served by this instance, and
/// every one behind it by whoever receives next.
///
/// Before replying, the terminal answers any parked [`OPERATION_READLINE`] or [`OPERATION_READRAW`] with
/// [`FLAG_RETRY`], because a reply capability cannot leave this process and a reader stranded
/// on it could never be woken. Then it replies r0 = [`QUIESCED`] and stops receiving until its
/// supervisor says [`CTL_RESUME`] or [`CTL_QUIT`] on its control endpoint.
///
/// **Served only on a copy badged [`SUPERVISOR_BADGE`]**, and refused with [`BAD_REQUEST`] on
/// any other, the unbadged copy every client holds included. An honored quiesce stops the
/// terminal until its control endpoint speaks, and only the supervisor holds that, so a quiesce
/// from anyone else would leave the terminal dead.
///
/// **Refused with [`BAD_REQUEST`] by a terminal started with no control endpoint**, whatever the
/// badge. Without one there is nobody to tell it to resume. The boot's own `line_editor` has one,
/// because `terminal_supervisor` builds it.
///
/// Name: provisional (the lane for milestone 23, 2026-09-26).
pub const OPERATION_QUIESCE: u64 = 8;

/// **The badge on the copy of a terminal's endpoint its supervisor quiesces it through**
/// (DECISIONS §232 (the `line_editor` swap contract): "only a supervisor sends it"). A terminal
/// serves [`OPERATION_QUIESCE`] on this badge and nothing else, and refuses [`OPERATION_QUIESCE`]
/// on every other. Both `line_editor` and `display_terminal` read it.
///
/// The quiesce still rides the served endpoint, so its FIFO does the draining (§232 refused a
/// separate control endpoint for that reason); the badge only says who sent it. Minting it takes
/// `GRANT` on the unbadged endpoint (§230 (badged endpoint capabilities)), which the supervisor
/// holds and its spawner holds, and no client of a boot does: the shell, `login` and the input
/// driver hold `WRITE` alone. A badged copy cannot be re-badged, so a [`RAW_ONLY_BADGE`] or
/// [`STDIN_BADGE`] holder cannot reach this one either. `terminal_supervisor` mints it for each
/// quiesce and hands it to the stand-in that makes the call.
///
/// Name: provisional, minted 2026-10-24 (UTC) by maintainer/terminal-quiesce-from-supervisor-only.
pub const SUPERVISOR_BADGE: u64 = 3;

/// **Whether `line_editor` serves `operation` on a capability badged `badge`**, `0` for an
/// unbadged one. Every badge is an allowlist, so an operation added to the contract later is
/// refused on it until somebody decides its holder may send it:
///
/// - unbadged, the copy the input driver, the shell and `login` hold: everything but
///   [`OPERATION_READ_LINE_BYTES`], so the shell's lines cannot be read eight bytes at a time
///   from under it, and [`OPERATION_QUIESCE`], so none of them can stop the terminal;
/// - [`RAW_ONLY_BADGE`]: [`OPERATION_RAWMODE`] and [`OPERATION_READRAW`], so nothing its holder
///   sends reaches the queue the shell reads its next command from;
/// - [`STDIN_BADGE`]: [`OPERATION_READ_LINE_BYTES`] alone;
/// - [`SUPERVISOR_BADGE`]: [`OPERATION_QUIESCE`] alone;
/// - any other badge: nothing.
///
/// `true` says only that the badge admits it; the terminal may still refuse it in its state (a
/// second parked read, a quiesce with no control endpoint). See notes/terminal-contract.md.
///
/// Name: provisional, minted 2026-10-24 (UTC) by maintainer/terminal-quiesce-from-supervisor-only.
pub const fn admits(badge: u64, operation: u64) -> bool {
    match badge {
        0 => !matches!(operation, OPERATION_READ_LINE_BYTES | OPERATION_QUIESCE),
        RAW_ONLY_BADGE => matches!(operation, OPERATION_RAWMODE | OPERATION_READRAW),
        STDIN_BADGE => operation == OPERATION_READ_LINE_BYTES,
        SUPERVISOR_BADGE => operation == OPERATION_QUIESCE,
        _ => false,
    }
}

/// The reply word to [`OPERATION_QUIESCE`]: "QUIT", the value `hot_swap_contract::QUIESCED` also uses.
pub const QUIESCED: u64 = 0x5155_4954;

/// Supervisor → quiesced terminal, on its control endpoint: go back to serving. The swap did not
/// commit, and this instance never lost its state.
pub const CTL_RESUME: u64 = 1;
/// Supervisor → quiesced terminal, on its control endpoint: exit. The replacement has taken
/// over.
pub const CTL_QUIT: u64 = 2;

/// Supervisor → quiesced terminal, on its control endpoint: **write your state into the run this
/// message carries** (milestone 23, for `display_terminal`, whose handoff comes from the machine's
/// swap reserve and so cannot be mapped at build). A `SEND_CAP` of the run; a plain `SEND` means
/// "into the run you already map", which is a replacement that absorbed from it earlier in the
/// same swap. Answered [`NOTE_WRITTEN`] or [`NOTE_NOT_WRITTEN`]. A replacement started with
/// [`START_ABSORB`] takes the same message, before it serves, and reads the run instead.
///
/// Name: provisional (the lane for milestone 23, 2026-10-19 UTC).
pub const CTL_HANDOFF: u64 = 3;

/// `line_editor`'s third start argument, a set of bits a supervisor passes (milestone 23 (a
/// capability-routed component OS with live replacement)). `START_HANDOFF`: a handoff page is
/// mapped at [`HANDOFF_VA`](super::component::HANDOFF_VA), so a quiesce writes the blob there.
/// `START_ABSORB`: this instance replaces another, so it reads that blob before serving and
/// tells its supervisor [`NOTE_ABSORBED`] or [`NOTE_NOT_ABSORBED`] on the slot after its control
/// endpoint.
///
/// Name: provisional (the lane for milestone 23, 2026-09-27).
pub const START_HANDOFF: u64 = 1 << 0;
/// See [`START_HANDOFF`].
pub const START_ABSORB: u64 = 1 << 1;
/// Terminal → supervisor: the blob was absorbed; this instance is serving.
pub const NOTE_ABSORBED: u64 = 1;
/// Terminal → supervisor: the blob was refused; this instance is leaving without serving.
/// The second word is the refusal: 0 no blob, 1 malformed, otherwise the layout found.
///
/// Name: ratified 2026-10-18 (calef: "Approve both"), the negation of [`NOTE_ABSORBED`].
pub const NOTE_NOT_ABSORBED: u64 = 2;

/// Terminal → supervisor, after [`CTL_HANDOFF`]: the blob is written. The second word is its
/// length in bytes.
///
/// Name: provisional (the lane for milestone 23, 2026-10-19 UTC).
pub const NOTE_WRITTEN: u64 = 3;
/// Terminal → supervisor, after [`CTL_HANDOFF`]: nothing was written, because no run came and
/// none is mapped, or the run would not map. The instance is still quiesced, and waits for
/// [`CTL_RESUME`] or [`CTL_QUIT`].
///
/// Name: provisional (the lane for milestone 23, 2026-10-19 UTC).
pub const NOTE_NOT_WRITTEN: u64 = 4;

/// Pack a request's first word from an opcode and a length/count.
pub const fn req(operation: u64, len: u64) -> u64 {
    (operation << OPERATION_SHIFT) | (len & 0xffff_ffff)
}
/// The opcode of a request word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}
/// The length/count of a request word.
pub const fn len(w0: u64) -> usize {
    (w0 & 0xffff_ffff) as usize
}

/// READLINE reply flag: end of input (^D on an empty line). The line length is 0.
pub const FLAG_EOF: u64 = 1 << 0;
/// READLINE reply flag: the read was interrupted (^C). The line length is 0. This is the
/// contract's hook for interrupt routing; see design/interrupt-routing.md.
pub const FLAG_INTERRUPTED: u64 = 1 << 1;
/// Read reply flag: **ask again** (calef, 2026-09-26, "2b"). The terminal is about to be
/// replaced and could not hold this read across the swap; nothing was typed away. Re-issue the
/// same request, unchanged, and the terminal that answers it resumes the line where it was.
///
/// An [`OPERATION_READLINE`] reply carries it in r1 with r0 = 0. An [`OPERATION_READRAW`] reply cannot,
/// because r1 is the data there, so it is r0 = 0 (never a byte count, which is 1..=8) with this
/// flag in r1. [`is_retry`] reads both.
///
/// Name: provisional (the lane for milestone 23, 2026-09-26).
pub const FLAG_RETRY: u64 = 1 << 2;

/// **Whether a read reply means "ask again".** One test for both read shapes, so a reader
/// cannot check the flag in the wrong register.
pub const fn is_retry(r0: u64, r1: u64) -> bool {
    r0 == 0 && r1 == FLAG_RETRY
}

/// The reply to a request whose opcode the terminal does not implement: r0 = this, r1 = 0.
/// A sentinel rather than silence, so a confused client fails fast instead of hanging.
pub const BAD_REQUEST: u64 = u64::MAX;

#[cfg(test)]
mod tests;
