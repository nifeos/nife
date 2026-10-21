//! Physical memory: find out where RAM is, and hand it out in 4 KiB frames.
//!
//! This is the bottom of the memory hierarchy. Page tables (milestone 4), the kernel
//! heap, DMA buffers (milestone 8), and user process pages (milestone 7) all ultimately
//! ask this for their memory, and there is nothing underneath it to ask.
//!
//! The allocator itself lives in the `frames` crate and the device tree parser in
//! `device_tree_blob`, because both are pure logic and belong in host-testable crates (DECISIONS
//! §7). What's left here is the part that can only happen on the real machine: the **bootstrap**.

use device_tree_blob::Region;
use page_frames::{FRAME_SIZE, PageFrame, PageFrameAllocator, Stats};

use crate::arch::mmu::{phys_to_virt, virt_to_phys};
use crate::println;
use crate::sync::{IrqSafeMutex, rank};

/// The frame allocator.
///
/// `IrqSafeMutex`, not a bare spinlock: an interrupt handler that tried to allocate while
/// the interrupted code held this lock would spin forever waiting for code that cannot
/// run. See sync.rs and DECISIONS §9.
///
/// The discipline that goes with it: **interrupt handlers do not allocate.** They record
/// what happened and defer the work. The lock being interrupt-safe is the belt; that rule
/// is the braces.
static ALLOCATOR: IrqSafeMutex<Option<PageFrameAllocator<'static>>> =
    IrqSafeMutex::new(rank::PAGE_FRAMES, None);

/// The most `/memory` nodes and `/memreserve` entries we'll cope with.
///
/// QEMU's `virt` has exactly one of the former and none of the latter. Real boards have
/// more, and a fixed-size array is the right shape here because **we have no heap yet**.
/// The `Vec` we'd reach for in userspace is precisely the thing this milestone is a
/// prerequisite for.
const MAX_REGIONS: usize = 16;

/// **Find out where RAM is and which devices the machine has, and bring the frame allocator up.**
///
/// One step on all three architectures, read from whatever this machine describes itself with: a
/// device tree on aarch64 and riscv64, the PVH handoff and ACPI on `x86_64`
/// (`arch::discovery::init`, milestone 890 (riscv64 and `x86_64` join the shared boot path)). Both
/// front ends end in [`bring_up_page_frames`], the half that does not care where RAM came from.
pub fn init() {
    #[cfg(target_arch = "x86_64")]
    crate::arch::discovery::init();
    #[cfg(not(target_arch = "x86_64"))]
    init_from_device_tree();
}

/// [`init`]'s device-tree front end.
#[cfg(not(target_arch = "x86_64"))]
fn init_from_device_tree() {
    // The blob's own physical address, which is claimed as forbidden below so the frame allocator
    // never hands out the tree it was built from.
    let dtb_phys = crate::DTB.load(core::sync::atomic::Ordering::Relaxed) as u64;
    let dtb = crate::device_tree().expect("device tree is unreadable");

    let mut ram = [Region { start: 0, size: 0 }; MAX_REGIONS];
    let ram_count = dtb
        .memory_regions(&mut ram)
        .expect("cannot read the memory map");
    let ram = &ram[..ram_count];
    assert!(!ram.is_empty(), "the device tree describes no RAM at all");

    let mut reserved = [Region { start: 0, size: 0 }; MAX_REGIONS];
    let reserved_count = dtb
        .reserved_regions(&mut reserved)
        .expect("cannot read the memory reservations");
    let reserved = &reserved[..reserved_count];

    record_interrupt_controllers(&dtb);
    record_devices(&dtb);

    // Everything that is already spoken for, and must not be allocated or scribbled on.
    //
    // The initial ramdisk matters and is easy to miss: the bootloader loaded a file into RAM for
    // us and told us where. Nobody else will protect it. Milestone 8 and 10 want to read
    // it, and if the allocator hands that memory out first, the bug lands a long way from
    // its cause.
    // Image + DTB + initial ramdisk (3), plus the legacy reservation block and the /reserved-memory
    // node (up to MAX_REGIONS each).
    let mut forbidden = [Region { start: 0, size: 0 }; 2 * MAX_REGIONS + 3];
    let mut n = 0;

    let mut claim = |r: Region| {
        if r.size > 0 {
            forbidden[n] = r;
            n += 1;
        }
    };

    claim(Region {
        start: image_start(),
        size: image_end() - image_start(),
    });
    claim(Region {
        start: dtb_phys,
        size: dtb.total_size() as u64,
    });
    if let Some(image) = dtb.initial_ramdisk().expect("cannot read /chosen") {
        INITIAL_RAMDISK_START.store(image.start as usize, core::sync::atomic::Ordering::Relaxed);
        INITIAL_RAMDISK_SIZE.store(image.size as usize, core::sync::atomic::Ordering::Relaxed);
        claim(image);
    }
    for r in reserved {
        claim(*r);
    }
    // The /reserved-memory node's children, distinct from the legacy reservation block above. This
    // is where RISC-V firmware (OpenSBI) reserves its own region below the kernel; without it the
    // allocator would hand out OpenSBI's PMP-protected memory and the first write would fault.
    let mut resv_mem = [Region { start: 0, size: 0 }; MAX_REGIONS];
    let resv_mem_count = dtb
        .reserved_memory_regions(&mut resv_mem)
        .expect("cannot read /reserved-memory");
    for r in &resv_mem[..resv_mem_count] {
        claim(*r);
    }
    let forbidden = &forbidden[..n];

    let forbidden = &forbidden[..n];

    bring_up_page_frames(ram, forbidden);
}

/// Find the interrupt controller in the tree: aarch64's GIC, or the RISC-V PLIC.
#[cfg(not(target_arch = "x86_64"))] // the device-tree front end
fn record_interrupt_controllers(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    // The interrupt controller, aarch64's GIC, found by its BINDING (milestone 227). It used to be
    // found by the node-name prefix `intc@`, taking `reg[0]` and `reg[1]` as distributor and CPU
    // interface. That is true of a GICv2 and false of a GICv3, whose `reg[1]` is the redistributor
    // array, and milestone 222 measured the result: a kernel that booted, printed `interrupts ON`,
    // and took none. `machine_discovery::gic` reads `compatible` instead, and an `intc@` node with a
    // binding it does not know is refused here, by name, rather than driven on a guess.
    // `arch::irq::init` then asks the hardware whether the tree is right before using it.
    //
    // aarch64 only: on RISC-V the interrupt controllers are the PLIC below and each hart's
    // `riscv,cpu-intc`, neither of which is this question.
    #[cfg(target_arch = "aarch64")]
    match machine_discovery::gic::discover(dtb) {
        Ok(found) => *GIC_REGIONS.lock() = found,
        Err(machine_discovery::gic::Refusal::UnknownController { compatible }) => panic!(
            "the device tree's interrupt controller is {:?}, which is not a GIC this kernel drives \
             (it drives {:?} and {:?}); refusing to guess",
            core::str::from_utf8(compatible).unwrap_or("<not UTF-8>"),
            machine_discovery::gic::GICV2_COMPATIBLE,
            machine_discovery::gic::GICV3_COMPATIBLE,
        ),
        Err(refusal) => panic!("cannot read the device tree's GIC: {refusal:?}"),
    }

    // The RISC-V interrupt controller (milestone 20): the PLIC. Found by its binding, because the
    // node's label is the board author's: QEMU spells it `plic@c000000` and the JH7110
    // `interrupt-controller@c000000`, so the old `plic@` name-prefix read found nothing on the
    // board this has to boot next (notes/visionfive2.md). The name prefix stays as a fallback for
    // a tree that names the node conventionally but states a compatible we do not know. aarch64
    // has neither, so there `plic_region` stays None; the GIC above is its equivalent. One
    // register block, unlike the GIC's distributor/CPU-interface pair.
    {
        let mut plic = [Region { start: 0, size: 0 }; 1];
        // One compatible list, shared with the context map, so the two lookups cannot disagree
        // about what a PLIC is (`machine_discovery::plic::COMPATIBLES` names each string's machine).
        let found = machine_discovery::plic::COMPATIBLES
            .iter()
            .any(|compat| matches!(dtb.node_reg_compatible(compat, &mut plic), Ok(n) if n >= 1))
            || matches!(dtb.node_reg(b"plic@", &mut plic), Ok(n) if n >= 1);
        if found {
            *PLIC_REGION.lock() = Some((plic[0].start, plic[0].size));
        }
    }
}

