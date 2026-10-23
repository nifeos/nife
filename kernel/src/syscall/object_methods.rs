//! **The methods of the smaller objects**: `Notification`, `Reboot`, `Timer`, `Process`, the
//! interrupt and virtio capabilities, `PortRange`, the rendezvous badge and reap, and the two helpers
//! [`invoke`](super::invoke) leaves its rare Reply and abort paths to. Each is out of line; see
//! notes/syscall-boundary.md.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// **A Reply method other than `REPLY`**: `REPLY_CAPABILITY` answers with `a0`, `a1` and the
/// capability in our slot `carried` (§255 (each socket is its own capability)). `GRANT` on it is
/// checked under the hold that files it, and a refused source consumes nothing, so this Reply can
/// still answer. Returns 0, or 1 when the answer went but the copy did not reach the caller (a full
/// table, or a caller no longer waiting), so the server can undo what it minted.
///
/// Out of line, handed the syscall's own registers, and looking the Reply up again, so the arm in
/// [`invoke`](super::invoke) is a jump; plain `REPLY` stays inline.
#[inline(never)]
pub(super) fn reply_other(
    slot: u64,
    method: u64,
    a0: u64,
    a1: u64,
    carried: u64,
) -> Result<i64, Error> {
    if method != abi::reply::REPLY_CAPABILITY {
        return Err(Error::BadMethod);
    }
    let reply = sched::current_cap(slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::Reply(tid) = reply.object else {
        return Err(Error::BadMethod);
    };
    // As for `REPLY`: minted WRITE-only and without GRANT.
    if !reply.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let filed = sched::ipc_reply_capability(tid, [a0, a1], carried)?;
    // One-shot, as for `REPLY`.
    let _ = sched::delete_current_cap(slot);
    Ok(if filed { 0 } else { 1 })
}

/// **Which error an aborted `SEND`, `SEND_CAP` or `CALL` answers.** `NotPermitted` when the
/// endpoint carries an interrupt and refused the deposit (DECISIONS §101 (notification objects),
/// amended 2026-09-26, ruling B), `Gone` when it was stale or revoked. `#[cold]`, because the three
/// arms that call it are on the IPC fastpath.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
pub(super) fn aborted_send_error() -> Error {
    if sched::take_ipc_refused() {
        Error::NotPermitted
    } else {
        Error::Gone
    }
}

/// The four `Notification` methods (milestone 151, DECISIONS §101). Rights are §101's: `WRITE` to
/// signal and to bind, `READ` to wait and to poll. `BIND` also needs `WRITE` on the thread it names,
/// because binding changes what that thread's receives return. See `abi::notification`.
#[inline(never)]
pub(super) fn notification_invoke(
    rights: Rights,
    id: sched::NotificationId,
    method: u64,
    a0: u64,
) -> Result<i64, Error> {
    match method {
        abi::notification::SIGNAL => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::notification_signal(id, a0)?;
            Ok(0)
        }
        abi::notification::WAIT => {
            if !rights.allows(Rights::READ) {
                return Err(Error::NotPermitted);
            }
            sched::notification_wait(id).map(|w| w as i64)
        }
        abi::notification::POLL => {
            if !rights.allows(Rights::READ) {
                return Err(Error::NotPermitted);
            }
            sched::notification_poll(id).map(|w| w as i64)
        }
        abi::notification::BIND => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            let thread = sched::current_cap(a0).map_err(|_| Error::NoSuchSlot)?;
            let Object::ThreadControlBlock(tid) = thread.object else {
                return Err(Error::WrongObject);
            };
            if !thread.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::notification_bind(id, tid)?;
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}

/// `Reboot::REBOOT` (DECISIONS §251 (restarting the machine is a kernel object the progenitor
/// hands out)): restart the machine, or answer one of the four `abi::Error::Reset…` reasons for
/// not. See kernel/src/reboot.rs.
#[inline(never)]
pub(super) fn reboot_invoke(method: u64) -> Result<i64, Error> {
    match method {
        abi::reboot::REBOOT => Err(crate::reboot::restart()),
        _ => Err(Error::BadMethod),
    }
}

/// The two `Timer` methods (milestone 106, DECISIONS §147). `WRITE` on the timer for both, and
/// for `ARM` also `WRITE` on the notification it names, because arming is signalling later and
/// `SIGNAL` takes `WRITE`. See `abi::timer`.
#[inline(never)]
pub(super) fn timer_invoke(
    rights: Rights,
    id: sched::TimerId,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    if !rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    match method {
        abi::timer::ARM => {
            let target = sched::current_cap(a1).map_err(|_| Error::NoSuchSlot)?;
            let Object::Notification(notification) = target.object else {
                return Err(Error::WrongObject);
            };
            if !target.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::timer_arm(id, a0, notification, a2)?;
            Ok(0)
        }
        abi::timer::CANCEL => sched::timer_cancel(id).map(i64::from),
        _ => Err(Error::BadMethod),
    }
}

/// `Rendezvous::BADGE`: **mint a badged copy of an endpoint**. The copy names `ep` with the
/// holder's own `rights` and `new_badge`, in a free slot of the caller's table, and returns the
/// slot; `OutOfMemory` when the table is full.
///
/// `NotPermitted` without `GRANT` (minting a delegatable view is delegation, the gate `SEND_CAP`
/// uses), for a zero badge, or for an already-badged source: a badge is set once and never
/// changed, seL4's rule, so a server can trust the badge it is delivered.
#[inline(never)]
pub(super) fn rendezvous_badge(
    ep: sched::RendezvousId,
    rights: Rights,
    held_badge: u64,
    new_badge: u64,
) -> Result<i64, Error> {
    if !rights.allows(Rights::GRANT) || new_badge == 0 || held_badge != 0 {
        return Err(Error::NotPermitted);
    }
    let slot = sched::grant(crate::cap::rendezvous_cap_badged(ep, rights, new_badge))
        .map_err(|_| Error::OutOfMemory)?;
    Ok(slot as i64)
}

/// `Rendezvous::REAP`: collect the corpse of `tid`, a thread this rendezvous supervises (DECISIONS
/// §32 (a supervisor may collect a corpse without being able to build one)). The control half
/// `rendezvous::SURVEY` is the view half of; the caller checked `READ`.
#[inline(never)]
pub(super) fn rendezvous_reap(ep: crate::sched::RendezvousId, tid: u64) -> Result<i64, Error> {
    sched::reap_supervised(ep, tid)?;
    Ok(0)
}

/// `Process::DESTROY` (milestone 812): end the process and every thread in it. `WRITE`.
#[inline(never)]
pub(super) fn process_invoke(
    rights: Rights,
    pid: crate::sched::ProcessId,
    method: u64,
) -> Result<i64, Error> {
    match method {
        abi::process::DESTROY => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::destroy_process(pid)?;
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}

/// `Irq::WAIT`: block on the endpoint the kernel routed this interrupt to; the interrupt arrives
/// as a message (`sched::irq_notify`). `WrongObject` when the line has no route.
#[inline(never)]
pub(super) fn irq_wait(frame: &mut TrapFrame, intid: u32) -> Result<i64, Error> {
    let ep = sched::irq_route(intid).ok_or(Error::WrongObject)?;
    let m = sched::ipc_receive(ep);
    // Unconditional: a bound notification's wake carries `BOUND` and its word here, and an
    // interrupt's mailbox has zeros in both, which an unbound driver reads as before.
    frame.set_arg(1, m[1]);
    frame.set_arg(4, m[4]);
    Ok(m[0] as i64)
}

/// `Irq::ACK`: unmask the interrupt at the controller, which masked it when it fired, once the
/// driver has serviced the device.
#[inline(never)]
pub(super) fn irq_ack(intid: u32) -> Result<i64, Error> {
    crate::arch::irq::enable(intid);
    Ok(0)
}

/// `Virtio`'s four register-level methods (`READ_REGISTER`, `WRITE_REGISTER`, `SETUP_QUEUE`,
/// `NOTIFY`). `DeviceRefused` for a DMA address outside the device's confinement.
#[inline(never)]
pub(super) fn virtio_invoke(id: usize, method: u64, a0: u64, a1: u64) -> Result<i64, Error> {
    use crate::virtio::TransportError;
    let map = |e: TransportError| match e {
        TransportError::DmaEscape => Error::DeviceRefused,
        _ => Error::WrongObject,
    };
    match method {
        abi::virtio::READ_REGISTER => crate::virtio::read_register(id, a0)
            .map(|v| v as i64)
            .ok_or(Error::WrongObject),
        abi::virtio::WRITE_REGISTER => crate::virtio::write_register(id, a0, a1 as u32)
            .map(|_| 0)
            .map_err(map),
        abi::virtio::SETUP_QUEUE => crate::virtio::setup_queue(id, a0 as u16, a1 as u16)
            .map(|_| 0)
            .map_err(map),
        abi::virtio::NOTIFY => crate::virtio::notify(id, a0 as u16).map(|_| 0).map_err(map),
        _ => Err(Error::BadMethod),
    }
}

/// `PortRange::REVOKE`: take the port range back from every other holder. Needs `GRANT`. The only
/// method a port capability answers; enforcement is at the context switch (the TSS I/O bitmap).
///
/// `#[cold]` as well as out of line, which keeps it out of the closure `script/fastpath-footprint`
/// measures (notes/syscall-boundary.md).
#[cfg(target_arch = "x86_64")]
#[cold]
#[inline(never)]
pub(super) fn port_range_invoke(
    rights: Rights,
    base: u16,
    count: u16,
    method: u64,
) -> Result<i64, Error> {
    match method {
        abi::port_range::REVOKE => {
            if !rights.allows(Rights::GRANT) {
                return Err(Error::NotPermitted);
            }
            crate::revoke::revoke_port_range_from_others(base, count);
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}
