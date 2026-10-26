//! The T-Head TH1520, on the host, before a byte of ours runs on one.
//!
//! Prep for milestone 89 (Scaleway EM-RV1: a second RISC-V implementation, rented). The RV1 is
//! rented by the hour, so everything the device tree can settle is settled here for free.
//! `fixtures/th1520.dts` is hand-written from Linux's `th1520.dtsi` and says where each fact came
//! from. The facts it cannot hold (the firmware's own tree, the OpenSBI version, whether the SBI
//! reset extension exists) are the first rented hour's work, listed in the milestone.

use device_tree_blob::{DeviceTreeBlob, Region};
use machine_discovery::plic::{COMPATIBLES, PlicContexts};
use machine_discovery::riscv64::*;

const TH1520: &[u8] = include_bytes!("fixtures/th1520.dtb");

fn tree(bytes: &[u8]) -> DeviceTreeBlob<'_> {
    DeviceTreeBlob::from_bytes(bytes).expect("fixture is a valid device tree")
}

/// **The PLIC is found by T-Head's binding, and its contexts read as 2h+1.** The node states
/// neither `sifive,plic-1.0.0` nor `riscv,plic0`; before `thead,c900-plic` joined
/// [`COMPATIBLES`] this returned an empty map and the kernel fell back to the formula without
/// knowing why. The formula happens to be right here, which is exactly why a silent fallback
/// would have gone unnoticed until a machine where it is not.
#[test]
fn the_th1520_plic_is_found_and_its_s_contexts_are_2h_plus_1() {
    let ctx = PlicContexts::from_device_tree(&tree(TH1520)).expect("the PLIC wiring parses");
    assert_eq!(ctx.len(), 4, "four C910 harts, each with an S context");
    for hart in 0..4 {
        assert_eq!(
            ctx.s_context(hart),
            Some(2 * hart + 1),
            "hart {hart}'s S context"
        );
    }
}

/// **The register block the kernel maps is found through the same list**, 40 bits up. The kernel's
/// `memory::initialize` walks [`COMPATIBLES`] with this call; the address is the reason
/// milestone 89 (Scaleway EM-RV1) needs a device window, because `KERNEL_VA_BASE + pa` cannot name
/// it under Sv39.
#[test]
fn the_th1520_plic_register_block_is_found_40_bits_up() {
    let dt = tree(TH1520);
    let mut plic = [Region { start: 0, size: 0 }; 1];
    let found = COMPATIBLES
        .iter()
        .any(|c| matches!(dt.node_reg_compatible(c, &mut plic), Ok(n) if n >= 1));
    assert!(
        found,
        "no compatible in the shared list names the TH1520's PLIC"
    );
    assert_eq!(plic[0].start, 0xff_d800_0000);
    assert_eq!(plic[0].size, 0x0100_0000);

    const SV39_HIGH_HALF: u64 = 1 << 38;
    assert!(
        plic[0].start >= SV39_HIGH_HALF,
        "the PLIC sits past the 256 GiB a pa + KERNEL_VA_BASE direct map can reach"
    );
}

/// **Four identical C910s, Sv39, and an extension this tree has no row for.** `xtheadvector` is
/// T-Head's RVV 0.7.1, not V; it must neither fail the parse nor turn into V. Sstc and Zicbom are
/// absent, which is why the timer stays on SBI TIME and DMA needs T-Head's own cache operations.
#[test]
fn four_c910s_parse_as_rv64gc_sv39_without_v_sstc_or_zicbom() {
    let cpu = Isa::from_device_tree(&tree(TH1520)).expect("the CPU nodes parse");
    assert_eq!(cpu.harts, 4);
    assert_eq!(cpu.described, 4);
    assert_eq!(cpu.base, Base::Rv64);
    assert_eq!(cpu.mmu, MmuType::Sv39);
    assert!(
        !cpu.legacy_isa_string,
        "the dtsi states riscv,isa-extensions"
    );
    assert!(!cpu.is_heterogeneous());
    assert!(
        cpu.common
            .contains(I.union(M).union(A).union(C).union(F).union(D))
    );
    assert!(!cpu.any.contains(V), "xtheadvector is not V");
    assert!(!cpu.any.contains(SSTC), "no Sstc: the timer is SBI TIME");
    assert!(
        !cpu.any.contains(ZICBOM),
        "no Zicbom: DMA needs th.dcache.*"
    );
}

/// **The console's interrupt is 36, read through a two-cell specifier.** The PLIC here states
/// `#interrupt-cells = <2>` (number, trigger), where QEMU's and the JH7110's state one. The second
/// cell must not be mistaken for the line.
#[test]
fn the_uart_interrupt_is_36_through_a_two_cell_specifier() {
    let interrupt_number =
        machine_discovery::interrupt_id::of_node(&tree(TH1520), b"serial@ffe7014000")
            .expect("the tree walks");
    assert_eq!(interrupt_number, Some(36));
}
