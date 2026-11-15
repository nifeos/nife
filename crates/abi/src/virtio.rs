/// `invoke(capability, READ_REGISTER, off, _, _)` -> register value. Reads are DMA-safe, so any register
/// in the device's own slot; an offset outside the slot, or not word-aligned, is `WrongObject`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`WRITE_REGISTER`]. Refused `READ_REG` (round 7's spelling;
/// `reg` is an abbreviation).
pub const READ_REGISTER: u64 = 0;
/// `invoke(capability, WRITE_REGISTER, off, val, _)` -> 0. Only DMA-*safe* registers (status, features,
/// interrupt-ack); the queue-address and notify registers are refused (they go through the
/// validated paths below).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`READ_REGISTER`]. Refused `WRITE_REG` (round 7's spelling).
pub const WRITE_REGISTER: u64 = 1;
/// `invoke(capability, SETUP_QUEUE, num, queue, _)` -> 0. The kernel programs the given queue's ring
/// addresses to the fixed offsets of that queue's ring block in the driver's DMA region, so the
/// driver never chooses them. `queue` selects the virtqueue (a virtio-net device uses receive =
/// 0, transmit = 1; the disk uses only queue 0, and passing 0 keeps its ABI byte-identical).
/// `WrongObject` when `queue` is out of range, the block does not fit the region, or the queue was never set up.
pub const SETUP_QUEUE: u64 = 2;
/// `invoke(capability, NOTIFY, queue, _, _)` -> 0, or `DeviceRefused` if a newly-published descriptor on
/// that queue points outside the driver's DMA region. On refusal the device is NOT told to go.
/// `queue` selects the virtqueue, as for `SETUP_QUEUE`; each queue keeps its own validated
/// high-water mark, so receive and transmit submits never interfere.
pub const NOTIFY: u64 = 3;

/// **Where a boot virtio device's DMA region says where it is**: the byte offset, inside the
/// region's first page, at which the kernel writes the region's own physical base as a
/// little-endian `u64` before it grants the region to anyone.
///
/// No method translates a virtual address and a supervisor building a driver has no spare `START`
/// word, so the page carries the base, in its last eight bytes because no driver's rings or buffers
/// reach them. The kernel writes it and the drivers that map the region read it, so it is one constant.
///
/// Name: provisional.
pub const DIRECT_MEMORY_ACCESS_REGION_BASE_PHYSICAL_ADDRESS_WORD_OFFSET: u64 = 4096 - 8;

// The word is the page's last eight bytes; a wrong operator in the subtraction would put it past
// the page or into a driver's rings, and nothing else runs the arithmetic.
const _: () = assert!(DIRECT_MEMORY_ACCESS_REGION_BASE_PHYSICAL_ADDRESS_WORD_OFFSET + 8 == 4096);
