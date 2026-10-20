use super::*;

/// The device-writable descriptor flag, bit 1. The kernel validator does not name it (it bounds
/// addresses whichever way the device moves the bytes), but the receive-direction tests set it
/// to build a descriptor the *device writes into*, which is what a virtio-net RX buffer is.
const VIRTQ_DESC_F_WRITE: u16 = 2;

// Read/write the direct map at a physical address. One set serves both the fake driver region
// and the fake shadow region below, since they take absolute addresses. Passed to
// `validate_and_shadow` as `&dyn Fn` (function items coerce).
fn r16(p: u64) -> u16 {
    // SAFETY: `phys` lies in the DMA region, which `mmu::map_everything` covers, so `phys_to_virt` names mapped memory. Volatile because this is the DRIVER's memory: the validator must read what is there now, not a value the compiler cached.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(p) as *const u16) }
}
fn r64(p: u64) -> u64 {
    // SAFETY: `phys` lies in the DMA region, which `mmu::map_everything` covers, so `phys_to_virt` names mapped memory. Volatile because this is the DRIVER's memory: the validator must read what is there now, not a value the compiler cached.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(p) as *const u64) }
}
fn w16(p: u64, v: u16) {
    // SAFETY: the shadow ring is a kernel-owned frame reached through the direct map, so this is an ordinary store to memory only the kernel names.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(p) as *mut u16, v) }
}
fn w64(p: u64, v: u64) {
    // SAFETY: the shadow ring is a kernel-owned frame reached through the direct map, so this is an ordinary store to memory only the kernel names.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(p) as *mut u64, v) }
}

/// Write descriptor `i` of the table at `desc`: { u64 addr; u32 len; u16 flags; u16 next }.
fn write_desc(desc: u64, i: u64, addr: u64, len: u32, flags: u16, next: u16) {
    let d = desc + i * 16;
    w64(d, addr);
    w64(d + 8, len as u64); // len in the low half; flags/next written over the high half next
    w16(d + 12, flags);
    w16(d + 14, next);
}

/// A fake driver DMA region and a fake shadow region, each a real frame reached through the
/// direct map. Returns their physical bases and the region size. Free both with [`free_regions`].
fn two_regions() -> (u64, u64, u64) {
    let driver = crate::memory::alloc().expect("no driver frame").addr();
    let shadow = crate::memory::alloc().expect("no shadow frame").addr();
    (driver, shadow, page_frames::FRAME_SIZE)
}
fn free_regions(driver: u64, shadow: u64) {
    crate::memory::free(page_frames::PageFrame::from_addr(driver));
    crate::memory::free(page_frames::PageFrame::from_addr(shadow));
}

/// Drive `validate_and_shadow` against the fake regions with the standard closures. Queue 0,
/// the single-queue shape the disk uses.
fn run(driver: u64, size: u64, shadow: u64, from: u16, to: u16) -> bool {
    run_q(driver, size, shadow, 0, from, to)
}

/// Drive `validate_and_shadow` for a specific queue, applying that queue's ring-block offset in
/// both the driver region and the shadow, exactly as `notify(id, queue)` does. Lets a test
/// exercise queue 1 (virtio-net's transmit queue) on its own block.
fn run_q(driver: u64, size: u64, shadow: u64, q: u16, from: u16, to: u16) -> bool {
    let block = queue_block(q);
    validate_and_shadow(
        driver,
        size,
        driver + block + DESC_OFF,
        driver + block + AVAIL_OFF,
        shadow + block + DESC_OFF,
        shadow + block + AVAIL_OFF,
        from,
        to,
        &r16,
        &r64,
        &w16,
        &w64,
    )
}

