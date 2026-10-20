/// `invoke(cap, READ_REGISTER, off, _, _)` -> register value. Reads are DMA-safe, so any register
/// in the device's own slot; an offset outside the slot, or not word-aligned, is `WrongObject`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`WRITE_REGISTER`]. His words: "Yes". Refused `READ_REG` (round 7's spelling;
/// `reg` is an abbreviation).
pub const READ_REGISTER: u64 = 0;
/// `invoke(cap, WRITE_REGISTER, off, val, _)` -> 0. Only DMA-*safe* registers (status, features,
/// interrupt-ack); the queue-address and notify registers are refused (they go through the
/// validated paths below).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`READ_REGISTER`]. His words: "Yes". Refused `WRITE_REG` (round 7's spelling).
pub const WRITE_REGISTER: u64 = 1;
/// `invoke(cap, SETUP_QUEUE, num, queue, _)` -> 0. The kernel programs the given queue's ring
/// addresses to the fixed offsets of that queue's ring block in the driver's DMA region, so the
/// driver never chooses them. `queue` selects the virtqueue (a virtio-net device uses receive =
/// 0, transmit = 1; the disk uses only queue 0, and passing 0 keeps its ABI byte-identical).
/// `BadQueue`/`WrongObject` if `queue` is out of range or the block does not fit the region.
pub const SETUP_QUEUE: u64 = 2;
/// `invoke(cap, NOTIFY, queue, _, _)` -> 0, or `DeviceRefused` if a newly-published descriptor on
/// that queue points outside the driver's DMA region. On refusal the device is NOT told to go.
/// `queue` selects the virtqueue, as for `SETUP_QUEUE`; each queue keeps its own validated
/// high-water mark, so receive and transmit submits never interfere.
pub const NOTIFY: u64 = 3;

/// **Where a boot virtio device's DMA region says where it is**: the byte offset, inside the
/// region's first page, at which the kernel writes the region's own physical base as a
/// little-endian `u64` before it grants the region to anyone.
///
/// Descriptors speak physical addresses and a process knows only virtual ones, and no method
/// translates one, deliberately. So a driver needs its region's physical base as a plain value,
/// and a supervisor that builds the driver has no spare `START` word to carry it in. The page
/// carries it instead. The last eight bytes of the first page, because no driver's rings or
/// control buffers reach them: `entropy` ends at `0x500`, `net_stack` at `0xF00`, `gpu_driver`
/// at `0x798` and `keyboard_driver` at `0x440`.
///
/// Shared by the kernel (which writes it), `system_initializer` (which reads it back for the
/// rng and the NIC) and `gpu_driver` and `keyboard_driver` (which read it themselves), so it is
/// a constant in a crate rather than a number in four places (AGENTS.md rule 7). Milestone 600
/// (provisional) moved it here; it was a number in two places before. Name: provisional.
pub const DMA_PHYS_OFFSET: u64 = 4096 - 8;

// The word is the page's last eight bytes, as the documentation says; a wrong operator in the
// subtraction put it past the page or in the middle of a driver's rings, and nothing ran the
// arithmetic. Mutation survivor triage, milestone 326 (turn a mutation score upward), batch 3.
const _: () = assert!(DMA_PHYS_OFFSET + 8 == 4096);
