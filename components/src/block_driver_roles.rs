//! The roles of one virtio driver program, which runs at EL0 (exception level 0, user mode) and
//! drives one virtio-blk or virtio-net device. It is not a general-purpose library: each entry
//! point takes the DMA (direct memory access) page's physical address from a `_start`, drives the
//! device for one role, and returns `!`.
//!
//! - [`run`], [`run_write`] and [`run_write_abandon`] are one-shot block test roles: read a file,
//!   write and read back a block, and die mid-write.
//! - [`run_attack`] and [`run_attack_indirect`] are attackers: each aims a descriptor at kernel
//!   memory and reports whether the kernel refused it.
//! - [`run_net`] is a one-shot virtio-net role: a DHCP (Dynamic Host Configuration Protocol)
//!   DISCOVER out and an OFFER back.
//! - [`run_block_server`] is the long-lived block server that the filesystem server reads and writes
//!   disk blocks through.
//!
//! The one-shot roles report to the kernel's test on slot 0; the table beside `report_code` lists
//! the codes. On a mediated grant the kernel hands the process the DMA page, an `Interrupt`
//! capability (slot 1) and a `VirtualIo` transport capability (slot 2); the device registers are not
//! mapped, so every access goes through the kernel, which confines the device to the DMA page. On a
//! direct grant the driver drives a virtio-pci function itself and the IOMMU (Input-Output Memory
//! Management Unit) confines the device; [`direct`] answers the same register vocabulary, so every
//! role runs unchanged on either grant. Modern virtio-mmio (version 2), virtio 1.x sections 4.2
//! (MMIO), 5.1 (net) and 5.2 (block).
//!
//! Every entry point drives a device, so the example on [`run`] is `no_run`. What can be checked
//! without one is pure and host-tested here: the accessors' window check, the descriptor chain,
//! the completion wait's bound, the configuration read's generation bracket, the directory lookup,
//! the DHCP frames and the Internet Protocol header checksum. The ring layout's proof is in
//! `direct_memory_access_validator`. See nifeos/record:notes/virtual-io.md ("The driver crate") and nifeos/record:notes/dma.md.
//!
//! Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-24
//! (UTC), option D, on milestone 784 (reuse what the tree wrote)'s items (c1) to (c4).
//! `virtio-drivers` assumes a driver lays out its own virtqueues, and nife's confinement does not
//! allow that: the kernel builds every ring itself, eight entries at fixed offsets, Kani-proved in
//! `direct_memory_access_validator`, so no driver can aim device DMA. See
//! nifeos/record:design/roadmap/0784-reuse-what-the-tree-wrote/the-virtio-drivers-stay-ours.md.
//!
//! Name: provisional. The module holds the one program's roles, so it takes the program's name
//! (`block_driver`) and says what it holds. The name `virtio` stays reserved for a library cut later
//! from what the six copies of the virtio-mmio handshake share.

use file_system_contract::block;
use user_mode_runtime::virtual_io::{read_register, virtual_io_ring_barrier, write_register};
use user_mode_runtime::{
    exit, interrupt_acknowledge, interrupt_wait, receive_request, reply, send,
};

/// The virtio block wire's fixed sector, in bytes: the unit the request header's `sector` field
/// and the device's capacity count in, whatever the disk's own logical block size.
///
/// Name: provisional, minted 2026-11-22 (UTC) by the file-system-contract-rename lane (split from
/// the contract's `SECTOR_BYTES`, ruled 2026-11-22).
pub const SECTOR_BYTES: u64 = 512;

/// The number of virtio sectors in one filesystem block (`BLOCK_BYTES / SECTOR_BYTES = 8`).
///
/// Name: provisional, minted 2026-11-22 (UTC) by the file-system-contract-rename lane (ruled
/// 2026-11-22: it divides by the wire sector, so it keeps its name and lives beside it).
pub const SECTORS_PER_BLOCK: u64 = (block::BLOCK_BYTES as u64) / SECTOR_BYTES;

// The kernel maps the DMA page at this fixed VA (must match kernel/src/user/virtual_io_service.rs).
// The device REGISTERS are NOT mapped: we drive the device through a `VirtualIo` capability (slot 2),
// so we cannot point it outside this DMA region. See kernel/src/virtual_io.rs.
const DIRECT_MEMORY_ACCESS_BASE_VIRTUAL_ADDRESS: u64 =
    address_space_layout::pair_page(0x0000_0000_0090_0000);

/// Capability slots the kernel handed us, by convention.
const REPORT: u64 = 0; // SEND: report the result back to the kernel
#[doc(alias = "irq")]
const INTERRUPT: u64 = 1; // WAIT/ACKNOWLEDGE the device interrupt
const VIRTUAL_IO: u64 = 2; // the device transport: read/write registers, set up the queue, submit

// The virtio-mmio version 2 register offsets, in bytes from the slot base (virtio 1.2, the MMIO
// device register layout). Each name spells the specification's register out and keeps its
// spelling as an alias. Names: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review; the
// program publishes none of them.

/// (`MagicValue`) The magic value register, which reads `0x74726976` ("virt") on every virtio-mmio
/// device.
#[doc(alias = "MagicValue")]
const MAGIC_VALUE_REGISTER: u64 = 0x000;
/// (`DeviceFeatures`) The 32 feature bits the device offers, from the word `DeviceFeaturesSel`
/// selects.
#[doc(alias = "DeviceFeatures")]
const DEVICE_FEATURES_REGISTER: u64 = 0x010;
/// (`DeviceFeaturesSel`) Which 32-bit word of the device's features `DeviceFeatures` reads.
#[doc(alias = "DeviceFeaturesSel")]
const DEVICE_FEATURES_SELECT_REGISTER: u64 = 0x014;
/// (`DriverFeatures`) The 32 feature bits the driver accepts, into the word `DriverFeaturesSel`
/// selects.
#[doc(alias = "DriverFeatures")]
const DRIVER_FEATURES_REGISTER: u64 = 0x020;
/// (`DriverFeaturesSel`) Which 32-bit word of the driver's features `DriverFeatures` writes.
#[doc(alias = "DriverFeaturesSel")]
const DRIVER_FEATURES_SELECT_REGISTER: u64 = 0x024;
/// (`InterruptStatus`) Why the device raised its interrupt.
#[doc(alias = "InterruptStatus")]
const INTERRUPT_STATUS_REGISTER: u64 = 0x060;
/// (`InterruptACK`) The driver writes back the `InterruptStatus` bits it handled.
#[doc(alias = "InterruptACK")]
const INTERRUPT_ACKNOWLEDGE_REGISTER: u64 = 0x064;
/// (`Status`) The device status: the handshake's bits, and 0 to reset.
#[doc(alias = "Status")]
const STATUS_REGISTER: u64 = 0x070;
/// (`ConfigGeneration`, virtio 1.2 section 4.2.2) The configuration generation: the device changes
/// it whenever the device-specific configuration changes, so a multi-word configuration read is
/// consistent when it reads the same value before and after. Name: provisional.
///
/// BUGS: the kernel's mediated Peripheral Component Interconnect (PCI) transport answers 0 for
/// this offset (`kernel/src/virtual_io.rs`), so on that grant the bracket in [`capacity_sectors`]
/// always holds and checks nothing. The mmio transport and the direct grant ([`direct`]) read the
/// device's real generation.
#[doc(alias = "ConfigGeneration")]
const CONFIGURATION_GENERATION_REGISTER: u64 = 0x0fc;

// Device status bits (virtio 1.2 section 2.1: ACKNOWLEDGE, DRIVER, DRIVER_OK, FEATURES_OK).
#[doc(alias = "ACKNOWLEDGE")]
const DEVICE_STATUS_ACKNOWLEDGE: u32 = 1;
#[doc(alias = "DRIVER")]
const DEVICE_STATUS_DRIVER: u32 = 2;
#[doc(alias = "DRIVER_OK")]
const DEVICE_STATUS_DRIVER_OK: u32 = 4;
#[doc(alias = "FEATURES_OK")]
const DEVICE_STATUS_FEATURES_OK: u32 = 8;

/// (`VIRTIO_F_VERSION_1`) Feature bit 32, mandatory for a modern device, as a bit of the high
/// 32-bit feature word. Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
#[doc(alias = "VIRTIO_F_VERSION_1")]
const FEATURE_VERSION_1_HIGH_WORD: u32 = 1;

/// (`VIRTIO_F_ACCESS_PLATFORM`) Feature bit 33, as a bit of the high 32-bit feature word. A device
/// behind an IOMMU offers it and refuses to run unless it is negotiated; the driver acknowledges it
/// only when offered, since acknowledging an unoffered feature fails negotiation. The DMA domain is
/// an identity map, so it changes no address (nifeos/record:notes/virtual-io.md). Name: provisional, minted
/// 2026-11-22 (UTC) by lane 1113-finish-review.
#[doc(alias = "VIRTIO_F_ACCESS_PLATFORM")]
const FEATURE_ACCESS_PLATFORM_HIGH_WORD: u32 = 1 << 1;

// The virtqueue we build: the size the kernel drives. Small: one request in flight is all a demo
// needs.
#[doc(alias = "Queue Size")]
const QUEUE_ENTRIES: usize = moho_abi::virtual_io::QUEUE_ENTRIES as usize;

// The descriptor flags (virtio 1.2, the split virtqueue's descriptor table). Names: provisional,
// minted 2026-11-22 (UTC) by lane 1113-finish-review.

/// (`VIRTQ_DESC_F_NEXT`) The descriptor continues the chain through its `next` field.
#[doc(alias = "VIRTQ_DESC_F_NEXT")]
const DESCRIPTOR_FLAG_NEXT: u16 = 1;
/// (`VIRTQ_DESC_F_WRITE`) The device writes the buffer: a read's data, a status byte, a receive
/// buffer.
#[doc(alias = "VIRTQ_DESC_F_WRITE")]
const DESCRIPTOR_FLAG_DEVICE_WRITES: u16 = 2;
/// (`VIRTQ_DESC_F_INDIRECT`) The buffer holds a table of further descriptors. Only the indirect
/// attacker sets it.
#[doc(alias = "VIRTQ_DESC_F_INDIRECT")]
const DESCRIPTOR_FLAG_INDIRECT: u16 = 4;

/// (`VIRTIO_RING_F_INDIRECT_DESC`) Feature bit 28, in the low feature word. The indirect attacker
/// asks for it; the kernel strips it during negotiation so the device never honors the flag above.
/// Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
#[doc(alias = "VIRTIO_RING_F_INDIRECT_DESC")]
const FEATURE_INDIRECT_DESCRIPTORS_LOW_WORD: u32 = 1 << 28;

// The block request types (virtio 1.2, the block device's request). Names: provisional, minted
// 2026-11-22 (UTC) by lane 1113-finish-review.

/// (`VIRTIO_BLK_T_IN`) A read: the device writes the data buffer.
#[doc(alias = "VIRTIO_BLK_T_IN")]
const BLOCK_REQUEST_READ: u32 = 0;
/// (`VIRTIO_BLK_T_OUT`) A write: the device reads the data buffer.
#[doc(alias = "VIRTIO_BLK_T_OUT")]
const BLOCK_REQUEST_WRITE: u32 = 1;
/// (`VIRTIO_BLK_T_FLUSH`) A flush: no data buffer at all, just the header and the status byte.
/// Legal only once `VIRTIO_BLK_F_FLUSH` is negotiated, which is why the block server probes for it
/// rather than assuming it (virtio 1.2 section 5.2.6; see `initialize_with_features` and
/// `block_flush`).
#[doc(alias = "VIRTIO_BLK_T_FLUSH")]
const BLOCK_REQUEST_FLUSH: u32 = 4;

/// (`VIRTIO_BLK_F_FLUSH`) Feature bit 9, in the low feature word: "the device supports a cache
/// flush command". The durability claim rests on it, so the driver reads it from the device and
/// never assumes it. The kernel's `sanitize_driver_features` must leave it alone;
/// `feature_negotiation_leaves_the_blk_flush_bit_alone` in `kernel/src/virtual_io.rs` holds that. See
/// nifeos/record:notes/virtual-io.md ("Feature negotiation"). Name: provisional, minted 2026-11-22 (UTC) by lane
/// 1113-finish-review.
#[doc(alias = "VIRTIO_BLK_F_FLUSH")]
const FEATURE_BLOCK_FLUSH_LOW_WORD: u32 = 1 << 9;