/// Build a fake DMA region and exercise the security-critical check: a descriptor pointing
/// OUTSIDE the region is refused, one inside is accepted, and a `next`-cycle cannot hang it.
#[test_case]
fn the_validator_refuses_a_descriptor_that_escapes_the_dma_region() {
    let (driver, shadow, size) = two_regions();
    let desc = driver + DESC_OFF;
    let avail = driver + AVAIL_OFF;

    // --- a GOOD chain: header + data + status, all inside the region ---
    write_desc(desc, 0, driver + 0x200, 16, VIRTQ_DESC_F_NEXT, 1);
    write_desc(desc, 1, driver + 0x400, 512, VIRTQ_DESC_F_NEXT, 2);
    write_desc(desc, 2, driver + 0x600, 1, 0, 0);
    w16(avail + 4, 0); // ring[0] = head 0
    w16(avail + 2, 1); // avail.idx = 1
    assert!(
        run(driver, size, shadow, 0, 1),
        "a chain wholly inside the region was rejected",
    );

    // --- the ATTACK: descriptor 1 points at kernel memory (the kernel image) ---
    write_desc(
        desc,
        1,
        0xffff_0000_4008_0000,
        512,
        VIRTQ_DESC_F_NEXT | 2,
        2,
    );
    assert!(
        !run(driver, size, shadow, 0, 1),
        "a descriptor pointing at kernel memory was NOT refused",
    );

    // --- a length that overflows the region by one byte ---
    write_desc(desc, 1, driver + size - 256, 512, VIRTQ_DESC_F_NEXT | 2, 2);
    assert!(
        !run(driver, size, shadow, 0, 1),
        "a descriptor running past the end of the region was NOT refused",
    );

    // --- a next-pointer cycle must terminate, not hang ---
    write_desc(desc, 0, driver + 0x200, 16, VIRTQ_DESC_F_NEXT, 1);
    write_desc(desc, 1, driver + 0x400, 16, VIRTQ_DESC_F_NEXT, 0); // 1 -> 0 -> 1 -> ...
    assert!(
        run(driver, size, shadow, 0, 1),
        "a bounded cyclic chain with valid addresses should still validate (and terminate)",
    );

    free_regions(driver, shadow);
}

/// **The shadow ring closes the time-of-check/time-of-use race.**
///
/// Validate a good chain (which copies it into the shadow), then mutate the DRIVER's descriptor
/// to point at kernel memory, exactly as a device fetching descriptors asynchronously would let
/// a driver do after the check. The device reads the SHADOW, so the shadow must still hold the
/// validated, in-region address: the mutation touched only the driver's own copy, which nothing
/// reads. This is the whole reason the shadow ring exists.
///
/// BUGS: the driver's frame and the shadow are separate allocations, so reading the shadow back
/// after writing the driver's copy cannot fail whatever the code does. The line the claim rests
/// on, that the device's queue registers are programmed with the shadow's address and never the
/// driver's, is asserted nowhere. Found by milestone 633 (an outside agent attacks the confinement claim)'s third outsider pass (2026-10-06 UTC),
/// by read; it is the structural disposition `notes/confinement-claims.md` gives row 17, with
/// the register programming named as the missing check.
#[test_case]
fn the_shadow_ring_is_immune_to_a_descriptor_mutated_after_validation() {
    let (driver, shadow, size) = two_regions();
    let desc = driver + DESC_OFF;
    let avail = driver + AVAIL_OFF;
    let shadow_desc = shadow + DESC_OFF;
    let shadow_avail = shadow + AVAIL_OFF;

    // One valid in-region descriptor, published as head 0.
    let good_addr = driver + 0x200;
    write_desc(desc, 0, good_addr, 512, 0, 0);
    w16(avail + 4, 0); // ring[0] = head 0
    w16(avail + 2, 1); // avail.idx = 1

    assert!(
        run(driver, size, shadow, 0, 1),
        "a valid descriptor was rejected"
    );
    assert_eq!(
        r64(shadow_desc),
        good_addr,
        "the shadow did not receive the validated descriptor",
    );
    assert_eq!(
        r16(shadow_avail + 2),
        1,
        "the shadow avail.idx was not published"
    );

    // The driver now aims its descriptor at kernel memory, AFTER the check. On async-DMA
    // hardware this is the race. The device reads the shadow, which must be untouched.
    w64(desc, 0xffff_0000_4008_0000);
    assert_eq!(
        r64(shadow_desc),
        good_addr,
        "a post-validation write to the driver's descriptor reached the shadow the device \
         reads: the TOCTOU race is open",
    );

    free_regions(driver, shadow);
}

