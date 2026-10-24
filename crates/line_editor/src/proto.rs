// This module's docs are on its `pub mod` line in `lib.rs`.

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
/// It gives the terminal a second writer without a second page: [`OPERATION_WRITE`] reads the one
/// client output page init maps, the problem §55 (the file behind a `>` is the shell itself)
/// records for `filesystem_contract`. Eight bytes is the request shape, not a choice: a served
/// request arrives through `receive_cap` with two data words, so a sixteen-byte
/// `byte_sink_contract` message is two of these. See notes/terminal-contract.md ("The IPC
/// protocol") and notes/sink-protocol.md.
pub const OPERATION_PRINT: u64 = 5;

/// Application → terminal: switch the terminal between line-discipline and raw mode
/// (milestone 169). `len` is 1 to enter raw mode, 0 to leave it. Replied immediately, r0 = 0.
///
/// While raw mode is on, [`OPERATION_BYTES`] bypasses [`super::LineDisc`] entirely, and each burst
/// the driver delivers reaches the application through [`OPERATION_READRAW`]. `OPERATION_READLINE`
/// is refused (`BAD_REQUEST`) in raw mode and `OPERATION_READRAW` outside it. Switching either way
/// abandons the line in progress in the mode being left, as `^C` does; history and the kill buffer
/// are untouched. See notes/terminal-contract.md ("The IPC protocol") and
/// design/roadmap/0169-kilo-editor.md.
pub const OPERATION_RAWMODE: u64 = 6;

/// Application → terminal: read raw bytes, no line discipline (milestone 169). Valid only
/// while raw mode ([`OPERATION_RAWMODE`]) is on; refused with `BAD_REQUEST` otherwise. Replied when
/// at least one byte is available, never held back to fill more (raw mode's whole point is
/// keystroke-at-a-time delivery, not batching): r0 = byte count (1..=8), r1 = the bytes packed
/// little-endian, the same register-only shape [`OPERATION_BYTES`] and [`OPERATION_PRINT`] already use, so
/// this needs no page either. **At most one read may be outstanding**, exactly like
/// [`OPERATION_READLINE`]; a second one while one is parked is refused with `BAD_REQUEST`.
pub const OPERATION_READRAW: u64 = 7;

/// The badge on a copy of the terminal endpoint that reads keystrokes and nothing else
/// (milestone 709 (a graphical terminal session on the no-keyboard arm holds only the raw half
/// of the boot discipline), fatal risk 7).
///
/// The terminal serves a badge by its value: on this one only [`OPERATION_RAWMODE`] and
/// [`OPERATION_READRAW`], refusing the rest with [`BAD_REQUEST`], and on an unknown one nothing.
/// The spawn service mints it (`abi::rendezvous::BADGE`) for a keyboardless `graphical_terminal`
/// session. A badged copy cannot be re-badged (§230 (badged endpoint capabilities)), so its holder
/// cannot widen it back (notes/terminal-contract.md, "Badged copies").
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

/// Supervisor → terminal: **stop serving so a replacement can take over** (milestone 23 (a
/// capability-routed component OS with live replacement); calef, 2026-09-26, "1a"). The served
/// endpoint's FIFO does the draining. Parked [`OPERATION_READLINE`] and [`OPERATION_READRAW`] get
/// [`FLAG_RETRY`] (a reply capability cannot leave the process), the reply is r0 = [`QUIESCED`],
/// and the terminal waits for [`CTL_RESUME`] or [`CTL_QUIT`] on its control endpoint. A terminal
/// with none refuses it with [`BAD_REQUEST`]. See notes/terminal-contract.md.
/// # BUGS
/// - The terminal does not check the sender, and the exposure is live on every boot: any unbadged
///   holder (input driver, shell, `login`, `system_initializer`, `terminal_supervisor`) can quiesce
///   the boot terminal, which `terminal_supervisor` starts with a control endpoint, until reboot.
///
/// Name: provisional (the lane for milestone 23, 2026-09-26).
pub const OPERATION_QUIESCE: u64 = 8;

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