// --- our DMA layout, offsets within the one DMA page ---
// The ring offsets are the layout the kernel builds and `direct_memory_access_validator` proves,
// named once in `moho_abi::virtual_io`, so a change to it is a compile error here and not a wrong
// offset a QEMU test finds.
use moho_abi::virtual_io::{
    AVAILABLE_RING_OFFSET_BYTES, DESCRIPTOR_TABLE_OFFSET_BYTES, RING_BLOCK_BYTES,
    USED_RING_OFFSET_BYTES,
};
const REQUEST_HEADER_OFFSET_BYTES: u64 = 0x200; // 16-byte blk request header
const DATA_OFFSET_BYTES: u64 = 0x400; // 512-byte block buffer
const STATUS_OFFSET_BYTES: u64 = 0x600; // 1-byte status

/// The length of a test role's data buffer at [`DATA_OFFSET_BYTES`]: one 512-byte block.
///
/// Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
const DATA_BUFFER_BYTES: usize = 512;

/// Read a device register: through the kernel on a mediated grant, where reads are DMA-safe so
/// any register is allowed, or from the mapped structures on a direct one ([`direct::read`]).
fn read_device_register(offset: u64) -> u32 {
    if direct::is_direct() {
        return direct::read(offset);
    }
    read_register(VIRTUAL_IO, offset) as u32
}
/// Write a DMA-*safe* device register (status, features, interrupt-ack): through the kernel on a
/// mediated grant, which owns the queue-address and notify registers, or directly on a direct one.
fn write_device_register(offset: u64, value: u32) {
    if direct::is_direct() {
        return direct::write(offset, value);
    }
    write_register(VIRTUAL_IO, offset, value as u64);
}

/// Set up `queue` with `entries` entries at the fixed layout's offsets in the DMA region: the
/// kernel places the rings on a mediated grant, this driver on a direct one
/// ([`direct::setup_queue`]). 0 on success.
fn setup_queue(slot: u64, entries: u64, queue: u64) -> i64 {
    if direct::is_direct() {
        return direct::setup_queue(entries, queue);
    }
    user_mode_runtime::virtual_io::setup_queue(slot, entries, queue)
}

/// Ring `queue`: through the kernel on a mediated grant, which validates every new descriptor
/// first and answers negative for one that escapes the region; one doorbell store on a direct
/// grant, where the IOMMU (Input-Output Memory Management Unit) is the confinement
/// ([`direct::notify`]).
fn notify(slot: u64, queue: u64) -> i64 {
    if direct::is_direct() {
        return direct::notify(queue);
    }
    user_mode_runtime::virtual_io::notify(slot, queue)
}

/// The DMA region's virtual base: the fixed page a mediated grant maps, or the window a direct
/// grant's startup page lists.
fn region() -> u64 {
    direct::region().unwrap_or(DIRECT_MEMORY_ACCESS_BASE_VIRTUAL_ADDRESS)
}

/// How many bytes from [`region()`] the two accessors below may reach: the first page of the DMA
/// region. Every role is handed at least that page, and every ring, header, status byte and packet
/// buffer this crate touches through them lies inside it. The block server's data pages above it
/// are the device's and the FS server's, and this program never reads or writes them.
const DIRECT_MEMORY_ACCESS_WINDOW_BYTES: u64 = 0x1000;

// The window starts on a page boundary, so an offset aligned for `T` is an address aligned for `T`:
// the mediated base by this assertion, the direct one by the check in [`direct::take`].
const _: () = assert!(
    DIRECT_MEMORY_ACCESS_BASE_VIRTUAL_ADDRESS.is_multiple_of(DIRECT_MEMORY_ACCESS_WINDOW_BYTES)
);

/// Whether an access of `size` bytes at `offset`, for a type aligned to `align`, lies wholly inside a
/// window of `window` bytes and is aligned. The check the two accessors below make before every
/// access, kept apart so it can be tested on the host.
const fn window_access_fits(offset: u64, size: usize, align: usize, window: u64) -> bool {
    match offset.checked_add(size as u64) {
        Some(end) => end <= window && offset.is_multiple_of(align as u64),
        None => false,
    }
}

/// Write `value` at byte `offset` of the DMA page. Panics, rather than write, when the access would
/// leave the page or is misaligned for `T`.
fn direct_memory_access_write<T: Copy>(offset: u64, value: T) {
    assert!(
        window_access_fits(
            offset,
            size_of::<T>(),
            align_of::<T>(),
            DIRECT_MEMORY_ACCESS_WINDOW_BYTES
        ),
        "a DMA page write outside the page or misaligned"
    );
    // SAFETY: the assertion above holds the access inside the first page of the DMA region, which
    // is mapped read/write at region() for every role (at DIRECT_MEMORY_ACCESS_BASE_VIRTUAL_ADDRESS on a mediated grant, at the
    // startup page's window on a direct one), and aligned for T, since region() is page aligned.
    unsafe { core::ptr::write_volatile((region() + offset) as *mut T, value) };
}

/// Read a `T` at byte `offset` of the DMA page. Panics, rather than read, when the access would leave
/// the page or is misaligned for `T`.
fn direct_memory_access_read<T: Copy>(offset: u64) -> T {
    assert!(
        window_access_fits(
            offset,
            size_of::<T>(),
            align_of::<T>(),
            DIRECT_MEMORY_ACCESS_WINDOW_BYTES
        ),
        "a DMA page read outside the page or misaligned"
    );
    // SAFETY: as in direct_memory_access_write: in bounds and aligned, checked above.
    unsafe { core::ptr::read_volatile((region() + offset) as *const T) }
}

/// How many times the driver reads `Status` after a reset before it gives up. The specification
/// has the driver wait for the device to answer 0, with no bound of its own; a device that never
/// does is not one this driver can initialize, and a bounded wait turns that into a stop.
const RESET_POLLS: u32 = 1_000_000;

/// Write 0 to the device's status, which resets it, and read `Status` back until it is 0, giving up
/// after `RESET_POLLS` reads. Returns whether the device reported the reset complete. `write` and
/// `read` are the status register's two accesses, so the loop can be tested without a device.
///
/// The specification has the driver read `Status` back as 0 before it reinitializes (virtio 1.2,
/// device initialization, reset step): a device may take time to reset, and a driver that writes
/// `ACKNOWLEDGE` first races the reset, which can then clear it.
fn reset_until_zero(mut write: impl FnMut(u32), mut read: impl FnMut() -> u32) -> bool {
    write(0);
    (0..RESET_POLLS).any(|_| read() == 0)
}

/// Reset the device and wait for it to say so. Stops the driver when it never does, as the other
/// handshake checks do.
fn reset_device() {
    assert!(
        reset_until_zero(
            |value| write_device_register(STATUS_REGISTER, value),
            || read_device_register(STATUS_REGISTER)
        ),
        "the device never read back Status 0 after a reset"
    );
}

/// The high feature word the driver acknowledges, given the word the device offered: `VERSION_1`
/// and, only if offered, `ACCESS_PLATFORM`. `None` when the device does not offer `VERSION_1`: the
/// specification forbids a driver to accept a feature the device did not offer, and a device
/// without it is not a modern device this driver can drive.
fn acknowledged_high_features(device_high: u32) -> Option<u32> {
    if device_high & FEATURE_VERSION_1_HIGH_WORD == 0 {
        return None;
    }
    Some(FEATURE_VERSION_1_HIGH_WORD | (device_high & FEATURE_ACCESS_PLATFORM_HIGH_WORD))
}

/// The virtio handshake and queue setup, shared by the real driver and the attack test. The queue
/// is set up through the kernel, which places the rings at fixed offsets in our DMA region and
/// programs the device with those addresses: we never choose them.
fn initialize() {
    initialize_with_features(0, false);
}

/// The handshake, with `driver_features_low` OR'd into the low feature word. The honest driver
/// passes 0; the indirect attacker passes `FEATURE_INDIRECT_DESCRIPTORS_LOW_WORD`, unmasked against the offer, so the
/// kernel is what strips it.
///
/// `want_flush` asks for `VIRTIO_BLK_F_FLUSH` only if the device offered it. Returns whether that
/// bit was negotiated. See nifeos/record:notes/virtual-io.md ("Feature negotiation").
fn initialize_with_features(driver_features_low: u32, want_flush: bool) -> bool {
    // The driver checks the magic and not `Version`: the kernel offers only modern (version 2)
    // devices on either bus, so a `Version` check here would test what the kernel already tested.
    assert!(read_device_register(MAGIC_VALUE_REGISTER) == 0x7472_6976); // "virt": we really are talking to a virtio device

    reset_device();
    write_device_register(STATUS_REGISTER, DEVICE_STATUS_ACKNOWLEDGE);
    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE | DEVICE_STATUS_DRIVER,
    );

    // The kernel filters the low word, so a ring-layout feature it cannot validate does not
    // survive.
    write_device_register(DEVICE_FEATURES_SELECT_REGISTER, 0);
    let device_low = read_device_register(DEVICE_FEATURES_REGISTER);
    let flush = want_flush && (device_low & FEATURE_BLOCK_FLUSH_LOW_WORD) != 0;
    write_device_register(DRIVER_FEATURES_SELECT_REGISTER, 0);
    write_device_register(
        DRIVER_FEATURES_REGISTER,
        driver_features_low
            | if flush {
                FEATURE_BLOCK_FLUSH_LOW_WORD
            } else {
                0
            },
    );

    // The high word: VERSION_1 (refused when not offered), ACCESS_PLATFORM only if offered, so
    // the ack stays a subset of the offer.
    write_device_register(DEVICE_FEATURES_SELECT_REGISTER, 1);
    let device_high = read_device_register(DEVICE_FEATURES_REGISTER);
    let acknowledged_high = acknowledged_high_features(device_high)
        .expect("the device does not offer VIRTIO_F_VERSION_1, so it is not a modern device");
    write_device_register(DRIVER_FEATURES_SELECT_REGISTER, 1);
    write_device_register(DRIVER_FEATURES_REGISTER, acknowledged_high);

    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE | DEVICE_STATUS_DRIVER | DEVICE_STATUS_FEATURES_OK,
    );
    assert!(read_device_register(STATUS_REGISTER) & DEVICE_STATUS_FEATURES_OK != 0);

    // The layout constants (DESCRIPTOR_TABLE_OFFSET_BYTES/AVAIL/USED) are the contract the kernel uses.
    assert!(setup_queue(VIRTUAL_IO, QUEUE_ENTRIES as u64, 0) == 0);

    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE
            | DEVICE_STATUS_DRIVER
            | DEVICE_STATUS_FEATURES_OK
            | DEVICE_STATUS_DRIVER_OK,
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
/// A driver's `_start`. No system call translates an address, so the DMA page's physical address
/// arrives as an argument.
///
/// ```no_run
/// fn start(direct_memory_access_physical_address: u64) -> ! {
///     block_driver_roles::run(direct_memory_access_physical_address)
/// }
/// ```
///
/// Name: provisional.
pub fn run(direct_memory_access_physical_address: u64) -> ! {
    initialize();

    // Read block 0: the nifefs superblock.
    read_block(direct_memory_access_physical_address, 0);

    // It must be a nifefs image.
    let directory = data_block();
    assert!(directory[..8] == nifefs::MAGIC);

    // Walk the directory, copied out of the block-0 buffer, to find "motd", then read its first
    // data block.
    let start_block = find_file(directory, b"motd").unwrap_or_else(|| report_code(0xE4));
    read_block(direct_memory_access_physical_address, start_block as u64);

    // Report the file's first 8 bytes. The kernel checks them against the known contents, which
    // proves the actual file data came off the disk and across the EL0 boundary.
    let mut head = [0u8; 8];
    for (i, b) in head.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(DATA_OFFSET_BYTES + i as u64);
    }
    send(REPORT, u64::from_le_bytes(head), 0, 0);

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (nifeos/record:notes/virtual-io.md).
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
///
/// Name: provisional.
pub fn run_write(direct_memory_access_physical_address: u64) -> ! {
    initialize();

    // The directory tells us where scratch lives; nothing about the sector is hardcoded.
    read_block(direct_memory_access_physical_address, 0);
    let directory = data_block();
    assert!(directory[..8] == nifefs::MAGIC);
    let scratch = find_file(directory, b"scratch").unwrap_or_else(|| report_code(0xE6)) as u64;

    // Write the pattern...
    for i in 0..DATA_BUFFER_BYTES {
        direct_memory_access_write::<u8>(DATA_OFFSET_BYTES + i as u64, pattern_byte(i));
    }
    write_block(direct_memory_access_physical_address, scratch);

    // ...wipe the buffer so a read that silently does nothing cannot pass, and read it back.
    for i in 0..DATA_BUFFER_BYTES {
        direct_memory_access_write::<u8>(DATA_OFFSET_BYTES + i as u64, 0);
    }
    read_block(direct_memory_access_physical_address, scratch);
    for i in 0..DATA_BUFFER_BYTES {
        assert!(direct_memory_access_read::<u8>(DATA_OFFSET_BYTES + i as u64) == pattern_byte(i));
    }
    let mut head = [0u8; 8];
    for (i, b) in head.iter_mut().enumerate() {
        *b = direct_memory_access_read::<u8>(DATA_OFFSET_BYTES + i as u64);
    }

    // The write must have landed on ITS block and nothing else: the superblock still parses and
    // the read-path file is still in the directory.
    read_block(direct_memory_access_physical_address, 0);
    let directory = data_block();
    assert!(directory[..8] == nifefs::MAGIC);
    assert!(find_file(directory, b"motd").is_some());

    send(REPORT, u64::from_le_bytes(head), 0, 0);

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (nifeos/record:notes/virtual-io.md).
    exit();
}