/// Record where the tree puts the devices the kernel drives or maps later: the JH7110's
/// controllers, the SMMUv3, the real-time clock, the UART's interrupt line and the PCIe host bridge.
#[cfg(not(target_arch = "x86_64"))] // the device-tree front end
fn record_devices(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    // The JH7110's STG clock and reset generator (milestone 220), which nothing else on any
    // machine this kernel boots has an analog of: every other device it drives came up already
    // running. Recorded **only when this machine is a JH7110**, because `jh7110_clock_and_reset::discover`
    // deliberately never fails to produce an address (it falls back to the corroborated constant
    // when a tree names no controller), and mapping that constant on a board that has nothing
    // there would be a device mapping of an address nobody named. The two ways to be a JH7110
    // that matter here: the tree names a clock controller, or it names the TRNG whose clocks this
    // exists to ungate. QEMU's `virt` board names neither, so `jh7110_clock_and_reset` stays None
    // there and `drivers::jh7110_clock_and_reset` is never reached.
    //
    // The SYS window and the PMIC bus's plan (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write),
    // provisional) ride the same guard, and
    // only it: their own discovery also falls back to constants, so it cannot be the evidence that
    // this machine is a JH7110. They are read here, while the tree is in hand, because the one
    // caller (the rebooting soak, just before its SBI reset) runs long after boot.
    record_jh7110(dtb);
    // The JH7110's two SD/MMC controllers (milestone 53 (the board's own peripherals: network and
    // storage on real silicon)), under the same guard: recorded only once `record_jh7110` has
    // established that this is a JH7110, because the vendor tree's `snps,dw-mshc` is a generic
    // binding that says nothing about which board it is on.
    record_jh7110_storage(dtb);
    // The JH7110's watchdog (milestone 593 (a wedged kernel resets itself), provisional), only in
    // a build whose soak arms it. Its own node is the guard, as the Ethernet port's is.
    #[cfg(all(target_arch = "riscv64", soak_watchdog))]
    record_jh7110_watchdog(dtb);

    // The SMMUv3 (milestone 16b), present only when the machine was started with
    // `iommu=smmuv3`. Absent, the kernel runs exactly as before; present, iommu::init drives it.
    {
        let mut smmu = [Region { start: 0, size: 0 }; 1];
        if let Ok(n) = dtb.node_reg(b"smmuv3@", &mut smmu)
            && n >= 1
        {
            *SMMU_REGION.lock() = Some((smmu[0].start, smmu[0].size));
        }
    }

    // The real-time clock (milestone 51), matched on `compatible` rather than on a node name.
    // This is the device where the name shortcut runs out: aarch64 `virt` calls it
    // `pl031@9010000` and riscv64 `virt` calls it `rtc@101000`, so no prefix finds both, while
    // `arm,pl031` and `google,goldfish-rtc` name exactly the register layouts a driver knows.
    // Both boots probe for both, and a machine with neither leaves this None, which the clock
    // service reports as "I do not know what time it is" (DECISIONS §42, §43).
    {
        let mut rtc = [Region { start: 0, size: 0 }; 1];
        for (compat, kind) in [
            (&b"arm,pl031"[..], clock_contract::rtc::PL031),
            (&b"google,goldfish-rtc"[..], clock_contract::rtc::GOLDFISH),
        ] {
            if let Ok(n) = dtb.node_reg_compatible(compat, &mut rtc)
                && n >= 1
            {
                *RTC_REGION.lock() = Some((rtc[0].start, rtc[0].size, kind));
                break;
            }
        }
    }

    // The console UART's interrupt line, decoded per the tree's own `#interrupt-cells` rather
    // than assumed (crates/machine_discovery/src/interrupt_id.rs). It was a constant, and the constant was the
    // last QEMU number on the interrupt path: the tour's driver demo armed PLIC source 10 on the
    // JH7110, whose UART0 interrupts on line 32, and boot 13 (2026-08-15) proved it on silicon
    // when a key press at the finished tour's prompt reached nothing (notes/visionfive2.md,
    // BUGS). A tree that does not say leaves this None, and the callers fall back to the QEMU
    // `virt` constant and print which source won.
    if let Ok(Some(irq)) = machine_discovery::interrupt_id::of_node(dtb, crate::console::UART_NODE)
    {
        *UART_IRQ.lock() = Some(irq);
    }

    // The PCIe host bridge, matched on the generic-ECAM binding both QEMU `virt` boards state.
    // `reg` is the ECAM config window; `ranges` carries the standard 7-cell PCI entries, of
    // which the 32-bit non-prefetchable memory entry is the window the kernel assigns BARs from
    // (`pci::mem32_window` owns that parse). The JH7110 has no such node (its PLDA controller
    // is `starfive,jh7110-pcie`, a different device with no driver here yet), so there this
    // stays None and no PCIe window is ever mapped or probed. It has to come from the tree: the
    // old QEMU constants put the BAR window at 0x4000_0000, which on the JH7110 is DRAM base,
    // already direct-mapped, and the first VisionFive 2 boot died on the mapper's overwrite
    // refusal (DECISIONS §43, notes/visionfive2.md).
    {
        let mut ecam = [Region { start: 0, size: 0 }; 1];
        if let Ok(n) = dtb.node_reg_compatible(b"pci-host-ecam-generic", &mut ecam)
            && n >= 1
            && let Ok(Some(ranges)) = dtb.node_prop_compatible(b"pci-host-ecam-generic", b"ranges")
            && let Some(mem32) = ::pci::mem32_window(ranges)
        {
            *PCI_REGIONS.lock() = Some(((ecam[0].start, ecam[0].size), mem32));
        }
    }
}

/// **Record the JH7110's SD/MMC controllers** (milestone 53 (the board's own peripherals: network
/// and storage on real silicon)): slot 0, the eMMC socket, and slot 1, the microSD slot, each as
/// the tree describes it, and nothing when the tree describes neither. Only on a machine
/// `record_jh7110` has already found to be a JH7110. Its own frame for `record_jh7110`'s reason.
#[inline(never)]
#[cfg(not(target_arch = "x86_64"))] // the device-tree front end
fn record_jh7110_storage(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    if JH7110_CRG.lock().is_none() {
        return;
    }
    let slot = |i| {
        designware_mobile_storage::jh7110::discover(dtb, i)
            .ok()
            .flatten()
    };
    let slots = [slot(0), slot(1)];
    if slots.iter().any(Option::is_some) {
        *JH7110_STORAGE.lock() = Some(slots);
    }
}

/// **Record the JH7110's clock-and-reset windows and the PMIC bus's plan** (milestones 220 and
/// 592), under the guard the comment above the call describes.
///
/// Its own frame, and `inline(never)` so it stays one: [`init_from_device_tree`] is already close
/// to the 4,096-byte guard page in an unoptimised build, and the two `Found`s and the `PmicBus`
/// here pushed it over (`script/stack-frame-check`, 4,352 bytes on aarch64, 2026-09-25). A call
/// keeps these locals off its frame instead of raising any ceiling.
#[inline(never)]
#[cfg(not(target_arch = "x86_64"))] // the device-tree front end
fn record_jh7110(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    let crg = jh7110_clock_and_reset::discover(dtb).ok();
    let has_trng = matches!(jh7110_entropy::discover(dtb), Ok(Some(_)));
    if let Some(found) = crg
        && (found.from_tree || has_trng)
    {
        *JH7110_CRG.lock() = Some(found);
        if let (Ok(sys), Ok(bus)) = (
            jh7110_clock_and_reset::discover_sys(dtb),
            jh7110_clock_and_reset::pmic_bus(dtb),
        ) {
            *JH7110_PMIC_BUS.lock() = Some((sys, bus));
        }
    }
    record_jh7110_ethernet(dtb);
}

/// **Record the JH7110's first Ethernet port and the three windows its bring-up writes**
/// (milestone 53 (the board's own peripherals: network and storage on real silicon)): the
/// controller, the AON and SYS clock-and-reset generators, and the AON syscon.
///
/// The guard is the port itself. A tree naming `starfive,jh7110-dwmac` or the vendor's
/// `starfive,jh7110-eqos-5.20` is a JH7110 by its own statement, which is a stronger warrant than
/// the TRNG guard above has; the three discoveries after it fall back to constants (both trees
/// agree on each) and so are only read once that is established. QEMU's `virt` names neither, so
/// nothing is recorded or mapped there. Its own frame for `record_jh7110`'s reason.
#[inline(never)]
#[cfg(not(target_arch = "x86_64"))] // the device-tree front end
fn record_jh7110_ethernet(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    let Ok(Some(port)) = designware_ethernet::jh7110::discover(dtb) else {
        return;
    };
    if let (Ok(aon), Ok(sys), Ok(syscon)) = (
        jh7110_clock_and_reset::discover_aon(dtb),
        jh7110_clock_and_reset::discover_sys(dtb),
        designware_ethernet::jh7110::discover_aon_syscon(dtb),
    ) {
        *JH7110_ETHERNET.lock() = Some(Jh7110Ethernet {
            port,
            aon,
            sys,
            syscon,
        });
    }
}

/// **Record the JH7110's watchdog and the SYS window its clocks and resets live in** (milestone
/// 593, provisional). The guard is the watchdog's own node: a tree naming `starfive,jh7110-wdt` or
/// the vendor's `starfive,dskit-wdt` is a JH7110 by its own statement, so the SYS discovery's
/// constant fallback is only read once that is established. QEMU's `virt` names neither. Its own
/// frame for `record_jh7110`'s reason.
#[inline(never)]
#[cfg(all(target_arch = "riscv64", soak_watchdog))]
fn record_jh7110_watchdog(dtb: &device_tree_blob::DeviceTreeBlob<'_>) {
    if let (Ok(Some(watchdog)), Ok(sys)) = (
        jh7110_watchdog::discover(dtb),
        jh7110_clock_and_reset::discover_sys(dtb),
    ) {
        *JH7110_WATCHDOG.lock() = Some((watchdog, sys));
    }
}

