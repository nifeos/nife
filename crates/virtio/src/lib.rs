#![no_std]
//! A virtio driver, block and net, at EL0.
//!
//! The kernel hands the process a DMA page, an `Irq` capability (slot 1) and a `Virtio` transport
//! capability (slot 2); the device registers are not mapped, so every access goes through the
//! kernel, which confines the device to the DMA page. Modern virtio-mmio (version 2), virtio 1.x
//! sections 4.2 (MMIO), 5.1 (net) and 5.2 (block).
//!
//! Every entry point drives a device and returns `!`, so the example on [`run`] is `no_run`; the
//! checkable parts of this subsystem are `direct_memory_access_validator` and
//! `filesystem_contract`. See notes/virtio.md ("The driver crate") and notes/dma.md.

//!
//! Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-24
//! (UTC), option D, on milestone 784 (reuse what the tree wrote)'s items (c1) to (c4).
//! `virtio-drivers` assumes a driver lays out its own virtqueues, and nife's confinement does not
//! allow that: the kernel builds every ring itself, eight entries at fixed offsets, Kani-proved in
//! `direct_memory_access_validator`, so no driver can aim device DMA. See
//! design/roadmap/0784-reuse-what-the-tree-wrote/the-virtio-drivers-stay-ours.md.
//!
//! Name: ratified 2026-09-14 (calef, working the unratified worklist), as a proper name rather than
//! on the acronym test: `virtio` is the specification's own name (virtio 1.x, OASIS), the string in
//! every QEMU flag and device tree node, so expanding it would rename a standard. That is the `elf`
//! and `pci` ground. It names the whole device family (milestone 63 (directory and package names)
//! refused `virtio_net` for the transport adapter because this crate also drives net).

use filesystem_contract::blk;
use user_mode_runtime::virtio::{
    notify, read_register, setup_queue, virtio_ring_barrier, write_register,
};
use user_mode_runtime::{
    exit, interrupt_acknowledge, interrupt_wait, receive_request, reply, send,
};

// The kernel maps the DMA page at this fixed VA (must match kernel/src/user/virtio_service.rs).
// The device REGISTERS are NOT mapped: we drive the device through a `Virtio` capability (slot 2),
// so we cannot point it outside this DMA region. See kernel/src/virtio.rs.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// Capability slots the kernel handed us, by convention.
const REPORT: u64 = 0; // SEND: report the result back to the kernel
const IRQ: u64 = 1; // WAIT/ACK the device interrupt
const VIRTIO: u64 = 2; // the device transport: read/write registers, set up the queue, submit

// --- virtio-mmio v2 register offsets (bytes from the slot base) ---
const MAGIC: u64 = 0x000;
const DEVICE_FEATURES: u64 = 0x010;
const DEVICE_FEATURES_SEL: u64 = 0x014;
const DRIVER_FEATURES: u64 = 0x020;
const DRIVER_FEATURES_SEL: u64 = 0x024;
const INTERRUPT_STATUS: u64 = 0x060;
const INTERRUPT_ACK: u64 = 0x064;
const STATUS: u64 = 0x070;

// Status bits.
const S_ACKNOWLEDGE: u32 = 1;
const S_DRIVER: u32 = 2;
const S_DRIVER_OK: u32 = 4;
const S_FEATURES_OK: u32 = 8;

// Feature bit 32: VIRTIO_F_VERSION_1. Mandatory for a modern device.
const F_VERSION_1_HI: u32 = 1; // bit 32 lives in the high 32-bit word

// Feature bit 33: VIRTIO_F_ACCESS_PLATFORM. A device behind an IOMMU offers it and refuses to run
// unless it is negotiated; acked only when offered, since acking an unoffered feature fails
// negotiation. The DMA domain is an identity map, so it changes no address (notes/virtio.md).
const F_ACCESS_PLATFORM_HI: u32 = 1 << 1; // bit 33 lives in the high 32-bit word

// The virtqueue we build. Small: one request in flight is all a demo needs.
const QSIZE: usize = 8;

// Descriptor flags.
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2; // device writes (i.e. our read buffer)
const VIRTQ_DESC_F_INDIRECT: u16 = 4; // points at a table of further descriptors (attack test)

// Feature bit 28 (low word): VIRTIO_RING_F_INDIRECT_DESC. The indirect attacker asks for it; the
// kernel strips it during negotiation so the device never honours the flag above.
const F_INDIRECT_DESC_LO: u32 = 1 << 28;

// blk request types.
const VIRTIO_BLK_T_IN: u32 = 0; // read: the device WRITES the data buffer
const VIRTIO_BLK_T_OUT: u32 = 1; // write: the device READS the data buffer
// Flush: no data buffer at all, just the header and the status byte. Legal only once
// `VIRTIO_BLK_F_FLUSH` is negotiated, which is why the block server probes for it rather than
// assuming it (virtio 1.2 §5.2.6; see `init_with_features` and `blk_flush`).
const VIRTIO_BLK_T_FLUSH: u32 = 4;

// Feature bit 9 (low word): VIRTIO_BLK_F_FLUSH, "the device supports a cache flush command". The
// durability claim rests on it, so it is read from the device, never assumed. The kernel's
// `sanitize_driver_features` must leave it alone;
// `feature_negotiation_leaves_the_blk_flush_bit_alone` in `kernel/src/virtio.rs` holds that. See
// notes/virtio.md ("Feature negotiation").
const F_FLUSH_LO: u32 = 1 << 9;

// --- our DMA layout, offsets within the one DMA page ---
const OFF_DESC: u64 = 0x000; // 16 * QSIZE = 128 bytes
const OFF_AVAIL: u64 = 0x080; // 6 + 2*QSIZE
const OFF_USED: u64 = 0x100; // 6 + 8*QSIZE
const OFF_HEADER: u64 = 0x200; // 16-byte blk request header
const OFF_DATA: u64 = 0x400; // 512-byte block buffer
const OFF_STATUS: u64 = 0x600; // 1-byte status

const BLOCK: usize = 512;

/// Read a device register, through the kernel. Reads are DMA-safe, so any register is allowed.
fn mr(off: u64) -> u32 {
    read_register(VIRTIO, off) as u32
}
/// Write a DMA-*safe* device register (status, features, interrupt-ack) through the kernel. The
/// queue-address and notify registers are NOT writable this way: the kernel owns them.
fn mw(off: u64, v: u32) {
    write_register(VIRTIO, off, v as u64);
}

