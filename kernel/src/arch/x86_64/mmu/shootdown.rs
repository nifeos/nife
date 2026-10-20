//! **TLB shootdown**: the one cross-CPU invalidation in flight, broadcast by NMI, and the
//! port-grant revocation that rides the same mechanism.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::instructions`, `super::irq` and
//! `super::segments` name the `x86_64` items the index imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The [`SHOOTDOWN_VA`] value meaning "discard everything", as opposed to one page. `u64::MAX` is
/// not a canonical address, so it can never be a real `va` and needs no separate flag word.
pub(super) const SHOOTDOWN_ALL: u64 = u64::MAX;

/// **The one shootdown in flight**, and the lock that makes it one.
///
/// A raw `AtomicBool` rather than an [`crate::sync::IrqSafeMutex`], deliberately, and the reason is
/// the whole design below: a core spinning here is still reachable, because the message that ends
/// the spin is an NMI. [`broadcast`] masks interrupts itself for as long as it holds this, because
/// not every caller arrives masked (see its own comment for the hang that taught that).
static SHOOTDOWN_LOCK: AtomicBool = AtomicBool::new(false);

/// The payload of the round in flight, read according to [`SHOOTDOWN_KIND`]: a virtual address (or
/// [`SHOOTDOWN_ALL`]) for [`KIND_PAGE`], and a packed `(base, count)` port range for
/// [`KIND_PORT_GRANT`]. Written under [`SHOOTDOWN_LOCK`], published by the `Release` store to
/// `SHOOTDOWN_PENDING` that follows it.
///
/// **One word for two kinds rather than two words**, because only one round is ever in flight and a
/// second static would be a second thing to keep in step with the first. The name kept its `_VA`
/// spelling: it is what every existing reader calls it, and renaming a static that carries the older
/// meaning in every one of them would be a naming decision this lane does not get to make.
static SHOOTDOWN_VA: AtomicU64 = AtomicU64::new(0);

/// **What the cores named by [`SHOOTDOWN_PENDING`] are being asked to do**, added by
/// milestone 315 (a port revoke that reaches every core). Until then the answer was always
/// "invalidate a translation" and did not need saying.
///
/// Written under [`SHOOTDOWN_LOCK`] and published by the same `Release` store to
/// `SHOOTDOWN_PENDING` that publishes [`SHOOTDOWN_VA`], so a handler that sees its own bit has seen
/// both halves of the message.
static SHOOTDOWN_KIND: AtomicU8 = AtomicU8::new(KIND_PAGE);

/// [`SHOOTDOWN_KIND`]: discharge [`SHOOTDOWN_VA`] from the TLB, one page or everything.
const KIND_PAGE: u8 = 0;

/// [`SHOOTDOWN_KIND`]: take the port range packed into [`SHOOTDOWN_VA`] out of this core's TSS I/O
/// permission bitmap if it is the one installed there (milestone 315). Nothing about memory, and it
/// shares this protocol for one reason: the NMI is the only message on x86 that reaches a core with
/// interrupts masked, which is where a revoked holder's core routinely is.
const KIND_PORT_GRANT: u8 = 1;

/// Pack a port range into [`SHOOTDOWN_VA`]'s word. Both halves are `u16`, so this is lossless and
/// needs no sentinel: a range is never confused with an address because [`SHOOTDOWN_KIND`] says
/// which one the word is.
const fn pack_port_range(base: u16, count: u16) -> u64 {
    (base as u64) << 16 | count as u64
}

/// **Which cores have not yet acknowledged**, as a bitmask of cpu ids. The sender sets it and spins
/// until it reads zero; each target clears its own bit from the NMI handler. A mask rather than a
/// countdown so that an NMI from any other source (or a late one from the previous round) finds its
/// bit already clear and is a no-op instead of an acknowledgement nobody owed.
static SHOOTDOWN_PENDING: AtomicUsize = AtomicUsize::new(0);

