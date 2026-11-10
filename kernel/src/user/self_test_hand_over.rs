//! **What the kernel hands the progenitor for the boot self-test's userspace half** (milestone
//! 1063): the page that carries the kernel's half (`boot_ladder::kernel_half`), the JH7110 TRNG
//! service the `clock` and `trng` checks read, and disk 0's virtio trio for `blockdev`. The
//! progenitor passes all of it to `boot_self_test` and keeps none of it. See
//! notes/self-test-checks.md, "The two halves".
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`.

//! # BUGS
//!
//! - The JH7110 TRNG service and disk 0's grant are started here, in the hand-over, by the kernel:
//!   milestone 811 (the boot services leave the kernel) moves every such starter to userspace.
//! - A TRNG service that never sends its readiness report leaves a kernel thread parked on the
//!   receive; the boot carries on after five seconds and `trng` says why.

use boot_ladder::kernel_half::{KernelHalf, flag, word};

use super::*;
use crate::capability_object::{
    Rights, interrupt_capability_rights, page_frame_capability, rendezvous_capability,
    virtio_capability_rights,
};

/// The progenitor's slot for the kernel-half page, `READ | GRANT`, granted on every boot.
pub(super) const SELF_TEST_PAGE_SLOT: u64 = 35;
/// The progenitor's slot for the JH7110 TRNG service's request endpoint, `WRITE | GRANT`, granted
/// only on a JH7110 whose service answered.
pub(super) const TRNG_SLOT: u64 = 36;
/// The progenitor's three slots for disk 0's virtio transport, its interrupt and its DMA page, all
/// carrying `GRANT`, granted only when this machine has a virtio disk.
pub(super) const DISK_SLOTS: [u64; 3] = [37, 38, 39];

/// **Grant the self-test's slots**: build the kernel-half page from the kernel's tally and the
/// facts below, then insert it and whatever devices this machine has.
pub(super) fn grant_self_test(tid: crate::thread::ThreadId) {
    let mut half = KernelHalf {
        tally: crate::self_test::kernel_tally(),
        ..KernelHalf::default()
    };
    if crate::self_test::injects() {
        half.flags |= flag::INJECT;
    }
    // Certainly mapped, and certainly not a user program's: the kernel's own fault counter.
    half.words[word::KERNEL_VIRTUAL_ADDRESS] =
        &raw const crate::arch::exceptions::USER_FAULTS as u64;

    if let Some(request) = start_jh7110_trng(&mut half) {
        insert(
            tid,
            rendezvous_capability(request, Rights::WRITE.union(Rights::GRANT)),
            TRNG_SLOT,
            "insert the JH7110 TRNG service",
        );
    }
    if let Some(disk) = wire_disk_zero(&mut half) {
        let [transport, interrupt, dma] = DISK_SLOTS;
        insert(
            tid,
            virtio_capability_rights(disk.vid, Rights::WRITE.union(Rights::GRANT)),
            transport,
            "insert disk 0's virtio transport",
        );
        insert(
            tid,
            interrupt_capability_rights(disk.intid, Rights::READ.union(Rights::GRANT)),
            interrupt,
            "insert disk 0's interrupt",
        );
        insert(
            tid,
            page_frame_capability(
                disk.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            dma,
            "insert disk 0's DMA page",
        );
    }

    let words = half.to_words();
    // Zeroed, so a previous occupant's bytes are not visible past the words written.
    let phys = crate::memory::alloc_zeroed()
        .expect("no frame for the self-test page")
        .addr();
    // SAFETY: `phys` names that frame, direct-mapped and owned by nobody else yet, and the words
    // are 120 bytes, far under `FRAME_SIZE`, so the copy stays inside the frame.
    unsafe {
        core::ptr::copy_nonoverlapping(
            words.as_ptr(),
            crate::arch::mmu::phys_to_virt(phys) as *mut u64,
            words.len(),
        );
    }
    insert(
        tid,
        page_frame_capability(phys, Rights::READ.union(Rights::GRANT)),
        SELF_TEST_PAGE_SLOT,
        "insert the self-test page",
    );
}

/// Insert `capability` at `slot` in the progenitor's table, or panic with `what`.
fn insert(
    tid: crate::thread::ThreadId,
    capability: crate::capability_object::Capability,
    slot: u64,
    what: &str,
) {
    let at = crate::sched::thread_control_block_insert_capability(tid, capability, Some(slot))
        .unwrap_or_else(|_| panic!("{what}"));
    assert_eq!(at, slot);
}

/// **Disk 0 for `blockdev`**: virtio-mmio first, else the PCIe virtio-blk, registered with the
/// kernel's virtio confinement, its interrupt routed and enabled, and a zeroed DMA page that
/// records its own physical base (`abi::virtio::DIRECT_MEMORY_ACCESS_REGION_BASE_PHYSICAL_ADDRESS_WORD_OFFSET`).
/// The hand-over's file service, installer and surveyor use disks of their own. `None` with no
/// virtio disk, which is every boot `boot-check` makes and every board.
fn wire_disk_zero(half: &mut KernelHalf) -> Option<VirtioBootGrant> {
    let (transport, intid, rid, bit) = if let Some(d) = crate::virtio::find_block_device() {
        (
            crate::virtio::Transport::Mmio {
                mmio_phys: d.mmio_phys,
            },
            d.intid,
            None, // virtio-mmio has no IOMMU in front of it on either board
            flag::DISK_MEMORY_MAPPED,
        )
    } else {
        let d = crate::pci::find_block_device()?;
        (
            crate::virtio::Transport::pci(&d),
            d.intid,
            Some(d.rid), // the PCIe requester id the IOMMU keys its tables on
            flag::DISK_PCI,
        )
    };
    // Zeroed first, so no stale descriptor is visible to the device or the driver's first read.
    let dma = crate::memory::alloc_zeroed()
        .expect("no DMA frame for disk 0")
        .addr();
    write_dma_phys(dma);
    crate::sched::bind_interrupt(intid, crate::sched::create_rendezvous());
    crate::arch::interrupt_controller::enable(intid);
    let vid = crate::virtio::register(transport, dma, FRAME_SIZE, rid);
    half.flags |= bit;
    Some(VirtioBootGrant { vid, intid, dma })
}

/// How long the hand-over waits for the TRNG service's readiness report, in seconds: far beyond an
/// honest bring-up, which takes milliseconds.
const TRNG_READY_SECONDS: u64 = 5;

/// **Bring the JH7110 TRNG up and wire its service**, recording in `half` what the clock-and-reset
/// controller and the service answered. `Some(request)` when the service sent its readiness report
/// in time. `None` on every machine but a JH7110, which is every machine CI boots.
fn start_jh7110_trng(half: &mut KernelHalf) -> Option<crate::sched::RendezvousId> {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    static REPORT: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
    static DONE: AtomicBool = AtomicBool::new(false);

    let device = entropy_service::jh7110_trng_device()?;
    half.flags |= flag::RANDOM_NUMBER_GENERATOR_FOUND;
    if device.status_okay {
        half.flags |= flag::RANDOM_NUMBER_GENERATOR_STATUS_OKAY;
    }
    half.words[word::RANDOM_NUMBER_GENERATOR_REGISTERS] = device.registers_physical_address;
    let Some(image) = program("jh7110_entropy") else {
        half.flags |= flag::RANDOM_NUMBER_GENERATOR_PROGRAM_MISSING;
        return None;
    };
    let start = crate::arch::timer::now();
    let w = entropy_service::ensure(image, entropy_service::Bus::Jh7110)?;
    half.flags |= flag::RANDOM_NUMBER_GENERATOR_WIRED;
    if let Some(report) = w.clock {
        half.flags |= flag::CLOCK_REPORTED;
        for (ok, bit) in [
            (report.has_clocks_running(), flag::CLOCK_RUNNING),
            (report.released, flag::CLOCK_RELEASED),
            (report.was_already_up(), flag::CLOCK_ALREADY_UP),
        ] {
            if ok {
                half.flags |= bit;
            }
        }
        let pack = |w: [u32; jh7110_clock_and_reset::MAXIMUM_RECORDED_CLOCKS]| {
            u64::from(w[0]) | (u64::from(w[1]) << 32)
        };
        half.words[word::CLOCK_BEFORE] = pack(report.clock_before);
        half.words[word::CLOCK_AFTER] = pack(report.clock_after);
        half.words[word::CLOCK_POLLS] = u64::from(report.polls);
    }
    if let Some(window) = entropy_service::jh7110_crg_window() {
        half.words[word::CLOCK_WINDOW] = window.base_physical_address;
        if window.from_tree {
            half.flags |= flag::CLOCK_FROM_TREE;
        }
    }

    // The report arrives on a helper thread, because a receive has no deadline.
    let ready = w.ready?;
    DONE.store(false, Ordering::SeqCst);
    crate::sched::spawn(move || {
        let report = crate::sched::ipc_receive(ready);
        for (slot, word) in REPORT.iter().zip(report) {
            slot.store(word, Ordering::Relaxed);
        }
        DONE.store(true, Ordering::SeqCst);
    })?;
    let deadline = start + TRNG_READY_SECONDS * crate::arch::timer::frequency();
    while !DONE.load(Ordering::SeqCst) && crate::arch::timer::now() < deadline {
        crate::sched::yield_now();
    }
    if !DONE.load(Ordering::SeqCst) {
        return None;
    }
    half.flags |= flag::RANDOM_NUMBER_GENERATOR_ANSWERED;
    half.words[word::RANDOM_NUMBER_GENERATOR_BRING_UP_TICKS] =
        crate::arch::timer::now().wrapping_sub(start);
    for (i, slot) in REPORT.iter().enumerate() {
        half.words[word::RANDOM_NUMBER_GENERATOR_REPORT + i] = slot.load(Ordering::Relaxed);
    }
    if (half.words[word::RANDOM_NUMBER_GENERATOR_REPORT + 1] >> 32) as u32
        & jh7110_entropy::STATUS_256_BIT_OUTPUT
        != 0
    {
        half.flags |= flag::RANDOM_NUMBER_GENERATOR_256_BIT;
    }
    Some(w.request)
}