/// **Bring the frame allocator up over a described machine**, given RAM and everything already
/// spoken for. The half of [`init`] that does not care where the description came from.
///
/// # Why this is a separate function
///
/// It is the narrow half of the seam milestone 20 promised and did not build: "put device discovery
/// behind a 'here is the hardware' interface (device tree today, ACPI/PCI later)". [`init`] above is
/// a **device-tree front end**, so on a machine without a device tree there is nothing it can do,
/// and `x86_64` has PVH's memory map and ACPI instead (milestone 161). Splitting the two apart is what
/// lets a third architecture reach the allocator at all.
///
/// **Narrow, and the rest is still owed.** What crosses this seam is RAM and reservations. The
/// device *windows* [`init`] also discovers (the interrupt controller, the RTC, the UART's interrupt
/// line, the PCIe ECAM range) are still read from the tree by the front end and stored in this
/// module's statics, which simply stay `None` on a machine with no tree. Widening it is its own
/// milestone; see notes/x86-port/acpi-and-pci.md.
///
/// `ram` is every region that is real, allocatable memory. `forbidden` is every region inside it
/// that something else already owns, and **it must include the kernel image**, or the allocator
/// hands out the code it is running from.
///
/// # BUGS
///
/// - **The type at this seam is still the device tree's.** `Region` is `device_tree_blob::Region`,
///   which is a plain `{ start, size }` pair and means nothing device-tree-specific, but a machine
///   with no device tree naming a device-tree type is a smell rather than a design. Moving it
///   belongs with the wider seam.
/// - **At most `MAX_REGIONS` RAM regions.** More than that indexes past the map below. Both `virt`
///   boards describe one; q35 describes three. A caller with more must decide what to drop, because
///   this cannot.
pub(crate) fn bring_up_page_frames(ram: &[Region], forbidden: &[Region]) {
    assert!(!ram.is_empty(), "the machine describes no RAM at all");

    // The whole span we have to be able to describe. Note this is the *span*, not the
    // *sum*: if a board has RAM at 0x4000_0000 and again at 0x8_0000_0000, we track
    // every frame between them and simply never free the hole. A bit of wasted bitmap
    // buys a much simpler index calculation.
    {
        let mut map = RAM.lock();
        for (i, r) in ram.iter().enumerate() {
            map.regions[i] = (r.start, r.size);
        }
        map.count = ram.len();
    }

    let base = ram.iter().map(|r| r.start).min().unwrap();

    // The span the bitmap must cover is RAM's, **widened by whatever `forbidden` reaches
    // past it**, not RAM's alone. Found on x86_64 (decisions §86's VT-d/NVMe data point,
    // 2026-08-25): QEMU's PVH loader places the initial ramdisk at a fixed offset below the top of
    // guest memory, sized for a smaller device set than this boot's VT-d-plus-NVMe ACPI
    // tables need. Attaching the NVMe controller grows those tables enough that the e820
    // map's "usable RAM" boundary moves down past the initial ramdisk's last few hundred bytes,
    // which land in the newly-`reserved` region instead. `bring_up_memory` (the x86_64
    // front end) correctly still claims that whole initial ramdisk region as `forbidden`, physical
    // bytes and all; what was missing is that `top` here was computed from `ram` alone, so
    // the bitmap never grew far enough to represent those last frames at all, and
    // `is_used` on them answered `None` (out of range) rather than the `Some(true)` a
    // frame the kernel has explicitly claimed should always give. Widening `top` to the
    // max of both closes that: the extra frames start (correctly) all-used by
    // `PageFrameAllocator::new`, no `mark_free` ever runs on them because they are not in
    // `ram`, and the `mark_used` below for `forbidden` now lands inside `total_frames`
    // instead of being silently clamped away. A forbidden region entirely inside RAM (the
    // common case, and every case on the other two architectures today) leaves `top`
    // unchanged.
    let top = ram
        .iter()
        .map(|r| r.end())
        .chain(forbidden.iter().map(|r| r.end()))
        .max()
        .unwrap();
    // Round up to a whole frame before sizing the bitmap: RAM regions are frame-aligned in
    // practice, but a `forbidden` region need not be (the initial ramdisk above is 4629504 bytes,
    // not a multiple of 4096), and `page_frames_in` truncates. Rounding down here would
    // silently drop the very last partial frame `top` was just widened to reach.
    let total_frames = PageFrameAllocator::page_frames_in(top.next_multiple_of(FRAME_SIZE) - base);

    // --- the bootstrap problem ---
    //
    // The allocator needs somewhere to put its bitmap. We have no allocator.
    //
    // The way out is to carve it, by hand, out of the very memory it is about to manage,
    // and then reserve those frames from itself. **The allocator's first act is to
    // allocate itself.**
    //
    // We used to just drop it immediately after the kernel image and hope. That worked,
    // but only by luck: `image_size` in the arm64 Image header stops at `__stack_top`,
    // so everything past `__image_end` is memory we never told the bootloader we wanted.
    // QEMU happens to place the DTB and the initial ramdisk 64 MiB higher up. Different firmware
    // need not.
    //
    // So instead: scan RAM for the first frame-aligned run that clears everything above.
    // Same answer in practice, but now it's proven rather than lucky, and it will keep
    // being right on hardware we haven't met.
    let bitmap_bytes = PageFrameAllocator::bitmap_bytes(total_frames);
    let bitmap_start = place_bitmap(bitmap_bytes as u64, ram, forbidden);
    BITMAP_START.store(bitmap_start as usize, core::sync::atomic::Ordering::Relaxed);
    BITMAP_BYTES.store(bitmap_bytes, core::sync::atomic::Ordering::Relaxed);

    // SAFETY: `place_bitmap` guarantees this range is inside a RAM region and overlaps
    // nothing that is already spoken for. Nothing else in the kernel touches it, and we
    // mark it used below so nothing ever will. 'static because the allocator outlives
    // everything.
    // `bitmap_start` is a physical address (it names frames). To *write* to it we need the
    // virtual name for the same bytes.
    let bitmap: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(phys_to_virt(bitmap_start) as *mut u8, bitmap_bytes)
    };

    // Everything starts USED. Memory is guilty until proven innocent: a frame is only
    // handed out once someone has said "this is real RAM." Default-free would cheerfully
    // allocate the MMIO hole and hand out the UART's registers as scratch space.
    let mut allocator = PageFrameAllocator::new(base, total_frames, bitmap);

    // Now prove innocence, region by region.
    for r in ram {
        allocator.mark_free(r.start, r.size);
    }

    // And immediately take back everything that is already spoken for. Order matters:
    // free first, then reserve, because reserving is what has to win.
    for r in forbidden {
        allocator.mark_used(r.start, r.size);
    }
    allocator.mark_used(bitmap_start, bitmap_bytes as u64);

    *ALLOCATOR.lock() = Some(allocator);
}

/// Find somewhere to put the frame bitmap that overlaps nothing already spoken for.
///
/// Scans RAM in order and returns the first frame-aligned run of `need` bytes that clears
/// every forbidden region. When a candidate collides, jump past the *end* of whatever it
/// hit rather than nudging forward by a frame: the regions are large and stepping through
/// a 200 KiB initial ramdisk one page at a time would be silly.
fn place_bitmap(need: u64, ram: &[Region], forbidden: &[Region]) -> u64 {
    for region in ram {
        let mut candidate = align_up(region.start, FRAME_SIZE);

        'scan: while candidate + need <= region.end() {
            for f in forbidden {
                if overlaps(candidate, need, f.start, f.size) {
                    candidate = align_up(f.start + f.size, FRAME_SIZE);
                    continue 'scan;
                }
            }
            return candidate;
        }
    }

    panic!("no room anywhere in RAM for a {need}-byte frame bitmap");
}

/// Do `[a, a+alen)` and `[b, b+blen)` share a byte?
fn overlaps(a: u64, alen: u64, b: u64, blen: u64) -> bool {
    a < b.saturating_add(blen) && b < a.saturating_add(alen)
}

/// **The fewest frames that were ever free at once on this boot**, and what was refused.
///
/// The frame ledger in `testing.rs` reads the allocator twice, before the first test and after the
/// last, and both of its gates are about that end state. What a boot actually runs out of is the
/// middle, and an exhaustion shows up far from the tests that spent the memory. This says how close
/// the run came, and the first refusal says where it went over (milestone 601 (the region table
/// prints its peak)).
///
/// Kept under the allocator lock by [`alloc`] and [`alloc_contiguous`], so a plain load and store
/// is enough (the lock orders them); atomics only because a `static` must be `Sync`.
///
/// # The ledger at the low-water
///
/// Measured 2026-09-26 on `main` at `484f3ebe` plus #1347, from the closing `frames:` lines:
///
/// | | free before the first test | lowest free | free after the last | refused |
/// |---|---|---|---|---|
/// | aarch64 | 57033 | **23654** | 34790 | 0 |
/// | riscv64 | 57882 | **24682** | 35780 | 0 |
/// | `x86_64` | 58841 | **35719** | 46813 | 0 |
///
/// Every low sits about 11,100 frames under its own end-of-suite figure, and the test named each
/// time is a progenitor test (`the_report_child_builder_parses_an_elf_and_builds_a_running_child`, or on
/// riscv64 `the_least_authority_demo_builder_passes_its_demo_an_argument`). Both carve a 12,288-page building
/// budget (`memory_region::create(12288)` in `kernel/src/user.rs`) and give it back when they
/// finish. So the ledger at the low-water is `notes/frames.md`'s held list plus that one
/// reservation. What moves the low from one milestone to the next is the held list growing; what
/// catches a load that no longer fits is the refusal count, which the same summary prints.
///
/// **Contiguity is not tracked per test**: `largest_free_run` per test never moved from the first
/// test's reading, and a bitmap scan per test overran the `x86_64` 17 GiB leg's time bound. A
/// refused `alloc_contiguous` is the contiguity failure itself, and that is counted here.
///
/// No allocation was refused on any architecture in that run, so a refusal line is news. Reports
/// and does not gate, for `sched::MAX_THREADS`'s reason: the held list grows on purpose, and the
/// frame ledger's two end-of-suite gates already hold the permanent part to account.
static FREE_LOW_WATER: AtomicUsize = AtomicUsize::new(usize::MAX);
/// How many allocations the allocator refused, and the largest request among them, in frames.
static REFUSED: AtomicUsize = AtomicUsize::new(0);
static LARGEST_REFUSED: AtomicUsize = AtomicUsize::new(0);

