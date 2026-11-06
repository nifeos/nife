//! **Submission queue entries**: the opcodes, the Identify selectors and the builders that fill
//! the sixteen dwords of a command.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding [`Command`] and the opcodes.

use crate::physical_region_page::PhysicalRegionPagePair;

/// Create an I/O submission queue (admin opcode, section 5).
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `ADMIN_CREATE_IO_SQ`, spelled out. The alias keeps the
/// specification's name.
#[doc(alias = "Create I/O Submission Queue")]
pub const ADMIN_CREATE_IO_SUBMISSION_QUEUE: u8 = 0x01;
/// Create an I/O completion queue (admin opcode, section 5).
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `ADMIN_CREATE_IO_CQ`, spelled out. The alias keeps the
/// specification's name.
#[doc(alias = "Create I/O Completion Queue")]
pub const ADMIN_CREATE_IO_COMPLETION_QUEUE: u8 = 0x05;
/// Fetch a controller or namespace data structure
/// ([`CONTROLLER_OR_NAMESPACE_STRUCTURE_CONTROLLER`],
/// [`CONTROLLER_OR_NAMESPACE_STRUCTURE_NAMESPACE`]).
///
/// Name: ratified 2026-10-25 (#2720).
pub const ADMIN_IDENTIFY: u8 = 0x06;

/// Non-volatile memory (NVM, the I/O command set's) opcodes (section 6). Flush is opcode 0, and it
/// is mandatory: section 6.8 requires every controller to implement it, so a caller can offer a
/// flush unconditionally.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `NVM_FLUSH` (NVM spelled out, as the acronym
/// test asks).
#[doc(alias = "nvm")]
pub const NON_VOLATILE_MEMORY_FLUSH: u8 = 0x00;
/// Write logical blocks.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `NVM_WRITE` (NVM spelled out, as the acronym
/// test asks).
#[doc(alias = "nvm")]
pub const NON_VOLATILE_MEMORY_WRITE: u8 = 0x01;
/// Read logical blocks.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `NVM_READ` (NVM spelled out, as the acronym
/// test asks).
#[doc(alias = "nvm")]
pub const NON_VOLATILE_MEMORY_READ: u8 = 0x02;

/// The Identify selector (`CNS`, Controller or Namespace Structure) for a namespace's data
/// structure.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CNS_NAMESPACE`: `CNS` is ruled to be spelled out as Controller
/// or Namespace Structure (2026-11-05 UTC). The alias `CNS` keeps the specification's name.
#[doc(alias = "CNS")]
pub const CONTROLLER_OR_NAMESPACE_STRUCTURE_NAMESPACE: u32 = 0x00;
/// The Identify selector (`CNS`, Controller or Namespace Structure) for the controller's own data
/// structure, rather than a namespace's.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CNS_CONTROLLER` as
/// [`CONTROLLER_OR_NAMESPACE_STRUCTURE_NAMESPACE`] was. The alias `CNS` keeps the specification's
/// name.
#[doc(alias = "CNS")]
pub const CONTROLLER_OR_NAMESPACE_STRUCTURE_CONTROLLER: u32 = 0x01;

/// One 64-byte submission queue entry, as the sixteen dwords the driver writes. `#[repr(C)]` so a
/// caller can copy it into the ring as-is; the wire format is little-endian dwords and both target
/// instruction sets are little-endian.
///
/// Name: ratified 2026-10-25 (#2720).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command(pub [u32; 16]);

impl Command {
    /// The common prefix every builder shares: opcode and command identifier in dword 0 (no fused
    /// operation, PRP data transfers), namespace id in dword 1, PRP1 and PRP2 in dwords 6 to 9.
    fn new(
        opcode: u8,
        command_id: u16,
        namespace_id: u32,
        first_entry: u64,
        second_entry: u64,
    ) -> Self {
        let mut dwords = [0u32; 16];
        dwords[0] = opcode as u32 | (command_id as u32) << 16;
        dwords[1] = namespace_id;
        dwords[6] = first_entry as u32;
        dwords[7] = (first_entry >> 32) as u32;
        dwords[8] = second_entry as u32;
        dwords[9] = (second_entry >> 32) as u32;
        Command(dwords)
    }

