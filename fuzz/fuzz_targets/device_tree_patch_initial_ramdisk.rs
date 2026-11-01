//! Fuzz the UEFI loader's device-tree rewriter, with arbitrary bytes as the firmware's tree.
//!
//! On aarch64 and riscv64 UEFI the loader copies the tree the firmware gave it with
//! `/chosen/linux,initrd-{start,end}` set, and hands the copy to the kernel
//! (`device_tree_patch::with_initial_ramdisk`). The rewriter has its own structure-block walker,
//! separate from `device_tree_blob`, so a tree it accepts is not thereby a tree the kernel's reader
//! has seen. The loader runs before the kernel: a panic here is a machine that never reaches it,
//! and a wrong `Ok` is a kernel that boots without its archive.
//!
//! The property: the rewrite either refuses, or its output parses with the kernel's reader and
//! `initial_ramdisk` reports exactly the start and end it was given (an empty or reversed range
//! reads back as no archive, which is what the reader defines). Also: a tree `output_len` refuses
//! is refused by the rewrite, the rewrite never writes more than `output_len`, and a buffer one byte
//! short of what it wrote is refused rather than overrun. notes/fuzzing/targets.md has the rest.

#![no_main]

use device_tree_blob::{DeviceTreeBlob, Region};
use device_tree_patch::{output_len, with_initial_ramdisk};
use libfuzzer_sys::fuzz_target;

/// The ramdisk ranges each input is rewritten with: a typical one, one hashed from the input so it
/// varies by input, the top of the address space, an empty range and a reversed one.
fn ranges(data: &[u8]) -> [(u64, u64); 5] {
    let h = data.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let start = h & !0xfff;
    [
        (0x4800_0000, 0x4849_0000),
        (start, start.saturating_add((h >> 40) & 0xff_ffff)),
        (u64::MAX - 0x1000, u64::MAX),
        (0x4800_0000, 0x4800_0000),
        (0x4849_0000, 0x4800_0000),
    ]
}

fuzz_target!(|data: &[u8]| {
    let Ok(len) = output_len(data) else {
        let mut out = [0u8; 64];
        assert!(
            with_initial_ramdisk(data, 0, 0x1000, &mut out).is_err(),
            "output_len refused a tree the rewrite accepted"
        );
        return;
    };
    // The header's sizes are the input's to choose, and the bound sums them.
    if len > 1 << 24 {
        return;
    }

    for (start, end) in ranges(data) {
        let mut out = vec![0u8; len];
        let Ok(n) = with_initial_ramdisk(data, start, end, &mut out) else {
            continue;
        };
        assert!(n <= len, "the rewrite wrote {n} bytes, past output_len {len}");

        let tree = DeviceTreeBlob::from_bytes(&out[..n])
            .unwrap_or_else(|e| panic!("the rewritten tree does not parse: {e:?}"));
        let expected = (end > start).then(|| Region {
            start,
            size: end - start,
        });
        assert_eq!(
            tree.initial_ramdisk(),
            Ok(expected),
            "the rewritten tree does not report the range it was given ({start:#x}..{end:#x})"
        );

        let mut short = vec![0u8; n - 1];
        assert!(
            with_initial_ramdisk(data, start, end, &mut short).is_err(),
            "a buffer one byte short of the output was accepted"
        );
    }
});
