//! **The program that holds two directory capabilities at once** (milestone 154 (a process that
//! holds two directory capabilities)).
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::fs;
use filesystem_fixture as fixture;
use grant_plan::nav::{TwoRoots, Which};
use user_mode_runtime::{call, exit, send};

use crate::channel::put_page;

/// The two-directory witness (milestone 154, design/roadmap/0154-multi-directory-namespace.md):
/// holds two `fs_subtree_caretaker`s, slot 0 labeled `a` (the fixture's `sub`) and slot 1 labeled
/// `b` (`other`), per `start_granted_two_dirs` in `kernel/src/user/fs_service/two_dirs.rs`.
///
/// It proves the block's deliverable: `/a/inner` and `/b/secret` resolve through
/// [`grant_plan::nav::TwoRoots`] and then open on the named caretaker; `/a/../b` is refused by
/// `TwoRoots::resolve` before any request is sent; and neither caretaker's tree is reachable
/// through the other's endpoint, asked directly of each (notes/dir-capability.md).
pub(crate) fn two_dir() -> ! {
    use filesystem_fixture::twodir as t;
    use fixture::tree;

    /// Grant A's endpoint: `fs_service::start_granted_two_dirs`' slot 0.
    const FILE_A: u64 = 0;
    /// Grant B's endpoint: slot 1.
    const FILE_B: u64 = 1;
    /// Where this role reports, one slot past the two directory endpoints.
    const REPORT_SLOT: u64 = 2;

    let mut v = 0u64;
    // The labels are this witness's own choice, independent of what the grants are named on
    // disk (`sub`, `other`): a namespace label is assigned by whoever composes the namespace,
    // not by the directory it points at, and using different words for the two makes that plain.
    let ns = TwoRoots::new(b"a", b"b");

    // `/a/inner`: resolves to grant A, one component deep, and opens through slot 0.
    match ns.resolve(b"/a/inner") {
        Ok((Which::A, cwd)) if cwd.depth() == 1 => {
            if dir_open_on(FILE_A, fs::ROOT, cwd.component(0)) >= 0 {
                v |= t::OPENED_A;
            } else {
                v |= t::GRANTED_ACCESS_FAILED;
            }
        }
        _ => v |= t::GRANTED_ACCESS_FAILED,
    }

    // `/b/secret`: the same claim, through the second grant and slot 1.
    match ns.resolve(b"/b/secret") {
        Ok((Which::B, cwd)) if cwd.depth() == 1 => {
            if dir_open_on(FILE_B, fs::ROOT, cwd.component(0)) >= 0 {
                v |= t::OPENED_B;
            } else {
                v |= t::GRANTED_ACCESS_FAILED;
            }
        }
        _ => v |= t::GRANTED_ACCESS_FAILED,
    }

    // The negative control: a path that ascends out of grant A and into grant B. Resolution
    // itself must refuse it, so nothing below sends a request over it at all.
    if ns.resolve(b"/a/../b").is_ok() {
        v |= t::DOT_DOT_CROSSED_MOUNTS;
    }

    // And from the wire's own side, independent of `TwoRoots` entirely: grant A's caretaker
    // holds nothing that names grant B's tree, and the reverse. `secret` is real, on the image,
    // and one directory entry away from the caretaker one hop up; a capability to `sub` reaching
    // it would be an escape whether or not any namespace logic tried to stop it.
    if dir_open_on(FILE_A, fs::ROOT, tree::SECRET.as_bytes()) >= 0 {
        v |= t::REACHED_B_VIA_A;
    }
    if dir_open_on(FILE_B, fs::ROOT, tree::INNER.as_bytes()) >= 0 {
        v |= t::REACHED_A_VIA_B;
    }

    send(REPORT_SLOT, fixture::VERDICT, v, 0);
    exit();
}

/// `OPEN` a name under a directory handle, over a chosen endpoint slot: [`two_dir`]'s own
/// helper, because every other role in this program talks to exactly one caretaker at slot
/// [`FILE`](crate::channel::FILE) and this is the one role that holds two.
fn dir_open_on(ep: u64, parent: u64, name: &[u8]) -> i64 {
    put_page(name);
    call(ep, fs::req(fs::OPEN, parent, name.len() as u64), 0).0 as i64
}
