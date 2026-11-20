//! **The directory this shell holds**: the page it shares with the FS server, the slots the
//! directory capability sits at, and the pattern and `echo` expansions bound to it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Where the page this shell shares with the FS server is mapped, **in the wiring that has one**.
///
/// It is `fs_service`'s `FILE_VA_CLIENT`, the address every FS client in this system maps its half
/// of the contract at, which is why [`OUT_VA`] moved out of the way rather than this.
pub(super) const FS_VA: u64 = address_space_layout::pair_page(0x0000_0000_0060_0000);

// SAFETY: the navigating wiring maps one page read/write at FS_VA before this shell runs, when it
// has an FS_VA at all (milestone 139 round 2; see `user_mode_runtime::mapped_window`, which is what
// collapsed the hand-rolled read_volatile/write_volatile loops below). Constructing the window
// touches no memory; every caller of `put_page`/`get_page` already runs behind a `dir.is_some()`
// check, which is only true in that wiring, matching the comment this replaces.
pub(super) const FS_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(FS_VA, filesystem_contract::PAGE_BYTES as u64) };

/// The directory capability's slot in the navigating wiring (`fs_service::start_granted_dir` grants
/// it at 0). A shell that was granted none has no such slot at all, which is why [`Nav::dir`] is an
/// `Option` rather than a constant: "this shell holds no directory" is a fact about a capability table.
pub(super) const DIR: u64 = 0;

/// The directory capability's slot in the **terminal** wiring, where slots 0..3 are already the
/// terminal, the spawn channel, the result channel and the budget. A shell wired this way holds a
/// filesystem *and* a way to spawn, which is the whole of what `>` and `<` needed (notes/pipes.md).
pub(super) const DIR_TERMINAL: u64 = 4;

// These two wrappers are the seam with the `swish` crate. The crate's versions take the
// directory read as a callback, because matching a pattern is pure and only *reading* the
// directory needs a capability; these bind that callback to this shell's [`Nav`].
/// [`swish::expansion`], against the directory this shell holds.
pub(super) fn expansion(
    nav: &mut Nav,
    spec: &ParsedInvocation,
) -> Result<grant_plan::expand::Expansion, Say> {
    swish::expansion(spec, &mut |token| nav.expand(token))
}

/// [`swish::echo`], against the directory this shell holds.
/// `$?` reads [`LAST`], which is the segment *before* this one: during `false || echo $?` the
/// shell is running the second segment and the first one's answer is what the word is about.
pub(super) fn echo(nav: &mut Nav, text: &[u8], out: &mut dyn FnMut(&[u8])) -> Say {
    swish::echo(text, last(), &mut |token| nav.expand(token), out)
}

/// Copy a name into the page shared with the FS server.
pub(super) fn put_page(bytes: &[u8]) {
    for (i, &b) in bytes
        .iter()
        .take(filesystem_contract::PAGE_BYTES)
        .enumerate()
    {
        FS_WINDOW.write_volatile::<u8>(i as u64, b);
    }
}

/// Copy `n` bytes out of that page (a listing landed there).
pub(super) fn get_page(n: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().take(n).enumerate() {
        *b = FS_WINDOW.read_volatile::<u8>(i as u64);
    }
}