/// **The abandoned write**, the kill-mid-write case: submit a write through the validated path,
/// report 1, then die (`brk`/`ebreak`) without waiting for the completion, acknowledging the
/// interrupt or touching the used ring. The test then runs the full writer against the same device,
/// which must succeed. See nifeos/record:notes/dma.md.
///
/// Name: provisional.
pub fn run_write_abandon(direct_memory_access_physical_address: u64) -> ! {
    initialize();

    read_block(direct_memory_access_physical_address, 0);
    let directory = data_block();
    assert!(directory[..8] == nifefs::MAGIC);
    let scratch = find_file(directory, b"scratch").unwrap_or_else(|| report_code(0xE6)) as u64;

    for i in 0..DATA_BUFFER_BYTES {
        direct_memory_access_write::<u8>(DATA_OFFSET_BYTES + i as u64, 0xAB);
    }
    let _used_before = submit_block(
        direct_memory_access_physical_address,
        scratch,
        BLOCK_REQUEST_WRITE,
    );

    send(REPORT, 1, 0, 0); // submitted; the kernel validated it and rang the device
    panic!(); // and now we are gone, mid-operation
}

/// **The attack, refused.** A malicious driver that points a descriptor at kernel memory and asks
/// the device to write there. The kernel validates the descriptor on submit and refuses, so the
/// device is never told to go and never touches the address. Reports 1 if the kernel refused
/// (correct), 0 if the notify went through (a hole). Used by the security test.
///
/// Name: provisional.
pub fn run_attack(_direct_memory_access_physical_address: u64) -> ! {
    initialize();

    // The forbidden target: the kernel's own image, which the device would happily DMA over if it
    // were ever handed this address.
    const KERNEL_VIRTUAL_ADDRESS: u64 = 0xffff_0000_4008_0000;

    // A single descriptor pointing OUTSIDE our DMA region, marked device-writable. The kernel walks
    // it on submit, sees KERNEL_VIRTUAL_ADDRESS is outside our region, and refuses.
    let refused = submit(
        &BLOCK_QUEUE,
        &[Descriptor {
            physical_address: KERNEL_VIRTUAL_ADDRESS,
            length_bytes: 512,
            flags: DESCRIPTOR_FLAG_DEVICE_WRITES,
        }],
    )
    .is_none();
    send(REPORT, refused as u64, 0, 0); // 1 = refused (good), 0 = it went through (bad)

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (nifeos/record:notes/virtual-io.md).
    exit();
}

/// **The indirect-descriptor attack, refused.** Negotiates `INDIRECT_DESC` and submits one
/// descriptor flagged `INDIRECT` whose inner table, in our own region, points the device at the
/// kernel; a validator that walked only the flat chain would let the device follow it out. The
/// kernel strips the feature and refuses the flag on submit. Reports 1 if refused (correct), 0 if
/// the notify went through (a hole).
///
/// Name: provisional.
pub fn run_attack_indirect(direct_memory_access_physical_address: u64) -> ! {
    // Ask for indirect descriptors. The kernel strips the bit, but we build the attack anyway to
    // prove the validator refuses the flag even if the feature ever slipped through.
    initialize_with_features(FEATURE_INDIRECT_DESCRIPTORS_LOW_WORD, false);

    const KERNEL_VIRTUAL_ADDRESS: u64 = 0xffff_0000_4008_0000;

    // The indirect TABLE lives in our region (the header scratch area). Its one entry aims the
    // device at kernel memory, device-writable.
    direct_memory_access_write::<u64>(REQUEST_HEADER_OFFSET_BYTES, KERNEL_VIRTUAL_ADDRESS); // the inner descriptor's address
    direct_memory_access_write::<u32>(REQUEST_HEADER_OFFSET_BYTES + 8, 512); // its length
    direct_memory_access_write::<u16>(
        REQUEST_HEADER_OFFSET_BYTES + 12,
        DESCRIPTOR_FLAG_DEVICE_WRITES,
    ); // its flags
    direct_memory_access_write::<u16>(REQUEST_HEADER_OFFSET_BYTES + 14, 0); // its next

    // Descriptor 0: flagged INDIRECT, pointing at the in-region table above. Wholly in-region, so only
    // the INDIRECT handling stands between this and a kernel-memory DMA. The kernel refuses the
    // indirect descriptor and never rings the device.
    let refused = submit(
        &BLOCK_QUEUE,
        &[Descriptor {
            physical_address: direct_memory_access_physical_address + REQUEST_HEADER_OFFSET_BYTES,
            length_bytes: 16,
            flags: DESCRIPTOR_FLAG_INDIRECT,
        }],
    )
    .is_none();
    send(REPORT, refused as u64, 0, 0); // 1 = refused (good), 0 = it went through (bad)

    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (nifeos/record:notes/virtual-io.md).
    exit();
}

/// The 512-byte block buffer in the DMA page, as a slice, so the parsers below read a slice. It is
/// a view and not a copy: a copy on the stack overflowed the driver's one-page stack in a debug
/// build. Call it only after a request has completed, and drop the slice before the next submit,
/// since the device writes this buffer while a read is in flight.
fn data_block() -> &'static [u8] {
    assert!(
        window_access_fits(
            DATA_OFFSET_BYTES,
            DATA_BUFFER_BYTES,
            1,
            DIRECT_MEMORY_ACCESS_WINDOW_BYTES
        ),
        "the data block lies outside the DMA page"
    );
    // SAFETY: the assertion above holds the buffer inside the first page of the DMA region, which
    // is mapped read/write at region() for every role, and a `u8` slice needs no alignment. The
    // device is idle on this buffer once the request completed, and the callers drop the slice
    // before the next submit.
    unsafe {
        core::slice::from_raw_parts(
            (region() + DATA_OFFSET_BYTES) as *const u8,
            DATA_BUFFER_BYTES,
        )
    }
}

/// Find a file in a nifefs directory's first block, `directory`, returning its start block. The
/// format is `nifefs`'s and the offsets come from that crate rather than being restated here:
/// header (magic 8, count u32), then entries of { name\[`NAME_LEN`\], `start_block` u32, len u32 },
/// little-endian.
///
/// Only the `nifefs::ENTRIES_IN_FIRST_BLOCK` entries inside the first block are visible, and an
/// entry `directory` is too short to hold is not. The disk this walks holds three files. A name
/// longer than `nifefs::NAME_LEN` names no entry, so it is `None` rather than a read into the next
/// entry.
fn find_file(directory: &[u8], name: &[u8]) -> Option<u32> {
    if name.len() > nifefs::NAME_LEN {
        return None;
    }
    let count = u32::from_le_bytes(directory.get(8..12)?.try_into().ok()?) as usize;
    (0..count.min(nifefs::ENTRIES_IN_FIRST_BLOCK)).find_map(|i| {
        let at = nifefs::HEADER_LEN + i * nifefs::ENTRY_LEN;
        let entry = directory.get(at..at + nifefs::ENTRY_LEN)?;
        let field = &entry[..nifefs::NAME_LEN];
        // The name must end here, so "motd" does not match "motdx": either the next byte is NUL
        // padding, or the name used every byte of the field and there is no padding to check.
        let ends = field.get(name.len()).is_none_or(|&byte| byte == 0);
        (field.starts_with(name) && ends).then(|| {
            u32::from_le_bytes(
                entry[nifefs::NAME_LEN..nifefs::NAME_LEN + 4]
                    .try_into()
                    .expect("four bytes"),
            )
        })
    })
}

/// One queue's rings in the DMA page, by offset, and its number for `notify`.
struct Queue {
    number: u64,
    descriptor_table: u64,
    available_ring: u64,
    used_ring: u64,
}

/// The disk's one queue.
const BLOCK_QUEUE: Queue = Queue {
    number: 0,
    descriptor_table: DESCRIPTOR_TABLE_OFFSET_BYTES,
    available_ring: AVAILABLE_RING_OFFSET_BYTES,
    used_ring: USED_RING_OFFSET_BYTES,
};

/// One buffer of a descriptor chain. [`submit`] adds `DESCRIPTOR_FLAG_NEXT` and the link itself, so
/// `flags` carries only the direction (`DESCRIPTOR_FLAG_DEVICE_WRITES`) or `DESCRIPTOR_FLAG_INDIRECT`.
#[derive(Clone, Copy)]
struct Descriptor {
    physical_address: u64,
    length_bytes: u32,
    flags: u16,
}

/// The flags and `next` field of descriptor `i` in a chain of `chain_length` laid out from head 0:
/// every descriptor but the last carries `DESCRIPTOR_FLAG_NEXT` and links to `i + 1`, and the last
/// links nowhere.
fn chained(i: usize, chain_length: usize, flags: u16) -> (u16, u16) {
    if i + 1 < chain_length {
        (flags | DESCRIPTOR_FLAG_NEXT, i as u16 + 1)
    } else {
        (flags, 0)
    }
}

/// **Submit one request**: write `chain` into `queue`'s descriptor table from head 0, publish head
/// 0 in the available ring, and ring the device. Returns the used ring's index from before the
/// submit, which [`complete`] waits to see move, or `None` when the transport refused the notify:
/// on a mediated grant the kernel checks every descriptor and refuses one that leaves the region.
///
/// Every request reuses head 0 and is waited for before the next is written, so one request is
/// in flight per queue and each costs one interrupt. It is the simplest shape a virtio driver can
/// take, and the slowest. It stays because it is fast enough for the one production path, the
/// block server. Measured through the confined block server, a block costs 39.0 microseconds at
/// the margin, against 38.7 to 53.3 microseconds for a raw virtio read on Linux at the same tier.
/// A 64 kibibyte sequential read runs at 189.9 mebibytes a second (Hypervisor.framework, `--smp`;
/// nifeos/record:notes/benchmarks.md, "Filesystem throughput"). A request already moves up to
/// `block::TRANSFER_BLOCKS` blocks as one descriptor, so the driver pays the interrupt per request
/// and not per block. Several requests in flight would need a free list of descriptor heads and a
/// completion that matches used-ring elements to their requests, and nothing measured yet asks
/// for either.
fn submit(queue: &Queue, chain: &[Descriptor]) -> Option<u16> {
    for (i, descriptor) in chain.iter().enumerate() {
        let (flags, next) = chained(i, chain.len(), descriptor.flags);
        write_descriptor(
            queue.descriptor_table,
            i as u64,
            descriptor.physical_address,
            descriptor.length_bytes,
            flags,
            next,
        );
    }
    // avail: { u16 flags; u16 idx; u16 ring[QUEUE_ENTRIES]; }, and used: { u16 flags; u16 idx; ... }.
    let used_before: u16 = direct_memory_access_read::<u16>(queue.used_ring + 2);
    let available_index: u16 = direct_memory_access_read::<u16>(queue.available_ring + 2);
    direct_memory_access_write::<u16>(
        queue.available_ring + 4 + (available_index as u64 % QUEUE_ENTRIES as u64) * 2,
        0,
    ); // ring[idx] = head 0
    virtual_io_ring_barrier(); // the descriptors and ring entry must be visible before we bump idx
    direct_memory_access_write::<u16>(queue.available_ring + 2, available_index.wrapping_add(1));
    virtual_io_ring_barrier(); // idx must be visible before we notify
    (notify(VIRTUAL_IO, queue.number) >= 0).then_some(used_before)
}