/// **An indirect descriptor is refused even when it points inside the region.**
///
/// A descriptor flagged `INDIRECT` points at a *table* of further descriptors we do not copy
/// into the shadow, so the device would follow it out of the region. A wholly in-region
/// indirect descriptor still has to be refused, because it is not the descriptor the device
/// ultimately acts on.
#[test_case]
fn the_validator_refuses_an_indirect_descriptor() {
    let (driver, shadow, size) = two_regions();
    let desc = driver + DESC_OFF;
    let avail = driver + AVAIL_OFF;

    // desc[0]: a legal in-region address, but flagged INDIRECT.
    write_desc(desc, 0, driver + 0x200, 128, VIRTQ_DESC_F_INDIRECT, 0);
    w16(avail + 4, 0); // ring[0] = head 0
    w16(avail + 2, 1); // avail.idx = 1

    assert!(
        !run(driver, size, shadow, 0, 1),
        "an indirect descriptor was accepted: the device could follow its unvalidated table \
         out of the DMA region",
    );

    free_regions(driver, shadow);
}

/// **Feature negotiation strips the ring-layout features the validator cannot police.**
///
/// A driver that asks for `INDIRECT_DESC` (bit 28) or `RING_PACKED` (bit 34) gets that bit
/// cleared before the device sees it, so the device never honours a descriptor format the
/// validator does not understand. Every other bit passes through untouched, so real device
/// features still negotiate.
#[test_case]
fn feature_negotiation_strips_indirect_and_packed() {
    // Low word (sel 0): INDIRECT_DESC at bit 28 is cleared; unrelated bits survive.
    let asked_lo = (1 << 28) | (1 << 5) | 1; // indirect + two blk feature bits
    let got_lo = sanitize_driver_features(0, asked_lo);
    assert_eq!(got_lo & (1 << 28), 0, "INDIRECT_DESC was not stripped");
    assert_eq!(got_lo, (1 << 5) | 1, "a non-ring feature bit was disturbed");

    // High word (sel 1): RING_PACKED is feature bit 34, i.e. bit 2 of the high word.
    let asked_hi = (1 << 2) | 1; // packed + VERSION_1 (bit 32 = high-word bit 0)
    let got_hi = sanitize_driver_features(1, asked_hi);
    assert_eq!(got_hi & (1 << 2), 0, "RING_PACKED was not stripped");
    assert_eq!(got_hi & 1, 1, "VERSION_1 must survive negotiation");
}

/// **`VIRTIO_BLK_F_FLUSH` survives negotiation** (milestone 55), which is the one feature bit
/// whose loss would be silent and expensive.
///
/// The block server asks for feature bit 9 and treats the answer as the truth about whether the
/// device can be flushed at all. If a future filter cleared it here, the block server would see
/// a device with no flush, `filesystem_contract::fs::SYNC` would start answering `EOPNOTSUPP`, and any
/// server would stop being able to back the `VOLUME_FULL_SYNC` bit it advertises to macOS. That
/// chain is long enough that the failure would surface as "Time Machine stopped working" rather
/// than as anything pointing here, so the bit gets its own assertion next to the two that *are*
/// stripped on purpose.
///
/// It is deliberately not folded into the test above: that one is about the ring-layout
/// features, and this one is about a device feature something depends on. Same function, two
/// different claims.
#[test_case]
fn feature_negotiation_leaves_the_blk_flush_bit_alone() {
    const F_FLUSH_LO: u32 = 1 << 9; // VIRTIO_BLK_F_FLUSH
    assert_eq!(
        sanitize_driver_features(0, F_FLUSH_LO),
        F_FLUSH_LO,
        "VIRTIO_BLK_F_FLUSH must reach the device: the block server's whole durability answer \
         is whether it was negotiated (crates/virtio's F_FLUSH_LO)"
    );
    // And it must survive alongside the bit that is stripped, because that is how the honest
    // driver and the indirect attacker differ: one asks for flush, the other for indirect.
    assert_eq!(
        sanitize_driver_features(0, F_FLUSH_LO | (1 << 28)),
        F_FLUSH_LO
    );
}

