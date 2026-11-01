//! The committed fuzz seed is the input that found the no-root-node bug, and it must stay refused.
//!
//! `fuzz/seeds/device_tree_patch_initial_ramdisk/no_root_node.dtb` is the 8043-byte input on which
//! `with_initial_ramdisk` returned `Ok` for a tree the kernel then found no archive in: a valid
//! header whose structure block, declared 7144 bytes long, begins with `END`. The unit tests build
//! that shape by hand; this holds the bytes the fuzzer produced. See notes/fuzzing/targets.md.

use device_tree_blob::DeviceTreeBlob;
use device_tree_patch::{Error, output_len, with_initial_ramdisk};

const NO_ROOT_NODE: &[u8] =
    include_bytes!("../../../fuzz/seeds/device_tree_patch_initial_ramdisk/no_root_node.dtb");

/// The seed is a tree both readers accept as far as the header, so it reaches the walker.
#[test]
fn the_seed_reaches_the_walker() {
    assert_eq!(NO_ROOT_NODE.len(), 8043);
    assert!(output_len(NO_ROOT_NODE).is_ok());
    assert_eq!(
        DeviceTreeBlob::from_bytes(NO_ROOT_NODE)
            .unwrap()
            .initial_ramdisk(),
        Ok(None),
        "the kernel's reader finds no archive in it"
    );
}

/// The rewrite refuses it, for any range, rather than report a tree with no `/chosen`.
#[test]
fn the_seed_is_refused() {
    let mut out = vec![0u8; output_len(NO_ROOT_NODE).unwrap()];
    for (start, end) in [(0x4800_0000, 0x4849_0000), (0, 1), (0x1000, 0x1000)] {
        assert_eq!(
            with_initial_ramdisk(NO_ROOT_NODE, start, end, &mut out),
            Err(Error::Malformed)
        );
    }
}
