//! The console UART's interrupt line, read from each machine's own tree, on the host.
//!
//! The same read answers 10 on QEMU riscv64's tree, 32 on both JH7110 fixtures, and 33 on the
//! aarch64 `virt` tree whose GIC speaks three-cell entries. The trees cover the three places
//! `interrupt-parent` lives: the serial node (QEMU riscv64), the root (QEMU aarch64), and `/soc`
//! (the mainline JH7110 dtsi). See `notes/machine-discovery.md` ("Interrupt ids").

use machine_discovery::interrupt_id;

const QEMU_RISCV_VIRT: &[u8] = include_bytes!("fixtures/qemu-riscv64-virt-smp4.dtb");
const JH7110: &[u8] = include_bytes!("fixtures/jh7110.dtb");
const JH7110_VENDOR: &[u8] = include_bytes!("fixtures/jh7110-vendor.dtb");
// The aarch64 machine's tree lives with the `device_tree_blob` crate's fixture tests; reuse it
// rather than committing a second copy that could drift from the one the parser is proven against.
const QEMU_AARCH64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-aarch64-virt.dtb");

/// The console node's name, as `kernel/src/console.rs` pins it: both QEMU riscv64 `virt` and the
/// JH7110 spell UART0 exactly this way.
const RISCV_UART: &[u8] = b"serial@10000000";

fn tree(bytes: &[u8]) -> device_tree_blob::DeviceTreeBlob<'_> {
    device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture is a valid device tree")
}

/// **QEMU `virt` says 10**, the number the old constant hardcoded: on the machine the constant
/// was written for, the tree and the constant agree, which is why the bug stayed invisible until
/// a board with a different answer ran the same kernel.
#[test]
fn qemu_riscv_virt_uart_is_plic_source_10() {
    let id = interrupt_id::of_node(&tree(QEMU_RISCV_VIRT), RISCV_UART).unwrap();
    assert_eq!(id, Some(10));
}

/// **Both JH7110 trees say 32**, the line boot 13 proved the kernel was not arming. The upstream
/// model states `interrupt-parent` once on `/soc` (inheritance), the vendor fixture on the serial
/// node itself; the answer must not depend on which spelling a board's firmware chose.
#[test]
fn jh7110_uart_is_plic_source_32_in_both_trees() {
    assert_eq!(
        interrupt_id::of_node(&tree(JH7110), RISCV_UART).unwrap(),
        Some(32),
        "the mainline-modeled tree, parent inherited from /soc"
    );
    assert_eq!(
        interrupt_id::of_node(&tree(JH7110_VENDOR), RISCV_UART).unwrap(),
        Some(32),
        "the vendor-modeled tree, parent on the node itself"
    );
}

/// **QEMU aarch64 `virt` says 33**: the pl011's `interrupts = <0 1 4>` is SPI 1 in the GIC's
/// three-cell form, and the INTID the GIC delivers is bank base 32 plus 1, exactly the number
/// the kernel has always hardcoded as `user::UART_RX_INTID`. The parent here is inherited from the
/// root, the third spelling.
#[test]
fn qemu_aarch64_virt_pl011_is_intid_33() {
    let id = interrupt_id::of_node(&tree(QEMU_AARCH64_VIRT), b"pl011@9000000").unwrap();
    assert_eq!(id, Some(33));
}

/// **A tree that does not say answers `None`, not a guess.** A node with no `interrupts` at all
/// (the memory node), and a node that wires its line through `interrupts-extended` instead (the
/// CLINT), both send the caller to its documented fallback rather than arming a number pulled
/// from the wrong property.
#[test]
fn a_tree_that_does_not_say_answers_none() {
    let dt = tree(QEMU_RISCV_VIRT);
    assert_eq!(interrupt_id::of_node(&dt, b"memory@").unwrap(), None);
    assert_eq!(interrupt_id::of_node(&dt, b"clint@").unwrap(), None);
    assert_eq!(interrupt_id::of_node(&dt, b"no-such-node@").unwrap(), None);
}

const INTERRUPT_SHAPES: &[u8] = include_bytes!("fixtures/interrupt-shapes.dtb");

/// **The GIC PPI bank folds like the SPI bank does**, base 16: the synthetic timer's
/// `<1 13 4>` is INTID 29, the shape every arm timer wires.
#[test]
fn a_ppi_entry_folds_into_its_bank() {
    let dt = tree(INTERRUPT_SHAPES);
    assert_eq!(interrupt_id::of_node(&dt, b"timer@").unwrap(), Some(29));
}

/// **Every malformed shape is a refusal, never a guess**: a PPI number past its 16-slot
/// bank, an interrupt type this decoder does not speak, a parent whose cell count it does
/// not speak, and an `interrupts` property shorter than that cell count. Each answers
/// `None` and sends the caller to its documented fallback.
#[test]
fn malformed_shapes_are_refused_not_guessed() {
    let dt = tree(INTERRUPT_SHAPES);
    assert_eq!(interrupt_id::of_node(&dt, b"badppi@").unwrap(), None);
    assert_eq!(interrupt_id::of_node(&dt, b"badtype@").unwrap(), None);
    assert_eq!(interrupt_id::of_node(&dt, b"oddcells@").unwrap(), None);
    assert_eq!(interrupt_id::of_node(&dt, b"short@").unwrap(), None);
}

/// **PPI 16 is refused, and 16 is the number that has to be got exactly right.** The bank holds
/// sixteen lines, 0 through 15, so 16 is the first one that is not a PPI; folded into the bank it
/// becomes INTID 32, which is SPI 0, a line a different device owns. `badppi@` above is 99, which
/// any reading of the bound turns away, so it proves the refusal happens without proving where.
#[test]
fn the_first_number_past_the_ppi_bank_is_refused() {
    let dt = tree(INTERRUPT_SHAPES);
    assert_eq!(interrupt_id::of_node(&dt, b"edgeppi@").unwrap(), None);
}
