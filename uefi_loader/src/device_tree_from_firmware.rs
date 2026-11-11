//! **The firmware's device tree, as a slice the loader may read**: the magic checked and the
//! header's `totalsize` held under `device_tree_blob::MAXIMUM_TOTALSIZE` before any slice of that
//! length exists.
//!
//! The firmware hands aarch64 and riscv64 a bare pointer in its configuration table, and the only
//! length is the one the tree states for itself. This is the one place the loader turns that
//! pointer into bytes, and it does it with the kernel's own reader, `DeviceTreeBlob::from_ptr`, so
//! the loader believes no tree the kernel would refuse. See notes/device-tree.md ("The parser's
//! bounds").
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane maintainer/device-tree-copy-checks, after
//! `device_tree_from_acpi` beside it.

use device_tree_blob::{DeviceTreeBlob, Error};

/// **The bytes of the device tree at `source`**, after `DeviceTreeBlob::from_ptr` has checked its
/// magic, bounded its `totalsize` by `device_tree_blob::MAXIMUM_TOTALSIZE` and parsed its header.
/// Refuses with the sentence for the firmware console.
///
/// # Safety
///
/// `source` must point at least 40 readable bytes (a device tree header's length) that stay valid
/// for `'a`, and, when they are a tree's header, at the `totalsize` bytes it states. That is the
/// promise UEFI makes for the table it installed under the device-tree GUID.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane maintainer/device-tree-copy-checks.
pub unsafe fn bytes<'a>(source: *const u8) -> Result<&'a [u8], &'static str> {
    // SAFETY: the caller's contract, which is `from_ptr`'s own.
    let tree = unsafe { DeviceTreeBlob::from_ptr(source) }.map_err(reason)?;
    // SAFETY: `from_ptr` accepted the tree, so `total_size` is its bounded `totalsize`, which the
    // caller's contract covers for `'a`.
    Ok(unsafe { core::slice::from_raw_parts(source, tree.total_size()) })
}

/// The firmware-console sentence for a tree `from_ptr` refused.
fn reason(error: Error) -> &'static str {
    match error {
        Error::BadMagic(_) => "the firmware's device tree has the wrong magic",
        Error::TooLarge(_) => "the firmware's device tree claims more than 2 MiB",
        Error::UnsupportedVersion(_) => "the firmware's device tree is a version too new to read",
        _ => "the firmware's device tree is truncated",
    }
}

#[cfg(test)]
mod tests {
    use device_tree_blob::MAXIMUM_TOTALSIZE;

    use super::*;

    const QEMU: &[u8] =
        include_bytes!("../../crates/device_tree_blob/tests/fixtures/qemu-aarch64-virt.dtb");

    #[test]
    fn a_good_tree_comes_back_at_the_length_it_states() {
        // SAFETY: `QEMU` is a whole tree in static memory.
        let got = unsafe { bytes(QEMU.as_ptr()) }.unwrap();
        assert_eq!(got.as_ptr(), QEMU.as_ptr());
        assert_eq!(got.len(), QEMU.len());
    }

    #[test]
    fn a_tree_with_the_wrong_magic_is_refused() {
        let mut tree = QEMU.to_vec();
        tree[0] ^= 0xff;
        // SAFETY: `tree` is longer than a header and outlives the call.
        let refused = unsafe { bytes(tree.as_ptr()) };
        assert_eq!(
            refused,
            Err("the firmware's device tree has the wrong magic")
        );
    }

    /// A `last_comp_version` past 17 says the tree cannot be read by a version 17 reader.
    #[test]
    fn a_tree_too_new_to_read_is_refused() {
        let mut tree = QEMU.to_vec();
        tree[24..28].copy_from_slice(&18u32.to_be_bytes());
        // SAFETY: `tree` is longer than a header and outlives the call.
        let refused = unsafe { bytes(tree.as_ptr()) };
        assert_eq!(
            refused,
            Err("the firmware's device tree is a version too new to read")
        );
    }

    /// The buffer is longer than the size claimed, so the slice the old copy would have built is
    /// owned memory and the test is sound whichever way it goes; the ceiling is what decides it.
    #[test]
    fn a_totalsize_past_the_ceiling_is_refused_before_a_slice_is_built() {
        let claimed = MAXIMUM_TOTALSIZE + 1;
        let mut tree = QEMU.to_vec();
        tree[4..8].copy_from_slice(&(claimed as u32).to_be_bytes());
        tree.resize(claimed + 64, 0);
        // SAFETY: `tree` is longer than `claimed` and outlives the call.
        let refused = unsafe { bytes(tree.as_ptr()) };
        assert_eq!(
            refused,
            Err("the firmware's device tree claims more than 2 MiB")
        );
    }
}
