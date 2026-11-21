//! The one node every host test and proof in this crate queues, and the helpers that mint its
//! token.
//!
//! A token is sound under two obligations, `Unqueued::new`'s: the node outlives every queue it
//! joins, and nothing names it outside the token while it is queued. Every caller meets them the
//! same way, so they are stated here once:
//!
//! - The node is declared before the object that queues it, so Rust drops the object first and a
//!   node parked when a test or harness ends was valid for all of its time there.
//! - The node is on no queue and is minted once, from the pointer every other access to it
//!   derives from (`Unqueued::new`'s provenance clause).
//!
//! The address wait tests keep a node of their own, because theirs carries the key it waits on.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by 1095-ipc-outstanding-findings. Considered:
//! `test_support`, which says less about what the module holds.

use super::*;

/// A minimal node: a link and nothing else.
pub(crate) struct N {
    next: Option<NonNull<N>>,
}

impl N {
    pub(crate) fn new() -> Self {
        N { next: None }
    }
}

// SAFETY: `next` and `set_next` read and write the same `next` field and nothing else, which is
// the whole of the `Node` contract.
unsafe impl Node for N {
    fn next(&self) -> Option<NonNull<Self>> {
        self.next
    }
    fn set_next(&mut self, next: Option<NonNull<Self>>) {
        self.next = next;
    }
}

/// Mint `n`'s token, from `&raw mut` on a proof's local rather than from a reference.
///
/// # Safety
/// `n` is `&raw mut` on a local that meets the module's two obligations and is not named again.
#[cfg(kani)]
pub(crate) unsafe fn mint(n: *mut N) -> Unqueued<N> {
    // SAFETY: this function's own contract is `Unqueued::new`'s, and `&raw mut` on a local is
    // never null.
    unsafe { Unqueued::new(NonNull::new_unchecked(n)) }
}

/// A host test's node, owned through the pointer `Box::into_raw` returned. Every access, the
/// token's and the test's, derives from that one pointer, and dropping it frees the node.
#[cfg(test)]
pub(crate) struct Owned(NonNull<N>);

#[cfg(test)]
impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came from `Box::into_raw` in `node`, and is freed only here.
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

#[cfg(test)]
impl core::ops::Deref for Owned {
    type Target = N;
    fn deref(&self) -> &N {
        // SAFETY: live until `drop`; the reference is the test's, taken between queue
        // operations.
        unsafe { self.0.as_ref() }
    }
}

#[cfg(test)]
impl core::ops::DerefMut for Owned {
    fn deref_mut(&mut self) -> &mut N {
        // SAFETY: as `deref`.
        unsafe { self.0.as_mut() }
    }
}

/// A fresh boxed node.
#[cfg(test)]
pub(crate) fn node() -> Owned {
    Owned(NonNull::new(Box::into_raw(Box::new(N::new()))).unwrap())
}

/// Mint a boxed node's token. Every test calls this once per node, on a node declared before the
/// object that queues it.
#[cfg(test)]
pub(crate) fn token(n: &mut Owned) -> Unqueued<N> {
    // SAFETY: the module's two obligations, which the test's declaration order and its one call
    // per node meet, and the pointer is the one every other access derives from.
    unsafe { Unqueued::new(n.0) }
}