/// Record an allocation's outcome against [`FREE_LOW_WATER`] and [`REFUSED`]. Called with the
/// allocator still held, so the reading is the one this allocation produced.
fn note_allocation(allocator: &PageFrameAllocator<'static>, count: usize, granted: bool) {
    publish_counts(allocator);
    let free = allocator.stats().free();
    if free < FREE_LOW_WATER.load(core::sync::atomic::Ordering::Relaxed) {
        FREE_LOW_WATER.store(free, core::sync::atomic::Ordering::Relaxed);
    }
    if !granted {
        REFUSED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        LARGEST_REFUSED.fetch_max(count, core::sync::atomic::Ordering::Relaxed);
    }
}

/// `(low_water, refused, largest_refused)`: see [`FREE_LOW_WATER`]. The low-water is `usize::MAX`
/// until the first allocation. Printed by the test suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub(crate) fn allocation_pressure() -> (usize, usize, usize) {
    use core::sync::atomic::Ordering::Relaxed;
    (
        FREE_LOW_WATER.load(Relaxed),
        REFUSED.load(Relaxed),
        LARGEST_REFUSED.load(Relaxed),
    )
}

pub fn alloc() -> Option<PageFrame> {
    let mut guard = ALLOCATOR.lock();
    let allocator = guard.as_mut()?;
    let frame = allocator.alloc();
    note_allocation(allocator, 1, frame.is_some());
    frame
}

/// Physically contiguous frames, for hardware that does DMA and has no MMU to hide a
/// scattered buffer behind. Milestone 8 needs this.
pub(crate) fn alloc_contiguous(count: usize) -> Option<PageFrame> {
    let mut guard = ALLOCATOR.lock();
    let allocator = guard.as_mut()?;
    let run = allocator.alloc_contiguous(count);
    note_allocation(allocator, count, run.is_some());
    run
}

/// A freshly allocated frame, zeroed.
///
/// **This exists so the "is it sound to write over this frame" argument is made once, here,
/// rather than at every caller.** Thirty-seven sites across the kernel used to allocate a frame
/// and then hand-write the same `// SAFETY:` comment over `core::ptr::write_bytes`: a fresh frame
/// is exclusively ours and the direct map reaches it, so zeroing it cannot race and cannot be seen
/// by anyone else. That is not thirty-seven facts, it is one fact about what [`alloc`] returns,
/// and the allocator is the only thing in the tree that can actually check it. Two of those
/// comments had already noticed they were copies (`user/rmle_service.rs`: *"a second copy of three
/// lines"*; `user/session_reviver_service.rs`: *"matches `fs_service::frame`'s own shape"*)
/// without anyone lifting it out. Milestone 139 round 8, and DECISIONS §94's rule that a body
/// copied verbatim into every caller is not per-caller, only its declaration is.
///
/// Callers who want the frame's bytes left alone still call [`alloc`]; nothing here is mandatory.
/// Zeroing is a `FRAME_SIZE` memory touch and it happens after the allocator lock is released,
/// which is why this is written as two statements rather than one expression.
pub fn alloc_zeroed() -> Option<PageFrame> {
    let frame = alloc()?;
    zero_frames(frame, 1);
    Some(frame)
}

/// Physically contiguous frames, zeroed. [`alloc_contiguous`] plus [`alloc_zeroed`]'s argument,
/// for the DMA regions that need both: contiguous because the device has no MMU to hide a
/// scattered buffer behind, zeroed because a stale word in a descriptor ring is a word the device
/// will act on.
pub fn alloc_contiguous_zeroed(count: usize) -> Option<PageFrame> {
    let frame = alloc_contiguous(count)?;
    zero_frames(frame, count);
    Some(frame)
}

/// The one hand-written assertion the two functions above exist to hold.
///
/// `frame` came out of this module's own allocator moments ago and `count` is the run length that
/// allocator returned, so nothing else in the system holds a reference to these bytes yet.
fn zero_frames(frame: PageFrame, count: usize) {
    // SAFETY: `frame` is a run of `count` frames the allocator has just marked used and handed to
    // this caller alone, so no other reference to these bytes exists; `phys_to_virt` is the direct
    // map, which covers all of RAM, so the destination is writable for the whole run.
    unsafe {
        core::ptr::write_bytes(
            phys_to_virt(frame.addr()) as *mut u8,
            0,
            count * FRAME_SIZE as usize,
        );
    }
}

/// **The free count, for the machine statistics page** (milestone 126 (the `procps` package),
/// DECISIONS §225 (`free` sees the machine and your share)), written by whoever just changed it and
/// under the same lock, so the page never shows a count the allocator never had.
fn publish_counts(allocator: &PageFrameAllocator<'_>) {
    let s = allocator.stats();
    crate::machine_statistics::frames(s.total, s.free());
}

pub fn free(frame: PageFrame) {
    let mut guard = ALLOCATOR.lock();
    let allocator = guard.as_mut().expect("freeing a frame before memory::init");
    allocator.free(frame);
    publish_counts(allocator);
}

pub fn stats() -> Option<Stats> {
    Some(ALLOCATOR.lock().as_ref()?.stats())
}

/// Free frames right now. For tests that prove reclamation actually returns memory (the memory
/// region flat-frame-count property, notes/memory-regions.md): a region reclaimed by object revocation should
/// bring this exactly back to where it stood before the region was created.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the reclamation tests in sched.rs and user/tests.rs
pub fn free_page_frames() -> usize {
    ALLOCATOR.lock().as_ref().map_or(0, |a| a.stats().free())
}

/// **Is the frame at `pa` allocated right now?** `None` when the allocator does not manage that
/// address at all (outside RAM, or before `memory::init`), which is also the answer that says the
/// direct map may not cover it. For a kill report walking tables that may have been written over: a
/// table the space still links whose frame reads as free was handed back while still in use.
///
/// Name: provisional (lane `maintainer/pipeline-fetch-fault`, 2026-10-14 UTC).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))] // the x86_64 kill report is the caller
pub fn frame_is_allocated(pa: u64) -> Option<bool> {
    ALLOCATOR
        .lock()
        .as_ref()?
        .is_used(PageFrame::from_addr(pa & !(FRAME_SIZE - 1)))
}

/// **The longest run of free frames**, in frames: what `alloc_contiguous` could still satisfy.
///
/// The companion to [`free_page_frames`], and the one the test boot actually runs out of. A boot can hold
/// a comfortable free total and still refuse a 128-page request because the free frames are in
/// pieces, which is exactly what milestone 107 measured (137 free, no run of 128) and read as
/// exhaustion. The frame ledger prints both, so the next person meets the distinction rather than
/// deducing it. See notes/frames.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the frame ledger in testing.rs is the caller
pub(crate) fn largest_free_run() -> usize {
    ALLOCATOR
        .lock()
        .as_ref()
        .map_or(0, |a| a.largest_free_run())
}

/// Is this address inside the kernel image?
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's bootstrap tests are the callers
pub fn is_in_kernel_image(addr: u64) -> bool {
    (image_start()..image_end()).contains(&addr)
}

/// Where the kernel image begins and ends, per the linker.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's bootstrap tests are the callers
pub fn image_bounds() -> (u64, u64) {
    (image_start(), image_end())
}

/// Is this frame currently marked used?
///
/// **The scoped form of [`free_page_frames`]**, and the one to reach for when a test knows which
/// frame it is asking about: a machine-wide free count moves whenever any other part of the kernel
/// allocates or frees, so a test bracketing it is asserting that the rest of the system held still.
/// `user/current_cpu_tests.rs` is the worked example (notes/load-sensitive-assertions.md).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's bootstrap tests and user/current_cpu_tests.rs
pub fn is_page_frame_used(frame: PageFrame) -> Option<bool> {
    ALLOCATOR.lock().as_ref()?.is_used(frame)
}

/// **The interrupt controller, as the device tree describes it**: which GIC version its binding
/// names, and its register blocks in the roles that version gives them, all **physical**. Stashed
/// at `init` because that is the only moment we have the device tree parsed, and milestone 5 needs
/// it much later. `None` on a machine whose tree names no GIC, and always on RISC-V and `x86_64`.
///
/// Until milestone 227 this was a bare (distributor, CPU interface) pair taken from whatever node
/// was named `intc@`, which is why a GICv3 machine's redistributor was once driven as a CPU
/// interface. The version travelling with the addresses is what keeps that from recurring.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))] // riscv has a PLIC, x86 an APIC
pub(crate) fn gic_regions() -> Option<machine_discovery::gic::Gic> {
    *GIC_REGIONS.lock()
}

/// The PLIC's register block (start, size), both **physical**, from the device tree. `None` on
/// aarch64 or before `init`. RISC-V's `mmu::init` maps it device-typed, like the GIC.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // the PLIC is riscv's controller; x86 answers with its APIC
pub(crate) fn plic_region() -> Option<(u64, u64)> {
    *PLIC_REGION.lock()
}