fn direct_memory_access_write<T>(off: u64, val: T) {
    // SAFETY: off is within the DMA page and T fits; the page is mapped read/write.
    unsafe { core::ptr::write_volatile((DMA_VA + off) as *mut T, val) };
}
fn direct_memory_access_read<T: Copy>(off: u64) -> T {
    // SAFETY: as above.
    unsafe { core::ptr::read_volatile((DMA_VA + off) as *const T) }
}

/// The virtio handshake and queue setup, shared by the real driver and the attack test. The queue
/// is set up through the kernel, which places the rings at fixed offsets in our DMA region and
/// programs the device with those addresses: we never choose them.
fn init() {
    init_with_features(0, false);
}

/// The handshake, with `driver_features_lo` OR'd into the low feature word. The honest driver
/// passes 0; the indirect attacker passes `F_INDIRECT_DESC_LO`, unmasked against the offer, so the
/// kernel is what strips it.
///
/// `want_flush` asks for `VIRTIO_BLK_F_FLUSH` only if the device offered it. Returns whether that
/// bit was negotiated. See notes/virtio.md ("Feature negotiation").
fn init_with_features(driver_features_lo: u32, want_flush: bool) -> bool {
    assert!(mr(MAGIC) == 0x7472_6976); // "virt": we really are talking to a virtio device

    mw(STATUS, 0);
    mw(STATUS, S_ACKNOWLEDGE);
    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

    // The kernel filters the low word, so a ring-layout feature it cannot validate does not
    // survive.
    mw(DEVICE_FEATURES_SEL, 0);
    let dev_lo = mr(DEVICE_FEATURES);
    let flush = want_flush && (dev_lo & F_FLUSH_LO) != 0;
    mw(DRIVER_FEATURES_SEL, 0);
    mw(
        DRIVER_FEATURES,
        driver_features_lo | if flush { F_FLUSH_LO } else { 0 },
    );

    // The high word: VERSION_1 always, ACCESS_PLATFORM only if offered, so the ack stays a subset
    // of the offer.
    mw(DEVICE_FEATURES_SEL, 1);
    let dev_hi = mr(DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI;
    }
    mw(DRIVER_FEATURES_SEL, 1);
    mw(DRIVER_FEATURES, ack_hi);

    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
    assert!(mr(STATUS) & S_FEATURES_OK != 0);

    // The layout constants (OFF_DESC/AVAIL/USED) are the contract the kernel uses.
    assert!(setup_queue(VIRTIO, QSIZE as u64, 0) == 0);

    mw(
        STATUS,
        S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
    );
    // `DriverFeatures` is write-only; FEATURES_OK sticking is what says the device accepted this
    // set.
    flush
}

/// The one-shot read driver: reads the superblock and the `motd` file off the disk, reports the
/// file's first eight bytes to the kernel to check, and exits.
///
/// # Examples
///
/// A driver's `_start`. No syscall translates an address, so the DMA page's physical address
/// arrives as an argument.
///
/// ```no_run
/// # use user_mode_runtime::exit;
/// fn start(direct_memory_access_phys: u64) -> ! {
///     virtio::run(direct_memory_access_phys)
/// }
/// ```
pub fn run(direct_memory_access_phys: u64) -> ! {
    init();

    // Read block 0: the nifefs superblock.
    read_block(direct_memory_access_phys, 0);

    // It must be a nifefs image.
    let mut magic = [0u8; 8];
    for (i, b) in magic.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }
    assert!(magic == nifefs::MAGIC);

    // Walk the directory, still in the block-0 buffer, to find "motd", then read its first data
    // block.
    let start_block = find_file(b"motd").unwrap_or_else(|| report_code(0xE4));
    read_block(direct_memory_access_phys, start_block as u64);

    // Report the file's first 8 bytes. The kernel checks them against the known contents, which
    // proves the actual file data came off the disk and across the EL0 boundary.
    let mut head = [0u8; 8];
    for (i, b) in head.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }
    send(REPORT, u64::from_le_bytes(head), 0, 0);

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (notes/virtio.md).
    exit();
}

/// The byte the write test puts at offset `i` of the scratch block. The first eight bytes are a
/// literal the kernel test asserts against; the rest is a cheap position-dependent function, so a
/// buffer that came back shifted, truncated, or stale cannot match it.
fn pattern_byte(i: usize) -> u8 {
    const HEAD: &[u8; 8] = b"CRKWRIT1";
    if i < 8 {
        HEAD[i]
    } else {
        (i as u8).wrapping_mul(31) ^ 0x5A
    }
}

/// **The write path, end to end.** Find the `scratch` file in the directory (the one block on the
/// disk the write tests own), write a pattern into its block, wipe the buffer, read the block
/// back, and verify every byte made the round trip through the device. Then re-read block 0 and
/// look motd up again, so the report also certifies the write did not eat the filesystem around
/// it. Reports the first 8 bytes of the read-back pattern.
pub fn run_write(direct_memory_access_phys: u64) -> ! {
    init();

    // The directory tells us where scratch lives; nothing about the sector is hardcoded.
    read_block(direct_memory_access_phys, 0);
    let mut magic = [0u8; 8];
    for (i, b) in magic.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }
    assert!(magic == nifefs::MAGIC);
    let scratch = find_file(b"scratch").unwrap_or_else(|| report_code(0xE6)) as u64;

    // Write the pattern...
    for i in 0..BLOCK {
        direct_memory_access_write::<u8>(OFF_DATA + i as u64, pattern_byte(i));
    }
    write_block(direct_memory_access_phys, scratch);

    // ...wipe the buffer so a read that silently does nothing cannot pass, and read it back.
    for i in 0..BLOCK {
        direct_memory_access_write::<u8>(OFF_DATA + i as u64, 0);
    }
    read_block(direct_memory_access_phys, scratch);
    for i in 0..BLOCK {
        assert!(direct_memory_access_read::<u8>(OFF_DATA + i as u64) == pattern_byte(i));
    }
    let mut head = [0u8; 8];
    for (i, b) in head.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }

    // The write must have landed on ITS block and nothing else: the superblock still parses and
    // the read-path file is still in the directory.
    read_block(direct_memory_access_phys, 0);
    for (i, b) in magic.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }
    assert!(magic == nifefs::MAGIC);
    assert!(find_file(b"motd").is_some());

    send(REPORT, u64::from_le_bytes(head), 0, 0);

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (notes/virtio.md).
    exit();
}