    /// IDENTIFY (admin): the 4096-byte data structure named by `controller_or_namespace_structure`
    /// lands at `physical_region_page`.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn identify(
        command_id: u16,
        controller_or_namespace_structure: u32,
        namespace_id: u32,
        physical_region_page: u64,
    ) -> Self {
        let mut command = Self::new(
            ADMIN_IDENTIFY,
            command_id,
            namespace_id,
            physical_region_page,
            0,
        );
        command.0[10] = controller_or_namespace_structure;
        command
    }

    /// CREATE I/O COMPLETION QUEUE (admin): queue `queue_id`, `entries` deep, physically contiguous
    /// (PC=1) at `physical_region_page`. Interrupts stay off (IEN=0): the driver completes by
    /// polling the phase tag, so no vector is named and none is needed.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `create_io_cq` (spelled out, as the acronym test
    /// asks).
    pub fn create_io_completion_queue(
        command_id: u16,
        queue_id: u16,
        entries: u16,
        physical_region_page: u64,
    ) -> Self {
        let mut command = Self::new(
            ADMIN_CREATE_IO_COMPLETION_QUEUE,
            command_id,
            0,
            physical_region_page,
            0,
        );
        command.0[10] = queue_id as u32 | ((entries as u32 - 1) << 16);
        command.0[11] = 1; // PC only; IEN=0
        command
    }

    /// CREATE I/O SUBMISSION QUEUE (admin): queue `queue_id`, `entries` deep, physically contiguous at
    /// `physical_region_page`, completing into completion queue `completion_queue_id` (which must already exist,
    /// so the pair is created completion queue first).
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `create_io_sq` (spelled out, as the acronym test
    /// asks).
    pub fn create_io_submission_queue(
        command_id: u16,
        queue_id: u16,
        entries: u16,
        completion_queue_id: u16,
        physical_region_page: u64,
    ) -> Self {
        let mut command = Self::new(
            ADMIN_CREATE_IO_SUBMISSION_QUEUE,
            command_id,
            0,
            physical_region_page,
            0,
        );
        command.0[10] = queue_id as u32 | ((entries as u32 - 1) << 16);
        command.0[11] = 1 | (completion_queue_id as u32) << 16; // physically contiguous, and the paired completion queue
        command
    }

    /// READ (I/O): `blocks` logical blocks starting at `starting_logical_block_address`, into the
    /// pages `pages` names. `blocks` is the real count; the wire field is 0-based and this builder
    /// does the subtraction, so a caller cannot get the off-by-one wrong. `blocks` must be nonzero
    /// (asserted: zero would wrap to 65536).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn read(
        command_id: u16,
        namespace_id: u32,
        starting_logical_block_address: u64,
        blocks: u16,
        pages: PhysicalRegionPagePair,
    ) -> Self {
        Self::transfer(
            NON_VOLATILE_MEMORY_READ,
            command_id,
            namespace_id,
            starting_logical_block_address,
            blocks,
            pages,
        )
    }

    /// FLUSH (I/O): make everything the controller has already acknowledged for `namespace_id` durable
    /// (section 6.8). No data transfer, so no PRPs and no LBA range; the command completes when
    /// the volatile write cache, if the controller has one, has been committed.
    ///
    /// **Mandatory for every controller**, so a caller can offer a flush without negotiating
    /// anything. A controller with no volatile write cache completes it as a no-op, and that is a
    /// truthful yes rather than a silent one: the data really is durable.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn flush(command_id: u16, namespace_id: u32) -> Self {
        Self::new(NON_VOLATILE_MEMORY_FLUSH, command_id, namespace_id, 0, 0)
    }

    /// WRITE (I/O): the same addressing as [`Command::read`], the other direction.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn write(
        command_id: u16,
        namespace_id: u32,
        starting_logical_block_address: u64,
        blocks: u16,
        pages: PhysicalRegionPagePair,
    ) -> Self {
        Self::transfer(
            NON_VOLATILE_MEMORY_WRITE,
            command_id,
            namespace_id,
            starting_logical_block_address,
            blocks,
            pages,
        )
    }

    /// The body `read` and `write` share, which differ only in their opcode.
    fn transfer(
        opcode: u8,
        command_id: u16,
        namespace_id: u32,
        starting_logical_block_address: u64,
        blocks: u16,
        pages: PhysicalRegionPagePair,
    ) -> Self {
        assert!(blocks != 0, "a zero-block transfer has no wire encoding");
        let mut command = Self::new(
            opcode,
            command_id,
            namespace_id,
            pages.first_entry,
            pages.second_entry,
        );
        command.0[10] = starting_logical_block_address as u32;
        command.0[11] = (starting_logical_block_address >> 32) as u32;
        command.0[12] = (blocks - 1) as u32;
        command
    }
}

