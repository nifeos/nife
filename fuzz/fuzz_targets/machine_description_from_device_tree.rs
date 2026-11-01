//! Fuzz every consumer that reads machine facts out of a device tree, with arbitrary bytes.
//!
//! The device tree is written by firmware (QEMU, OpenSBI, U-Boot) and read by the kernel at boot,
//! before any process exists. `device_tree_blob_walk` reaches the blob reader with five fixed
//! needles; this target reaches what the kernel builds on top of it: the CPU list, the RISC-V ISA
//! record, the PLIC context map, the interrupt identifier of a node, and the JH7110 device finders.
//! Kani covers the ISA letters and the blob's leaf readers, not these walks, whose cell counts,
//! phandles and `reg` lengths are numbers the tree chooses.
//!
//! Beyond "no panic and no hang" it asserts the counts each result reports stay inside the arrays
//! behind them. Seeds are the `.dtb` fixtures of `device_tree_blob` and `machine_discovery`
//! (`script/fuzz` passes both directories). notes/fuzzing.md has the rest.

#![no_main]

use device_tree_blob::DeviceTreeBlob;
use libfuzzer_sys::fuzz_target;
use machine_discovery::cpu_list::{CpuList, MAXIMUM_CPU_NODES};
use machine_discovery::interrupt_id;
use machine_discovery::jh7110::{
    clock_and_reset, designware_ethernet, designware_mobile_storage, entropy,
};
use machine_discovery::plic::PlicContexts;
use machine_discovery::riscv64::Isa;

fuzz_target!(|data: &[u8]| {
    let Ok(dt) = DeviceTreeBlob::from_bytes(data) else {
        return;
    };

    if let Ok(list) = CpuList::from_device_tree(&dt) {
        assert!(list.len <= MAXIMUM_CPU_NODES);
        assert_eq!(list.cpus().len(), list.len);
        assert!(list.described >= list.len);
        assert_eq!(list.is_truncated(), list.described > list.len);
        for cpu in list.cpus() {
            let _ = cpu.is_startable();
        }
    }

    let _ = Isa::from_device_tree(&dt);

    if let Ok(contexts) = PlicContexts::from_device_tree(&dt) {
        assert!(contexts.len() <= machine_discovery::plic::MAXIMUM_CONTEXT_HARTS);
        for hart in 0..contexts.len().saturating_add(2) {
            let _ = contexts.s_context(hart);
        }
    }

    // Node names the kernel asks for, the shapes `interrupt-shapes` exercises, and the empty name.
    for node in [
        &b""[..],
        b"serial",
        b"uart",
        b"pl011",
        b"rtc",
        b"plic",
        b"virtio_mmio",
        b"ethernet",
    ] {
        let _ = interrupt_id::of_node(&dt, node);
    }

    let _ = clock_and_reset::discover(&dt);
    let _ = clock_and_reset::discover_aon(&dt);
    let _ = clock_and_reset::discover_sys(&dt);
    let _ = clock_and_reset::pmic_bus(&dt);
    let _ = designware_ethernet::discover(&dt);
    let _ = designware_ethernet::discover_aon_syscon(&dt);
    let _ = entropy::discover(&dt);
    for index in 0..3u8 {
        let _ = designware_mobile_storage::discover(&dt, index);
    }
});
