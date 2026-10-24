//! Fuzz the device-tree parser with arbitrary bytes, through every accessor the kernel calls on a
//! blob during boot.
//!
//! The device tree is parsed before anything else exists, on both ISAs, from bytes QEMU, OpenSBI or
//! a board's firmware wrote; a panic here is a kernel that cannot boot and cannot say why.
//! `crates/device_tree_blob`'s Kani harnesses prove the leaf readers (`be32`, `be64`) total but not
//! the walkers, whose depth counter, per-depth stacks and current-node slot are indexed by numbers
//! the blob controls. This target found an out-of-bounds index in `node_reg` within seconds of its
//! first run (`crates/device_tree_blob/tests/hostile.rs`). notes/fuzzing/targets.md has the rest.

#![no_main]

use device_tree_blob::{DeviceTreeBlob, Region};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(dtb) = DeviceTreeBlob::from_bytes(data) else {
        return;
    };

    // Small on purpose. A caller in the kernel passes a fixed array off its own stack, so the
    // interesting paths are the ones where the output runs out and the parser has to say
    // `TooManyRegions` instead of writing past the end.
    let mut out = [Region { start: 0, size: 0 }; 8];

    let _ = dtb.total_size();
    let _ = dtb.reserved_regions(&mut out);
    let _ = dtb.memory_regions(&mut out);
    let _ = dtb.reserved_memory_regions(&mut out);
    let _ = dtb.initial_ramdisk();

    // The three lookups that take a caller-supplied needle, with the arch layer's real needles
    // (`intc`/`plic`, `pl031`/`goldfish-rtc`, `virtio_mmio`). The empty prefix reaches the decode
    // path cheapest, since it matches the first node at depth >= 2, and for that reason it cannot
    // reach the deep-nesting bug: matching stops at the first candidate (notes/fuzzing.md's BUGS).
    for prefix in [&b""[..], b"intc", b"plic", b"memory", b"virtio_mmio"] {
        let _ = dtb.node_reg(prefix, &mut out);
        let _ = dtb.node_prop(prefix, b"reg");
        let _ = dtb.node_prop(prefix, b"compatible");
        // The inherited variant keeps a per-depth value stack the blob's nesting indexes, which is
        // the same shape as the `node_reg` cell stacks this target already caught a real
        // out-of-bounds in. `interrupts` beside `interrupt-parent` because that pair is the
        // kernel's real read (the console UART's line).
        let _ = dtb.node_prop_inherited(prefix, b"interrupt-parent");
        let _ = dtb.node_prop(prefix, b"interrupts");
    }
    // The phandle walk: whatever `interrupt-parent` bytes the blob offers, followed, plus a
    // constant needle so the walk runs even when the property is absent.
    if let Ok(Some(parent)) = dtb.node_prop_inherited(b"", b"interrupt-parent")
        && parent.len() >= 4
    {
        let ph = u32::from_be_bytes([parent[0], parent[1], parent[2], parent[3]]);
        let _ = dtb.phandle_prop(ph, b"#interrupt-cells");
    }
    let _ = dtb.phandle_prop(1, b"#interrupt-cells");
    for compat in [
        &b""[..],
        b"arm,pl031",
        b"google,goldfish-rtc",
        b"riscv,plic0",
    ] {
        let _ = dtb.node_reg_compatible(compat, &mut out);
    }

    // `Region::end()` is a `pub fn` on a `pub struct` whose fields came out of the blob, so it has
    // to hold on its own. Called on whatever the walkers actually produced rather than on invented
    // numbers, so a region the parser is willing to return is a region this is willing to add up.
    if let Ok(n) = dtb.memory_regions(&mut out) {
        for r in &out[..n] {
            let _ = r.end();
        }
    }
});
