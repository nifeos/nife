//! **Is this file on that disk?** The one device-path question the loader asks (the live stick
//! proposal's G1, `design/roadmap/0773-a-live-stick.md`).
//!
//! A UEFI device path is a run of nodes, each `type: u8, subtype: u8, length: u16` (little-endian,
//! header included) and then its data, ending in the end-of-entire-path node (type `0x7F`, subtype
//! `0xFF`). A partition's path is its disk's with one `HARDDRIVE` node appended (recalled from the
//! UEFI specification; measured on OVMF in boot 3 of `cargo xtask install-boot`), so **"is this
//! partition on that disk" is "does the disk's path, without its end node, begin the
//! partition's"**. Comparing bytes is enough, because every node carries its own length: equal
//! first `n` bytes are the same first nodes, cut at the same boundaries. Nothing decodes a node.

//! # BUGS
//!
//! - **A firmware that builds a partition's path some other way** (a vendor node in the middle, a
//!   `MESSAGING` node rewritten between the disk and its partitions) would make every disk look
//!   foreign. The loader then boots the image in its own file and boots no slot, which is the safe
//!   direction: an installed machine on such a firmware would lose its rollback, not its boot.
//!   OVMF builds them the specified way; no other firmware has been measured.

/// The end-of-entire-path node's type and subtype.
const END: (u8, u8) = (0x7F, 0xFF);

/// The longest path walked before giving up. Real ones are under a hundred bytes; the bound is what
/// keeps a corrupt length from walking the firmware's memory.
pub const MAX_LEN: usize = 1024;

/// **How many bytes of nodes come before the end node**, reading each 4-byte header through
/// `header_at(offset)`. `None` for a malformed path: a node shorter than its own header, a header
/// that cannot be read, or no end within [`MAX_LEN`].
///
/// A closure rather than a slice because the caller holds a firmware pointer of unknown length, and
/// reading past the end node to make a slice would read memory the firmware never promised.
pub fn length(mut header_at: impl FnMut(usize) -> Option<[u8; 4]>) -> Option<usize> {
    let mut at = 0;
    while at <= MAX_LEN {
        let h = header_at(at)?;
        if (h[0], h[1]) == END {
            return Some(at);
        }
        let len = usize::from(u16::from_le_bytes([h[2], h[3]]));
        if len < 4 {
            return None;
        }
        at += len;
    }
    None
}

/// **Is the medium at `own` on the disk at `disk`?** Both are a path's nodes without the end node,
/// as [`length`] measures them. True for a partition of that disk and for the disk itself (a stick
/// formatted without a partition table); false for an empty disk path, which names nothing.
pub fn is_on(own: &[u8], disk: &[u8]) -> bool {
    !disk.is_empty() && own.starts_with(disk)
}

/// **Does this path pass through an NVMe namespace?** `path` is nodes without the end node, as
/// [`length`] measures them. The node is messaging (type 3), subtype `0x17`.
///
/// It is how the loader says `boot_slot::medium::NVME`: the kernel's internal disk is the NVMe one,
/// so a file on an NVMe namespace is a boot from that disk.
pub fn is_nvme(path: &[u8]) -> bool {
    let mut at = 0;
    while let Some(h) = path.get(at..at + 4) {
        if (h[0], h[1]) == (0x03, 0x17) {
            return true;
        }
        let len = usize::from(u16::from_le_bytes([h[2], h[3]]));
        if len < 4 {
            return false;
        }
        at += len;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One node: header, then `data`.
    fn node(kind: u8, sub: u8, data: &[u8]) -> Vec<u8> {
        let mut n = vec![kind, sub];
        n.extend_from_slice(&((4 + data.len()) as u16).to_le_bytes());
        n.extend_from_slice(data);
        n
    }

    fn pci(dev: u8) -> Vec<u8> {
        node(0x01, 0x01, &[0, dev])
    }

    fn nvme() -> Vec<u8> {
        node(0x03, 0x17, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    }

    fn harddrive(n: u8) -> Vec<u8> {
        node(0x04, 0x01, &[n; 38])
    }

    fn end() -> Vec<u8> {
        node(0x7F, 0xFF, &[])
    }

    fn measure(path: &[u8]) -> Option<usize> {
        length(|at| path.get(at..at + 4).map(|h| [h[0], h[1], h[2], h[3]]))
    }

    fn path(nodes: &[Vec<u8>]) -> Vec<u8> {
        nodes.concat()
    }

    #[test]
    fn a_partition_is_on_its_own_disk_and_not_on_another() {
        let nvme_disk = path(&[pci(3), nvme(), end()]);
        let esp = path(&[pci(3), nvme(), harddrive(2), end()]);
        let stick = path(&[pci(0x1f), node(0x03, 0x12, &[0; 6]), end()]);
        let stick_esp = path(&[pci(0x1f), node(0x03, 0x12, &[0; 6]), harddrive(1), end()]);

        let n = |p: &Vec<u8>| p[..measure(p).unwrap()].to_vec();
        assert!(is_on(&n(&esp), &n(&nvme_disk)));
        // The case G1 is about: a file on the stick is not on the disk that carries the slots.
        assert!(!is_on(&n(&stick_esp), &n(&nvme_disk)));
        assert!(is_on(&n(&stick_esp), &n(&stick)));
        // A stick with no partition table: the file's medium is the disk itself.
        assert!(is_on(&n(&stick), &n(&stick)));
    }

    /// The same bytes in the data of a different node must not match: lengths are in the bytes
    /// compared, so a prefix cannot end partway through a node.
    #[test]
    fn a_disk_whose_path_differs_only_in_node_data_is_another_disk() {
        let a = path(&[pci(3), nvme()]);
        let b = path(&[pci(4), nvme()]);
        let own = path(&[pci(4), nvme(), harddrive(1)]);
        assert!(!is_on(&own, &a));
        assert!(is_on(&own, &b));
    }

    #[test]
    fn an_nvme_partition_is_nvme_and_a_stick_is_not() {
        let esp = path(&[pci(3), nvme(), harddrive(2)]);
        let stick = path(&[pci(0x1f), node(0x03, 0x12, &[0; 6]), harddrive(1)]);
        assert!(is_nvme(&esp));
        assert!(!is_nvme(&stick));
        // NVMe's subtype under another type is not NVMe.
        assert!(!is_nvme(&node(0x01, 0x17, &[0; 4])));
        assert!(!is_nvme(&[]));
    }

    #[test]
    fn an_empty_disk_path_names_nothing() {
        assert!(!is_on(&path(&[pci(3)]), &[]));
    }

    #[test]
    fn malformed_paths_are_refused_rather_than_walked() {
        // A node claiming to be shorter than its own header would never advance.
        assert_eq!(measure(&[0x01, 0x01, 0x02, 0x00]), None);
        // No end node before the data runs out.
        assert_eq!(measure(&pci(3)), None);
        // No end node within the bound.
        let long = path(&vec![pci(3); MAX_LEN / 6 + 2]);
        assert_eq!(measure(&[long, end()].concat()), None);
        // The empty path is just an end node.
        assert_eq!(measure(&end()), Some(0));
        // A node of only its header is the smallest there is, and is walked past.
        assert_eq!(measure(&path(&[node(0x01, 0x01, &[]), end()])), Some(4));
        assert!(is_nvme(&path(&[node(0x01, 0x01, &[]), nvme()])));
    }
}
