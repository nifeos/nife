//! **The bounds check and transfer builder** for one namespace.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding [`Namespace`].

use crate::command::Command;
use crate::physical_region_page::physical_region_page_pair;

/// **What a transfer needs to know about the namespace it addresses**: its capacity and how many
/// logical blocks one transfer unit covers. A driver fills it once from IDENTIFY
/// ([`crate::IdentifyNamespace::capacity_bytes`], [`crate::IdentifyNamespace::blocks_per_transfer`])
/// and asks it whether a block exists and for the command that moves one.
///
/// # Examples
///
/// ```
/// use non_volatile_memory_express::{NON_VOLATILE_MEMORY_READ, Namespace};
///
/// // An 8 MiB namespace of 512-byte logical blocks holds 2048 blocks of 4096 bytes, numbered 0 to
/// // 2047, and each one is 8 logical blocks.
/// let namespace = Namespace::new(8 << 20, 8);
/// assert!(namespace.holds_whole_block(2047, 4096));
/// assert!(!namespace.holds_whole_block(2048, 4096));
///
/// let command = namespace.transfer_command(7, 1, 2047, 4096, 0x4001_5000, 4096, false).unwrap();
/// assert_eq!(command.0[0] & 0xff, NON_VOLATILE_MEMORY_READ as u32);
/// assert!(namespace.transfer_command(7, 1, 2048, 4096, 0x4001_5000, 4096, false).is_none());
/// ```
///
/// Name: ratified 2026-11-03 (#3118). A namespace's capacity and its logical blocks per transfer,
/// what the bounds check and the transfer builder need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Namespace {
    /// The namespace's capacity in bytes, the word [`crate::IdentifyNamespace::capacity_bytes`]
    /// uses for the same quantity.
    ///
    /// Name: ratified 2026-11-03 (#3118).
    pub capacity_bytes: u64,
    /// The logical blocks in one transfer unit, the `unit` every call on this value passes.
    pub blocks_per_transfer: u16,
}

impl Namespace {
    /// A namespace of `capacity_bytes`, whose transfer unit covers `blocks_per_transfer` logical
    /// blocks. The struct is `#[non_exhaustive]`, so this is how code outside the crate builds one.
    ///
    /// Name: ratified 2026-11-06 (UTC). The constructor of a `#[non_exhaustive]` struct.
    pub fn new(capacity_bytes: u64, blocks_per_transfer: u16) -> Self {
        Namespace {
            capacity_bytes,
            blocks_per_transfer,
        }
    }

    /// **Is `block` a whole `unit`-byte block inside this namespace?** A driver's own bounds
    /// check, run before a command is built rather than left to the controller's status field,
    /// because a driver that asks for an LBA past the end learns about it far from where it
    /// computed it.
    ///
    /// `false` for any `block` whose transfer would run past [`Namespace::capacity_bytes`], for one
    /// whose end overflows a `u64`, and for a `unit` of zero.
    ///
    /// Name: ratified 2026-11-03 (#3118). The check is that the entire block fits (`end <= capacity`),
    /// not only that its start lies inside, hence `holds_whole_block`.
    pub fn holds_whole_block(&self, block: u64, unit: u64) -> bool {
        match block.checked_add(1).and_then(|n| n.checked_mul(unit)) {
            Some(end) => end <= self.capacity_bytes,
            None => false,
        }
    }

    /// **The NVM command for one whole-block transfer**, or `None` when the arithmetic will not
    /// close: a block outside the namespace ([`Namespace::holds_whole_block`]), an LBA that overflows,
    /// or a buffer that is not expressible as a PRP pair (which for a page-aligned `unit`-byte
    /// buffer with `unit <= 2 * page_size` it always is; see
    /// [`physical_region_page_pair`]).
    ///
    /// `cid` and `nsid` go to the command as they are, `data_physical_address` is the buffer's bus
    /// address, and `write` picks [`Command::write`] over [`Command::read`].
    ///
    /// Name: ratified 2026-11-03 (#3118). Ratified 2026-10-26 (UTC), on #2749, as a free function;
    /// the method on `Namespace` keeps the word.
    #[allow(clippy::too_many_arguments)]
    pub fn transfer_command(
        &self,
        cid: u16,
        nsid: u32,
        block: u64,
        unit: u64,
        data_physical_address: u64,
        page_size: u64,
        write: bool,
    ) -> Option<Command> {
        if !self.holds_whole_block(block, unit) {
            return None;
        }
        let starting_logical_block_address = block.checked_mul(self.blocks_per_transfer as u64)?;
        let pages = physical_region_page_pair(data_physical_address, unit, page_size)?;
        let blocks = self.blocks_per_transfer;
        Some(if write {
            Command::write(cid, nsid, starting_logical_block_address, blocks, pages)
        } else {
            Command::read(cid, nsid, starting_logical_block_address, blocks, pages)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::NON_VOLATILE_MEMORY_READ;

    /// The last block of the namespace is servable and the one after it is not, which is the
    /// check that keeps an out-of-range request from becoming a command the controller answers
    /// with a status the caller has to decode backwards.
    #[test]
    fn a_transfer_command_refuses_a_block_the_namespace_does_not_have() {
        const SIZE: u64 = 8 * 1024 * 1024;
        const LAST: u64 = SIZE / 4096 - 1;
        let namespace = Namespace::new(SIZE, 8);
        assert!(namespace.holds_whole_block(LAST, 4096));
        assert!(!namespace.holds_whole_block(LAST + 1, 4096));
        assert!(
            !namespace.holds_whole_block(u64::MAX, 4096),
            "no overflow, no wrap-around"
        );

        // And the command the last block produces addresses the LBA the geometry says it should.
        let command = namespace
            .transfer_command(7, 1, LAST, 4096, 0x4001_5000, 4096, false)
            .expect("the last block is servable");
        assert_eq!(command.0[0] & 0xff, NON_VOLATILE_MEMORY_READ as u32);
        assert_eq!(
            command.0[10] as u64 | (command.0[11] as u64) << 32,
            LAST * 8
        );
        assert!(
            namespace
                .transfer_command(7, 1, LAST + 1, 4096, 0x4001_5000, 4096, false)
                .is_none()
        );
    }
}