/// **The JH7110's SD/MMC controllers** (milestone 53 (the board's own peripherals: network and
/// storage on real silicon)): slot 0 (the eMMC socket) and slot 1 (the microSD slot), each `None`
/// when the tree does not describe it, and the whole answer `None` on every machine that is not a
/// JH7110, which is every machine CI boots. RISC-V's `mmu::init` maps the first page of each.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // no JH7110 anywhere but a JH7110
pub(crate) fn jh7110_storage() -> Option<[Option<designware_mobile_storage::jh7110::Slot>; 2]> {
    *JH7110_STORAGE.lock()
}

/// The JH7110's STG clock-and-reset window (milestone 220): where it is, and **whether the device
/// tree said so or the corroborated constant did**. `None` on every machine that is not a
/// `StarFive` JH7110, which is every machine this repository's CI boots. RISC-V's `mmu::init`
/// maps it device-typed, like the PLIC.
///
/// The whole `Found` rather than a `(start, size)` pair, because `from_tree` is the field a bench
/// transcript most needs and a pair would drop it: an address nobody on that machine confirmed
/// and an address the firmware named are different claims, and by the time anyone reads the log
/// they cannot be told apart from the number alone.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // no JH7110 anywhere but a JH7110
pub(crate) fn jh7110_clock_and_reset() -> Option<jh7110_clock_and_reset::Found> {
    *JH7110_CRG.lock()
}

/// **The JH7110's SYS clock-and-reset window, and the plan that brings the PMIC's I2C bus back
/// up** (milestone 592, provisional). `None` on every machine that is not a JH7110, under exactly
/// [`jh7110_clock_and_reset()`]'s guard. RISC-V's `mmu::init` maps the window device-typed.
///
/// The only caller is the rebooting soak, just before SBI SRST: on radon OpenSBI performs every
/// reset as an I2C write to the AXP15060, and U-Boot hands over with that bus's clock gated and its
/// reset asserted. See `design/roadmap/0592-radons-reboot-dies-in-opensbis-pmic-write.md`.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // no JH7110 anywhere but a JH7110
pub(crate) fn jh7110_pmic_bus() -> Option<(
    jh7110_clock_and_reset::Found,
    jh7110_clock_and_reset::PmicBus,
)> {
    *JH7110_PMIC_BUS.lock()
}

/// **The JH7110's first Ethernet port and the windows its bring-up needs** (milestone 53), or
/// `None` on every machine whose tree names no such port, which is every machine CI boots.
/// RISC-V's `mmu::init` maps all four windows device-typed.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // no JH7110 anywhere but a JH7110
pub(crate) fn jh7110_ethernet() -> Option<Jh7110Ethernet> {
    *JH7110_ETHERNET.lock()
}

/// **The SYS clock-and-reset window, if anything on this machine needs it**: the PMIC bus's
/// (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)), the Ethernet port's
/// (milestone 53) or the watchdog's (milestone 593). All come from the same `discover_sys`, so
/// they are the same window, and `mmu::init` maps it once through this rather than once per user.
#[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // no JH7110 anywhere but a JH7110
pub(crate) fn jh7110_sys_window() -> Option<jh7110_clock_and_reset::Found> {
    let sys = jh7110_pmic_bus()
        .map(|(sys, _)| sys)
        .or_else(|| jh7110_ethernet().map(|e| e.sys));
    #[cfg(all(target_arch = "riscv64", soak_watchdog))]
    let sys = sys.or_else(|| jh7110_watchdog().map(|(_, sys)| sys));
    sys
}

/// **The JH7110's watchdog and the SYS window that holds its clocks and resets** (milestone 593,
/// provisional), or `None` on every machine whose tree names no such watchdog, which is every
/// machine CI boots. RISC-V's `mmu::init` maps both device-typed.
#[cfg(all(target_arch = "riscv64", soak_watchdog))]
pub(crate) fn jh7110_watchdog() -> Option<(jh7110_watchdog::Discovered, jh7110_clock_and_reset::Found)>
{
    *JH7110_WATCHDOG.lock()
}

/// What [`jh7110_ethernet`] answers: the port as the tree describes it, and the three windows
/// outside the controller that its bring-up writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Jh7110Ethernet {
    /// The controller, its PHY's settings, its station address if the tree has one.
    pub port: designware_ethernet::jh7110::Discovered,
    /// The AON clock-and-reset window: `gmac0`'s bus clocks, transmit clock and resets.
    pub aon: jh7110_clock_and_reset::Found,
    /// The SYS clock-and-reset window: `gmac0`'s transmit and PTP clocks.
    pub sys: jh7110_clock_and_reset::Found,
    /// The AON syscon: `gmac0`'s interface select.
    pub syscon: designware_ethernet::jh7110::Syscon,
}

/// The PCIe host bridge's windows, from the device tree: `(ecam, mem32)`, each `(start, size)`,
/// all **physical**. ECAM is the config window the bridge's `reg` names; `mem32` is the 32-bit
/// non-prefetchable memory window BARs are placed in. `None` before `init`, and on a machine
/// whose tree describes no `pci-host-ecam-generic` bridge, which is the JH7110's honest state:
/// its own PCIe controller is a different device, undriven until its own milestone. Absence
/// degrades the way an absent virtio device does: `mmu::map_everything` maps no window and every
/// probe in kernel/src/pci.rs reports nobody home without touching MMIO.
pub(crate) fn pci_regions() -> Option<((u64, u64), (u64, u64))> {
    *PCI_REGIONS.lock()
}

/// **Record the PCIe host bridge's windows directly**, for a machine with no device tree to read
/// them from. `x86_64`'s counterpart of `init`'s `pci-host-ecam-generic` node parse: ACPI's MCFG
/// names the ECAM window, and there is no `_CRS` reader for the BAR window (that needs an AML
/// interpreter, which this kernel does not have), so `main.rs` supplies both from what it already
/// knows and fills the same static every consumer already reads. See notes/x86-port/acpi-and-pci.md.
///
/// Its only caller is `x86_64`'s boot tour; the other two architectures fill the same static from
/// their device tree inside `init` instead.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_pci_regions(ecam: (u64, u64), mem32: (u64, u64)) {
    *PCI_REGIONS.lock() = Some((ecam, mem32));
}

/// The SMMUv3's register block (start, size), both **physical**, from the device tree. `None`
/// when the machine has no SMMU (riscv, or aarch64 without `iommu=smmuv3`). Presence here is what
/// gates the whole aarch64 IOMMU path: no node, no register reads, no faults on a machine that
/// never had the device.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))] // VT-d init takes the DMAR's units, not one base (milestone 594)
pub(crate) fn smmu_region() -> Option<(u64, u64)> {
    *SMMU_REGION.lock()
}

/// **Every VT-d unit's register block** (start, size), both **physical**, one per DRHD the DMAR
/// named, in table order; `None` past the last. All `None` on a machine with no VT-d unit, or
/// before [`record_vtd_region`] has run. Presence here is what gates `mmu::map_everything` mapping
/// each window, the same role [`smmu_region`] plays for the SMMU. Every unit and not only one
/// since milestone 594 (every VT-d unit translates its own devices), which brings every unit up. Name: provisional.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn vtd_regions() -> [Option<(u64, u64)>; machine_discovery::acpi::MAX_DRHDS] {
    *VTD_REGIONS.lock()
}

/// **Record one VT-d unit's register block**, for a machine with no device tree to read it from.
/// `x86_64`'s counterpart of [`record_pci_regions`]: ACPI's DMAR names the DRHDs, and `main.rs`
/// calls this once per unit before `arch::mmu::init()` runs, so every window is recorded before
/// `mmu::map_everything` decides what to map. Must run before `mmu::init`; calling it after would
/// record a fact the fine map can no longer act on. A unit past the array is a panic, not a
/// silent drop: the DMAR decoder holds no more units than this array does.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_vtd_region(base: u64, size: u64) {
    let mut g = VTD_REGIONS.lock();
    let slot = g
        .iter_mut()
        .find(|r| r.is_none())
        .expect("more VT-d units than the DMAR decoder records");
    *slot = Some((base, size));
}

/// **Every AMD-Vi unit's register block** (start, size), both physical, one per IVHD the IVRS
/// named, in table order; `None` past the last. [`vtd_regions`]' counterpart on an AMD machine,
/// and read by `mmu::map_everything` for the same reason. Name: provisional (lane `amd-vi`).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn amd_vi_regions() -> [Option<(u64, u64)>; machine_discovery::acpi::ivrs::MAX_IVHDS] {
    *AMD_VI_REGIONS.lock()
}

/// **Record one AMD-Vi unit's register block.** Must run before `mmu::init`, as
/// [`record_vtd_region`] must, and a unit past the array is a panic for the same reason. Name:
/// provisional (lane `amd-vi`).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_amd_vi_region(base: u64, size: u64) {
    let mut g = AMD_VI_REGIONS.lock();
    let slot = g
        .iter_mut()
        .find(|r| r.is_none())
        .expect("more AMD-Vi units than the IVRS decoder records");
    *slot = Some((base, size));
}

/// **The screen's aperture** (start, size), both **physical**, as the boot handoff described it.
/// `None` on a machine that reported no linear framebuffer, which is every machine booted by
/// anything but `uefi_loader` today.
///
/// Presence here is what gates `mmu::map_everything` mapping the window device-typed, the same role
/// [`vtd_regions`] plays for VT-d. The console does not read this: it holds the address it was armed
/// with. This exists so that the *mapping* survives the fine map replacing the boot map, which is
/// the moment a console armed on the boot tables would otherwise start faulting.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn framebuffer() -> Option<(u64, u64)> {
    *FRAMEBUFFER.lock()
}