/// **The abandoned write**, the kill-mid-write case: submit a write through the validated path,
/// report 1, then die (`brk`/`ebreak`) without waiting for the completion, acknowledging the
/// interrupt or touching the used ring. The test then runs the full writer against the same device,
/// which must succeed. See notes/dma.md.
pub fn run_write_abandon(direct_memory_access_phys: u64) -> ! {
    init();

    read_block(direct_memory_access_phys, 0);
    let mut magic = [0u8; 8];
    for (i, b) in magic.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(OFF_DATA + i as u64);
    }
    assert!(magic == nifefs::MAGIC);
    let scratch = find_file(b"scratch").unwrap_or_else(|| report_code(0xE6)) as u64;

    for i in 0..BLOCK {
        direct_memory_access_write::<u8>(OFF_DATA + i as u64, 0xAB);
    }
    let _used_before = submit_block(direct_memory_access_phys, scratch, VIRTIO_BLK_T_OUT);

    send(REPORT, 1, 0, 0); // submitted; the kernel validated it and rang the device
    panic!(); // and now we are gone, mid-operation
}

/// **The attack, refused.** A malicious driver that points a descriptor at kernel memory and asks
/// the device to write there. The kernel validates the descriptor on submit and refuses, so the
/// device is never told to go and never touches the address. Reports 1 if the kernel refused
/// (correct), 0 if the notify went through (a hole). Used by the security test.
pub fn run_attack(direct_memory_access_phys: u64) -> ! {
    init();

    // The forbidden target: the kernel's own image, which the device would happily DMA over if it
    // were ever handed this address.
    const KERNEL_ADDR: u64 = 0xffff_0000_4008_0000;

    // A single descriptor pointing OUTSIDE our DMA region, marked device-writable.
    write_desc(0, KERNEL_ADDR, 512, VIRTQ_DESC_F_WRITE, 0);

    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier();

    // Submit. The kernel walks the descriptor, sees KERNEL_ADDR is outside our region, and refuses.
    let r = notify(VIRTIO, 0);
    send(REPORT, if r < 0 { 1 } else { 0 }, 0, 0); // 1 = refused (good), 0 = it went through (bad)

    let _ = direct_memory_access_phys;
    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (notes/virtio.md).
    exit();
}

/// **The indirect-descriptor attack, refused.** Negotiates `INDIRECT_DESC` and submits one
/// descriptor flagged `INDIRECT` whose inner table, in our own region, points the device at the
/// kernel; a validator that walked only the flat chain would let the device follow it out. The
/// kernel strips the feature and refuses the flag on submit. Reports 1 if refused (correct), 0 if
/// the notify went through (a hole).
pub fn run_attack_indirect(direct_memory_access_phys: u64) -> ! {
    // Ask for indirect descriptors. The kernel strips the bit, but we build the attack anyway to
    // prove the validator refuses the flag even if the feature ever slipped through.
    init_with_features(F_INDIRECT_DESC_LO, false);

    const KERNEL_ADDR: u64 = 0xffff_0000_4008_0000;

    // The indirect TABLE lives in our region (the header scratch area). Its one entry aims the
    // device at kernel memory, device-writable.
    direct_memory_access_write::<u64>(OFF_HEADER, KERNEL_ADDR); // inner desc addr
    direct_memory_access_write::<u32>(OFF_HEADER + 8, 512); // inner desc len
    direct_memory_access_write::<u16>(OFF_HEADER + 12, VIRTQ_DESC_F_WRITE); // inner desc flags
    direct_memory_access_write::<u16>(OFF_HEADER + 14, 0); // inner desc next

    // desc[0]: flagged INDIRECT, pointing at the in-region table above. Wholly in-region, so only
    // the INDIRECT handling stands between this and a kernel-memory DMA.
    write_desc(
        0,
        direct_memory_access_phys + OFF_HEADER,
        16,
        VIRTQ_DESC_F_INDIRECT,
        0,
    );

    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier();

    // The kernel refuses the indirect descriptor and never rings the device.
    let r = notify(VIRTIO, 0);
    send(REPORT, if r < 0 { 1 } else { 0 }, 0, 0); // 1 = refused (good), 0 = it went through (bad)

    let _ = direct_memory_access_phys;
    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (notes/virtio.md).
    exit();
}

/// Find a file in the nifefs directory sitting in the block-0 buffer, returning its start
/// block. The format is `nifefs`'s and the offsets come from that crate rather than being
/// restated here: header (magic 8, count u32), then entries of
/// { name\[`NAME_LEN`\], `start_block` u32, len u32 }.
///
/// Only the `nifefs::ENTRIES_IN_FIRST_BLOCK` entries inside block 0 are visible; reading further
/// would read past the DMA data buffer. The disk this walks holds three files.
fn find_file(name: &[u8]) -> Option<u32> {
    let count = direct_memory_access_read::<u32>(OFF_DATA + 8) as usize;
    for i in 0..count.min(nifefs::ENTRIES_IN_FIRST_BLOCK) as u64 {
        let entry = OFF_DATA + nifefs::HEADER_LEN as u64 + i * nifefs::ENTRY_LEN as u64;
        let mut matches = true;
        for (j, &want) in name.iter().enumerate() {
            if direct_memory_access_read::<u8>(entry + j as u64) != want {
                matches = false;
                break;
            }
        }
        // The name must end here, so "motd" does not match "motdx": either the next byte is NUL
        // padding, or the name used every byte of the field and there is no padding to check.
        let ends = name.len() == nifefs::NAME_LEN
            || direct_memory_access_read::<u8>(entry + name.len() as u64) == 0;
        if matches && ends {
            return Some(direct_memory_access_read::<u32>(
                entry + nifefs::NAME_LEN as u64,
            ));
        }
    }
    None
}