/// **Make every other online CPU discharge `va` from its TLB, and do not return until they have.**
///
/// # Why this is an NMI and not an ordinary IPI
///
/// This is called from inside [`unmap_page`], which holds `KERNEL_MMU`, an `IrqSafeMutex`: **this
/// core reaches here with interrupts masked, and so does every other core running the same code.**
/// (`flush_asid` reaches it unmasked, which is why [`broadcast`] masks for itself.)
/// A maskable IPI would therefore deadlock on the first concurrent spawn-and-reap: core A holds
/// `KERNEL_MMU` and waits for core B's acknowledgement, while core B spins for `KERNEL_MMU` with
/// interrupts off and can never take the message that would let A finish. That is not a corner
/// case; it is what two cores running the thread tests do continuously.
///
/// notes/riscv-tlb-shootdown.md already names the property that makes the RISC-V protocol work,
/// and it is exactly the one at issue: *"The IPI arrives as an M-mode software interrupt, so a hart
/// with S-mode interrupts masked still services it. That is not a footnote: without it, any kernel
/// code that disables interrupts and spins would deadlock whoever was flushing, and this kernel
/// disables interrupts routinely."* RISC-V gets that from a privilege level below the kernel's, and
/// aarch64 does not need it at all because `tlbi ..., is` is broadcast by the hardware. x86 has
/// neither, and has exactly one delivery mode `cli` cannot suppress, so the NMI is not a stylistic
/// choice here: it is the only message that keeps the guarantee.
///
/// # What it does not do
///
/// Nothing before the local APIC is up, and nothing when this is the only online core: boot maps a
/// great many pages with nobody to tell.
///
/// # BUGS
///
/// **One page per round trip, outside two batched paths.** A thread reap unmaps
/// [`crate::thread::STACK_PAGES`] pages and pays for six full shootdowns, where a batched protocol
/// (collect the range, send once) would pay for one. The two paths that move whole runs batch:
/// `revoke::revoke_region` through [`super::unmap_user_pages`] and `PageFrame::MAP` through
/// [`super::UserMapBatch`]. Each keeps the obligation inside the arch layer (a closure, a `Drop`)
/// rather than handing its caller an undischarged one, which is what `paging::TlbFlush` exists to
/// prevent. The reap is worth the same move once `script/bench` says it matters.
pub(super) fn shoot_down_others(va: u64) {
    broadcast(KIND_PAGE, va);
}

/// **Take a port range out of every other online core's TSS bitmap** (milestone 315), the remote
/// half of `segments::revoke_port_grant_everywhere`, which carries the defect this closes and the
/// measurement that found it.
///
/// It is [`shoot_down_others`]'s protocol with a different step at the far end, and reusing it is
/// the point: the acknowledgement mask, the single-round lock and the NMI delivery are the parts
/// that are hard to get right, and there is no second copy of them to keep in step.
pub(in crate::arch::x86_64) fn revoke_port_grant_others(base: u16, count: u16) {
    broadcast(KIND_PORT_GRANT, pack_port_range(base, count));
}

/// Send `kind` with `word` to every other online core and spin until each has acknowledged. The
/// body [`shoot_down_others`] and [`revoke_port_grant_others`] share; the doc above
/// `shoot_down_others` is the design, because that is the caller the design was written for.
fn broadcast(kind: u8, word: u64) {
    // Nothing to tell, or nothing to tell it with. `is_local_apic_ready` is the earlier of the two:
    // `mmu::init` maps the whole machine before `init_local_apic` runs.
    if !super::irq::is_local_apic_ready() {
        return;
    }

    // **Masked for the whole round, whatever the caller held** (2026-10-13 UTC). This function was
    // written for `unmap_page`, which calls it under `KERNEL_MMU` and so with interrupts already
    // off, and it assumed every caller would. `flush_asid` does not: an `AddressSpace` drop calls it
    // with no lock held and interrupts on. A tick landing in the ack wait below then preempted a
    // thread that held `SHOOTDOWN_LOCK`. When that thread was switched back in, its `finish_switch`
    // reaped the corpse that had run in between, the reap freed the corpse's kernel stack, and
    // `unmap_page` came back here to spin, interrupts masked, on the lock its own thread held. The
    // other core then spun on `KERNEL_MMU`, also masked, and the machine stopped taking ticks, so
    // even the test watchdogs went quiet. That is the x86_64 hang in
    // `a_child_can_neither_learn_nor_forge_its_label`, recorded in the BUGS of milestone 105 (the
    // two forks), caught with both cores' stacks under gdb. Masking here also pins the `cpu::id()`
    // read below to the core the round runs on, which a preemptible caller could otherwise leave
    // behind.
    let irqs_were_enabled = crate::arch::interrupts::disable();
    #[cfg(test)]
    if crate::arch::interrupts::is_enabled() {
        ROUND_RAN_UNMASKED.store(true, Ordering::Relaxed);
    }
    let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
    if others == 0 {
        crate::arch::interrupts::restore(irqs_were_enabled);
        return;
    }

    // One round at a time. See this static's own comment for why a raw spin is the right lock here
    // and not a lapse: a core waiting here is still reachable by the only message that matters.
    while SHOOTDOWN_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }

    SHOOTDOWN_KIND.store(kind, Ordering::Relaxed);
    SHOOTDOWN_VA.store(word, Ordering::Relaxed);
    // Release: this publishes the two stores above. A handler that sees its own bit set has, by the
    // matching `Acquire` load, also seen the message that bit is about.
    SHOOTDOWN_PENDING.store(others, Ordering::Release);

    for cpu in cpu_set::cpus_in(others) {
        // The local APIC id *is* the cpu id on this port: `smp::seat_cpus_from_acpi` seats every
        // core at the slot its own APIC id names, which is what makes `smp::hwid(id) == Some(id)`
        // for a filled slot. Asserted rather than assumed, because the whole protocol is addressed
        // by this number.
        debug_assert_eq!(
            crate::smp::hwid(cpu),
            Some(cpu as u64),
            "shootdown addressed cpu {cpu}, whose local APIC id is not its own id"
        );
        super::irq::send_nmi(cpu as u8);
    }

    while SHOOTDOWN_PENDING.load(Ordering::Acquire) != 0 {
        core::hint::spin_loop();
    }

    SHOOTDOWN_LOCK.store(false, Ordering::Release);
    crate::arch::interrupts::restore(irqs_were_enabled);
}