/// **A PCI queue in range but never set up has no doorbell, and ringing it would leave the
/// BAR** (milestone 112).
///
/// `notify(id, queue)` checked `queue < MAX_QUEUES` and nothing else, so a userspace driver
/// holding a virtio capability could `NOTIFY` a queue it had never `SETUP_QUEUE`d. On the PCI
/// transport `notify_addr[queue]` is zero until `setup_queue` resolves it from
/// `queue_notify_off`, so the ring became a `write_volatile` of a `u16` through
/// `phys_to_virt(0)`: a kernel store, at a physical address inside no BAR, at a moment the
/// driver chose.
///
/// This is a **unit test of the predicate, not of the syscall**, and deliberately so: it builds
/// the two transport values by hand and touches no device, so it runs on both ISAs and in the
/// mmio-only configurations where no PCI virtio device exists at all. Reaching the real syscall
/// path would need a live PCI function, which only one of the boot configurations has.
#[test_case]
fn a_pci_queue_has_no_doorbell_until_it_is_set_up() {
    // Plausible-looking BAR addresses. Nothing here is ever dereferenced: `is_doorbell_ready`
    // only compares the resolved doorbell against zero.
    let mut t = Transport::Pci {
        common: 0x4010_0000,
        notify_base: 0x4010_3000,
        notify_mult: 4,
        device_type: 2,
        notify_addr: [0; MAX_QUEUES],
        isr: 0x4010_2000,
        msix_vector: pci::VIRTIO_MSIX_NO_VECTOR,
        device_config: None,
    };
    assert!(
        !t.is_doorbell_ready(0),
        "a PCI queue reported a doorbell before setup_queue resolved one; ringing it would \
         write through phys_to_virt(0)",
    );

    // What setup_queue does to the field, without the register traffic.
    if let Transport::Pci { notify_addr, .. } = &mut t {
        notify_addr[0] = 0x4010_3000;
    }
    assert!(
        t.is_doorbell_ready(0),
        "a resolved doorbell was still reported as absent, which would refuse every notify",
    );
    assert!(
        !t.is_doorbell_ready(1),
        "setting up queue 0 must not vouch for queue 1: each queue resolves its own doorbell",
    );

    // The mmio transport has one fixed notify register, so every in-range queue is ringable.
    let m = Transport::Mmio {
        mmio_phys: 0x0a00_0000,
    };
    assert!(
        m.is_doorbell_ready(0) && m.is_doorbell_ready(1),
        "the mmio transport gained a per-queue doorbell it does not have",
    );
}

/// **A jump in `avail.idx` larger than the ring is refused, not walked.**
///
/// The available ring holds only `QSIZE` slots, so at most `QSIZE` descriptors can be newly
/// available between notifies. A hostile driver that advances `avail.idx` by tens of thousands
/// would otherwise make the walk loop that many times, under the `DEVICES` lock with interrupts
/// masked. Every descriptor and ring slot here is individually valid, so the ONLY thing that can
/// refuse the oversized batch is the jump-size guard.
#[test_case]
fn the_validator_refuses_more_new_entries_than_the_ring_can_hold() {
    let (driver, shadow, size) = two_regions();
    let desc = driver + DESC_OFF;
    let avail = driver + AVAIL_OFF;

    // Fill the whole ring with individually valid single-descriptor entries.
    for i in 0..QSIZE as u64 {
        write_desc(desc, i, driver + 0x200 + i * 8, 8, 0, 0);
        w16(avail + 4 + i * 2, i as u16); // ring[i] = head i
    }

    // Exactly QSIZE new entries is legal: the ring holds that many.
    assert!(
        run(driver, size, shadow, 0, QSIZE),
        "a batch of exactly QSIZE valid entries was refused",
    );

    // One more than the ring can hold is refused, though every descriptor is valid.
    assert!(
        !run(driver, size, shadow, 0, QSIZE + 1),
        "a jump of QSIZE+1 was walked instead of refused: a hostile avail.idx could spin the \
         validator up to 65535 times with interrupts masked",
    );

    free_regions(driver, shadow);
}