/// Build and publish the three-descriptor chain for one block transfer, and ring the device
/// through the kernel. Returns the used-ring index from before the submit, which
/// [`complete_block`] compares against. A read (`T_IN`) marks the data descriptor device-writable
/// and a write (`T_OUT`) does not; the kernel checks every address on NOTIFY, whatever the flag.
fn submit_block(direct_memory_access_phys: u64, sector: u64, req_type: u32) -> u16 {
    // The request header the device reads: type, reserved, sector.
    direct_memory_access_write::<u32>(OFF_HEADER, req_type);
    direct_memory_access_write::<u32>(OFF_HEADER + 4, 0);
    direct_memory_access_write::<u64>(OFF_HEADER + 8, sector);
    direct_memory_access_write::<u8>(OFF_STATUS, 0xff); // the device overwrites this with 0 on success

    let data_flags = if req_type == VIRTIO_BLK_T_IN {
        VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE // read: the device fills our buffer
    } else {
        VIRTQ_DESC_F_NEXT // write: the device consumes our buffer
    };

    // desc[0]: header, device reads it.          NEXT -> 1
    write_desc(
        0,
        direct_memory_access_phys + OFF_HEADER,
        16,
        VIRTQ_DESC_F_NEXT,
        1,
    );
    // desc[1]: data buffer, direction above.     NEXT -> 2
    write_desc(
        1,
        direct_memory_access_phys + OFF_DATA,
        BLOCK as u32,
        data_flags,
        2,
    );
    // desc[2]: status byte, device WRITES it.  (end of chain)
    write_desc(
        2,
        direct_memory_access_phys + OFF_STATUS,
        1,
        VIRTQ_DESC_F_WRITE,
        0,
    );

    // Publish the head descriptor (index 0) in the available ring.
    // avail: { u16 flags; u16 idx; u16 ring[QSIZE]; }
    let used_before: u16 = direct_memory_access_read::<u16>(OFF_USED + 2);
    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0); // ring[idx] = head 0
    virtio_ring_barrier(); // the descriptor and ring entry must be visible before we bump idx
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier(); // idx must be visible before we notify

    // The kernel rings the device only if every published descriptor stays inside our DMA region.
    if notify(VIRTIO, 0) < 0 {
        report_code(0xE5); // the kernel refused our request (a descriptor escaped our region)
    }

    used_before
}

/// Wait for the completion of a submitted transfer and confirm the device reported success.
///
/// The completion is the used ring advancing, not the wakeup: the line is shared, and a wakeup can
/// be spurious, coalesced, or a dead driver's pending completion. So loop until `used.idx` moves.
/// See notes/virtio.md ("A completion is the used ring advancing") and notes/dma.md.
fn complete_block(used_before: u16) {
    loop {
        // The kernel masks the line and delivers the interrupt as a notification; its pending count
        // returns WAIT at once if the interrupt fired before this call.
        interrupt_wait(IRQ);

        // Quiet the device (read its interrupt-status, acknowledge it), then re-enable the line at
        // the controller, which the kernel masked when it fired.
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ);

        virtio_ring_barrier();
        if direct_memory_access_read::<u16>(OFF_USED + 2) != used_before {
            break; // our request is on the used ring; this wakeup was really ours
        }
        // The used ring has not moved, so that wakeup belonged to someone else (a stale completion
        // from a dead driver, most likely). Wait again for our own.
    }
    let st = direct_memory_access_read::<u8>(OFF_STATUS);
    if st != 0 {
        report_code(0xE200 | st as u64); // device reported a non-OK status
    }
}

/// Read one block from `sector` into the DMA data buffer, start to finish.
fn read_block(direct_memory_access_phys: u64, sector: u64) {
    let used_before = submit_block(direct_memory_access_phys, sector, VIRTIO_BLK_T_IN);
    complete_block(used_before);
}

/// Write the DMA data buffer to `sector`, start to finish (milestone 32 (a real filesystem:
/// RedoxFS behind a capability FS server) phase 1): the read's chain, data direction flipped.
fn write_block(direct_memory_access_phys: u64, sector: u64) {
    let used_before = submit_block(direct_memory_access_phys, sector, VIRTIO_BLK_T_OUT);
    complete_block(used_before);
}

/// Report a diagnostic code to the kernel and stop. Distinct from the magic, so the kernel's
/// "not the nifefs magic" branch prints it. Only reached on a bring-up failure.
fn report_code(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (notes/virtio.md).
    exit();
}

/// Write one 16-byte descriptor: { u64 addr; u32 len; u16 flags; u16 next; }.
fn write_desc(i: u64, addr: u64, len: u32, flags: u16, next: u16) {
    let base = OFF_DESC + i * 16;
    direct_memory_access_write::<u64>(base, addr);
    direct_memory_access_write::<u32>(base + 8, len);
    direct_memory_access_write::<u16>(base + 12, flags);
    direct_memory_access_write::<u16>(base + 14, next);
}

// =================================================================================================
// A virtio-net driver, at EL0, behind the multi-queue DMA confinement (milestone 30 (the network
// stack as a confined component)).
//
// Receive is queue 0 (the device writes a frame) and transmit is queue 1 (it reads one), both
// through the one `Virtio` capability. `run_net` proves both with a DHCP exchange against slirp.
// See notes/virtio.md ("The roles").
// =================================================================================================

/// The two virtqueues, matching the kernel's contract (receive = 0, transmit = 1).
const NET_RX_Q: u64 = 0;
const NET_TX_Q: u64 = 1;

/// The modern virtio-net header, prepended to every frame in both directions. 12 bytes because we
/// negotiate `VIRTIO_F_VERSION_1` (the `num_buffers` field is then always present).
const NET_HDR_LEN: u64 = 12;

// Our DMA-page layout for the net driver. The queue rings follow the kernel's per-queue stride
// (RING_BLOCK = 0x200, with desc/avail/used at +0x000/+0x080/+0x100 inside each queue's block, see
// kernel/src/virtio.rs), and the packet buffers live above both queues' blocks. Everything fits in
// the one 4 KiB DMA page the spawn service hands us.
const NET_RX_DESC: u64 = 0x000;
const NET_RX_AVAIL: u64 = 0x080;
const NET_RX_USED: u64 = 0x100;
const NET_TX_DESC: u64 = 0x200;
const NET_TX_AVAIL: u64 = 0x280;
// The transmit used ring lives at 0x300 (queue 1's block + USED_OFF), but the driver never reads it:
// it does not wait on transmit completions, only on the receive side, so there is no constant for it.
/// One receive buffer: the 12-byte virtio header plus room for a full 1514-byte Ethernet frame.
const NET_RX_BUF: u64 = 0x400;
const NET_RX_BUF_LEN: u64 = 0x600;
/// One transmit buffer: the header plus the DHCP frame we build.
const NET_TX_BUF: u64 = 0xA00;

