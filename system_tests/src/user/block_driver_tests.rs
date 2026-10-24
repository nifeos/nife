//! `block_driver_tests`: the block driver's confinement and the PCIe transport: DMA
//! descriptors that escape the driver's region are refused, blocks are written and read back, and a
//! driver killed mid-write leaves the device sane.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use core::sync::atomic::Ordering;

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use super::test_programs::blk_image;
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use super::*;
#[cfg(target_arch = "aarch64")]
use crate::arch::exceptions::USER_FAULTS;
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use crate::sched;

/// **The DMA-confinement fix, end to end.** A malicious driver at EL0 holds a real `Virtio`
/// capability and its own DMA region, and points a descriptor at the kernel's image, asking
/// the device to write there. Because the device has no IOMMU, this would succeed if the
/// driver could ring it directly. The kernel validates every descriptor on submit and refuses
/// this one, so the device is never told to go and never touches the kernel. The driver
/// reports `1` when it was refused.
// RISC-V twin: `riscv_virtio_tests::the_kernel_refuses_a_dma_descriptor_that_escapes_the_drivers_region`. Gated here rather than run twice: that
// module drives the same property on the other instruction set, through the same `block_driver` and
// `net_stack` binaries this leg now uses, and a second copy would double the suite's slowest tests
// to prove nothing new; the two legs differ by ISA and by nothing else. See this module's comment
// on the two kinds of gate.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_kernel_refuses_a_dma_descriptor_that_escapes_the_drivers_region() {
    let Some(report) = virtio_service::start_attacker(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let refused = sched::ipc_receive(report)[0];
    assert_eq!(
        refused, 1,
        "a malicious driver's descriptor pointing at kernel memory was NOT refused: the \
         device could have DMA'd over the kernel",
    );
}

/// **The indirect-descriptor escape, end to end.** The direct-descriptor test above proves the
/// obvious case. This proves the subtle one: a driver that negotiates `INDIRECT_DESC` and
/// submits an indirect descriptor whose inner table (in its own region) aims the device at the
/// kernel. A validator that walked only the flat chain would pass the outer descriptor and let
/// the device follow the table out. The kernel strips the feature and refuses the flag, so the
/// device is never rung. The driver reports `1` when it was refused.
// RISC-V twin: `riscv_virtio_tests::the_kernel_refuses_an_indirect_descriptor_escape`. Gated here rather than run twice: that
// module drives the same property on the other instruction set, through the same `block_driver` and
// `net_stack` binaries this leg now uses, and a second copy would double the suite's slowest tests
// to prove nothing new; the two legs differ by ISA and by nothing else. See this module's comment
// on the two kinds of gate.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_kernel_refuses_an_indirect_descriptor_escape() {
    let Some(report) = virtio_service::start_attacker_indirect(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let refused = sched::ipc_receive(report)[0];
    assert_eq!(
        refused, 1,
        "an indirect descriptor whose inner table pointed at kernel memory was NOT refused: \
         the device could have followed it out of the driver's region",
    );
}

/// **The PCIe transport end to end** (DECISIONS §18 (the PCIe transport: one driver, two buses)):
/// the same driver reads the same file off the disk QEMU attached as `virtio-blk-pci`, found by
/// ECAM enumeration, BARs placed by the kernel, and the completion arriving as an interrupt the
/// kernel turned into a message. On aarch64 that is INTx through the GIC (SPI 3 + swizzle). On
/// `x86_64` it is an **MSI-X** message the device writes straight to the local APIC, because a
/// legacy INTx pin on `q35` goes through a PIRQ router only ACPI's `_PRT` describes (milestone 215
/// (a PCI function's interrupt reaches nothing on `x86_64`)). The assertion is the same either way:
/// a driver binds an intid and waits.
// RISC-V twin: `riscv_virtio_tests::a_userspace_driver_reads_a_file_over_the_pcie_transport`, the
// same property through the same `block_driver` and `net_stack` binaries; a second copy here would
// double the suite's slowest tests to prove nothing new. See this module's comment on the two
// kinds of gate.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test_case]
fn a_userspace_driver_reads_a_file_over_the_pcie_transport() {
    use crate::arch::exceptions::ROUTED_IRQS;

    // **The baseline is sampled before the device is wired.** The trap handler bumps
    // `ROUTED_IRQS` immediately *before* `sched::irq_notify`, and `Irq::WAIT` receives on the
    // endpoint notify signals, so a driver cannot report without this counter having moved, and
    // until `start_pci` runs there is nothing to count. The check guards that chain: a driver that
    // polls the used ring, or a handler that counts after `irq_notify`, breaks it and this fires,
    // where the first assertion alone would pass a polled driver. See
    // notes/load-sensitive-assertions.md.
    let irqs_before = ROUTED_IRQS.load(Ordering::Relaxed);

    let Some(report) = virtio_service::start_pci(blk_image()) else {
        crate::testing::skip!("no virtio-pci disk on the bus");
    };

    let word = sched::ipc_receive(report)[0];

    assert_eq!(
        &word.to_le_bytes(),
        b"nife: re",
        "the driver reported the wrong file contents over pci",
    );
    let irqs_after = ROUTED_IRQS.load(Ordering::Relaxed);
    assert!(
        irqs_after > irqs_before,
        "the read completed but the device's interrupt was never delivered to this kernel \
         ({irqs_before} routed before the device was wired, {irqs_after} after the report)",
    );
}