/// **Record the screen's aperture**, from the boot command line (`machine_discovery::framebuffer`).
///
/// Must run before `mmu::init`, exactly as [`record_vtd_region`] must, and for the identical
/// reason: `mmu::map_everything` reads this to decide what to map, and a fact recorded afterwards
/// is a fact the fine map can no longer act on. The failure that would cause is unusually nasty,
/// which is why it is stated twice: the console keeps writing to an address that stopped being
/// mapped, so the first `println!` after the `mov cr3` takes a page fault, and the fault handler
/// prints, and it faults.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_framebuffer(base: u64, size: u64) {
    *FRAMEBUFFER.lock() = Some((base, size));
}

/// The real-time clock's register block and which binding it is: `(start, size, kind)`, the
/// address **physical** and the kind one of `clock_contract::rtc`. `None` on a machine whose device
/// tree describes no RTC we can drive, which is a state the clock service has an answer for rather
/// than a case it papers over.
///
/// The kind travels with the address on purpose. The clock service picks its register layout from
/// what the machine said, not from `target_arch`, because the VisionFive 2 is riscv64 and has
/// neither of these devices; an ISA-keyed driver would compile clean and read garbage on the first
/// real board (DECISIONS §43).
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "shell")),
    allow(dead_code)
)]
pub(crate) fn rtc_region() -> Option<(u64, u64, u64)> {
    *RTC_REGION.lock()
}

/// The console UART's interrupt line, as the machine states it: PLIC source 10 on QEMU's
/// riscv64 `virt`, 32 on the JH7110, GIC INTID 33 on QEMU's aarch64 `virt`, and on `x86_64` the
/// global system interrupt COM1's legacy ISA IRQ 4 resolves to (milestone 176), filled by
/// [`record_uart_irq`] rather than by [`init`] since there is no tree to read. `None` before
/// either has run, and on a device-tree machine whose tree does not say (no `interrupts`, no
/// resolvable parent, or an entry shape the decoder refuses; see
/// `machine_discovery::interrupt_id`). The callers own the fallback (`user::UART_RX_INTID`, the
/// QEMU constant) and print which source won, so a bench transcript is diagnosable.
pub(crate) fn uart_irq() -> Option<u32> {
    *UART_IRQ.lock()
}

/// **Record the console UART's interrupt line directly**, for a machine with no device tree to
/// read it from. `x86_64`'s counterpart of `init`'s [`machine_discovery::interrupt_id::of_node`]
/// call. It takes COM1's **legacy** line, ISA IRQ 4, because that is what an intid is on `x86_64`;
/// `arch::irq::enable` resolves it through any MADT override when the line is armed. It fills the
/// same static every consumer already reads (`user::uart_irq_and_source`), which the input driver's
/// `Irq` capability is minted from since milestone 505 (an x86_64 input driver that never lets the
/// core idle).
///
/// Its only caller is `x86_64`'s boot tour; the other two architectures fill the same static from
/// their device tree inside `init` instead.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_uart_irq(irq: u32) {
    *UART_IRQ.lock() = Some(irq);
}

/// The RAM regions the device tree told us about.
///
/// The MMU needs these: with paging on, a physical address the kernel cannot *name* is a
/// physical address it cannot use, and it must be able to touch any frame the allocator
/// hands it (to zero a new page table, to fill a new user page).
pub(crate) fn ram_regions() -> impl Iterator<Item = (u64, u64)> {
    // Copy the whole map out under ONE lock acquisition, then iterate freely. 256 bytes.
    //
    // The alternative (an iterator that holds the lock, or takes it per element) would keep a
    // kernel lock live across arbitrary caller code, with interrupts masked the whole time.
    // That violates "keep critical sections short" (DECISIONS §9) for no benefit at all.
    let map = *RAM.lock();
    (0..map.count).map(move |i| map.regions[i])
}

/// Where the frame bitmap landed, and how big it is. Test support.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's bootstrap tests are the callers
pub fn bitmap_region() -> (u64, u64) {
    (
        BITMAP_START.load(core::sync::atomic::Ordering::Relaxed) as u64,
        BITMAP_BYTES.load(core::sync::atomic::Ordering::Relaxed) as u64,
    )
}

/// **Record where the loader put the initial ramdisk**, for a front end that is not the device
/// tree's (milestone 161).
///
/// [`init`] reads the bounds out of `/chosen` and stores them itself, which works on the two
/// architectures that have a device tree. `x86_64` gets them from PVH's module list instead
/// (`arch::x86_64::machine::initial_ramdisk`), so the storing has to be reachable from there. This
/// is the same seam [`bring_up_page_frames`] is: the fact crosses it, the *source* of the fact does
/// not.
///
/// **It does not reserve the region**, and the asymmetry with [`init`] is deliberate. A caller here
/// is already building the `forbidden` slice it hands [`bring_up_page_frames`], so doing it in both
/// places would either double-reserve or, worse, make each side assume the other did it. The x86
/// front end passes the same region both ways, one line apart, where a reader can see it.
/// Only a device-tree-less front end calls this, so on the two architectures that have a tree it is
/// dead, and saying so here is cheaper than the alternative (`#[cfg(target_arch = "x86_64")]`),
/// which would make a seam look like an x86 detail.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `record_initrd` (the foreign spelling, which stops at the
/// boundary where it arrives).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub fn record_initial_ramdisk(start: u64, size: u64) {
    INITIAL_RAMDISK_START.store(start as usize, core::sync::atomic::Ordering::Relaxed);
    INITIAL_RAMDISK_SIZE.store(size as usize, core::sync::atomic::Ordering::Relaxed);
}

/// The initial ramdisk, if the bootloader gave us one. Test support.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd_region` (the foreign spelling, which stops at the
/// boundary where it arrives).
pub fn initial_ramdisk_region() -> Option<(u64, u64)> {
    let start = INITIAL_RAMDISK_START.load(core::sync::atomic::Ordering::Relaxed) as u64;
    let size = INITIAL_RAMDISK_SIZE.load(core::sync::atomic::Ordering::Relaxed) as u64;
    (size > 0).then_some((start, size))
}

/// **Where the loader left a copy of its own file**, from the second PVH module
/// (`arch::x86_64::machine::boot_file`). Milestone 198 (a package manager, and the trivial install
/// that makes a second customer possible)'s rung 2a.
///
/// Recorded exactly the way [`record_initial_ramdisk`] is, and for the same reason: the caller is
/// already building the `forbidden` slice, so this stores the fact and reserves nothing.
///
/// **What it is for, in one sentence**: the running system does not otherwise have its own file,
/// and an installer has to write that file to a disk's EFI system partition. `uefi_loader`'s
/// `place_boot_file` has the argument.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_boot_file(start: u64, size: u64) {
    BOOT_FILE_START.store(start as usize, core::sync::atomic::Ordering::Relaxed);
    BOOT_FILE_SIZE.store(size as usize, core::sync::atomic::Ordering::Relaxed);
}

/// **The boot file, if this boot came from one a loader could read back.** `None` on every boot
/// that did not: QEMU's `-kernel` PVH path, the device-tree architectures, and a UEFI boot whose
/// volume did not hold the removable-media path. A system that cannot answer this cannot install
/// itself, which is a limitation rather than a failure.
///
/// Only `user::install_service` reads it, and that module is `x86_64` only for the reasons stated
/// at its declaration, so on the other two architectures this is dead. Saying so here is cheaper
/// than a `cfg` that would make a portable fact look like an x86 detail, which is
/// [`record_initial_ramdisk`]'s own reasoning for the same attribute.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn boot_file_region() -> Option<(u64, u64)> {
    let start = BOOT_FILE_START.load(core::sync::atomic::Ordering::Relaxed) as u64;
    let size = BOOT_FILE_SIZE.load(core::sync::atomic::Ordering::Relaxed) as u64;
    (size > 0).then_some((start, size))
}

/// **Which boot slot started this image** (`arch::x86_64::machine::boot_slot`), recorded at memory
/// bring-up from the token the chooser wrote onto the kernel's command line.
///
/// Stored as `slot + 1` so that zero is "no chooser started this boot", which is the ordinary case
/// and has to be distinguishable from slot 0 rather than confused with it: confirming slot 0 on a
/// machine that actually booted slot 1 would mark an image good that never ran.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_boot_slot(slot: Option<u8>) {
    let encoded = slot.map_or(0, |s| s as usize + 1);
    BOOT_SLOT_PLUS_ONE.store(encoded, core::sync::atomic::Ordering::Relaxed);
}

/// **The boot slot this image was started from, if a chooser started it.**
///
/// `None` for a stick, a `-kernel` boot, and an installed machine whose chooser fell back to the
/// image in its own file. All three are boots with nothing to confirm, and `None` is the answer
/// that leaves the disk alone.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn boot_slot() -> Option<u8> {
    match BOOT_SLOT_PLUS_ONE.load(core::sync::atomic::Ordering::Relaxed) {
        0 => None,
        n => Some((n - 1) as u8),
    }
}

/// **Whether this boot came from the internal NVMe disk** (`arch::x86_64::machine`), from the
/// `boot_slot::medium` token `uefi_loader` writes. `false` for a stick, a `-kernel` boot, and every
/// other architecture, all of which leave the internal disk unmounted: RedoxFS writes on mount, and
/// a boot that did not come from the disk has not been asked to change it (the live stick
/// proposal's G2; calef's ruling on PR #1652, 2026-10-04 UTC).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn record_booted_from_nvme(yes: bool) {
    BOOTED_FROM_NVME.store(yes, core::sync::atomic::Ordering::Relaxed);
}