/// Our source MAC. Locally administered; slirp does not care what it is, only that DHCP names it.
const NET_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
/// A recognizable DHCP transaction id, so the OFFER we accept is unambiguously the reply to our
/// DISCOVER and not some unrelated broadcast.
const NET_XID: u32 = 0x3903_F326;

fn wr_be16(off: u64, v: u16) {
    direct_memory_access_write::<u8>(off, (v >> 8) as u8);
    direct_memory_access_write::<u8>(off + 1, v as u8);
}
fn wr_be32(off: u64, v: u32) {
    direct_memory_access_write::<u8>(off, (v >> 24) as u8);
    direct_memory_access_write::<u8>(off + 1, (v >> 16) as u8);
    direct_memory_access_write::<u8>(off + 2, (v >> 8) as u8);
    direct_memory_access_write::<u8>(off + 3, v as u8);
}
fn rd_be16(off: u64) -> u16 {
    ((direct_memory_access_read::<u8>(off) as u16) << 8)
        | direct_memory_access_read::<u8>(off + 1) as u16
}
fn rd_be32(off: u64) -> u32 {
    ((direct_memory_access_read::<u8>(off) as u32) << 24)
        | ((direct_memory_access_read::<u8>(off + 1) as u32) << 16)
        | ((direct_memory_access_read::<u8>(off + 2) as u32) << 8)
        | (direct_memory_access_read::<u8>(off + 3) as u32)
}