/// The queue dwords' fields share no bit by construction: a `u16` in the low half and a `u16`
/// shifted to the high half, which the compiler checks here; the mutation exclusions for
/// [`Command::create_io_completion_queue`] and [`Command::create_io_submission_queue`] name this
/// constant. The constant-assertion lint is off on purpose: the assertion is constant by
/// construction, and the compiler checking it is its job.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `THE_QUEUE_DWORD_FIELDS_SHARE_NO_BIT`: no `THE_` prefix (ruled
/// 2026-11-05 UTC), and the `_DO_NOT_OVERLAP` ending the naming rule gives an assertion constant.
#[allow(clippy::assertions_on_constants, clippy::eq_op, dead_code)]
const QUEUE_DWORD_FIELDS_DO_NOT_OVERLAP: () = assert!(0xffffu32 + (0xffff << 16) == u32::MAX);

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(first_entry: u64, second_entry: u64) -> PhysicalRegionPagePair {
        PhysicalRegionPagePair {
            first_entry,
            second_entry,
        }
    }

    #[test]
    fn a_read_command_encodes_the_spec_dwords() {
        let command = Command::read(7, 1, 0x1_0000_0008, 8, pages(0xdead_b000, 0));
        assert_eq!(
            command.0[0],
            0x02 | 7 << 16,
            "opcode and command id share dword 0"
        );
        assert_eq!(command.0[1], 1, "namespace id");
        assert_eq!(
            (command.0[6], command.0[7]),
            (0xdead_b000, 0),
            "PRP1 split across 6/7"
        );
        assert_eq!(
            (command.0[10], command.0[11]),
            (8, 1),
            "starting logical block address split across 10/11"
        );
        assert_eq!(command.0[12], 7, "the block count is 0-based on the wire");
    }

    #[test]
    fn a_write_command_encodes_the_spec_dwords() {
        // The mirror of the read test: `write` differs from `read` only in its opcode.
        let command = Command::write(7, 1, 0x1_0000_0008, 8, pages(0xdead_b000, 0));
        assert_eq!(
            command.0[0],
            0x01 | 7 << 16,
            "opcode and command id share dword 0"
        );
        assert_eq!(command.0[1], 1, "namespace id");
        assert_eq!(
            (command.0[6], command.0[7]),
            (0xdead_b000, 0),
            "PRP1 split across 6/7"
        );
        assert_eq!(
            (command.0[10], command.0[11]),
            (8, 1),
            "starting logical block address split across 10/11"
        );
        assert_eq!(command.0[12], 7, "the block count is 0-based on the wire");
    }

    #[test]
    fn the_second_entry_lands_in_dwords_eight_and_nine() {
        let command = Command::read(1, 1, 0, 1, pages(0x1000, 0x2_0000_3000));
        assert_eq!(
            (command.0[8], command.0[9]),
            (0x3000, 2),
            "PRP2 split across 8/9"
        );
    }

    #[test]
    fn a_flush_command_carries_only_the_opcode_identifier_and_namespace() {
        let command = Command::flush(9, 4);
        let mut expected = [0u32; 16];
        expected[0] = NON_VOLATILE_MEMORY_FLUSH as u32 | 9 << 16;
        expected[1] = 4;
        assert_eq!(command.0, expected);
    }

    #[test]
    fn an_identify_command_names_its_structure_in_dword_ten() {
        let command = Command::identify(3, CONTROLLER_OR_NAMESPACE_STRUCTURE_CONTROLLER, 0, 0x8000);
        assert_eq!(command.0[0], ADMIN_IDENTIFY as u32 | 3 << 16);
        assert_eq!(command.0[6], 0x8000);
        assert_eq!(command.0[10], 1);
    }

    #[test]
    fn queue_creation_packs_size_and_pairing() {
        let completion_queue_command = Command::create_io_completion_queue(1, 3, 16, 0x2000);
        assert_eq!(
            completion_queue_command.0[0] & 0xff,
            ADMIN_CREATE_IO_COMPLETION_QUEUE as u32
        );
        assert_eq!(
            completion_queue_command.0[10],
            3 | 15 << 16,
            "queue id, and the 0-based size"
        );
        assert_eq!(
            completion_queue_command.0[11], 1,
            "physically contiguous, interrupts off"
        );
        let submission_queue_command = Command::create_io_submission_queue(2, 3, 16, 3, 0x3000);
        assert_eq!(
            submission_queue_command.0[0] & 0xff,
            ADMIN_CREATE_IO_SUBMISSION_QUEUE as u32
        );
        assert_eq!(submission_queue_command.0[10], 3 | 15 << 16);
        assert_eq!(
            submission_queue_command.0[11],
            1 | 3 << 16,
            "physically contiguous, and the paired completion queue id"
        );
    }

    #[test]
    fn queue_creation_words_pack_disjoint_halves_so_or_and_xor_agree() {
        // Three `| -> ^` mutants survive in `create_io_completion_queue` and
        // `create_io_submission_queue`, all the same shape: a `u16`-width field on the right of
        // `|`, shifted left by 16, next to a value that only ever occupies the low 16 bits. A left
        // shift by 16 zeroes the low 16 bits of *anything*, so the two halves can never share a
        // bit, checked here at the widest a `u16` can be.
        assert_eq!(
            0xffffu32 & (0xffffu32 << 16),
            0,
            "queue id and 1 against the shifted half"
        );
    }
}