/// **A userspace driver writes a block and reads it back.** Milestone 32 phase 1: the write
/// verb, end to end, through the same validated transport as the read path. The driver
/// writes a pattern to the scratch block, wipes its buffer, reads the block back, verifies
/// every byte in-process, re-checks the superblock and directory around it, and reports the
/// read-back head. A matching report therefore certifies the round trip AND that the write
/// landed only on its own block.
// RISC-V twin: `riscv_virtio_tests::a_userspace_driver_writes_a_block_and_reads_it_back`. Gated here rather than run twice: that
// module drives the same property on the other instruction set, through the same `block_driver` and
// `net_stack` binaries this leg now uses, and a second copy would double the suite's slowest tests
// to prove nothing new; the two legs differ by ISA and by nothing else. See this module's comment
// on the two kinds of gate.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_userspace_driver_writes_a_block_and_reads_it_back() {
    let Some(report) = virtio_service::start_writer(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "the driver did not read back the pattern it wrote",
    );
}

/// The same write round trip over the PCIe transport (DECISIONS §18): the write verb must
/// hold on both buses, exactly as the read path does, or the transport seam has a
/// direction-shaped hole.
// RISC-V twin: `riscv_virtio_tests::a_userspace_driver_writes_a_block_over_the_pcie_transport`. Gated here rather than run twice: that
// module drives the same property on the other instruction set, through the same `block_driver` and
// `net_stack` binaries this leg now uses, and a second copy would double the suite's slowest tests
// to prove nothing new; the two legs differ by ISA and by nothing else. See this module's comment
// on the two kinds of gate.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test_case]
fn a_userspace_driver_writes_a_block_over_the_pcie_transport() {
    let Some(report) = virtio_service::start_writer_pci(blk_image()) else {
        crate::testing::skip!("no virtio-pci disk on the bus");
    };
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "the driver did not read back the pattern it wrote over pci",
    );
}

/// **A driver killed mid-write leaves the device and the transport sane.** Errors here eat
/// filesystems, so this is the write path's teardown proof: a driver submits a validated
/// write and dies (panics, is killed, is reaped) without ever collecting the completion,
/// acknowledging the interrupt, or advancing its ring bookkeeping. The device still owes a
/// completion into the dead driver's DMA region, which is safe precisely because that frame
/// is kernel-allocated and deliberately never reclaimed on thread death (`map_physical`'s
/// "Drop leaves it alone" rule): the DMA lands in memory the allocator never re-issued.
/// Then the full writer runs against the SAME device, resets it, and must complete its own
/// round trip, which proves the abandoned request wedged nothing: not the device, not the
/// validator's per-registration state, not the disk.
// RISC-V twin: `riscv_virtio_tests::a_driver_killed_mid_write_leaves_the_device_and_transport_sane`. Gated here rather than run twice: that
// module drives the same property on the other instruction set, through the same `block_driver` and
// `net_stack` binaries this leg now uses, and a second copy would double the suite's slowest tests
// to prove nothing new; the two legs differ by ISA and by nothing else. See this module's comment
// on the two kinds of gate.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_driver_killed_mid_write_leaves_the_device_and_transport_sane() {
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let Some(report) = virtio_service::start_write_abandoner(blk_image()) else {
        crate::testing::skip!("no virtio disk attached");
    };

    // 1 = the kernel validated the write and rang the device; the request is genuinely in
    // flight (or already complete) when the driver dies.
    assert_eq!(
        sched::ipc_receive(report)[0],
        1,
        "the abandoner never got its write submitted",
    );

    // The deliberate death: panic -> brk -> killed. Wait for the kill so the survivor below
    // runs against a device whose previous operator is really gone.
    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "the abandoner never died; nothing was killed mid-write",
    );

    // The survivor: the same full write-verify driver, same physical device. It must succeed
    // from a clean device reset, in-flight completion and all.
    let report = virtio_service::start_writer(blk_image())
        .expect("the disk vanished between the abandoner and the survivor");
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        &word.to_le_bytes(),
        b"CRKWRIT1",
        "after a mid-write kill, a fresh driver could not use the device",
    );
}