/// The IPv4 header checksum over `len` bytes at `off`: sum the 16-bit words, fold the carries, and
/// complement. The checksum field itself must be zero when this runs.
fn ip_checksum(off: u64, len: u64) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < len {
        sum += rd_be16(off + i) as u32;
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// The virtio-net handshake and both-queue setup. Unlike the disk's `init`, this sets up receive AND
/// transmit, through the kernel, which places each queue's rings at its own block.
fn init_net() {
    assert!(mr(MAGIC) == 0x7472_6976); // "virt"

    mw(STATUS, 0);
    mw(STATUS, S_ACKNOWLEDGE);
    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

    // Negotiate no low-word net features: no checksum offload, no MRG_RXBUF, so one frame lands in
    // one buffer behind one 12-byte header, which is the simplest correct thing.
    mw(DRIVER_FEATURES_SEL, 0);
    mw(DRIVER_FEATURES, 0);

    // The high word: VERSION_1 always, ACCESS_PLATFORM only if offered (behind an IOMMU), the same
    // conditional ack the disk driver makes so one binary works on the bare NIC and the confined one.
    mw(DEVICE_FEATURES_SEL, 1);
    let dev_hi = mr(DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI;
    }
    mw(DRIVER_FEATURES_SEL, 1);
    mw(DRIVER_FEATURES, ack_hi);

    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
    assert!(mr(STATUS) & S_FEATURES_OK != 0);

    // Both queues, through the kernel: it programs each queue's ring addresses to that queue's block
    // in our DMA region, so we never choose them.
    assert!(setup_queue(VIRTIO, QSIZE as u64, NET_RX_Q) == 0);
    assert!(setup_queue(VIRTIO, QSIZE as u64, NET_TX_Q) == 0);

    mw(
        STATUS,
        S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
    );
}

/// Write one descriptor into an explicit descriptor table (the disk's `write_desc` is hardwired to
/// queue 0's; the NIC has two tables).
fn write_desc_at(desc_base: u64, i: u64, addr: u64, len: u32, flags: u16, next: u16) {
    let b = desc_base + i * 16;
    direct_memory_access_write::<u64>(b, addr);
    direct_memory_access_write::<u32>(b + 8, len);
    direct_memory_access_write::<u16>(b + 12, flags);
    direct_memory_access_write::<u16>(b + 14, next);
}

/// Post the single receive buffer as a device-writable descriptor and ring queue 0. Re-callable: it
/// re-uses descriptor head 0 with a fresh available-ring slot each time, so the device fills the
/// same buffer again. `direct_memory_access_phys` is the DMA page's physical base (descriptors
/// speak physical).
fn post_rx(direct_memory_access_phys: u64, avail_idx: u16) {
    write_desc_at(
        NET_RX_DESC,
        0,
        direct_memory_access_phys + NET_RX_BUF,
        NET_RX_BUF_LEN as u32,
        VIRTQ_DESC_F_WRITE, // the device WRITES the received frame here
        0,
    );
    direct_memory_access_write::<u16>(NET_RX_AVAIL + 4 + (avail_idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(NET_RX_AVAIL + 2, avail_idx.wrapping_add(1));
    virtio_ring_barrier();
    // The kernel validates the receive descriptor (device-writable, in our region).
    if notify(VIRTIO, NET_RX_Q) < 0 {
        report_code(0xE7);
    }
}

/// Transmit the frame already built at `NET_TX_BUF + NET_HDR_LEN`, of `frame_len` bytes: zero the
/// virtio header in front of it, post one device-readable descriptor, and ring queue 1.
fn send_frame(direct_memory_access_phys: u64, frame_len: u64, avail_idx: u16) {
    for i in 0..NET_HDR_LEN {
        direct_memory_access_write::<u8>(NET_TX_BUF + i, 0);
    }
    write_desc_at(
        NET_TX_DESC,
        0,
        direct_memory_access_phys + NET_TX_BUF,
        (NET_HDR_LEN + frame_len) as u32,
        0, // device READS the buffer (transmit)
        0,
    );
    direct_memory_access_write::<u16>(NET_TX_AVAIL + 4 + (avail_idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(NET_TX_AVAIL + 2, avail_idx.wrapping_add(1));
    virtio_ring_barrier();
    if notify(VIRTIO, NET_TX_Q) < 0 {
        report_code(0xE8);
    }
}

/// Build a DHCP DISCOVER (Ethernet + IPv4 + UDP + BOOTP/DHCP) at `NET_TX_BUF + NET_HDR_LEN`, and
/// return the Ethernet frame length. The broadcast flag is set so slirp's OFFER comes back to the
/// broadcast address, where our single RX buffer will catch it.
fn build_dhcp_discover() -> u64 {
    let f = NET_TX_BUF + NET_HDR_LEN;

    // --- Ethernet: to broadcast, from us, IPv4 ---
    for i in 0..6u64 {
        direct_memory_access_write::<u8>(f + i, 0xff);
        direct_memory_access_write::<u8>(f + 6 + i, NET_MAC[i as usize]);
    }
    wr_be16(f + 12, 0x0800);

    let ip = f + 14;
    let udp = ip + 20;
    let dhcp = udp + 8;

    // --- BOOTP/DHCP payload ---
    direct_memory_access_write::<u8>(dhcp, 1); // op = BOOTREQUEST
    direct_memory_access_write::<u8>(dhcp + 1, 1); // htype = Ethernet
    direct_memory_access_write::<u8>(dhcp + 2, 6); // hlen
    direct_memory_access_write::<u8>(dhcp + 3, 0); // hops
    wr_be32(dhcp + 4, NET_XID);
    wr_be16(dhcp + 8, 0); // secs
    wr_be16(dhcp + 10, 0x8000); // flags = broadcast
    for i in 12..28u64 {
        direct_memory_access_write::<u8>(dhcp + i, 0); // ciaddr/yiaddr/siaddr/giaddr
    }
    for i in 0..6u64 {
        direct_memory_access_write::<u8>(dhcp + 28 + i, NET_MAC[i as usize]); // chaddr
    }
    for i in 34..236u64 {
        direct_memory_access_write::<u8>(dhcp + i, 0); // rest of chaddr, sname, file
    }
    wr_be32(dhcp + 236, 0x6382_5363); // DHCP magic cookie

    // Options: message type DISCOVER, a parameter request list, end.
    let mut o = dhcp + 240;
    direct_memory_access_write::<u8>(o, 53); // option 53: DHCP message type
    direct_memory_access_write::<u8>(o + 1, 1);
    direct_memory_access_write::<u8>(o + 2, 1); // DISCOVER
    o += 3;
    direct_memory_access_write::<u8>(o, 55); // option 55: parameter request list
    direct_memory_access_write::<u8>(o + 1, 3);
    direct_memory_access_write::<u8>(o + 2, 1); // subnet mask
    direct_memory_access_write::<u8>(o + 3, 3); // router
    direct_memory_access_write::<u8>(o + 4, 6); // DNS
    o += 5;
    direct_memory_access_write::<u8>(o, 255); // end
    o += 1;

    let dhcp_len = o - dhcp;
    let udp_len = 8 + dhcp_len;
    let ip_total = 20 + udp_len;

    // --- UDP: bootpc(68) -> bootps(67), checksum 0 (optional for IPv4) ---
    wr_be16(udp, 68);
    wr_be16(udp + 2, 67);
    wr_be16(udp + 4, udp_len as u16);
    wr_be16(udp + 6, 0);

    // --- IPv4 header ---
    direct_memory_access_write::<u8>(ip, 0x45); // version 4, IHL 5
    direct_memory_access_write::<u8>(ip + 1, 0x00);
    wr_be16(ip + 2, ip_total as u16);
    wr_be16(ip + 4, 0); // identification
    wr_be16(ip + 6, 0); // flags/fragment
    direct_memory_access_write::<u8>(ip + 8, 64); // TTL
    direct_memory_access_write::<u8>(ip + 9, 17); // protocol = UDP
    wr_be16(ip + 10, 0); // checksum placeholder
    wr_be32(ip + 12, 0x0000_0000); // src 0.0.0.0
    wr_be32(ip + 16, 0xffff_ffff); // dst 255.255.255.255
    let csum = ip_checksum(ip, 20);
    wr_be16(ip + 10, csum);

    14 + ip_total
}

/// Wait for the receive queue's used ring to advance, discarding wakeups that were really the
/// transmit completion (the device's interrupt line is shared across both queues). Returns false if
/// no receive completion arrives within a bounded number of wakeups. Mirrors the disk's
/// `complete_block`: the used ring, not the wakeup, is the completion.
fn wait_rx(rx_used_before: u16) -> bool {
    for _ in 0..64 {
        // Blocks until the device raises its line.
        interrupt_wait(IRQ);
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        // Re-enable the line the kernel masked when it fired.
        interrupt_acknowledge(IRQ);
        virtio_ring_barrier();
        if direct_memory_access_read::<u16>(NET_RX_USED + 2) != rx_used_before {
            return true;
        }
    }
    false
}

/// Parse the frame in the receive buffer as a DHCP OFFER for our transaction. Returns the offered
/// address (`yiaddr`) on a match, `None` otherwise, so an unrelated broadcast is skipped rather than
/// mistaken for the reply.
fn parse_dhcp_offer() -> Option<u32> {
    let f = NET_RX_BUF + NET_HDR_LEN;
    if rd_be16(f + 12) != 0x0800 {
        return None; // not IPv4
    }
    let ip = f + 14;
    let ihl = (direct_memory_access_read::<u8>(ip) & 0x0f) as u64 * 4;
    if ihl < 20 || direct_memory_access_read::<u8>(ip + 9) != 17 {
        return None; // not a sane IPv4/UDP header
    }
    let udp = ip + ihl;
    if rd_be16(udp + 2) != 68 {
        return None; // not addressed to the DHCP client port
    }
    let dhcp = udp + 8;
    if direct_memory_access_read::<u8>(dhcp) != 2 || rd_be32(dhcp + 4) != NET_XID {
        return None; // not a BOOTREPLY for our transaction
    }
    if rd_be32(dhcp + 236) != 0x6382_5363 {
        return None; // no DHCP magic cookie
    }
    let yiaddr = rd_be32(dhcp + 16);

    // Scan the options for message type 53 == 2 (OFFER).
    let mut o = dhcp + 240;
    let end = NET_RX_BUF + NET_RX_BUF_LEN;
    let mut is_offer = false;
    while o + 1 < end {
        let tag = direct_memory_access_read::<u8>(o);
        if tag == 255 {
            break; // end
        }
        if tag == 0 {
            o += 1; // pad
            continue;
        }
        let len = direct_memory_access_read::<u8>(o + 1) as u64;
        if tag == 53 && len >= 1 && direct_memory_access_read::<u8>(o + 2) == 2 {
            is_offer = true;
        }
        o += 2 + len;
    }

    (is_offer && yiaddr != 0).then_some(yiaddr)
}

/// **The virtio-net headline: a DHCP round trip from an EL0 driver.** Bring the NIC up, post a
/// receive buffer, transmit a DHCP DISCOVER, and wait for slirp's OFFER. Reports the offered IPv4
/// address (`yiaddr`) as a little-endian word, which the kernel test checks lands in QEMU user-mode
/// networking's 10.0.2.0/24. TX and RX, both queues, both directions of the confinement, proven end
/// to end with no TCP/IP stack in the loop.
pub fn run_net(direct_memory_access_phys: u64) -> ! {
    init_net();

    let mut rx_avail: u16 = 0;
    post_rx(direct_memory_access_phys, rx_avail);
    rx_avail = rx_avail.wrapping_add(1);

    let frame_len = build_dhcp_discover();
    send_frame(direct_memory_access_phys, frame_len, 0);

    let mut rx_used: u16 = 0;
    for _ in 0..8 {
        if !wait_rx(rx_used) {
            report_code(0xE9); // no receive completion arrived
        }
        rx_used = direct_memory_access_read::<u16>(NET_RX_USED + 2);

        if let Some(yiaddr) = parse_dhcp_offer() {
            send(REPORT, yiaddr as u64, 0, 0);
            exit(); // one-shot: reported the DHCP offer, so exit and be reaped, do not spin
        }

        // Not our OFFER (some other broadcast): re-post the buffer and wait for the next frame.
        post_rx(direct_memory_access_phys, rx_avail);
        rx_avail = rx_avail.wrapping_add(1);
    }
    report_code(0xEA); // frames arrived, but none was our DHCP OFFER
}

// ---------------------------------------------------------------------------------------------
// The block server (milestone 32 phase 2): init the device once, then answer the FS server's
// read/write/size/flush requests forever. The FS server never touches the DMA region, and the
// device confinement is unchanged. See notes/virtio.md ("The block server") and notes/fs-server.md.
// ---------------------------------------------------------------------------------------------

/// The block server's request endpoint (slot 0): the FS server CALLs here; this role `RECEIVE_CAPs` the
/// request plus the one-shot Reply that names the caller. IRQ (1) and VIRTIO (2) are as every role.
const BLK_REQ: u64 = 0;

/// Where the kernel maps the data region shared with the FS server (the block buffer). Distinct
/// from the DMA region, which the FS server must never see, and from [`DMA_VA`].
///
/// The block server's DMA region is `1 + blk::TRANSFER_BLOCKS` contiguous pages
/// (`kernel/src/user/fs_service.rs`): page 0 holds the rings, request header and status, and pages
/// `1..=blk::TRANSFER_BLOCKS` are the data buffer, which is the region shared with the FS server.
/// `BLK_OFF_DATA` is that page-1 offset. See notes/virtio.md ("The block server").
const BLK_OFF_DATA: u64 = 0x1000;

/// The block server's readiness endpoint (slot 3): SEND once after the device is up, so the test
/// can tell a device-bring-up hang from a first-read hang. See `kernel/src/user/fs_service.rs`.
const BLK_READY: u64 = 3;

/// virtio-mmio device-config window; virtio-blk's capacity (u64, in 512-byte sectors) is at its
/// start. Reads are DMA-safe, so the kernel's `READ_REGISTER` permits this offset.
const CONFIG: u64 = 0x100;

/// **The block server.** Bring the device up, then serve blk IPC forever. Every request is a CALL
/// answered through the kernel-minted Reply, so this server can answer a client it was never wired
/// to, exactly once each. The transfer unit is one filesystem block; the bulk rides in the shared
/// page, the control (opcode, block index, result) rides in the message, the §10 split.
pub fn run_blk_server(direct_memory_access_phys: u64) -> ! {
    // Whether VIRTIO_BLK_F_FLUSH was negotiated decides `blk::FLUSH`: a device round trip, or a
    // loud refusal, never a yes that does nothing.
    let can_flush = init_with_features(0, true);
    // The device is up: signal readiness, so a hang here (bring-up) is distinct from a hang in the
    // first read completion. SEND rendezvous, so this also paces the test past bring-up.
    send(BLK_READY, filesystem_contract::READY, 0, 0);
    // How many device flushes have completed. Answered back on every successful `blk::FLUSH`, so a
    // client can tell a real round trip from a constant yes: see `filesystem_contract::blk::FLUSH`.
    let mut flushes: i64 = 0;
    loop {
        // RECEIVE_CAP: (first word, the Reply, second word = the starting block index). The Reply is
        // typed by the kernel's `x4` (milestone 706 (a `CALL` server can tell a Reply from a
        // delegation)): a SEND_CAP's capability is deleted, and a request with no Reply is served
        // with nobody to answer.
        let req = receive_request(BLK_REQ);
        let (w0, block) = (req.w0, req.w1);
        let to = req.delivered.into_reply();
        // Clamp, as `fs_service.rs` does for the file channel: a request is never trusted to stay
        // inside the shared region just because its packing could spell a larger count.
        let count = blk::req_blocks(w0).min(blk::TRANSFER_BLOCKS) as u64;
        let r0: i64 = match filesystem_contract::operation(w0) {
            blk::READ => {
                blk_read(direct_memory_access_phys, block, count);
                0
            }
            blk::WRITE => {
                blk_write(direct_memory_access_phys, block, count);
                0
            }
            blk::SIZE => disk_size_bytes(),
            // The one verb that can refuse for a reason outside this program. A device that never
            // offered the feature gets EOPNOTSUPP rather than a zero, because a zero here is a lie
            // about somebody's data; see `filesystem_contract::blk::FLUSH`.
            blk::FLUSH => {
                if !can_flush {
                    -95 // EOPNOTSUPP: this device has no flush, and pretending otherwise is the bug
                } else if blk_flush(direct_memory_access_phys) {
                    flushes += 1;
                    flushes
                } else {
                    -5 // EIO: the device took the request and completed it with a non-OK status
                }
            }
            _ => -22, // EINVAL: an opcode this server does not implement
        };
        // Answer through the one-shot Reply, which the kernel consumes.
        if let Some(to) = to {
            reply(to, r0 as u64, 0);
        }
    }
}

/// Complete a block-server transfer by waiting on the completion interrupt, not by polling the
/// used ring, with the same loop as [`complete_block`]. A WAIT per read fits the test watchdog,
/// since whole-block requests keep the mount's read count in the low hundreds. See notes/virtio.md
/// ("A completion is the used ring advancing").
fn complete_blk(used_before: u16) {
    // QEMU completes inside `NOTIFY`, so the interrupt is usually already pending and the kernel's
    // pending-signal count (milestone 9 (a virtio-blk driver at EL0, and an interrupt becomes a
    // message)) returns this WAIT at once.
    loop {
        interrupt_wait(IRQ);
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ);
        virtio_ring_barrier();
        if direct_memory_access_read::<u16>(OFF_USED + 2) != used_before {
            break; // our request is on the used ring; this wakeup was really ours
        }
    }
    let st = direct_memory_access_read::<u8>(OFF_STATUS);
    if st != 0 {
        panic!(); // the device reported a non-OK status
    }
}

/// Read `count` contiguous filesystem blocks starting at `block` in a single virtio request,
/// straight into the data pages, which are the FS server's shared region: no per-block loop and no
/// copy (milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 4),
/// since a device round trip costs far more than the transfer.
fn blk_read(direct_memory_access_phys: u64, block: u64, count: u64) {
    let used_before = submit_blk(
        direct_memory_access_phys,
        block * blk::SECTORS_PER_BLOCK,
        VIRTIO_BLK_T_IN,
        count,
    );
    complete_blk(used_before);
}

/// Write `count` contiguous filesystem blocks from the data pages (the FS server filled them before
/// this request) in a single virtio request. The one direction flag differs from the read; the
/// addresses are identical.
fn blk_write(direct_memory_access_phys: u64, block: u64, count: u64) {
    let used_before = submit_blk(
        direct_memory_access_phys,
        block * blk::SECTORS_PER_BLOCK,
        VIRTIO_BLK_T_OUT,
        count,
    );
    complete_blk(used_before);
}

/// **Ask the device to make everything it has acknowledged durable**, and wait for it to say it
/// did. `true` if the device completed with `VIRTIO_BLK_S_OK`.
///
/// A status of 0 says the device finished its flush; it cannot prove the hardware beneath is
/// honest. Unlike [`complete_blk`] this does not panic on a bad status such as
/// `VIRTIO_BLK_S_UNSUPP` (2): the long-lived block server's death would block every filesystem
/// client. See notes/virtio.md ("The block server").
fn blk_flush(direct_memory_access_phys: u64) -> bool {
    let used_before = submit_flush(direct_memory_access_phys);
    complete_flush(used_before)
}

/// A flush's **two**-descriptor chain: the 16-byte request header the device reads, and the status
/// byte it writes. No data descriptor, because a flush moves no data (virtio 1.2 §5.2.6.1); a chain
/// with one would be describing a transfer the device was never asked to make.
fn submit_flush(direct_memory_access_phys: u64) -> u16 {
    direct_memory_access_write::<u32>(OFF_HEADER, VIRTIO_BLK_T_FLUSH);
    direct_memory_access_write::<u32>(OFF_HEADER + 4, 0);
    // The sector field is unused for a flush and the spec says to set it to 0; written so no stale
    // sector from the previous request is left in the header.
    direct_memory_access_write::<u64>(OFF_HEADER + 8, 0);
    direct_memory_access_write::<u8>(OFF_STATUS, 0xff);

    write_desc(
        0,
        direct_memory_access_phys + OFF_HEADER,
        16,
        VIRTQ_DESC_F_NEXT,
        1,
    );
    write_desc(
        1,
        direct_memory_access_phys + OFF_STATUS,
        1,
        VIRTQ_DESC_F_WRITE,
        0,
    );

    let used_before: u16 = direct_memory_access_read::<u16>(OFF_USED + 2);
    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier();

    // As `submit_blk`: the kernel validates every descriptor against the region.
    if notify(VIRTIO, 0) < 0 {
        panic!();
    }
    used_before
}

/// [`complete_blk`]'s wait, returning the device's verdict instead of panicking on it. See
/// [`blk_flush`] for why the difference is deliberate.
fn complete_flush(used_before: u16) -> bool {
    loop {
        interrupt_wait(IRQ);
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ);
        virtio_ring_barrier();
        if direct_memory_access_read::<u16>(OFF_USED + 2) != used_before {
            break; // our request is on the used ring; this wakeup was really ours
        }
    }
    direct_memory_access_read::<u8>(OFF_STATUS) == 0
}

/// Build and publish the three-descriptor chain for a `count`-block transfer and ring the device
/// through the kernel. Mirrors [`submit_block`] but with one data descriptor spanning `count`
/// contiguous blocks at [`BLK_OFF_DATA`] (milestone 138 step 4): virtio-blk asks only that its
/// length be whole sectors. Returns the used-ring index from before the submit, which
/// [`complete_block`] compares against.
fn submit_blk(direct_memory_access_phys: u64, sector: u64, req_type: u32, count: u64) -> u16 {
    direct_memory_access_write::<u32>(OFF_HEADER, req_type);
    direct_memory_access_write::<u32>(OFF_HEADER + 4, 0);
    direct_memory_access_write::<u64>(OFF_HEADER + 8, sector);
    direct_memory_access_write::<u8>(OFF_STATUS, 0xff);

    let data_flags = if req_type == VIRTIO_BLK_T_IN {
        VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE // read: the device fills the data pages
    } else {
        VIRTQ_DESC_F_NEXT // write: the device consumes the data pages
    };
    let data_len = count * blk::BLOCK_SIZE as u64;
    write_desc(
        0,
        direct_memory_access_phys + OFF_HEADER,
        16,
        VIRTQ_DESC_F_NEXT,
        1,
    );
    write_desc(
        1,
        direct_memory_access_phys + BLK_OFF_DATA,
        data_len as u32,
        data_flags,
        2,
    );
    write_desc(
        2,
        direct_memory_access_phys + OFF_STATUS,
        1,
        VIRTQ_DESC_F_WRITE,
        0,
    );

    let used_before: u16 = direct_memory_access_read::<u16>(OFF_USED + 2);
    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0);
    virtio_ring_barrier();
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier();

    // The kernel validates every descriptor against the 2-page region before ringing; a refusal
    // here is a block-server bug, so fault rather than limp on.
    if notify(VIRTIO, 0) < 0 {
        panic!();
    }
    used_before
}

/// The disk's size in bytes, from virtio-blk's capacity register (sectors * 512). RedoxFS asks for
/// this once, at open, to size its allocator; the value is the whole extent the image lives in.
fn disk_size_bytes() -> i64 {
    let lo = mr(CONFIG) as u64;
    let hi = mr(CONFIG + 4) as u64;
    let sectors = lo | (hi << 32);
    (sectors * 512) as i64
}
