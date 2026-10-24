//! **The device windows**: the virtio-mmio bus q35 does not have, and the PCI BAR window
//! and interrupt base the arch contract names.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

// ---------------------------------------------------------------------------------------------
// The device windows. Both `virt` machines describe theirs in a device tree; q35 describes its PCIe
// window in ACPI's MCFG and has no virtio-mmio bus at all.
// ---------------------------------------------------------------------------------------------

/// **There is no virtio-mmio transport on q35.** Both `virt` machines put eight virtio devices on a
/// memory-mapped bus the kernel finds by walking fixed slots; a PC has never had such a thing, and
/// every virtio device on q35 is a PCI function. So this base is zero and [`VIRTIO_SLOTS`] is zero,
/// which is what makes `virtio::find_block_device` find nothing rather than probe an address that
/// answers with bus noise. The PCIe transport (DECISIONS §18) is the one that carries here, which is
/// the arrangement that transport was built for.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const VIRTIO_MMIO_BASE: u64 = 0;
/// Bytes between consecutive virtio-mmio transports. Meaningless with no bus; see
/// [`VIRTIO_MMIO_BASE`].
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const VIRTIO_SLOT_STRIDE: u64 = 0x1000;
/// **Zero virtio-mmio slots**, which is the fact, not a placeholder. See [`VIRTIO_MMIO_BASE`].
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const VIRTIO_SLOTS: u64 = 0;
/// How many slots the probe walks: none, see [`VIRTIO_SLOTS`]. A function because riscv64's answer
/// depends on the device tree.
pub const fn virtio_slots() -> u64 {
    VIRTIO_SLOTS
}
/// The interrupt the first virtio-mmio slot would raise. Unreachable with no slots.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const VIRTIO_IRQ_BASE: u32 = 0;

/// Bytes of PCI BAR space the kernel maps for device registers, matching what the other two
/// architectures reserve. 2 MiB covers every BAR QEMU hands out here; where the window goes is
/// [`memory_mapped_io_window()`](super::memory_mapped_io_window())'s answer. It stays small on
/// purpose: it holds only the BARs this kernel places (a BAR the machine placed is adopted where it
/// stands, `pci::place_bars`), and every byte of it is mapped with 4 KiB leaves at boot.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PCI_BAR_MAPPED: u64 = 0x20_0000;

/// The first interrupt a PCI function raises: zero, and nothing on this architecture reads it.
///
/// A function's interrupt is bound by `arch::x86_64::irq::alloc_msi_vector`, which hands
/// `kernel/src/pci.rs` the address and data for the function's MSI-X table, so there is no
/// board-specific routing to encode. q35's legacy INTx reaches an IO APIC input named only in AML,
/// which this tree cannot interpret. The constant survives because portable code names it; `pci.rs`
/// reaches it only when `alloc_msi_vector` answers `None`, which it never does here.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PCI_IRQ_BASE: u32 = 0;
