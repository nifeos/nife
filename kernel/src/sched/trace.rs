use core::sync::atomic::{AtomicU64, Ordering};

/// Events per core. 16 is enough to see the whole final approach to a wedge (a block, the
/// wakes around it, the switch that stranded something) without turning the dump into a log.
pub(crate) const DEPTH: usize = 16;

/// What happened. The discriminant is packed into the entry's top byte.
#[derive(Clone, Copy)]
#[repr(u8)]
pub(crate) enum Event {
    /// `schedule()` picked `tid` and marked it Running on this core.
    SwitchTo = 1,
    /// The thread running here marked itself Blocked (aux = low byte of the rendezvous name).
    BlockSelf = 2,
    /// This core moved `tid` Blocked -> Ready onto a run queue (rendezvous, reply, or abort).
    Wake = 3,
    /// This core saw `tid` still on a cpu and parked the wake (`wake_pending`).
    WakeDeferred = 4,
    /// This core completed a parked wake in `finish_switch`.
    WakeCompleted = 5,
    /// This core pushed `tid` into cpu `aux`'s inbox (placement or load-aware wake).
    PlaceRemote = 6,
    /// This core served a steal: handed `tid` to requester cpu `aux`.
    StealServe = 7,
    /// This core drained its inbox; the tid field carries the count moved.
    InboxDrain = 8,
    /// This core REFUSED a wake of `tid`: the target was parked in IPC and the waker had
    /// delivered nothing (no message, no signal, no abort). The boot-8 gate firing; on a
    /// healthy boot this event never appears, so its presence in a bench dump is the finding.
    WakeRefused = 9,
    /// This core switched into `tid`, and the core it last ran on was a different one: the
    /// cross-core handoff count. [`Event::PlaceRemote`] misses every migration an IPC workload
    /// makes, because a rendezvous wake readies its peer on the waker's core. `aux` is the core it
    /// came from. Recorded only by a `soak` build, whose `last_cpu` comparison is gated off the
    /// switch; the variant stays so [`dump`]'s `11 => "moved"` arm names something.
    #[cfg_attr(not(feature = "soak_test"), allow(dead_code))]
    Migrated = 11,
    /// This core set `ipc_served` on `tid`: a delivery completed its parked IPC. `aux` names the
    /// delivering site (1 send, 2 receive-collect, 3 `send_cap`, 4 `receive_cap`-collect, 5 call,
    /// 6 reply, 7 irq signal, 8 death message, 9 a notification signal to a waiter, 10 one to its
    /// bound receiver), so a dump says who served a thread (notes/visionfive2.md, fifth stop).
    Served = 10,
}

/// How many discriminants [`Event`] has, which sizes the per-core totals below.
///
/// One more than the largest variant, because the discriminants start at 1 so that a zeroed
/// ring slot is distinguishable from a recorded `SwitchTo`. Read only by the `soak`-gated
/// counter array; a `const` costs no bytes either way, so it stays visible to every build.
#[cfg_attr(not(feature = "soak_test"), allow(dead_code))]
pub(crate) const KINDS: usize = 12;

struct Ring {
    seq: AtomicU64,
    slots: [AtomicU64; DEPTH],
    /// A total per event kind beside the ring, which a soak needs: a refused wake hours into a
    /// run has long scrolled out of sixteen slots. Per core, as the ring is, so no shared atomic
    /// sits on the IPC fastpath. `soak_test` only: the load-add-store per event was part of what
    /// pushed `ipc_fastpath` 5.7% over its bound on aarch64.
    #[cfg(feature = "soak_test")]
    counts: [AtomicU64; KINDS],
}

#[allow(clippy::declare_interior_mutable_const)]
const EMPTY_RING: Ring = Ring {
    seq: AtomicU64::new(0),
    slots: [const { AtomicU64::new(0) }; DEPTH],
    #[cfg(feature = "soak_test")]
    counts: [const { AtomicU64::new(0) }; KINDS],
};

static RINGS: [Ring; crate::cpu::MAX_CPUS] = [EMPTY_RING; crate::cpu::MAX_CPUS];

/// Record an event on the calling core's ring.
///
/// `#[inline(never)]`, so the fast paths carry one copy rather than a per-CPU read, bounds check,
/// panic pad and packed store per site (milestone 758): inlined, the ring index was among the
/// largest lines in `script/fastpath-footprint`'s closures on every architecture.
///
/// Context: every call site has interrupts masked (under `IPC_TABLES` or in interrupt context), so
/// the owning core cannot interleave with itself.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.trace.record")
)]
pub(crate) fn record(kind: Event, tid: u64, aux: u8) {
    let ring = &RINGS[crate::cpu::id()];
    let seq = ring.seq.fetch_add(1, Ordering::Relaxed);
    // kind in the top byte, aux below it, the tid's low 48 bits under that. A tid is
    // (generation << 32) | slot with slot < 256; 48 bits keeps 16 bits of generation,
    // plenty to disambiguate in a dump.
    let entry = ((kind as u64) << 56) | ((aux as u64) << 48) | (tid & 0x0000_FFFF_FFFF_FFFF);
    ring.slots[(seq as usize) % DEPTH].store(entry, Ordering::Relaxed);
    // The running total, on this core's own line, one writer per core. Soak builds only: see the
    // field.
    #[cfg(feature = "soak_test")]
    {
        let total = &ring.counts[kind as usize];
        total.store(
            total.load(Ordering::Relaxed).wrapping_add(1),
            Ordering::Relaxed,
        );
    }
}

/// How many times `kind` has happened on this machine since boot, summed over every core: a total
/// rather than a window, so a soak's heartbeat still sees a refusal from hours ago.
///
/// The sum is racy on purpose: cores keep counting while it is read, and "is this still zero" and
/// "how much did it grow" both survive a reader a few events behind.
#[cfg(feature = "soak_test")]
pub(crate) fn counted(kind: Event) -> u64 {
    let mut total = 0u64;
    for ring in &RINGS {
        total = total.wrapping_add(ring.counts[kind as usize].load(Ordering::Relaxed));
    }
    total
}

/// Print core `cpu`'s ring, oldest first. Racy against that core's own writes, by design.
pub(crate) fn dump(cpu: usize) {
    let ring = &RINGS[cpu];
    let seq = ring.seq.load(Ordering::Relaxed);
    if seq == 0 {
        return;
    }
    let start = seq.saturating_sub(DEPTH as u64);
    crate::print!("    core {cpu} events [{start}..{seq}):");
    for s in start..seq {
        let e = ring.slots[(s as usize) % DEPTH].load(Ordering::Relaxed);
        let (kind, aux, tid) = (e >> 56, (e >> 48) & 0xff, e & 0x0000_FFFF_FFFF_FFFF);
        let name = match kind {
            1 => "switch",
            2 => "block",
            3 => "wake",
            4 => "wake?",
            5 => "wake+",
            6 => "place",
            7 => "steal",
            8 => "drain",
            9 => "refuse",
            10 => "serve",
            11 => "moved",
            _ => "?",
        };
        crate::print!(" {name}:{tid:#x}");
        if matches!(kind, 2 | 6 | 7 | 10 | 11) {
            crate::print!("/{aux}");
        }
    }
    crate::println!();
}