/// **The receive direction is confined: a device-WRITABLE descriptor aimed outside the region is
/// refused** (milestone 30, the multi-queue confinement's second, proved direction). virtio-net's
/// receive queue is the direction where the *device writes into* the driver's memory: the driver
/// posts an empty buffer and the device fills it with a packet. A hostile driver posts a receive
/// buffer pointing at kernel memory and lets the device write a packet over the kernel image. The
/// validator bounds the address whether the device reads or writes it, so this is refused before
/// the device is rung; the in-region receive buffer that validates first proves the refusal is
/// about the address, not the direction flag. This is the same property milestone 32's write path
/// relied on, now asserted for the direction where the device is the writer.
#[test_case]
fn the_validator_refuses_an_rx_descriptor_that_escapes_the_region() {
    let (driver, shadow, size) = two_regions();
    let desc = driver + DESC_OFF;
    let avail = driver + AVAIL_OFF;

    // A legitimate receive posting: one device-writable buffer, wholly in-region.
    write_desc(desc, 0, driver + 0x400, 1514, VIRTQ_DESC_F_WRITE, 0);
    w16(avail + 4, 0); // ring[0] = head 0
    w16(avail + 2, 1); // avail.idx = 1
    assert!(
        run(driver, size, shadow, 0, 1),
        "an in-region device-writable receive buffer was refused",
    );

    // The attack: the same device-writable buffer, now aimed at kernel memory, so the device
    // would DMA a received packet straight over the kernel image.
    write_desc(desc, 0, 0xffff_0000_4008_0000, 1514, VIRTQ_DESC_F_WRITE, 0);
    assert!(
        !run(driver, size, shadow, 0, 1),
        "a device-writable receive descriptor pointing at kernel memory was NOT refused: the \
         device could overwrite the kernel with a received packet",
    );

    free_regions(driver, shadow);
}

/// **A second queue validates on its own ring block, independent of queue 0** (milestone 30). A
/// virtio-net device drives receive on queue 0 and transmit on queue 1; each queue's rings live
/// at `queue * RING_BLOCK` in both the driver region and the shadow, so a submit on one queue
/// never reads or writes the other's shadow. Validate a good chain on queue 1, confirm it landed
/// in queue 1's shadow block while a sentinel in queue 0's shadow block stayed put, then confirm
/// an escape on queue 1 is refused the same as on queue 0.
#[test_case]
fn a_second_queue_validates_on_its_own_block() {
    let (driver, shadow, size) = two_regions();
    let q1 = queue_block(1);
    let desc1 = driver + q1 + DESC_OFF;
    let avail1 = driver + q1 + AVAIL_OFF;

    // A sentinel in queue 0's shadow descriptor slot. Frames from `alloc` are not zeroed, so we
    // plant a known value and assert queue-1 validation leaves it untouched.
    const SENTINEL: u64 = 0xC0FF_EE00_C0FF_EE00;
    w64(shadow + DESC_OFF, SENTINEL);

    // A good transmit chain on queue 1: header + data, wholly in-region, above every ring block.
    let data = MAX_QUEUES as u64 * RING_BLOCK; // first byte past all queues' rings
    write_desc(desc1, 0, driver + data, 60, VIRTQ_DESC_F_NEXT, 1);
    write_desc(desc1, 1, driver + data + 64, 8, 0, 0);
    w16(avail1 + 4, 0); // ring[0] = head 0
    w16(avail1 + 2, 1); // avail.idx = 1
    assert!(
        run_q(driver, size, shadow, 1, 0, 1),
        "a valid chain on queue 1 was refused",
    );

    // It landed in queue 1's shadow block, and queue 0's shadow block is untouched.
    assert_eq!(
        r64(shadow + q1 + DESC_OFF),
        driver + data,
        "queue 1's validated descriptor did not reach queue 1's shadow block",
    );
    assert_eq!(
        r64(shadow + DESC_OFF),
        SENTINEL,
        "validating queue 1 disturbed queue 0's shadow block",
    );

    // An escape on queue 1 is refused, the same confinement as queue 0.
    write_desc(desc1, 0, 0xffff_0000_4008_0000, 60, VIRTQ_DESC_F_NEXT, 1);
    assert!(
        !run_q(driver, size, shadow, 1, 0, 1),
        "a queue-1 descriptor escaping the region was NOT refused",
    );

    free_regions(driver, shadow);
}