/// **Serve a shootdown NMI**: the receiving half of [`shoot_down_others`].
///
/// Called from the trap path for vector 2 and from nowhere else. Returns whether this NMI was one
/// of ours, so the caller can tell a shootdown from an NMI this kernel did not send.
///
/// # It names its core from the local APIC
///
/// `isr_paranoid` (trap.s) has already put the kernel's GS base in place, so `cpu::id()` would
/// answer correctly here even in the windows around `swapgs` where the interrupted code held the
/// user's. This still names its core from the local APIC's own id register, which is hardware
/// ground truth and needs no per-CPU pointer, so the shootdown does not depend on the paranoid
/// entry being right. Nothing here takes a lock or prints.
pub fn serve_shootdown_nmi() -> bool {
    let me = super::irq::local_apic_id() as usize;
    let bit = 1usize << me;

    // Acquire: pairs with the sender's `Release` store, so seeing our bit means seeing its `va`.
    if SHOOTDOWN_PENDING.load(Ordering::Acquire) & bit == 0 {
        return false;
    }

    let word = SHOOTDOWN_VA.load(Ordering::Relaxed);
    match SHOOTDOWN_KIND.load(Ordering::Relaxed) {
        // The port revoke (milestone 315). It names this core by the APIC id read above rather than
        // by `cpu::id()`, for the reason this function's own heading gives.
        KIND_PORT_GRANT => {
            super::segments::revoke_installed_port_grant_on(me, (word >> 16) as u16, word as u16);
        }
        // SAFETY: rewriting CR3 with the value it already holds changes no mapping and invalidates
        // every non-global entry, which with `CR4.PGE` clear is every entry. See `flush_asid`.
        _ if word == SHOOTDOWN_ALL => unsafe { install(read_cr3()) },
        _ => super::instructions::invlpg(word),
    }

    // Release: the invalidate above is complete before the sender may observe the acknowledgement
    // and go on to hand the frame to somebody else.
    SHOOTDOWN_PENDING.fetch_and(!bit, Ordering::Release);
    true
}

/// Set if a round ever ran with interrupts enabled: the witness [`tests`] reads. Test builds only.
#[cfg(test)]
static ROUND_RAN_UNMASKED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
mod tests {
    use super::*;

    /// **A shootdown round masks interrupts even when its caller did not.** `flush_asid` is the
    /// caller that arrives unmasked (an `AddressSpace` drop), and a tick taken inside its round
    /// could preempt the holder of [`SHOOTDOWN_LOCK`] and deadlock the machine; [`broadcast`]'s
    /// comment has the hang. Red without the masking: this calls `flush_asid` with interrupts on,
    /// on the two-core machine the x86_64 leg boots, and the round records that it ran unmasked.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.mmu.shootdown.tests.a_shootdown_round_masks_interrupts_whatever_its_caller_held.patch`
    ///
    /// Name: provisional (lane `maintainer/supervision-label-hang`, 2026-10-13 UTC).
    #[test_case]
    fn a_shootdown_round_masks_interrupts_whatever_its_caller_held() {
        let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
        if others == 0 {
            crate::testing::skip!("one core online: no round is sent, so there is nothing to mask");
        }
        let was = crate::arch::interrupts::disable();
        crate::arch::interrupts::enable();
        ROUND_RAN_UNMASKED.store(false, Ordering::Relaxed);
        super::super::flush_asid(0);
        let unmasked = ROUND_RAN_UNMASKED.load(Ordering::Relaxed);
        crate::arch::interrupts::restore(was);
        assert!(
            !unmasked,
            "a shootdown round ran with interrupts enabled: a tick inside it can preempt the \
             holder of SHOOTDOWN_LOCK and wedge every core"
        );
    }
}
