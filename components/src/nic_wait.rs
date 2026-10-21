//! **Waiting for the NIC between polls**, for `net_stack`'s `service_until` and its DHCP bring-up.
//!
//! Name: provisional, the riscv-cpu-net-flake lane, 2026-10-13 (UTC). Moved out of `net_stack.rs`
//! when the wait learned its caller's deadline, because §282 (when a file is split, the parent
//! becomes an index) lets `net_stack.rs` only shrink. Named for what it does.

use core::sync::atomic::{AtomicU64, Ordering};

use smoltcp::time::Duration;
use socket_contract::polled_wait;

use super::*;

/// **Wait for the NIC to need attention again, without stalling smoltcp's timers or the caller's
/// bound.**
///
/// The obvious loop, "block on the NIC interrupt, service on each wakeup," has a hole that SMP timing
/// on riscv exposed and that a lone core hid: smoltcp drives TCP retransmits, delayed ACKs, and DNS
/// timeouts from *its* clock, advanced only when we call `poll`. If we block on the interrupt alone
/// and the exchange is waiting on one of those timers (a segment we must retransmit because its ACK
/// was dropped), no RX interrupt is coming (the peer is waiting for that retransmit), so the block
/// never returns and the whole pipeline deadlocks with every core idle. aarch64 happened never to
/// drop a segment and so never hit it; riscv under the SMP scatter did. See notes/net/the-outbound-gates.md.
///
/// So the wait ends at the earliest of a frame, smoltcp's next timer and `deadline_ms` (the
/// caller's bound, on `instant`'s clock). It blocks on the interrupt alone only when neither a timer
/// nor a deadline exists.
///
/// **The deadline is what makes `service_until`'s 15 s bound a bound** (2026-10-13 UTC). A listener
/// with nobody connecting, or a connection whose peer has gone silent, leaves smoltcp no timer, and
/// before this the wait blocked until a frame that might never come. The inbound prober holding a
/// connection to nobody (milestone 514 (the HVF std listener hangs in some full runs)) sends
/// nothing, so the std listener's `ACCEPT` never expired, its retries never ran, and the CPU
/// matrix's 2026-10-12 red was a 60 s "lost-wakeup" watchdog rather than a failed accept. A
/// filtered run of the hand-written listener test, which starts no prober, hung the same way every
/// time. `an_accept_nobody_answers_ends_at_the_stacks_bound` (riscv and aarch64) holds it.
/// When a polled NIC last handed `net_stack` a frame, on the counter. One server thread reads and
/// writes it; an atomic only because a `static` must be `Sync`.
static LAST_FRAME: AtomicU64 = AtomicU64::new(0);

/// Note that a polled NIC just handed over a frame, which starts [`poll_without_interrupt`]'s
/// tick of looking instead of sleeping.
pub(super) fn frame_taken() {
    LAST_FRAME.store(now(), Ordering::Relaxed);
}

pub(super) fn wait_for_nic(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    deadline_ms: Option<i64>,
) {
    match dev {
        // A polled NIC sleeps at most `e1000e_transport::POLL_MS`, so the caller's bound holds.
        Nic::Gigabit(nic) => return poll_without_interrupt(iface, nic.frame_waiting(), sockets),
        Nic::DesignWare(nic) => {
            return poll_without_interrupt(iface, nic.frame_waiting(), sockets);
        }
        Nic::Virtio(_) => {}
    }
    let at = instant();
    let left =
        deadline_ms.map(|end| Duration::from_millis((end - at.total_millis()).max(0) as u64));
    let Some(delay) = (match (iface.poll_delay(at, sockets), left) {
        (Some(timer), Some(left)) => Some(timer.min(left)),
        (timer, left) => timer.or(left),
    }) else {
        interrupt_wait(IRQ);
        dev.ack_irq();
        return;
    };
    // Wait for a frame **or** that deadline, whichever comes first (milestone 106 (a wait that ends
    // on either the interrupt or the deadline)). The notification in `WAKE` is bound to this
    // thread, so `Irq::WAIT` ends on either and the hart is idle in between. Until 2026-09-26 this
    // yielded and re-polled instead, spinning a hart through every retransmit backoff
    // (`notes/timed-wait.md`, section 5).
    let micros = delay.total_micros();
    let ticks = abi::timer::counter_ticks_for(
        micros / 1_000_000,
        ((micros % 1_000_000) * 1000) as u32,
        cntfrq(),
    );
    if timer_arm(RETRANSMIT, now().saturating_add(ticks), WAKE, 1) == 0 {
        interrupt_wait(IRQ); // 1: a frame; BOUND: the deadline
        // Whichever ended the wait, leave nothing behind: disarm, and clear a fire that raced the
        // frame, so a stale deadline never ends the next `receive_cap(STACK)` as a bound delivery.
        if timer_cancel(RETRANSMIT) == 0 {
            notification_poll(WAKE);
        }
    } else {
        // No timer granted (a spawner that predates milestone 106): the old yield, so the caller
        // re-polls and smoltcp's timer still fires, at a hart's cost.
        user_mode_runtime::yield_now();
    }
    dev.ack_irq();
}

/// **A polled NIC's wait** (milestone 494 (a driver for the network card a PC actually has)): the
/// `e1000e` server's, and since milestone 53 (the board's own peripherals: network and storage on
/// real silicon) radon's Ethernet port's. Neither holds an interrupt
/// (`kernel/src/user/e1000e_service.rs` says why). A frame already waiting returns at once. Within
/// a scheduler tick of the last frame taken, the wait is a yield, because a sleep could not end
/// before that tick and a stream's next segment comes sooner (`socket_contract::polled_wait` has
/// the argument and its measurements). After that it sleeps until smoltcp's own deadline or
/// `e1000e_transport::POLL_MS`, whichever is sooner, and the timer rounds that up to a tick. The
/// timer and notification are the same pair the virtio server's retransmit wait uses (slots 5
/// and 6).
fn poll_without_interrupt(iface: &mut Interface, frame_waiting: bool, sockets: &mut SocketSet) {
    if frame_waiting {
        return;
    }
    // `cntfrq`'s discipline: a process that cannot know the tick dies saying so, rather than
    // guessing one and silently paying a tick per round trip again.
    let tick_hz = user_mode_runtime::tick_hz()
        .expect("the scheduler tick is unknown: this thread's current-CPU page carries none");
    let tick = cntfrq() / tick_hz.get();
    let due = iface
        .poll_delay(instant(), sockets)
        .map(|d| d.total_micros());
    let micros = match polled_wait::next(
        now(),
        LAST_FRAME.load(Ordering::Relaxed),
        tick,
        due,
        e1000e_transport::POLL_MS * 1000,
    ) {
        polled_wait::Next::Look => {
            user_mode_runtime::yield_now();
            return;
        }
        polled_wait::Next::Sleep(0) => return,
        polled_wait::Next::Sleep(micros) => micros,
    };
    let ticks = abi::timer::counter_ticks_for(0, (micros * 1000) as u32, cntfrq());
    if sleep_until(RETRANSMIT, WAKE, now().saturating_add(ticks)) < 0 {
        user_mode_runtime::yield_now();
    }
}