/// **Submit one block request**: the 16-byte header the device reads (type, reserved, sector), the
/// data buffer at `data` (an offset in the DMA region and a length) if the request moves any, and
/// the status byte the device writes. A read (`T_IN`) marks the data descriptor device-writable
/// and a write (`T_OUT`) does not; the kernel checks every address on NOTIFY, whatever the flag.
fn submit_block_request(
    direct_memory_access_physical_address: u64,
    request_type: u32,
    sector: u64,
    data: Option<(u64, u32)>,
) -> Option<u16> {
    direct_memory_access_write::<u32>(REQUEST_HEADER_OFFSET_BYTES, request_type);
    direct_memory_access_write::<u32>(REQUEST_HEADER_OFFSET_BYTES + 4, 0);
    direct_memory_access_write::<u64>(REQUEST_HEADER_OFFSET_BYTES + 8, sector);
    direct_memory_access_write::<u8>(STATUS_OFFSET_BYTES, 0xff); // the device overwrites this with 0 on success

    let header = Descriptor {
        physical_address: direct_memory_access_physical_address + REQUEST_HEADER_OFFSET_BYTES,
        length_bytes: 16,
        flags: 0,
    };
    let status = Descriptor {
        physical_address: direct_memory_access_physical_address + STATUS_OFFSET_BYTES,
        length_bytes: 1,
        flags: DESCRIPTOR_FLAG_DEVICE_WRITES,
    };
    match data {
        Some((offset, length_bytes)) => {
            let data = Descriptor {
                physical_address: direct_memory_access_physical_address + offset,
                length_bytes,
                // read: the device fills the buffer; write: the device consumes it
                flags: if request_type == BLOCK_REQUEST_READ {
                    DESCRIPTOR_FLAG_DEVICE_WRITES
                } else {
                    0
                },
            };
            submit(&BLOCK_QUEUE, &[header, data, status])
        }
        None => submit(&BLOCK_QUEUE, &[header, status]),
    }
}

/// Submit one 512-byte block transfer from the DMA data buffer for a test role. Reports `0xE5`
/// when the kernel refuses it.
fn submit_block(direct_memory_access_physical_address: u64, sector: u64, request_type: u32) -> u16 {
    submit_block_request(
        direct_memory_access_physical_address,
        request_type,
        sector,
        Some((DATA_OFFSET_BYTES, DATA_BUFFER_BYTES as u32)),
    )
    // The kernel refused our request: a descriptor escaped our region.
    .unwrap_or_else(|| report_code(0xE5))
}

/// How many wakeups a one-shot test role waits for its request's completion before it reports that
/// none came. QEMU completes a request inside `NOTIFY`, so the first wakeup is nearly always the
/// completion; the rest absorb stale and shared-line wakeups.
const COMPLETION_WAKEUPS: u32 = 64;

/// Whether the used ring's index moved off `used_before` within `bound` wakeups, or ever when
/// `bound` is `None`. `wake_and_read` waits for one wakeup and returns the used index after it, so
/// the loop can be tested without a device.
///
/// The completion is the used ring advancing, not the wakeup: the line is shared, and a wakeup can
/// be spurious, coalesced, or a dead driver's pending completion. See nifeos/record:notes/virtual-io.md ("A
/// completion is the used ring advancing") and nifeos/record:notes/dma.md.
fn used_ring_moved(
    bound: Option<u32>,
    used_before: u16,
    mut wake_and_read: impl FnMut() -> u16,
) -> bool {
    match bound {
        Some(wakeups) => (0..wakeups).any(|_| wake_and_read() != used_before),
        None => loop {
            if wake_and_read() != used_before {
                return true;
            }
        },
    }
}

/// **Wait for `queue`'s used ring to move off `used_before`**, waking on the device interrupt, for
/// at most `bound` wakeups (`None`: for as long as it takes). Returns whether it moved.
///
/// A one-shot test role passes [`COMPLETION_WAKEUPS`] and reports when it runs out (`0xE9` for the
/// net receive, `0xEB` for a block request), so a device that never completes fails the test
/// instead of hanging the suite. The block server passes `None`: giving up would leave its request
/// on the ring at descriptor head 0, and the next request would rewrite descriptors the device may
/// still be reading. Neither helps against a device that raises no interrupt at all, since each
/// wakeup is a WAIT that blocks until the line fires.
fn complete(queue: &Queue, used_before: u16, bound: Option<u32>) -> bool {
    used_ring_moved(bound, used_before, || {
        // The kernel masks the line and delivers the interrupt as a notification; its pending count
        // returns WAIT at once if the interrupt fired before this call.
        interrupt_wait(INTERRUPT);
        // Quiet the device (read its interrupt-status, acknowledge it), then re-enable the line at
        // the controller, which the kernel masked when it fired.
        let interrupt_status = read_device_register(INTERRUPT_STATUS_REGISTER);
        write_device_register(INTERRUPT_ACKNOWLEDGE_REGISTER, interrupt_status);
        interrupt_acknowledge(INTERRUPT);
        virtual_io_ring_barrier();
        direct_memory_access_read::<u16>(queue.used_ring + 2)
    })
}

/// The status byte the device wrote for the last block request: 0 is `VIRTIO_BLK_S_OK`.
fn block_status() -> u8 {
    direct_memory_access_read::<u8>(STATUS_OFFSET_BYTES)
}

/// Wait for a test role's block transfer to complete, and confirm the device reported success.
/// Reports `0xEB` when no completion arrives within [`COMPLETION_WAKEUPS`], and `0xE200 | status`
/// on any other status.
fn complete_block(used_before: u16) {
    if !complete(&BLOCK_QUEUE, used_before, Some(COMPLETION_WAKEUPS)) {
        report_code(0xEB);
    }
    let status = block_status();
    if status != 0 {
        report_code(0xE200 | status as u64); // device reported a non-OK status
    }
}

/// Read one block from `sector` into the DMA data buffer, start to finish.
fn read_block(direct_memory_access_physical_address: u64, sector: u64) {
    let used_before = submit_block(
        direct_memory_access_physical_address,
        sector,
        BLOCK_REQUEST_READ,
    );
    complete_block(used_before);
}

/// Write the DMA data buffer to `sector`, start to finish (milestone 32 (a real filesystem:
/// RedoxFS behind a capability FS server) phase 1): the read's chain, data direction flipped.
fn write_block(direct_memory_access_physical_address: u64, sector: u64) {
    let used_before = submit_block(
        direct_memory_access_physical_address,
        sector,
        BLOCK_REQUEST_WRITE,
    );
    complete_block(used_before);
}

/// Report a diagnostic code to the kernel and stop. A one-shot role sends
/// `0xDEAD_0000_0000_0000 | code` on its report slot, distinct from every success value, so the
/// kernel's test fails and prints the code. Only reached when a role cannot finish.
///
/// | code | role | what happened |
/// |---|---|---|
/// | `0xE4` | [`run`] | the directory has no `motd` file |
/// | `0xE5` | the block test roles | the kernel refused a block request's notify: a descriptor left the region |
/// | `0xE6` | [`run_write`], [`run_write_abandon`] | the directory has no `scratch` file |
/// | `0xE7` | [`run_net`] | the kernel refused the receive buffer's notify |
/// | `0xE8` | [`run_net`] | the kernel refused the transmit frame's notify |
/// | `0xE9` | [`run_net`] | no receive completion within [`COMPLETION_WAKEUPS`] wakeups |
/// | `0xEA` | [`run_net`] | frames arrived, and none was the OFFER for this transaction |
/// | `0xEB` | the block test roles | no block completion within [`COMPLETION_WAKEUPS`] wakeups |
/// | `0xE200 \| status` | the block test roles | the device completed a request with a status other than 0 |
///
/// The attackers report 1 (refused) or 0 (not refused) instead, and the long-lived block server
/// reports nothing: it answers each client with an error.
fn report_code(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    // Exit rather than spin: leaked spinners starve later tests past the watchdog
    // (nifeos/record:notes/virtual-io.md).
    exit();
}

// =================================================================================================
// A virtio-net driver, at EL0, behind the multi-queue DMA confinement (milestone 30 (the network
// stack as a confined component)).
//
// Receive is queue 0 (the device writes a frame) and transmit is queue 1 (it reads one), both
// through the one `VirtualIo` capability. `run_net` proves both with a DHCP exchange against slirp.
// See nifeos/record:notes/virtual-io.md ("The roles").
// =================================================================================================

/// The two virtqueues, matching the kernel's contract (receive = 0, transmit = 1).
const NET_RECEIVE_QUEUE_NUMBER: u64 = 0;
const NET_TRANSMIT_QUEUE_NUMBER: u64 = 1;

/// The modern virtio-net header, prepended to every frame in both directions. 12 bytes because we
/// negotiate `VIRTIO_F_VERSION_1` (the `num_buffers` field is then always present).
const NET_HEADER_BYTES: u64 = 12;

// Our DMA-page layout for the net driver. Each queue's rings sit in its ring block at the kernel's
// offsets (`moho_abi::virtual_io`), and the packet buffers live above both queues' blocks.
// Everything fits in the one 4 KiB DMA page the spawn service hands us.
const NET_RECEIVE_RING_BLOCK_OFFSET_BYTES: u64 = NET_RECEIVE_QUEUE_NUMBER * RING_BLOCK_BYTES;
const NET_TRANSMIT_RING_BLOCK_OFFSET_BYTES: u64 = NET_TRANSMIT_QUEUE_NUMBER * RING_BLOCK_BYTES;
const NET_RECEIVE_DESCRIPTOR_TABLE_OFFSET_BYTES: u64 =
    NET_RECEIVE_RING_BLOCK_OFFSET_BYTES + DESCRIPTOR_TABLE_OFFSET_BYTES;
const NET_RECEIVE_AVAILABLE_RING_OFFSET_BYTES: u64 =
    NET_RECEIVE_RING_BLOCK_OFFSET_BYTES + AVAILABLE_RING_OFFSET_BYTES;
const NET_RECEIVE_USED_RING_OFFSET_BYTES: u64 =
    NET_RECEIVE_RING_BLOCK_OFFSET_BYTES + USED_RING_OFFSET_BYTES;
const NET_TRANSMIT_DESCRIPTOR_TABLE_OFFSET_BYTES: u64 =
    NET_TRANSMIT_RING_BLOCK_OFFSET_BYTES + DESCRIPTOR_TABLE_OFFSET_BYTES;
const NET_TRANSMIT_AVAILABLE_RING_OFFSET_BYTES: u64 =
    NET_TRANSMIT_RING_BLOCK_OFFSET_BYTES + AVAILABLE_RING_OFFSET_BYTES;
// The driver never waits on transmit completions, only on the receive side; [`submit`] reads the
// transmit used ring's index and nothing reads its elements.
const NET_TRANSMIT_USED_RING_OFFSET_BYTES: u64 =
    NET_TRANSMIT_RING_BLOCK_OFFSET_BYTES + USED_RING_OFFSET_BYTES;
/// One receive buffer: the 12-byte virtio header plus room for a full 1514-byte Ethernet frame.
const NET_RECEIVE_BUFFER_OFFSET_BYTES: u64 = 0x400;
const NET_RECEIVE_BUFFER_BYTES: u64 = 0x600;
/// One transmit buffer: the header plus the DHCP frame we build.
const NET_TRANSMIT_BUFFER_OFFSET_BYTES: u64 = 0xA00;

