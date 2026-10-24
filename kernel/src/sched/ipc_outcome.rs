//! **How a blocking IPC learned it failed**: resolving a rendezvous name, and the aborted and
//! refused flags the syscall layer reads and clears after the primitive returns.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The rendezvous behind a name, or `None` if the name no longer resolves, which a stale
/// `Rendezvous` capability makes ordinary user input. The `'static` is the page's pinning made a
/// lifetime: while the name resolves the page is pinned and direct-mapped.
///
/// Context: caller holds `IPC_TABLES`, which serializes every access to what it returns.
pub(super) fn rendezvous_of(
    sched: &IpcTables,
    ep: RendezvousId,
) -> Option<&'static mut Rendezvous> {
    let phys = *sched.rendezvous_table.get(ep)?;
    // SAFETY: retyped exclusively for this rendezvous, its region pinned while the name resolves,
    // direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *(crate::arch::mmu::phys_to_virt(phys) as *mut Rendezvous) })
}

/// Mark thread `tid`'s blocking IPC as aborted (a stale rendezvous, or one revoked while it
/// blocked); the syscall layer reads and clears it after the primitive returns, and hands back an
/// error. Drops any capability staged for the aborted send.
///
/// `#[cold]`: every call site is the miss of a `rendezvous_of`, on `script/fastpath-footprint`'s
/// closures, so inlined it would put the abort path on the fast path four times over.
///
/// Context: caller holds `IPC_TABLES`.
#[cold]
#[inline(never)]
pub(super) fn set_ipc_aborted(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        t.handshake.abort();
        // A staged delegation or minted `Reply` would otherwise ride this thread's next plain
        // `SEND` to a different rendezvous. The sender keeps its own copy.
        t.outgoing_cap = None;
    }
}

/// Refuse thread `tid`'s send, because the rendezvous carries an interrupt (§101 (notification
/// objects) ruling B): an abort, so the common path pays only the existing `take_ipc_aborted`
/// branch, plus the reason that branch reads ([`take_ipc_refused`]). The sender never parked, so
/// the boot-8 gate is not weakened. `#[cold]` for [`set_ipc_aborted`]'s reason.
///
/// Context: caller holds `IPC_TABLES`.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
pub(super) fn set_ipc_refused(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        t.handshake.abort();
        t.ipc_refused = true;
    }
}

/// **Read and clear why the current thread's aborted send was aborted**: `true` when the rendezvous
/// carries an interrupt and refused it ([`set_ipc_refused`]), `false` when it was stale or revoked.
/// Called by the syscall layer only after [`take_ipc_aborted`] returned `true`, so an IPC that was
/// not aborted never pays for it.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
pub fn take_ipc_refused() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let tid = current_thread_id();
    sched
        .threads
        .get_mut(tid)
        .map(|t| core::mem::take(&mut t.ipc_refused))
        .unwrap_or(false)
}

/// Read and clear the current thread's IPC-aborted flag (object revocation), right after a
/// rendezvous IPC primitive returns: `true` means the rendezvous was stale or revoked while the
/// thread blocked, so the caller gets an error instead of the placeholder result. Kernel-side IPC
/// never sets it, because kernel endpoints are never revoked.
pub fn take_ipc_aborted() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let tid = current_thread_id();
    sched
        .threads
        .get_mut(tid)
        .map(|t| t.handshake.take_aborted())
        .unwrap_or(false)
}