/// What [`record_booted_from_nvme`] recorded.
pub(crate) fn booted_from_nvme() -> bool {
    BOOTED_FROM_NVME.load(core::sync::atomic::Ordering::Relaxed)
}

/// Written once at boot, before any reader exists; see [`INITIAL_RAMDISK_START`].
static BOOTED_FROM_NVME: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Where [`record_boot_slot`] puts it. Zero means no chooser; see [`INITIAL_RAMDISK_START`] for why
/// this is an atomic written once at boot rather than a lock.
static BOOT_SLOT_PLUS_ONE: AtomicUsize = AtomicUsize::new(0);

/// Where [`record_boot_file`] puts the boot file's base. See [`INITIAL_RAMDISK_START`] for why this
/// is an atomic written once at boot rather than a lock.
static BOOT_FILE_START: AtomicUsize = AtomicUsize::new(0);
/// How many bytes of it there are; zero means there is none.
static BOOT_FILE_SIZE: AtomicUsize = AtomicUsize::new(0);

use core::sync::atomic::AtomicUsize;

/// The RAM map, kept so the MMU can map it.
///
/// **One lock, not sixteen.** This started life as `[IrqSafeMutex<(u64, u64)>; 16]`, which was
/// sixteen locks for one piece of data and took one of them *per element* while iterating.
/// That got the concurrency story exactly backwards: this is not shared mutable state, it is a
/// **constant that happens to be computed at boot**, written once while single-threaded and
/// read forever after.
///
/// Fixed-size rather than a `Vec` because `memory::init` runs *before* the heap exists. It is
/// the last place in the kernel with that excuse.
#[derive(Clone, Copy)]
struct RamMap {
    regions: [(u64, u64); MAX_REGIONS],
    count: usize,
}

static RAM: IrqSafeMutex<RamMap> = IrqSafeMutex::new(
    rank::RAM,
    RamMap {
        regions: [(0, 0); MAX_REGIONS],
        count: 0,
    },
);

/// The GIC the device tree names, with its version (milestone 227). Physical addresses.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
static GIC_REGIONS: IrqSafeMutex<Option<machine_discovery::gic::Gic>> =
    IrqSafeMutex::new(rank::RAM, None);

/// The PLIC's single register block, from the device tree (milestone 20). `None` on aarch64 (no such
/// node) and until `init` has run.
static PLIC_REGION: IrqSafeMutex<Option<(u64, u64)>> = IrqSafeMutex::new(rank::RAM, None);

/// The JH7110's SD/MMC controllers (milestone 53). `None` until `init`, and on every machine that
/// is not a JH7110.
static JH7110_STORAGE: IrqSafeMutex<Option<[Option<designware_mobile_storage::jh7110::Slot>; 2]>> =
    IrqSafeMutex::new(rank::RAM, None);

/// The JH7110's STG clock-and-reset window, from the device tree or from the constant both
/// published trees agree on (milestone 220). `None` until `init`, and on every machine that is
/// not a JH7110.
static JH7110_CRG: IrqSafeMutex<Option<jh7110_clock_and_reset::Found>> =
    IrqSafeMutex::new(rank::RAM, None);

/// The JH7110's SYS clock-and-reset window and the PMIC bus's plan (milestone 592). `None` until
/// `init`, and on every machine that is not a JH7110.
static JH7110_PMIC_BUS: IrqSafeMutex<
    Option<(
        jh7110_clock_and_reset::Found,
        jh7110_clock_and_reset::PmicBus,
    )>,
> = IrqSafeMutex::new(rank::RAM, None);

/// The JH7110's watchdog and its SYS window (milestone 593). `None` until `init`, and on every
/// machine whose tree names no such watchdog.
#[cfg(all(target_arch = "riscv64", soak_watchdog))]
static JH7110_WATCHDOG: IrqSafeMutex<
    Option<(jh7110_watchdog::Discovered, jh7110_clock_and_reset::Found)>,
> = IrqSafeMutex::new(rank::RAM, None);

/// The JH7110's first Ethernet port and its bring-up windows (milestone 53). `None` until `init`,
/// and on every machine whose tree names no such port.
static JH7110_ETHERNET: IrqSafeMutex<Option<Jh7110Ethernet>> = IrqSafeMutex::new(rank::RAM, None);

/// The generic-ECAM PCIe host bridge's windows: (ecam, mem32), each (base, size). Physical.
type PciWindows = ((u64, u64), (u64, u64));

/// `None` until `init`, and on a machine whose device tree has no such node (the JH7110).
static PCI_REGIONS: IrqSafeMutex<Option<PciWindows>> = IrqSafeMutex::new(rank::RAM, None);

/// The SMMUv3's register block, from the device tree (milestone 16b). `None` unless the machine was
/// started with `-machine virt,iommu=smmuv3` (and always on riscv, whose IOMMU is a PCI function
/// found by enumeration, not a platform device with a node).
static SMMU_REGION: IrqSafeMutex<Option<(u64, u64)>> = IrqSafeMutex::new(rank::RAM, None);

/// VT-d's register blocks (base, size), one per DRHD in table order (milestone 161 (the
/// `x86_64` kernel port), roadmap item 6; every unit since milestone 594 (every VT-d unit
/// translates its own devices)). All `None` on a machine with no `-device intel-iommu`, or
/// before they are recorded. `x86_64`'s
/// counterpart of `SMMU_REGION`: filled by `main.rs`'s boot tour rather than by `init`, for the
/// same reason [`record_pci_regions`] is, and read by `mmu::map_everything` so the register file
/// is a mapped device window before `arch::iommu::init` ever reads it (`arch::x86_64::iommu`'s
/// own accessors go through `phys_to_virt`, which is arithmetic, not a promise the address is
/// mapped).
static VTD_REGIONS: IrqSafeMutex<[Option<(u64, u64)>; machine_discovery::acpi::MAX_DRHDS]> =
    IrqSafeMutex::new(rank::RAM, [None; machine_discovery::acpi::MAX_DRHDS]);

/// AMD-Vi's register blocks (base, size), one per IVHD the IVRS names (lane `amd-vi`). The same
/// role as [`VTD_REGIONS`] on an AMD machine, kept apart rather than shared because the two are
/// named for the hardware they hold and a machine has one kind or the other.
static AMD_VI_REGIONS: IrqSafeMutex<
    [Option<(u64, u64)>; machine_discovery::acpi::ivrs::MAX_IVHDS],
> = IrqSafeMutex::new(rank::RAM, [None; machine_discovery::acpi::ivrs::MAX_IVHDS]);

/// The screen the boot handoff described (milestone 243). See [`framebuffer`].
static FRAMEBUFFER: IrqSafeMutex<Option<(u64, u64)>> = IrqSafeMutex::new(rank::RAM, None);

/// The RTC's register block and binding kind (milestone 51). `None` until `init` has run, and on a
/// machine whose device tree describes neither RTC we can drive.
static RTC_REGION: IrqSafeMutex<Option<(u64, u64, u64)>> = IrqSafeMutex::new(rank::RAM, None);

/// The console UART's interrupt line, from the device tree. `None` until `init`, and on a tree
/// that does not say.
static UART_IRQ: IrqSafeMutex<Option<u32>> = IrqSafeMutex::new(rank::RAM, None);

static BITMAP_START: AtomicUsize = AtomicUsize::new(0);
static BITMAP_BYTES: AtomicUsize = AtomicUsize::new(0);
static INITIAL_RAMDISK_START: AtomicUsize = AtomicUsize::new(0);
static INITIAL_RAMDISK_SIZE: AtomicUsize = AtomicUsize::new(0);

/// The boot banner's memory line. Only the banner in `main.rs` calls it, and the test build and the
/// `bench` boot mode both compile the banner out, so it has no caller in exactly those two.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    let Some(s) = stats() else {
        println!("  memory          : uninitialized");
        return;
    };

    let mib = |frames: usize| (frames as u64 * FRAME_SIZE) / (1024 * 1024);
    let kib = |frames: usize| (frames as u64 * FRAME_SIZE) / 1024;

    println!(
        "  memory          : {} MiB total, {} MiB free ({} KiB in use)",
        mib(s.total),
        mib(s.free()),
        kib(s.used),
    );
}

/// `__image_start` and `__image_end` are invented by the linker script, which is the only
/// thing that knows where we ended up. See notes/linker-scripts.md.
///
/// **They are VIRTUAL addresses**, because the kernel is linked high. Everything in this
/// module deals in *physical* frames, so we convert on the way in. Getting this backwards
/// would reserve frames that don't exist and hand out the ones that hold our code.
///
/// `pub(crate)` since milestone 161: an architecture that assembles its own forbidden list
/// (`x86_64`, which has no device tree to read one from) needs the image's bounds to put in it,
/// and the alternative was a second copy of this in `arch/`.
pub(crate) fn image_start() -> u64 {
    unsafe extern "C" {
        static __image_start: core::ffi::c_void;
    }
    virt_to_phys((&raw const __image_start) as u64)
}

pub(crate) fn image_end() -> u64 {
    unsafe extern "C" {
        static __image_end: core::ffi::c_void;
    }
    virt_to_phys((&raw const __image_end) as u64)
}

fn align_up(value: u64, to: u64) -> u64 {
    value.div_ceil(to) * to
}

#[cfg(test)]
mod tests {
    //! Tests for the physical memory map and the frame allocator.
    //!
    //! The allocator's *logic* is tested exhaustively on the host (`cargo test -p frames`, 14
    //! tests, no emulator). What only the real machine can tell us is whether we pointed it at the
    //! right memory, and whether the frames it hands out are actually reachable. That is all these
    //! check.