/// **The IOMMU faults a DMA that escapes the domain, in hardware** (milestone 16b, both ISAs).
///
/// The shadow ring proves the *software* refuses an out-of-region descriptor before the device
/// is rung; this proves the *hardware* would stop the device even if that software were bypassed.
/// Confine the PCIe disk to its domain, then point its available ring at a frame the domain does
/// not map and kick it. The device reads that ring at an out-of-domain IOVA, the IOMMU faults,
/// and the fault turns up in the fault/event queue. A pass here also means the IOMMU is really in
/// force: if translation were absent (a missing `iommu=smmuv3` / `riscv-iommu-pci`, or the
/// `iommu_platform=on` that puts the device behind it), the escaping read would succeed silently
/// and no fault would appear, so the assertion fails loudly rather than passing on a fiction.
///
/// Skipped only when there is no PCIe disk on the bus (nothing to confine). An IOMMU that is
/// absent while a disk *is* present is the failure this test exists to catch, so that path
/// asserts rather than skips.
///
/// The record below is AMD-Vi's (lane `amd-vi`), replayed on the `NIFE_IOMMU=amd` machine; the
/// other three units carry this claim without one.
/// Falsification: replayable `kernel/falsifications/virtio.tests.the_iommu_faults_a_dma_that_escapes_the_domain.patch`
#[test_case]
fn the_iommu_faults_a_dma_that_escapes_the_domain() {
    let Some(d) = crate::pci::find_block_device() else {
        // No PCIe disk attached: nothing to confine, nothing to prove. (The test runners always
        // attach one, so this branch is for a bare boot, not the parity gate.)
        crate::testing::skip!("no PCIe disk on the bus, so there is nothing to confine");
    };

    // The IOMMU must be up. If it is not while a PCIe disk is present, every PCIe DMA is
    // bypassing translation: fail loudly, do not skip.
    assert!(
        crate::iommu::is_active(),
        "a PCIe disk is present but the IOMMU is not active: DMA is bypassing translation \
         (is iommu=smmuv3 / -device riscv-iommu-pci missing from the runner?)",
    );

    // **Quiesce every OTHER block device the runner attaches** (found 2026-09-27, x86_64
    // only, but harmless and cheap everywhere: DECISIONS-adjacent, see the BUGS entry this
    // replaces in `arch/x86_64/iommu.rs`). This test's runners attach a second disk, the
    // RedoxFS fixture from milestone 303 (`x86_64`'s FS service has a server and no disk it can
    // find), that no code in this filtered boot ever registers, so
    // it never gets a VT-d context entry. Traced under QEMU's `-d trace:vtd_*`: the moment
    // THIS test's own `confine` below runs its context-cache/IOTLB invalidation, that
    // *other*, unconfined device attempts a real access of its own and correctly faults
    // default-deny (reason 0x2) -- nothing is unconfined in the security sense, the fault
    // fires exactly as it should. But `CAP.NFR` reports one fault-recording register, and
    // that unrelated device keeps refaulting the same address on every retry, so it can win
    // the single slot every time and starve the fault this test provokes on device `d`. A
    // plain virtio reset (`STATUS = 0`) on every other block device stops its DMA outright,
    // the same way `provoke_iommu_escape`'s own reset stops a queue THIS device was left
    // running by an earlier test. Bounded at 8: no runner this tree has attaches more than
    // two block devices, and a bound turns a discovery bug into a fast loop instead of a hang.
    for n in 0..8 {
        let Some(mut other) = find_block_device_n(n) else {
            break;
        };
        if other.rid != Some(d.rid) {
            other.transport.write_reg(REG_STATUS, 0);
        }
    }

    // Register (and thereby confine) the device to its DMA region + shadow page.
    let dma = crate::memory::alloc_zeroed().expect("no DMA frame").addr();
    let id = register(
        Transport::pci(&d),
        dma,
        page_frames::FRAME_SIZE,
        Some(d.rid),
    );

    // A frame the domain does not map: the device's escape target.
    let victim = crate::memory::alloc().expect("no victim frame").addr();
    let victim_page = victim & !0xfff;

    // Drain any stale fault first, so what we observe is ours.
    while crate::iommu::take_fault().is_some() {}

    provoke_iommu_escape(id, victim);

    // Poll the fault/event queue. QEMU records the fault as it processes the kick under TCG, so a
    // bounded spin is plenty; the loop bound turns "no fault ever" into a failure, not a hang.
    let mut fault = None;
    for _ in 0..2_000_000 {
        if let Some(f) = crate::iommu::take_fault() {
            fault = Some(f);
            break;
        }
        core::hint::spin_loop();
    }

    let f = fault.expect(
        "the device read an out-of-domain address and the IOMMU recorded no fault: it is not \
         confining the device in hardware",
    );
    match f.addr {
        Some(addr) => assert_eq!(
            addr & !0xfff,
            victim_page,
            "the IOMMU faulted, but on {addr:#x} (code {:#x}, rid {:#x}), not the escape \
             frame {victim_page:#x}",
            f.code,
            f.rid,
        ),
        // **AMD-Vi under QEMU 11.1.1 reports no address** (`arch/x86_64/iommu.rs`'s
        // `Fault::addr` says why). The fault is then tied to this device by its requester id
        // and nothing more, which is weaker: it says the device was refused something, not
        // that it was refused the victim. What still makes it falsifiable on that machine is
        // the record above this test: with translation off in the device table entry, the
        // escape lands and no fault is recorded at all.
        None => assert_eq!(
            f.rid, d.rid,
            "the IOMMU faulted (code {:#x}) for requester {:#x}, not this disk",
            f.code, f.rid,
        ),
    }
}

