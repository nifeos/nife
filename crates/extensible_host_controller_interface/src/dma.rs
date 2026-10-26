//! **The DMA region's layout**, in pages from its base, which the kernel allocates and confines and
//! the driver lays its structures out in. Both ends read these constants; nothing else joins them.
//!
//! The pages through [`SCRATCHPAD_ARRAY`] are the driver's own and are mapped into it. The
//! controller's scratchpad pages follow and are **not** mapped: the driver writes their physical addresses into
//! [`SCRATCHPAD_ARRAY`] and never touches them, so a process that cannot read the controller's
//! private memory cannot be confused by it. The IOMMU confines the device to the whole region.

/// The device context base address array: one pointer per slot, entry 0 the scratchpad array.
pub const DEVICE_CONTEXT_ARRAY: u64 = 0;
/// The command ring.
pub const COMMAND_RING: u64 = 1;
/// The event ring's one segment.
pub const EVENT_RING: u64 = 2;
/// The event ring segment table (one entry).
pub const EVENT_SEGMENT_TABLE: u64 = 3;
/// The input context, rewritten for each command that takes one.
pub const INPUT_CONTEXT: u64 = 4;
/// The output device context of the one slot this driver drives at a time.
pub const DEVICE_CONTEXT: u64 = 5;
/// Endpoint 0's transfer ring.
pub const CONTROL_RING: u64 = 6;
/// The keyboard's interrupt IN transfer ring.
pub const INTERRUPT_RING: u64 = 7;
/// The control transfers' data buffer: descriptors land here.
pub const CONTROL_BUFFER: u64 = 8;
/// The interrupt transfers' buffers: one [`REPORT_SLOT`]-byte slot per queued report.
pub const REPORT_BUFFERS: u64 = 9;
/// The scratchpad buffer array: one pointer per scratchpad page.
pub const SCRATCHPAD_ARRAY: u64 = 10;

/// How many TRBs a ring page holds, the last of a producer ring's being its link.
pub const RING_ENTRIES: u16 = 256;
/// The byte stride of one report buffer, a cache line so no two transfers share one.
pub const REPORT_SLOT: u64 = 64;
/// How many interrupt transfers the driver keeps queued, each into its own report slot. More than
/// one, so a key arriving while the driver handles the last does not wait for a requeue.
pub const REPORTS_QUEUED: u16 = 4;

/// The most scratchpad pages the scratchpad array's one page can name.
pub const MAX_SCRATCHPADS: u16 = 512;

// The queued reports fit their page, the ring holds every one of them and its link, and the
// scratchpad array's page names every scratchpad this layout accepts. Checked by the compiler, so
// an edit to any one constant that breaks the others fails the build.
const _: () = assert!(REPORTS_QUEUED as u64 * REPORT_SLOT <= crate::PAGE_BYTES);
const _: () = assert!(REPORTS_QUEUED < RING_ENTRIES - 1);
const _: () = assert!(MAX_SCRATCHPADS as u64 * 8 == crate::PAGE_BYTES);