/// Our source MAC. Locally administered; slirp does not care what it is, only that DHCP names it.
const NET_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
/// (`xid`) A recognizable DHCP transaction id, so the OFFER we accept is unambiguously the reply to our
/// DISCOVER and not some unrelated broadcast.
#[doc(alias = "xid")]
const NET_TRANSACTION_ID: u32 = 0x3903_F326;

/// The IPv4 header checksum over `header`: sum the 16-bit big-endian words, fold the carries, and
/// complement. The checksum field itself must be zero when this runs, and a header with the result
/// written into that field sums to zero (`RFC 1071`).
fn ip_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = header
        .chunks(2)
        // An odd trailing byte is summed as the high byte of a zero-padded word (RFC 1071).
        .map(|word| u32::from(word[0]) << 8 | u32::from(word.get(1).copied().unwrap_or(0)))
        .fold(0u32, u32::wrapping_add);
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// The virtio-net handshake and both-queue setup. Unlike the disk's `initialize`, this sets up
/// receive AND transmit, through the kernel, which places each queue's rings at its own block.
fn initialize_net() {
    assert!(read_device_register(MAGIC_VALUE_REGISTER) == 0x7472_6976); // "virt"

    reset_device();
    write_device_register(STATUS_REGISTER, DEVICE_STATUS_ACKNOWLEDGE);
    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE | DEVICE_STATUS_DRIVER,
    );

    // Negotiate no low-word net features: no checksum offload, no MRG_RXBUF, so one frame lands in
    // one buffer behind one 12-byte header, which is the simplest correct thing.
    write_device_register(DRIVER_FEATURES_SELECT_REGISTER, 0);
    write_device_register(DRIVER_FEATURES_REGISTER, 0);

    // The high word: VERSION_1 always, ACCESS_PLATFORM only if offered (behind an IOMMU), the same
    // conditional ack the disk driver makes so one binary works on the bare NIC and the confined one.
    write_device_register(DEVICE_FEATURES_SELECT_REGISTER, 1);
    let device_high = read_device_register(DEVICE_FEATURES_REGISTER);
    let acknowledged_high = acknowledged_high_features(device_high)
        .expect("the device does not offer VIRTIO_F_VERSION_1, so it is not a modern device");
    write_device_register(DRIVER_FEATURES_SELECT_REGISTER, 1);
    write_device_register(DRIVER_FEATURES_REGISTER, acknowledged_high);

    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE | DEVICE_STATUS_DRIVER | DEVICE_STATUS_FEATURES_OK,
    );
    assert!(read_device_register(STATUS_REGISTER) & DEVICE_STATUS_FEATURES_OK != 0);

    // Both queues, through the kernel: it programs each queue's ring addresses to that queue's block
    // in our DMA region, so we never choose them.
    assert!(setup_queue(VIRTUAL_IO, QUEUE_ENTRIES as u64, NET_RECEIVE_QUEUE_NUMBER) == 0);
    assert!(setup_queue(VIRTUAL_IO, QUEUE_ENTRIES as u64, NET_TRANSMIT_QUEUE_NUMBER) == 0);

    write_device_register(
        STATUS_REGISTER,
        DEVICE_STATUS_ACKNOWLEDGE
            | DEVICE_STATUS_DRIVER
            | DEVICE_STATUS_FEATURES_OK
            | DEVICE_STATUS_DRIVER_OK,
    );
}

/// The receive queue's rings.
const NET_RECEIVE_QUEUE: Queue = Queue {
    number: NET_RECEIVE_QUEUE_NUMBER,
    descriptor_table: NET_RECEIVE_DESCRIPTOR_TABLE_OFFSET_BYTES,
    available_ring: NET_RECEIVE_AVAILABLE_RING_OFFSET_BYTES,
    used_ring: NET_RECEIVE_USED_RING_OFFSET_BYTES,
};

/// The transmit queue's rings.
const NET_TRANSMIT_QUEUE: Queue = Queue {
    number: NET_TRANSMIT_QUEUE_NUMBER,
    descriptor_table: NET_TRANSMIT_DESCRIPTOR_TABLE_OFFSET_BYTES,
    available_ring: NET_TRANSMIT_AVAILABLE_RING_OFFSET_BYTES,
    used_ring: NET_TRANSMIT_USED_RING_OFFSET_BYTES,
};

/// Write one 16-byte descriptor, { u64 addr; u32 len; u16 flags; u16 next; }, as entry `index` of
/// the descriptor table at offset `descriptor_table` in the DMA page.
fn write_descriptor(
    descriptor_table: u64,
    index: u64,
    physical_address: u64,
    length_bytes: u32,
    flags: u16,
    next: u16,
) {
    let at = descriptor_table + index * 16;
    direct_memory_access_write::<u64>(at, physical_address);
    direct_memory_access_write::<u32>(at + 8, length_bytes);
    direct_memory_access_write::<u16>(at + 12, flags);
    direct_memory_access_write::<u16>(at + 14, next);
}

/// Post the single receive buffer as a device-writable descriptor and ring queue 0. Re-callable: it
/// re-uses descriptor head 0 with a fresh available-ring slot each time, so the device fills the
/// same buffer again. Reports `0xE7` when the kernel refuses it. `direct_memory_access_physical_address` is
/// the DMA page's physical base (descriptors speak physical).
fn post_receive(direct_memory_access_physical_address: u64) {
    // The kernel validates the receive descriptor (device-writable, in our region).
    submit(
        &NET_RECEIVE_QUEUE,
        &[Descriptor {
            physical_address: direct_memory_access_physical_address
                + NET_RECEIVE_BUFFER_OFFSET_BYTES,
            length_bytes: NET_RECEIVE_BUFFER_BYTES as u32,
            flags: DESCRIPTOR_FLAG_DEVICE_WRITES, // the device WRITES the received frame here
        }],
    )
    .unwrap_or_else(|| report_code(0xE7));
}

/// Transmit the frame already built at `NET_TRANSMIT_BUFFER_OFFSET_BYTES + NET_HEADER_BYTES`, of `frame_bytes` bytes: zero
/// the virtio header in front of it, post one device-readable descriptor, and ring queue 1.
/// Reports `0xE8` when the kernel refuses it.
fn send_frame(direct_memory_access_physical_address: u64, frame_bytes: u64) {
    for i in 0..NET_HEADER_BYTES {
        direct_memory_access_write::<u8>(NET_TRANSMIT_BUFFER_OFFSET_BYTES + i, 0);
    }
    submit(
        &NET_TRANSMIT_QUEUE,
        &[Descriptor {
            physical_address: direct_memory_access_physical_address
                + NET_TRANSMIT_BUFFER_OFFSET_BYTES,
            length_bytes: (NET_HEADER_BYTES + frame_bytes) as u32,
            flags: 0, // device READS the buffer (transmit)
        }],
    )
    .unwrap_or_else(|| report_code(0xE8));
}

/// The length of the Ethernet frame [`dhcp_discover`] builds: Ethernet 14, IPv4 20, UDP 8, the
/// fixed BOOTP fields and magic cookie 240, and nine bytes of options.
const DHCP_DISCOVER_BYTES: usize = 14 + 20 + 8 + 240 + 9;

// The frame fits the transmit buffer behind its virtio header, inside the DMA page.
const _: () = assert!(
    NET_TRANSMIT_BUFFER_OFFSET_BYTES + NET_HEADER_BYTES + DHCP_DISCOVER_BYTES as u64
        <= DIRECT_MEMORY_ACCESS_WINDOW_BYTES
);

/// A DHCP DISCOVER (Ethernet + IPv4 + UDP + BOOTP/DHCP), as the Ethernet frame. The broadcast
/// flag is set so slirp's OFFER comes back to the broadcast address, where the single receive
/// buffer will catch it.
fn dhcp_discover() -> [u8; DHCP_DISCOVER_BYTES] {
    let mut frame = [0u8; DHCP_DISCOVER_BYTES];

    // --- Ethernet: to broadcast, from us, IPv4 ---
    frame[0..6].fill(0xff);
    frame[6..12].copy_from_slice(&NET_MAC);
    frame[12..14].copy_from_slice(&0x0800u16.to_be_bytes());

    let ip = 14;
    let udp = ip + 20;
    let dhcp = udp + 8;

    // --- BOOTP/DHCP payload. Every field not written here is zero: secs, ciaddr, yiaddr, siaddr,
    // giaddr, the rest of chaddr, sname and file. ---
    frame[dhcp] = 1; // op = BOOTREQUEST
    frame[dhcp + 1] = 1; // htype = Ethernet
    frame[dhcp + 2] = 6; // hlen
    frame[dhcp + 4..dhcp + 8].copy_from_slice(&NET_TRANSACTION_ID.to_be_bytes());
    frame[dhcp + 10..dhcp + 12].copy_from_slice(&0x8000u16.to_be_bytes()); // flags = broadcast
    frame[dhcp + 28..dhcp + 34].copy_from_slice(&NET_MAC); // chaddr
    frame[dhcp + 236..dhcp + 240].copy_from_slice(&0x6382_5363u32.to_be_bytes()); // DHCP magic cookie

    // Options: message type DISCOVER (53), a parameter request list (55) asking for the subnet
    // mask, router and DNS, then end (255).
    frame[dhcp + 240..].copy_from_slice(&[53, 1, 1, 55, 3, 1, 3, 6, 255]);

    let udp_bytes = (DHCP_DISCOVER_BYTES - udp) as u16;
    let ip_total_bytes = (DHCP_DISCOVER_BYTES - ip) as u16;

    // --- UDP: bootpc(68) -> bootps(67), checksum 0 (optional for IPv4) ---
    frame[udp..udp + 2].copy_from_slice(&68u16.to_be_bytes());
    frame[udp + 2..udp + 4].copy_from_slice(&67u16.to_be_bytes());
    frame[udp + 4..udp + 6].copy_from_slice(&udp_bytes.to_be_bytes());

    // --- IPv4 header: identification, flags and fragment 0, source 0.0.0.0 ---
    frame[ip] = 0x45; // version 4, IHL 5
    frame[ip + 2..ip + 4].copy_from_slice(&ip_total_bytes.to_be_bytes());
    frame[ip + 8] = 64; // TTL
    frame[ip + 9] = 17; // protocol = UDP
    frame[ip + 16..ip + 20].fill(0xff); // destination 255.255.255.255
    let checksum = ip_checksum(&frame[ip..ip + 20]);
    frame[ip + 10..ip + 12].copy_from_slice(&checksum.to_be_bytes());

    frame
}

/// Copy [`dhcp_discover`]'s frame into the transmit buffer behind the virtio header, and return its
/// length.
fn build_dhcp_discover() -> u64 {
    let frame = dhcp_discover();
    for (i, &byte) in frame.iter().enumerate() {
        direct_memory_access_write::<u8>(
            NET_TRANSMIT_BUFFER_OFFSET_BYTES + NET_HEADER_BYTES + i as u64,
            byte,
        );
    }
    frame.len() as u64
}

/// Wait for the receive queue's used ring to advance, discarding wakeups that were really the
/// transmit completion (the device's interrupt line is shared across both queues). Returns false if
/// no receive completion arrives within [`COMPLETION_WAKEUPS`].
fn wait_receive(receive_used_before: u16) -> bool {
    complete(
        &NET_RECEIVE_QUEUE,
        receive_used_before,
        Some(COMPLETION_WAKEUPS),
    )
}

/// Parse the frame in the receive buffer as a DHCP OFFER for our transaction. `used_slot` is the
/// receive used ring element the device just published; its length is how many bytes the device
/// wrote, virtio header included, and nothing past it is read. Returns the offered address
/// (`yiaddr`) on a match, `None` otherwise, so an unrelated broadcast is skipped rather than
/// mistaken for the reply.
fn parse_dhcp_offer(used_slot: u16) -> Option<u32> {
    let element =
        NET_RECEIVE_USED_RING_OFFSET_BYTES + 4 + (used_slot as u64 % QUEUE_ENTRIES as u64) * 8;
    let written = direct_memory_access_read::<u32>(element + 4) as u64;
    let frame_bytes = written
        .min(NET_RECEIVE_BUFFER_BYTES)
        .saturating_sub(NET_HEADER_BYTES);
    let frame = NET_RECEIVE_BUFFER_OFFSET_BYTES + NET_HEADER_BYTES;
    parse_offer_in_frame(Frame {
        length_bytes: frame_bytes,
        byte: |at| direct_memory_access_read::<u8>(frame + at),
    })
}