/// **A device cannot write the shadow page it reads its rings from** (provisional milestone 767
/// (AMD-Vi hardening before the first AMD boot); every IOMMU this tree drives).
///
/// The shadow page is the one frame in a virtio device's domain whose whole purpose is that
/// only the kernel shapes it, and the device's job there is to read. Until 767 every page of
/// every domain was mapped read-write, so the device could have rewritten the descriptors the
/// kernel had validated. This points the device's used ring (the one structure it writes) into
/// the shadow page and kicks it, and the write must not land: `provoke_write_to_shadow` says
/// why the word is a canary and why no fault record is asked for.
///
/// Falsification: replayable `kernel/falsifications/virtio.tests.a_device_write_to_the_read_only_shadow_is_refused.patch`
#[test_case]
fn a_device_write_to_the_read_only_shadow_is_refused() {
    let Some(d) = crate::pci::find_block_device() else {
        crate::testing::skip!("no PCIe disk on the bus, so there is nothing to confine");
    };
    assert!(
        crate::iommu::is_active(),
        "a PCIe disk is present but the IOMMU is not active: DMA is bypassing translation",
    );
    // Quiesce every other block device, for the reason the escape test above gives.
    for n in 0..8 {
        let Some(mut other) = find_block_device_n(n) else {
            break;
        };
        if other.rid != Some(d.rid) {
            other.transport.write_reg(REG_STATUS, 0);
        }
    }
    let dma = crate::memory::alloc_zeroed().expect("no DMA frame").addr();
    let id = register(
        Transport::pci(&d),
        dma,
        page_frames::FRAME_SIZE,
        Some(d.rid),
    );
    let flags = provoke_write_to_shadow(id);
    assert_eq!(
        flags, 0xffff,
        "the device rewrote its read-only shadow page (used.flags {flags:#06x}): the domain \
         maps the shadow writable",
    );
    while crate::iommu::take_fault().is_some() {}
}

/// **A register read cannot leave the device's slot.** The mmio arm reads `slot base + off`
/// through the direct map, so before this bound a `Virtio` capability read any physical address.
#[test_case]
fn a_register_read_outside_the_slot_is_refused() {
    assert!(register_in_slot(0));
    assert!(register_in_slot(SLOT_STRIDE - 4));
    assert!(!register_in_slot(SLOT_STRIDE));
    assert!(!register_in_slot(2));
    assert!(!register_in_slot(u64::MAX - 3));
    assert_eq!(read_register(0, SLOT_STRIDE), None);
}
