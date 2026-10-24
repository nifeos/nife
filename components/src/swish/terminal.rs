//! **Printing through the terminal**: the pages shared with it, staging bytes into the outgoing
//! one, and the sentences a builtin says.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

// Pages shared with the terminal (must match the wiring in the progenitor). Distinct from
// [`FS_VA`], because a shell with a `>` holds both; `wiring.rs` asserts the windows disjoint.
pub(super) const OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_00c0_0000); // we write; the terminal reads

pub(super) const LINE_VA: u64 = address_space_map::pair_page(0x0000_0000_00b0_0000); // the terminal writes; we read

// SAFETY: the wiring (`crates/system_initializer`'s `SH_OUT_VA` grant) maps one page read/write at
// OUT_VA before this shell runs, in every wiring that has a terminal at all (milestone 139 round
// 3; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled
// read_volatile/write_volatile loop in `stage` below, the same way [`FS_WINDOW`] collapsed the FS
// cluster's in round 2). Constructing a window touches no memory.
pub(super) const OUT_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(OUT_VA, user_mode_runtime::mapped_window::PAGE) };

// SAFETY: as `OUT_WINDOW`'s, but the wiring's `LINE_VA` grant, mapped read-only, and this is what
// collapsed the hand-rolled loop in `read_line` below.
pub(super) const LINE_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(LINE_VA, user_mode_runtime::mapped_window::PAGE) };

/// Print through the terminal: write the text into the shared page, CALL `OPERATION_WRITE`. The reply
/// means the bytes are on the wire and the page is ours again.
pub(super) fn print(s: &[u8]) {
    let n = s.len().min(4096);
    stage(s, n);
    call(TERM, proto::req(proto::OPERATION_WRITE, n as u64), 0);
}

/// Copy `n` bytes into the outgoing shared page.
pub(super) fn stage(s: &[u8], n: usize) {
    for (i, &b) in s[..n].iter().enumerate() {
        OUT_WINDOW.w8(i as u64, b);
    }
}

/// Print a small unsigned number in base 10.
pub(super) fn print_num(v: u64) {
    swish::write_num(v, &mut print);
}

/// Print what a builtin had to say. Every line is a statement about a name or a capability, never
/// about a policy, and the wording is [`swish::write_say`]'s so a host test can hold it: this shell
/// does not get to invent a friendlier word for a refusal.
pub(super) fn say(s: Say) {
    // Which kind of no (milestone 67 (`swish` the language)): an errno is a failure; everything
    // else is this shell declining from what it holds.
    match s {
        Say::Nothing => {}
        Say::Failed(_) => failed(),
        _ => refused(),
    }
    swish::write_say(s, &mut print);
}

pub(super) fn help() {
    swish::write_help(&mut print);
}