/// A received Ethernet frame of `length_bytes` bytes, read one byte at a time through `byte`. Every
/// read past `length_bytes` is `None`, so a parse cannot see stale bytes from an earlier frame. On the device the
/// bytes come from the DMA page; in the tests, from a slice.
struct Frame<F: Fn(u64) -> u8> {
    length_bytes: u64,
    byte: F,
}

impl<F: Fn(u64) -> u8> Frame<F> {
    fn u8(&self, at: u64) -> Option<u8> {
        (at < self.length_bytes).then(|| (self.byte)(at))
    }
    fn big_endian_u16(&self, at: u64) -> Option<u16> {
        Some(u16::from_be_bytes([self.u8(at)?, self.u8(at + 1)?]))
    }
    fn big_endian_u32(&self, at: u64) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.u8(at)?,
            self.u8(at + 1)?,
            self.u8(at + 2)?,
            self.u8(at + 3)?,
        ]))
    }
    /// The same frame cut to `length_bytes` bytes, when that is shorter: a header's own length
    /// field may only shrink what is read, never grow it.
    fn truncated(self, length_bytes: u64) -> Self {
        Frame {
            length_bytes: self.length_bytes.min(length_bytes),
            byte: self.byte,
        }
    }
}

/// [`parse_dhcp_offer`]'s parse, on a frame that starts at its Ethernet header. Every read is
/// bounded by the frame's length, then by the IPv4 total length, then by the UDP length.
fn parse_offer_in_frame<F: Fn(u64) -> u8>(frame: Frame<F>) -> Option<u32> {
    if frame.big_endian_u16(12)? != 0x0800 {
        return None; // not IPv4
    }
    let ip = 14;
    // The IPv4 header's own length (IHL), counted in 32-bit words.
    let header_bytes = (frame.u8(ip)? & 0x0f) as u64 * 4;
    if header_bytes < 20 || frame.u8(ip + 9)? != 17 {
        return None; // not a sane IPv4/UDP header
    }
    let packet_bytes = frame.big_endian_u16(ip + 2)? as u64;
    let frame = frame.truncated(ip + packet_bytes);
    let udp = ip + header_bytes;
    if frame.big_endian_u16(udp + 2)? != 68 {
        return None; // not addressed to the DHCP client port
    }
    let transport_bytes = frame.big_endian_u16(udp + 4)? as u64;
    let frame = frame.truncated(udp + transport_bytes);
    let dhcp = udp + 8;
    if frame.u8(dhcp)? != 2 || frame.big_endian_u32(dhcp + 4)? != NET_TRANSACTION_ID {
        return None; // not a BOOTREPLY for our transaction
    }
    if frame.big_endian_u32(dhcp + 236)? != 0x6382_5363 {
        return None; // no DHCP magic cookie
    }
    let yiaddr = frame.big_endian_u32(dhcp + 16)?;

    // Scan the options for message type 53 == 2 (OFFER). An option whose length runs past the
    // frame ends the scan.
    let mut option = dhcp + 240;
    let mut is_offer = false;
    while let Some(tag) = frame.u8(option) {
        if tag == 255 {
            break; // end
        }
        if tag == 0 {
            option += 1; // pad
            continue;
        }
        let Some(option_bytes) = frame.u8(option + 1) else {
            break;
        };
        let option_bytes = option_bytes as u64;
        if option + 2 + option_bytes > frame.length_bytes {
            break;
        }
        if tag == 53 && option_bytes >= 1 && frame.u8(option + 2) == Some(2) {
            is_offer = true;
        }
        option += 2 + option_bytes;
    }

    (is_offer && yiaddr != 0).then_some(yiaddr)
}

/// **The virtio-net headline: a DHCP round trip from an EL0 driver.** Bring the NIC up, post a
/// receive buffer, transmit a DHCP DISCOVER, and wait for slirp's OFFER. Reports the offered IPv4
/// address (`yiaddr`) as a little-endian word, which the kernel test checks lands in QEMU user-mode
/// networking's 10.0.2.0/24. TX and RX, both queues, both directions of the confinement, proven end
/// to end with no TCP/IP stack in the loop.
///
/// Name: provisional.
pub fn run_net(direct_memory_access_physical_address: u64) -> ! {
    initialize_net();

    post_receive(direct_memory_access_physical_address);

    let frame_bytes = build_dhcp_discover();
    send_frame(direct_memory_access_physical_address, frame_bytes);

    let mut receive_used: u16 = 0;
    for _ in 0..8 {
        if !wait_receive(receive_used) {
            report_code(0xE9); // no receive completion arrived
        }
        receive_used = direct_memory_access_read::<u16>(NET_RECEIVE_USED_RING_OFFSET_BYTES + 2);

        // One buffer is posted at a time, so the element just before the used index is its frame.
        if let Some(yiaddr) = parse_dhcp_offer(receive_used.wrapping_sub(1)) {
            send(REPORT, yiaddr as u64, 0, 0);
            exit(); // one-shot: reported the DHCP offer, so exit and be reaped, do not spin
        }

        // Not our OFFER (some other broadcast): re-post the buffer and wait for the next frame.
        post_receive(direct_memory_access_physical_address);
    }
    report_code(0xEA); // frames arrived, but none was our DHCP OFFER
}

// ---------------------------------------------------------------------------------------------
// The block server (milestone 32 phase 2): init the device once, then answer the FS server's
// read/write/size/flush requests forever. The FS server never touches the DMA region, and the
// device confinement is unchanged. See nifeos/record:notes/virtual-io.md ("The block server") and nifeos/record:notes/fs-server.md.
// ---------------------------------------------------------------------------------------------

/// The block server's request endpoint (slot 0): the FS server CALLs here; this role `RECEIVE_CAPABILITY`s the
/// request plus the one-shot Reply that names the caller. Interrupt (1) and VIRTIO (2) are as every
/// role.
///
/// Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
const BLOCK_REQUEST_ENDPOINT: u64 = 0;

/// Where the kernel maps the data region shared with the FS server (the block buffer). Distinct
/// from the DMA region, which the FS server must never see, and from [`DIRECT_MEMORY_ACCESS_BASE_VIRTUAL_ADDRESS`].
///
/// The block server's DMA region is `1 + block::TRANSFER_BLOCKS` contiguous pages
/// (`kernel/src/user/fs_service.rs`): page 0 holds the rings, request header and status, and pages
/// `1..=block::TRANSFER_BLOCKS` are the data buffer, which is the region shared with the FS server.
/// `BLOCK_SERVER_DATA_OFFSET_BYTES` is that page-1 offset. See nifeos/record:notes/virtual-io.md ("The block server").
const BLOCK_SERVER_DATA_OFFSET_BYTES: u64 = 0x1000;

/// The block server's readiness endpoint (slot 3): SEND once after the device is up, so the test
/// can tell a device-bring-up hang from a first-read hang. See `kernel/src/user/fs_service.rs`.
///
/// Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
const BLOCK_READY_ENDPOINT: u64 = 3;

/// (`Config`) The virtio-mmio device configuration window; virtio-blk's capacity (u64, in 512-byte
/// sectors) is at its start. Reads are DMA-safe, so the kernel's `READ_REGISTER` permits this
/// offset.
///
/// Name: provisional, minted 2026-11-22 (UTC) by lane 1113-finish-review.
#[doc(alias = "Config")]
const DEVICE_CONFIGURATION_REGISTER: u64 = 0x100;