    /// Proves we read a plausible memory map out of the device tree.
    ///
    /// The allocator logic is tested exhaustively on the host (`cargo test -p frames`,
    /// 14 tests, no emulator). What *only* the real machine can tell us is whether we
    /// pointed it at the right memory, so that's all this checks.
    ///
    /// This used to assert an exact `256 * 1024 * 1024`, QEMU's runner-supplied `-m 256M`. Wrong
    /// as a general claim: the VisionFive 2's device tree describes 4 GiB (`reg = <0x0 0x40000000
    /// 0x1 0x0>`, bench 2026-08-21), regardless of what the board's own 8 GiB of physical DRAM
    /// U-Boot's banner claims, so an exact QEMU literal was never going to survive a second
    /// machine. The invariant this test actually owes is that the parse produced something real:
    /// nonzero, and not some implausibly tiny remnant of a misparsed `reg` (a wrong cell width or
    /// endianness reads back as a handful of bytes, not gigabytes).
    #[test_case]
    fn memory_map_came_from_the_device_tree() {
        use page_frames::FRAME_SIZE;

        let s = crate::memory::stats().expect("allocator not initialized");

        // If this reads zero, or something absurd, we have misparsed `reg` (which is big-endian,
        // and whose cell width is declared by the *parent* node, both of which are easy to get
        // wrong). 16 MiB is well below every machine this kernel has run on (QEMU's 256 MiB
        // runner default, the VisionFive 2's 4 GiB tree) and well above what a cell-width or
        // endianness bug would produce.
        let total_bytes = s.total as u64 * FRAME_SIZE;
        assert!(
            total_bytes >= 16 * 1024 * 1024,
            "unexpectedly small RAM size: {total_bytes} bytes; likely a misparsed reg property"
        );

        // Some memory must already be spoken for: at minimum the kernel image, the
        // bitmap, and the device tree. A zero here means we reserved nothing, which
        // means we are about to hand out our own code.
        assert!(s.used > 0, "nothing is reserved?");
        assert!(s.free() > 0, "no free memory at all?");
    }

    /// **The one that matters.** Every frame the kernel image touches must be reserved.
    ///
    /// This states the invariant `mark_used` exists to maintain, directly. Our image ends
    /// at 0x40097010, which is not frame-aligned, so the last frame is only *partly*
    /// ours. Round that end down instead of up and the frame stays free, the allocator
    /// hands it out, something writes to it, and the tail of the kernel is quietly
    /// overwritten. The crash lands somewhere else entirely, much later, in code that did
    /// nothing wrong.
    ///
    /// Checking the bitmap directly is both stronger and cheaper than draining the
    /// allocator: it covers *every* frame of the image, and it allocates nothing.
    #[test_case]
    fn every_frame_of_the_kernel_image_is_reserved() {
        use page_frames::{FRAME_SIZE, PageFrame};

        let (start, end) = crate::memory::image_bounds();
        let mut addr = start - start % FRAME_SIZE; // round DOWN to the containing frame

        while addr < end {
            assert_eq!(
                crate::memory::is_page_frame_used(PageFrame::from_addr(addr)),
                Some(true),
                "frame {addr:#x} overlaps the kernel image but is marked FREE"
            );
            addr += FRAME_SIZE;
        }
    }

    /// And prove `alloc` actually respects that bitmap.
    ///
    /// Keep this array SMALL. It was `[Option<PageFrame>; 1024]` (16 KiB) on a 64 KiB stack,
    /// and it silently overflowed into .bss, .data, and .text, and hung the machine while
    /// printing something unrelated. See notes/stack.md. The canary catches that now, but
    /// the right move is to not do it.
    #[test_case]
    fn allocator_never_hands_out_the_kernel() {
        let mut taken = [None; 64];

        for slot in taken.iter_mut() {
            let Some(frame) = crate::memory::alloc() else {
                break;
            };
            assert!(
                !crate::memory::is_in_kernel_image(frame.addr()),
                "allocator handed out {:#x}, which is inside the kernel image",
                frame.addr()
            );
            *slot = Some(frame);
        }

        // `iter_mut().take()` and not `into_iter()`, which is the same lesson as the array size
        // above one step further on. `into_iter` on an array consumes it BY VALUE, and a debug
        // build gives that move its own stack temporary: `script/stack-frame-check` measured
        // `<[Option<PageFrame>; 64] as IntoIterator>::into_iter` at **4224 bytes**, over the 4096-byte
        // guard page, on both ISAs. Taking each slot in place copies one `Option<PageFrame>` instead.
        for slot in taken.iter_mut() {
            if let Some(frame) = slot.take() {
                crate::memory::free(frame);
            }
        }
    }

    /// Proves a frame we were given is real, writable memory that nothing else owns.
    ///
    /// Host tests prove the *bookkeeping* is right. Only the machine can prove the
    /// bookkeeping corresponds to actual RAM. Writing a pattern and reading it back is
    /// the cheapest way to find out we've been handing out an MMIO hole.
    #[test_case]
    fn an_allocated_frame_is_real_memory() {
        use page_frames::FRAME_SIZE;

        let frame = crate::memory::alloc().expect("out of memory");
        // The allocator speaks physical; we must name it virtually to touch it.
        let ptr = crate::arch::mmu::phys_to_virt(frame.addr()) as *mut u64;
        let words = (FRAME_SIZE / 8) as usize;

        // SAFETY: the allocator just gave us this frame, so we own it exclusively. The
        // MMU is off, so the physical address is directly usable.
        unsafe {
            for i in 0..words {
                core::ptr::write_volatile(ptr.add(i), 0xcafe_f00d_0000_0000 | i as u64);
            }
            for i in 0..words {
                assert_eq!(
                    core::ptr::read_volatile(ptr.add(i)),
                    0xcafe_f00d_0000_0000 | i as u64,
                    "frame {:#x} word {i} did not hold what we wrote",
                    frame.addr()
                );
            }
        }

        crate::memory::free(frame);
    }

    /// The bitmap must not sit on top of anything already spoken for.
    ///
    /// We used to place it immediately after the kernel image and hope. That worked, but
    /// only because QEMU happens to put the device tree 64 MiB higher up. `image_size` in
    /// the arm64 Image header stops at `__stack_top`, so everything past `__image_end` is
    /// memory we never told the bootloader we wanted, and different firmware need not
    /// leave it alone. Now the placement is scanned and proven; this checks it.
    #[test_case]
    fn bitmap_overlaps_nothing() {
        let (bstart, bsize) = crate::memory::bitmap_region();
        assert!(bsize > 0, "bitmap has no size?");

        let (istart, iend) = crate::memory::image_bounds();
        assert!(
            bstart + bsize <= istart || bstart >= iend,
            "bitmap {bstart:#x}+{bsize:#x} overlaps the kernel image {istart:#x}..{iend:#x}"
        );

        let dtb = crate::DTB.load(core::sync::atomic::Ordering::Relaxed) as u64;
        assert!(
            bstart + bsize <= dtb || bstart >= dtb + 64 * 1024,
            "bitmap {bstart:#x}+{bsize:#x} is sitting on the device tree at {dtb:#x}"
        );

        if let Some((istart, isize)) = crate::memory::initial_ramdisk_region() {
            assert!(
                bstart + bsize <= istart || bstart >= istart + isize,
                "bitmap {bstart:#x}+{bsize:#x} is sitting on the initial ramdisk"
            );
        }
    }

    /// If the bootloader gave us an initial ramdisk, the allocator must never hand it out.
    ///
    /// Only meaningful when QEMU is run with `-initrd`, which the default test run isn't.
    /// It asserts the invariant when there IS one and **skips** when there isn't, which is the
    /// right shape: the check exists so that the day someone adds `-initrd` to the runner, this
    /// catches it rather than milestone 10 catching it.
    ///
    /// It used to say "passes trivially when there isn't", and did: it returned, and the harness
    /// counted a pass for a boot where the invariant was never looked at. That is the defect
    /// milestone 214 (design/roadmap/0214-print-and-return-skips.md) swept, in its silent form:
    /// a test that prints "skipping" and returns is counted as passed, and one that prints
    /// nothing at all is counted as passed too.
    #[test_case]
    fn initial_ramdisk_is_reserved_if_present() {
        use page_frames::{FRAME_SIZE, PageFrame};

        let Some((start, size)) = crate::memory::initial_ramdisk_region() else {
            crate::testing::skip!("no initial ramdisk region on this boot (no -initrd passed)");
        };

        let mut addr = start - start % FRAME_SIZE;
        while addr < start + size {
            assert_eq!(
                crate::memory::is_page_frame_used(PageFrame::from_addr(addr)),
                Some(true),
                "frame {addr:#x} is part of the initial ramdisk but is marked FREE"
            );
            addr += FRAME_SIZE;
        }
    }

    /// Proves alloc and free actually balance, on the real memory map.
    #[test_case]
    fn alloc_and_free_balance() {
        let before = crate::memory::stats().unwrap();

        let a = crate::memory::alloc().unwrap();
        let b = crate::memory::alloc_contiguous(8).unwrap();

        assert_eq!(crate::memory::stats().unwrap().used, before.used + 9);

        crate::memory::free(a);
        for i in 0..8u64 {
            crate::memory::free(page_frames::PageFrame::from_addr(
                b.addr() + i * page_frames::FRAME_SIZE,
            ));
        }

        assert_eq!(
            crate::memory::stats().unwrap(),
            before,
            "frames leaked or were double-counted"
        );
    }
}