/// **The block server.** Bring the device up, then serve blk IPC forever. Every request is a CALL
/// answered through the kernel-minted Reply, so this server can answer a client it was never wired
/// to, exactly once each. The transfer unit is one filesystem block; the bulk rides in the shared
/// page, the control (opcode, block index, result) rides in the message, the §10 split.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1113-names-qemu-test. Renamed from
/// `run_blk_server`, because `blk` is a failing abbreviation (nifeos/record:design/naming.md).
pub fn run_block_server(direct_memory_access_physical_address: u64) -> ! {
    // Whether VIRTIO_BLK_F_FLUSH was negotiated decides `block::FLUSH`: a device round trip, or a
    // loud refusal, never a yes that does nothing.
    let can_flush = initialize_with_features(0, true);
    // The capacity, read once: every transfer is checked against it before it reaches the device,
    // so a request past the end is refused here rather than completed by the device as an error.
    let capacity_sectors = capacity_sectors();
    // The device is up: signal readiness, so a hang here (bring-up) is distinct from a hang in the
    // first read completion. SEND rendezvous, so this also paces the test past bring-up.
    send(BLOCK_READY_ENDPOINT, file_system_contract::READY, 0, 0);
    // How many device flushes have completed. Answered back on every successful `block::FLUSH`, so a
    // client can tell a real round trip from a constant yes: see `file_system_contract::block::FLUSH`.
    let mut flushes: i64 = 0;
    loop {
        // RECEIVE_CAPABILITY: (first word, the Reply, second word = the starting block index). The Reply is
        // typed by the kernel's `x4` (milestone 706 (a `CALL` server can tell a Reply from a
        // delegation)): a SEND_CAPABILITY's capability is deleted, and a request with no Reply is served
        // with nobody to answer.
        let request = receive_request(BLOCK_REQUEST_ENDPOINT);
        let (first_word, block) = (request.w0, request.w1);
        let to = request.delivered.into_reply();
        // Refuse a count past the shared region: a request is never trusted to stay inside it just
        // because its packing could spell a larger count, and a clamped transfer would move fewer
        // blocks than asked without saying so.
        let Some(count) = block::request_block_count(first_word) else {
            if let Some(to) = to {
                reply(to, file_system_contract::error::word(22) as u64, 0); // EINVAL
            }
            continue;
        };
        let count = count as u64;
        let operation = file_system_contract::operation(first_word);
        let answer: i64 = match operation {
            block::READ | block::WRITE => {
                match transfer_first_sector(block, count, capacity_sectors) {
                    // EINVAL: some block of the request lies past the end of the disk.
                    None => file_system_contract::error::word(22),
                    Some(sector) => {
                        let done = if operation == block::READ {
                            block_read(direct_memory_access_physical_address, sector, count)
                        } else {
                            block_write(direct_memory_access_physical_address, sector, count)
                        };
                        // EIO: the device took the request and completed it with a non-OK status.
                        if done {
                            0
                        } else {
                            file_system_contract::error::word(5)
                        }
                    }
                }
            }
            block::GET_SIZE_BYTES => disk_size_bytes(capacity_sectors),
            // The one verb that can refuse for a reason outside this program. A device that never
            // offered the feature gets EOPNOTSUPP rather than a zero, because a zero here is a lie
            // about somebody's data; see `file_system_contract::block::FLUSH`.
            block::FLUSH => {
                if !can_flush {
                    -95 // EOPNOTSUPP: this device has no flush, and pretending otherwise is the bug
                } else if block_flush(direct_memory_access_physical_address) {
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
            reply(to, answer as u64, 0);
        }
    }
}

/// The first sector of a `count`-block transfer starting at filesystem block `block`, or `None`
/// when any block of it lies past a disk of `capacity_sectors` sectors, or when the arithmetic
/// would overflow. The block index arrives from a client, so nothing about it is trusted.
fn transfer_first_sector(block: u64, count: u64, capacity_sectors: u64) -> Option<u64> {
    let first = block.checked_mul(SECTORS_PER_BLOCK)?;
    let end = first.checked_add(count.checked_mul(SECTORS_PER_BLOCK)?)?;
    (count >= 1 && end <= capacity_sectors).then_some(first)
}

/// Complete a block-server request by waiting on the completion interrupt, not by polling the used
/// ring, with [`complete`]. `true` if the device completed with `VIRTIO_BLK_S_OK`. It does not
/// panic on another status: the long-lived block server's death would block every filesystem
/// client, so the verdict goes back to the one that asked. A WAIT per read fits the test watchdog,
/// since whole-block requests keep the mount's read count in the low hundreds. See nifeos/record:notes/virtual-io.md
/// ("A completion is the used ring advancing").
///
/// The wait has no bound, unlike a test role's. Giving up would leave the request on the ring with
/// its descriptors at head 0, and the next request would rewrite them while the device may still
/// read them; a server that stops answering is the safer failure.
fn complete_block_server_request(used_before: u16) -> bool {
    // QEMU completes inside `NOTIFY`, so the interrupt is usually already pending and the kernel's
    // pending-signal count (milestone 9 (a virtio-blk driver at EL0, and an interrupt becomes a
    // message)) returns this WAIT at once.
    complete(&BLOCK_QUEUE, used_before, None);
    block_status() == 0
}

/// Read `count` contiguous filesystem blocks starting at `sector` in a single virtio request,
/// straight into the data pages, which are the FS server's shared region: no per-block loop and no
/// copy (milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 4),
/// since a device round trip costs far more than the transfer. The caller has checked the range
/// with [`transfer_first_sector`]. `true` if the device reported success.
fn block_read(direct_memory_access_physical_address: u64, sector: u64, count: u64) -> bool {
    let used_before = submit_block_server_transfer(
        direct_memory_access_physical_address,
        sector,
        BLOCK_REQUEST_READ,
        count,
    );
    complete_block_server_request(used_before)
}

/// Write `count` contiguous filesystem blocks from the data pages (the FS server filled them before
/// this request) in a single virtio request. The one direction flag differs from the read; the
/// addresses are identical. `true` if the device reported success.
fn block_write(direct_memory_access_physical_address: u64, sector: u64, count: u64) -> bool {
    let used_before = submit_block_server_transfer(
        direct_memory_access_physical_address,
        sector,
        BLOCK_REQUEST_WRITE,
        count,
    );
    complete_block_server_request(used_before)
}

/// **Ask the device to make everything it has acknowledged durable**, and wait for it to say it
/// did. `true` if the device completed with `VIRTIO_BLK_S_OK`.
///
/// A status of 0 says the device finished its flush; it cannot prove the hardware beneath is
/// honest. A bad status such as `VIRTIO_BLK_S_UNSUPP` (2) is `false`, as for a transfer. See
/// nifeos/record:notes/virtual-io.md ("The block server").
fn block_flush(direct_memory_access_physical_address: u64) -> bool {
    let used_before = submit_flush(direct_memory_access_physical_address);
    complete_block_server_request(used_before)
}

/// A flush's **two**-descriptor chain: the 16-byte request header the device reads, and the status
/// byte it writes. No data descriptor, because a flush moves no data (virtio 1.2 §5.2.6.1); a chain
/// with one would be describing a transfer the device was never asked to make. The sector field is
/// unused for a flush and the specification says to set it to 0, so no stale sector from the
/// previous request is left in the header.
fn submit_flush(direct_memory_access_physical_address: u64) -> u16 {
    // As `submit_block_request`: the kernel validates every descriptor against the region.
    submit_block_request(
        direct_memory_access_physical_address,
        BLOCK_REQUEST_FLUSH,
        0,
        None,
    )
    .expect("the kernel refused the block server's flush, a server bug")
}

/// Submit a `count`-block transfer: as [`submit_block`], but with one data descriptor spanning
/// `count` contiguous blocks at [`BLOCK_SERVER_DATA_OFFSET_BYTES`] (milestone 138 step 4):
/// virtio-blk asks only that its length be whole sectors. Returns the used-ring index from before
/// the submit, which [`complete_block_server_request`] compares against.
fn submit_block_server_transfer(
    direct_memory_access_physical_address: u64,
    sector: u64,
    request_type: u32,
    count: u64,
) -> u16 {
    let data_bytes = count * block::BLOCK_BYTES as u64;
    submit_block_request(
        direct_memory_access_physical_address,
        request_type,
        sector,
        Some((BLOCK_SERVER_DATA_OFFSET_BYTES, data_bytes as u32)),
    )
    // The kernel validates every descriptor against the region before ringing; a refusal here is a
    // block-server bug, so fault rather than limp on.
    .expect("the kernel refused a block-server transfer, a server bug")
}

/// How many times [`consistent_read`] reads the configuration before it gives up on a device whose
/// generation never holds still. A device changes its configuration rarely (a resize), so a second
/// attempt nearly always succeeds; the rest bound a device that misbehaves.
const CONFIGURATION_READ_ATTEMPTS: u32 = 16;

/// Read a multi-word configuration value with `read`, bracketed by `generation`, until the
/// generation is the same before and after (virtio 1.2 section 2.5.1). `None` after
/// [`CONFIGURATION_READ_ATTEMPTS`] attempts that each saw it change. Takes closures so the
/// bracket can be tested without a device.
fn consistent_read(
    mut generation: impl FnMut() -> u32,
    mut read: impl FnMut() -> u64,
) -> Option<u64> {
    (0..CONFIGURATION_READ_ATTEMPTS).find_map(|_| {
        let before = generation();
        let value = read();
        (generation() == before).then_some(value)
    })
}

/// The disk's capacity in 512-byte sectors, from virtio-blk's 64-bit capacity field, read as two
/// 32-bit halves inside one configuration generation so a resize between them cannot pair one
/// capacity's low half with another's high half.
fn capacity_sectors() -> u64 {
    consistent_read(
        || read_device_register(CONFIGURATION_GENERATION_REGISTER),
        || {
            read_device_register(DEVICE_CONFIGURATION_REGISTER) as u64
                | ((read_device_register(DEVICE_CONFIGURATION_REGISTER + 4) as u64) << 32)
        },
    )
    .expect("the disk's configuration generation never held still across a capacity read")
}

/// The disk's size in bytes (sectors * 512), for `block::GET_SIZE_BYTES`. RedoxFS asks for this once, at open,
/// to size its allocator; the value is the whole extent the image lives in. A capacity too large to
/// state in a non-negative `i64` answers `i64::MAX` rather than overflow or turn negative, which a
/// client would read as an error.
fn disk_size_bytes(capacity_sectors: u64) -> i64 {
    capacity_sectors
        .saturating_mul(SECTOR_BYTES)
        .min(i64::MAX as u64) as i64
}

// The host tests run on the host only: the bare-metal targets the package also builds for have no
// libtest, and `--all-targets` would otherwise try to build the harness for them.
#[cfg(all(test, not(target_os = "none")))]
mod tests;

/// **Proofs of the pure logic the driver runs on bytes a device or a disk image wrote**: the
/// directory lookup over any bytes, the Internet Protocol header checksum over any header, and the
/// two bounded waits. The DHCP frame is a constant and the write pattern takes no outside input, so
/// host tests cover both (`tests.rs`). See nifeos/record:notes/verification.md.
///
/// Name: provisional.
#[cfg(kani)]
mod verification {
    use super::*;

    /// The longest directory slice the totality proof walks: the header and two entries, so a
    /// count past the slice, an entry cut short and a second entry are all reached.
    const DIRECTORY_BYTES: usize = nifefs::HEADER_LEN + 2 * nifefs::ENTRY_LEN;

    /// **The directory lookup is total**: for any directory bytes up to two entries long, cut at
    /// any length, and any name up to one byte longer than the name field, `find_file` returns
    /// without a panic or an out-of-bounds read, whatever count the header claims.
    /// Falsification: replayable `components/falsifications/block_driver_roles.verification.the_directory_lookup_is_total.patch`
    #[kani::proof]
    #[kani::unwind(36)]
    fn the_directory_lookup_is_total() {
        let directory: [u8; DIRECTORY_BYTES] = kani::any();
        let directory_length: usize = kani::any();
        kani::assume(directory_length <= DIRECTORY_BYTES);
        let name: [u8; nifefs::NAME_LEN + 1] = kani::any();
        let name_length: usize = kani::any();
        kani::assume(name_length <= nifefs::NAME_LEN + 1);
        let _ = find_file(&directory[..directory_length], &name[..name_length]);
    }

    /// **The lookup finds an entry exactly when its name field holds the name and then ends**, and
    /// returns that entry's start block: for one entry of any bytes and any name of up to four
    /// bytes, a hit means the field opens with the name and the next byte is padding, and a miss
    /// means it does not.
    /// Falsification: replayable `components/falsifications/block_driver_roles.verification.the_lookup_matches_a_whole_name_only.patch`
    #[kani::proof]
    #[kani::unwind(36)]
    fn the_lookup_matches_a_whole_name_only() {
        const SHORT: usize = 4;
        let mut directory = [0u8; nifefs::HEADER_LEN + nifefs::ENTRY_LEN];
        directory[8..12].copy_from_slice(&1u32.to_le_bytes());
        let entry: [u8; nifefs::ENTRY_LEN] = kani::any();
        directory[nifefs::HEADER_LEN..].copy_from_slice(&entry);
        let name: [u8; SHORT] = kani::any();
        let name_length: usize = kani::any();
        kani::assume(name_length <= SHORT);
        let name = &name[..name_length];

        let whole = entry[..name_length] == *name && entry[name_length] == 0;
        let start = u32::from_le_bytes(
            entry[nifefs::NAME_LEN..nifefs::NAME_LEN + 4]
                .try_into()
                .expect("four bytes"),
        );
        assert_eq!(find_file(&directory, name), whole.then_some(start));
    }

    /// **A header carrying its own checksum sums to zero**: for any 20-byte header, writing the
    /// checksum computed over it (with the checksum field zero) into that field makes the checksum
    /// of the result zero, the check a receiver makes.
    /// Falsification: replayable `components/falsifications/block_driver_roles.verification.a_header_carrying_its_checksum_sums_to_zero.patch`
    #[kani::proof]
    #[kani::unwind(11)]
    fn a_header_carrying_its_checksum_sums_to_zero() {
        let mut header: [u8; 20] = kani::any();
        header[10] = 0;
        header[11] = 0;
        let checksum = ip_checksum(&header);
        header[10..12].copy_from_slice(&checksum.to_be_bytes());
        assert_eq!(ip_checksum(&header), 0);
    }

    /// **A bounded completion wait takes at most its bound of wakeups**, returns true exactly when
    /// one of them saw the used index move, and takes all of them when none did: for any bound up
    /// to four and any sequence of used indices.
    /// Falsification: replayable `components/falsifications/block_driver_roles.verification.a_bounded_wait_stops_at_its_bound.patch`
    #[kani::proof]
    #[kani::unwind(6)]
    fn a_bounded_wait_stops_at_its_bound() {
        let bound: u32 = kani::any();
        kani::assume(bound <= 4);
        let used_before: u16 = kani::any();
        let mut wakeups = 0u32;
        let mut saw_a_move = false;
        let moved = used_ring_moved(Some(bound), used_before, || {
            wakeups += 1;
            let used: u16 = kani::any();
            saw_a_move |= used != used_before;
            used
        });
        assert!(wakeups <= bound);
        assert_eq!(moved, saw_a_move);
        if !moved {
            assert_eq!(wakeups, bound);
        }
    }

    /// **A bracketed configuration read returns only a value read inside one generation**: for
    /// any generation and value sequences, a returned value is one whose read had the same
    /// generation before and after it.
    /// Falsification: replayable `components/falsifications/block_driver_roles.verification.a_configuration_read_holds_inside_one_generation.patch`
    #[kani::proof]
    #[kani::unwind(18)]
    fn a_configuration_read_holds_inside_one_generation() {
        let mut generations = [0u32; 2];
        let mut reads = 0u32;
        let mut last_value = 0u64;
        let mut generation_calls = 0u32;
        let read = consistent_read(
            || {
                let generation: u32 = kani::any();
                generations[(generation_calls % 2) as usize] = generation;
                generation_calls += 1;
                generation
            },
            || {
                reads += 1;
                last_value = kani::any();
                last_value
            },
        );
        assert!(reads <= CONFIGURATION_READ_ATTEMPTS);
        if let Some(value) = read {
            assert_eq!(value, last_value);
            assert_eq!(generations[0], generations[1]);
        }
    }
}

// The direct grant's transport stays inline, so this crate root declares no child module but its
// tests (§282 (when a file is split, the parent becomes an index)).
/// **The direct grant's transport**: the driver drives its virtio-pci function's registers
/// itself, from the pages its starter mapped, and the IOMMU (Input-Output Memory Management
/// Unit), not the kernel, confines what the device reaches. A driver granted mediated never
/// enters this module.
///
/// The rest of the crate speaks virtio-mmio's register vocabulary, the one the kernel's
/// `VirtualIo` capability answers. On a direct grant `read`, `write`, `setup_queue` and
/// `notify` answer the same vocabulary from the common configuration, the ISR (Interrupt
/// Status Register) byte and the doorbells, as the kernel's own `Transport::Pci` does for a
/// mediated function (`kernel/src/virtual_io.rs`), so every driver role runs unchanged on either
/// grant. The rings sit at the offsets the mediated grant uses, in the driver's own region, and
/// the device reads them there. See
/// nifeos/record:design/roadmap/1062-the-device-control-planes-leave-the-kernel.md, "Step 7".
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport, after the
/// grant it serves.
pub mod direct {

    use core::sync::atomic::{AtomicU64, Ordering};

    use driver_process_contract::virtual_io::{DATA_PLANE_KIND, HandOff, REGISTER_KIND};

    // The mmio offsets the rest of the crate reads and writes (virtio 1.2 section 4.2.2).
    use super::{
        CONFIGURATION_GENERATION_REGISTER, DEVICE_CONFIGURATION_REGISTER, DEVICE_FEATURES_REGISTER,
        DEVICE_FEATURES_SELECT_REGISTER, DRIVER_FEATURES_REGISTER, DRIVER_FEATURES_SELECT_REGISTER,
        INTERRUPT_STATUS_REGISTER, MAGIC_VALUE_REGISTER, STATUS_REGISTER,
    };

    // The common configuration's registers (virtio 1.2 section 4.1.4.3).
    const COMMON_DEVICE_FEATURE_SELECT: u64 = 0x00;
    const COMMON_DEVICE_FEATURE: u64 = 0x04;
    const COMMON_DRIVER_FEATURE_SELECT: u64 = 0x08;
    const COMMON_DRIVER_FEATURE: u64 = 0x0c;
    const COMMON_CONFIGURATION_INTERRUPT_VECTOR: u64 = 0x10;
    const COMMON_DEVICE_STATUS: u64 = 0x14;
    #[doc(alias = "config_generation")]
    const COMMON_CONFIGURATION_GENERATION: u64 = 0x15;
    const COMMON_QUEUE_SELECT: u64 = 0x16;
    const COMMON_QUEUE_ENTRIES_REGISTER: u64 = 0x18;
    const COMMON_QUEUE_INTERRUPT_VECTOR: u64 = 0x1a;
    const COMMON_QUEUE_ENABLE: u64 = 0x1c;
    const COMMON_QUEUE_NOTIFY_OFF: u64 = 0x1e;
    const COMMON_QUEUE_DESCRIPTOR: u64 = 0x20;
    const COMMON_QUEUE_DRIVER: u64 = 0x28;
    const COMMON_QUEUE_DEVICE: u64 = 0x30;

    // The stride between queues' ring blocks in the DMA region, and each ring's offset in its
    // block: the layout the mediated grant's kernel uses (`moho_abi::virtual_io`), so a driver
    // role writes its descriptors at one place on either grant.
    use moho_abi::virtual_io::{
        AVAILABLE_RING_OFFSET_BYTES, DESCRIPTOR_TABLE_OFFSET_BYTES, RING_BLOCK_BYTES,
        USED_RING_OFFSET_BYTES,
    };

    /// The queues a driver role uses: a disk one, a NIC two.
    const QUEUES: usize = 2;

    /// The register window's base, zero while the grant is mediated.
    static REGISTERS: AtomicU64 = AtomicU64::new(0);
    /// The DMA region's base, zero while the grant is mediated.
    static REGION: AtomicU64 = AtomicU64::new(0);
    /// The three handoff words, [`HandOff::pack`]'s.
    static WORDS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
    /// Each queue's doorbell, as a virtual address, zero until [`setup_queue`] resolves it.
    static DOORBELLS: [AtomicU64; QUEUES] = [const { AtomicU64::new(0) }; QUEUES];

    /// **What a direct grant gives the driver to run**: its role, and its DMA region's physical and
    /// virtual bases.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
    #[derive(Debug, Clone, Copy)]
    pub struct Granted {
        /// The role to run, in the driver's own numbering.
        pub role: u64,
        /// The DMA region's physical base, which every descriptor names.
        pub physical_address: u64,
        /// Where the region is mapped.
        pub virtual_address: u64,
    }

    /// **Take a direct grant, if this process holds one**: the startup page lists a register window
    /// and a DMA region, and `words` are a handoff whose structures fit the window. `None` for a
    /// process granted mediated, whose startup page lists no register window; a direct grant whose
    /// words do not unpack, or do not fit, panics, because no kernel sends them.
    ///
    /// Called once, first thing in `_start`, before any register access.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
    pub fn take(words: [u64; 3]) -> Option<Granted> {
        // SAFETY: every spawner maps a startup page at its address, read-only, for the process's
        // life; a process the kernel builds by hand is never this driver.
        let page = unsafe { startup_page_contract::StartupPage::mapped() }?;
        let registers = page.window(REGISTER_KIND)?;
        let region = page
            .window(DATA_PLANE_KIND)
            .expect("a direct grant lists its DMA region");
        // The DMA accessors check offsets against one page and rely on this base for alignment.
        assert!(
            region.pages >= 1
                && region
                    .virtual_address
                    .is_multiple_of(super::DIRECT_MEMORY_ACCESS_WINDOW_BYTES),
            "a direct grant's DMA region is page aligned and at least one page"
        );
        let hand_off = HandOff::unpack(words).expect("a direct grant's words unpack");
        assert!(
            hand_off.fits(registers.pages),
            "the handoff's structures fit the register window"
        );
        REGISTERS.store(registers.virtual_address, Ordering::Relaxed);
        REGION.store(region.virtual_address, Ordering::Relaxed);
        for (slot, word) in WORDS.iter().zip(words) {
            slot.store(word, Ordering::Relaxed);
        }
        Some(Granted {
            role: u64::from(hand_off.role),
            physical_address: hand_off.data_plane_physical_address,
            virtual_address: region.virtual_address,
        })
    }

    /// **Whether this process holds a direct grant.**
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
    pub fn is_direct() -> bool {
        REGISTERS.load(Ordering::Relaxed) != 0
    }

    /// The DMA region's virtual base on a direct grant, or `None` on a mediated one.
    pub(crate) fn region() -> Option<u64> {
        match REGION.load(Ordering::Relaxed) {
            0 => None,
            va => Some(va),
        }
    }

    fn hand_off() -> HandOff {
        let words = core::array::from_fn(|n| WORDS[n].load(Ordering::Relaxed));
        HandOff::unpack(words).expect("take() stored a handoff that unpacks")
    }

    /// One register of the window, by its byte offset there.
    fn at(offset: u64) -> u64 {
        REGISTERS.load(Ordering::Relaxed) + offset
    }

    fn load<T: Copy>(offset: u64) -> T {
        // SAFETY: `take` checked the structures fit the window the starter mapped, device-typed and
        // read-write, for the process's life; every offset here is a structure base plus a register
        // inside that structure's specified layout.
        unsafe { core::ptr::read_volatile(at(offset) as *const T) }
    }

    fn store<T: Copy>(offset: u64, value: T) {
        // SAFETY: as `load`.
        unsafe { core::ptr::write_volatile(at(offset) as *mut T, value) }
    }

    /// **Answer a read in the mmio vocabulary**, from the common configuration, the ISR byte or the
    /// device configuration. Reading the ISR byte acknowledges the interrupt and deasserts the
    /// legacy pin interrupt.
    pub(crate) fn read(offset: u64) -> u32 {
        let h = hand_off();
        let common = u64::from(h.common);
        match offset {
            // virtio-pci has no magic register; answered so a driver's check means the same here.
            MAGIC_VALUE_REGISTER => 0x7472_6976,
            DEVICE_FEATURES_REGISTER => load::<u32>(common + COMMON_DEVICE_FEATURE),
            INTERRUPT_STATUS_REGISTER => u32::from(load::<u8>(u64::from(h.interrupt_status))),
            STATUS_REGISTER => u32::from(load::<u8>(common + COMMON_DEVICE_STATUS)),
            CONFIGURATION_GENERATION_REGISTER => {
                u32::from(load::<u8>(common + COMMON_CONFIGURATION_GENERATION))
            }
            o if o == DEVICE_CONFIGURATION_REGISTER || o == DEVICE_CONFIGURATION_REGISTER + 4 => {
                match h.device {
                    Some(device) => {
                        load::<u32>(u64::from(device) + (o - DEVICE_CONFIGURATION_REGISTER))
                    }
                    None => 0,
                }
            }
            _ => 0,
        }
    }

    /// **Answer a write in the mmio vocabulary.** The interrupt acknowledgement has no virtio-pci
    /// register: the ISR read already acknowledged.
    pub(crate) fn write(offset: u64, value: u32) {
        let common = u64::from(hand_off().common);
        match offset {
            DEVICE_FEATURES_SELECT_REGISTER => {
                store::<u32>(common + COMMON_DEVICE_FEATURE_SELECT, value);
            }
            DRIVER_FEATURES_SELECT_REGISTER => {
                store::<u32>(common + COMMON_DRIVER_FEATURE_SELECT, value);
            }
            DRIVER_FEATURES_REGISTER => store::<u32>(common + COMMON_DRIVER_FEATURE, value),
            STATUS_REGISTER => store::<u8>(common + COMMON_DEVICE_STATUS, value as u8),
            _ => {}
        }
    }

    /// **Set up queue `queue` with `size` entries**, its rings at the mediated layout's offsets in
    /// the driver's own region, aimed at `physical_address`'s pages. Writes the armed interrupt
    /// table entry into the queue's and the configuration's vector, resolves the queue's doorbell
    /// and enables it. Returns 0, or -1 for a queue past what this transport tracks or a device
    /// that refuses the vector.
    pub(crate) fn setup_queue(size: u64, queue: u64) -> i64 {
        if queue as usize >= QUEUES {
            return -1;
        }
        let h = hand_off();
        let common = u64::from(h.common);
        store::<u16>(common + COMMON_QUEUE_SELECT, queue as u16);
        store::<u16>(common + COMMON_QUEUE_ENTRIES_REGISTER, size as u16);
        if let Some(vector) = h.interrupt_vector {
            store::<u16>(common + COMMON_QUEUE_INTERRUPT_VECTOR, vector);
            store::<u16>(common + COMMON_CONFIGURATION_INTERRUPT_VECTOR, vector);
            if load::<u16>(common + COMMON_QUEUE_INTERRUPT_VECTOR) != vector {
                return -1;
            }
        }
        let block = h.data_plane_physical_address + queue * RING_BLOCK_BYTES;
        store::<u64>(
            common + COMMON_QUEUE_DESCRIPTOR,
            block + DESCRIPTOR_TABLE_OFFSET_BYTES,
        );
        store::<u64>(
            common + COMMON_QUEUE_DRIVER,
            block + AVAILABLE_RING_OFFSET_BYTES,
        );
        store::<u64>(common + COMMON_QUEUE_DEVICE, block + USED_RING_OFFSET_BYTES);
        let notify_offset = load::<u16>(common + COMMON_QUEUE_NOTIFY_OFF);
        DOORBELLS[queue as usize].store(at(h.doorbell(notify_offset)), Ordering::Relaxed);
        store::<u16>(common + COMMON_QUEUE_ENABLE, 1);
        0
    }

    /// **Ring queue `queue`'s doorbell**: one store, no system call. Returns 0, or -1 for a queue
    /// [`setup_queue`] has not resolved.
    pub(crate) fn notify(queue: u64) -> i64 {
        let Some(doorbell) = DOORBELLS
            .get(queue as usize)
            .map(|d| d.load(Ordering::Relaxed))
        else {
            return -1;
        };
        if doorbell == 0 {
            return -1;
        }
        // The ring writes before the doorbell, to the device and not only to other CPUs.
        user_mode_runtime::virtual_io::doorbell_barrier();
        // SAFETY: `setup_queue` resolved the doorbell inside the notify structure, which `take`
        // checked lies in the device-typed window.
        unsafe { core::ptr::write_volatile(doorbell as *mut u16, queue as u16) };
        0
    }
}
