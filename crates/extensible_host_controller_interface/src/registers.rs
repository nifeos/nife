//! **Register offsets and the capability registers' facts** (xHCI 1.2 section 5).
//!
//! The register file has four parts, each found from the one before: the **capability** registers
//! at offset 0 (read-only facts about this controller), the **operational** registers at
//! `CAPLENGTH` (run, reset, ring bases, then one register set per port at 0x400), the **runtime**
//! registers at `RTSOFF` (one set per interrupter, from 0x20), and the **doorbell array** at
//! `DBOFF` (one 32-bit doorbell per device slot, slot 0 being the command ring's).
//!
//! Name: ratified 2026-10-26 (UTC), on #2760. Refused `regs` (an abbreviation).

/// **Capability Registers Length** (`CAPLENGTH`, xHCI specification), the capability register
/// whose low byte is how many bytes the capability registers take, so where the operational
/// registers start; the dword read here carries `HCIVERSION` in its high half.
///
/// Name: provisional. The specification's term spelled out with `_BYTES`, because a length carries
/// its unit. Refused `CAPLENGTH` (the specification's mnemonic, kept as the alias),
/// `CAPABILITY_REGISTERS_LENGTH_REGISTER` (a length without its unit).
#[doc(alias = "CAPLENGTH")]
pub const CAPABILITY_REGISTERS_LENGTH_BYTES_REGISTER: u64 = 0x00;
/// **Structural Parameters 1** (`HCSPARAMS1`, xHCI specification), a capability register: slots,
/// interrupters, ports.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `HCSPARAMS1` (the specification's mnemonic,
/// kept as the alias).
#[doc(alias = "HCSPARAMS1")]
pub const STRUCTURAL_PARAMETERS_1_REGISTER: u64 = 0x04;
/// **Structural Parameters 2** (`HCSPARAMS2`, xHCI specification), a capability register: the
/// scratchpad count, among others.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `HCSPARAMS2` (the specification's mnemonic,
/// kept as the alias).
#[doc(alias = "HCSPARAMS2")]
pub const STRUCTURAL_PARAMETERS_2_REGISTER: u64 = 0x08;
/// **Capability Parameters 1** (`HCCPARAMS1`, xHCI specification), a capability register: the
/// context size and the extended capabilities pointer, among others.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `HCCPARAMS1` (the specification's mnemonic,
/// kept as the alias).
#[doc(alias = "HCCPARAMS1")]
pub const CAPABILITY_PARAMETERS_1_REGISTER: u64 = 0x10;
/// **Doorbell Offset** (`DBOFF`, xHCI specification), a capability register: the doorbell array's
/// offset.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `DBOFF` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "DBOFF")]
pub const DOORBELL_OFFSET_REGISTER: u64 = 0x14;
/// **Runtime Register Space Offset** (`RTSOFF`, xHCI specification), a capability register: the
/// runtime registers' offset.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `RTSOFF` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "RTSOFF")]
pub const RUNTIME_REGISTER_SPACE_OFFSET_REGISTER: u64 = 0x18;

/// **USB Command** (`USBCMD`, xHCI specification), an operational register, from
/// [`CAPABILITY_REGISTERS_LENGTH_BYTES_REGISTER`]'s offset: run, reset, interrupts.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `USBCMD` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "USBCMD")]
pub const USB_COMMAND_REGISTER: u64 = 0x00;
/// **USB Status** (`USBSTS`, xHCI specification), an operational register.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `USBSTS` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "USBSTS")]
pub const USB_STATUS_REGISTER: u64 = 0x04;
/// **Page Size** (`PAGESIZE`, xHCI specification), an operational register: a bitmap of the page
/// sizes the controller supports; bit 0 is 4 KiB.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `PAGESIZE` (the specification's mnemonic,
/// kept as the alias).
#[doc(alias = "PAGESIZE")]
pub const PAGE_SIZE_BITMAP_REGISTER: u64 = 0x08;
/// **Command Ring Control** (`CRCR`, xHCI specification), an operational register: the command
/// ring's base and cycle.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `CRCR` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "CRCR")]
pub const COMMAND_RING_CONTROL_REGISTER: u64 = 0x18;
/// **Device Context Base Address Array Pointer** (`DCBAAP`, xHCI specification), an operational
/// register: the physical address of the array that holds each device context's physical address.
///
/// Name: provisional. The specification's term spelled out, with `PHYSICAL` before the address the
/// array holds, since an address is virtual or physical. Refused `DCBAAP` (the specification's
/// mnemonic, kept as the alias), `DEVICE_CONTEXT_BASE_ADDRESS_ARRAY_POINTER_REGISTER` (an address
/// that does not say it is physical).
#[doc(alias = "DCBAAP")]
pub const DEVICE_CONTEXT_BASE_PHYSICAL_ADDRESS_ARRAY_POINTER_REGISTER: u64 = 0x30;
/// **Configure** (`CONFIG`, xHCI specification), an operational register whose low byte is how
/// many slots are enabled.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `CONFIG` (`config` expands; kept as the
/// alias), `CONFIGURATION` (a register offset ends `_REGISTER`).
#[doc(alias = "CONFIG")]
pub const CONFIGURATION_REGISTER: u64 = 0x38;
/// Port 1's **Port Status and Control** register (`PORTSC`, xHCI specification); port `n`'s is
/// `0x10 * (n - 1)` further.
#[doc(alias = "PORTSC")]
pub(crate) const FIRST_PORT_STATUS_AND_CONTROL_REGISTER: u64 = 0x400;

/// `USBCMD` Run/Stop.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `CMD_RUN` (spelled out).
pub const COMMAND_RUN: u32 = 1 << 0;
/// `USBCMD` Host Controller Reset.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `CMD_RESET` (spelled out).
pub const COMMAND_RESET: u32 = 1 << 1;
/// `USBCMD` Interrupter Enable.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `CMD_INTERRUPTS` (spelled out).
pub const COMMAND_INTERRUPTS: u32 = 1 << 2;

/// `USBSTS` `HCHalted`.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_HALTED` (spelled out).
pub const STATUS_HALTED: u32 = 1 << 0;
/// `USBSTS` Host System Error.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_HOST_SYSTEM_ERROR` (spelled out).
pub const STATUS_HOST_SYSTEM_ERROR: u32 = 1 << 2;
/// `USBSTS` Event Interrupt, write one to clear.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_EVENT_INTERRUPT` (spelled out).
pub const STATUS_EVENT_INTERRUPT: u32 = 1 << 3;
/// `USBSTS` Port Change Detect, write one to clear.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_PORT_CHANGE` (spelled out).
pub const STATUS_PORT_CHANGE: u32 = 1 << 4;
/// `USBSTS` Controller Not Ready.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_NOT_READY` (spelled out).
pub const STATUS_NOT_READY: u32 = 1 << 11;
/// `USBSTS` Host Controller Error.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STS_CONTROLLER_ERROR` (spelled out).
pub const STATUS_CONTROLLER_ERROR: u32 = 1 << 12;

/// Interrupter 0's register set, from [`RUNTIME_REGISTER_SPACE_OFFSET_REGISTER`]'s offset; its
/// first register is [`INTERRUPTER_MANAGEMENT_REGISTER`].
pub(crate) const INTERRUPTER_0_REGISTER: u64 = 0x20;
/// **Interrupter Management** (`IMAN`, xHCI specification), an interrupter register: pending and
/// enable.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `IMAN` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "IMAN")]
pub const INTERRUPTER_MANAGEMENT_REGISTER: u64 = 0x00;
/// **Interrupter Moderation** (`IMOD`, xHCI specification), an interrupter register: the
/// moderation interval.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `IMOD` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "IMOD")]
pub const INTERRUPTER_MODERATION_REGISTER: u64 = 0x04;
/// **Event Ring Segment Table Size** (`ERSTSZ`, xHCI specification), an interrupter register: how
/// many entries the event ring segment table holds.
///
/// Name: provisional. The specification's term spelled out with `_ENTRIES`, because a size
/// carries its unit. Refused `ERSTSZ` (the specification's mnemonic, kept as the alias),
/// `EVENT_RING_SEGMENT_TABLE_SIZE_REGISTER` (a size without its unit).
#[doc(alias = "ERSTSZ")]
pub const EVENT_RING_SEGMENT_TABLE_SIZE_ENTRIES_REGISTER: u64 = 0x08;
/// **Event Ring Segment Table Base Address** (`ERSTBA`, xHCI specification), an interrupter
/// register: the segment table's physical address.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `ERSTBA` (the specification's mnemonic, kept
/// as the alias), `EVENT_RING_SEGMENT_TABLE_BASE_ADDRESS` (an address says it is physical).
#[doc(alias = "ERSTBA")]
pub const EVENT_RING_SEGMENT_TABLE_BASE_PHYSICAL_ADDRESS_REGISTER: u64 = 0x10;
/// **Event Ring Dequeue Pointer** (`ERDP`, xHCI specification), an interrupter register: the
/// physical address of the next event the driver will read.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `ERDP` (the specification's mnemonic, kept
/// as the alias).
#[doc(alias = "ERDP")]
pub const EVENT_RING_DEQUEUE_POINTER_REGISTER: u64 = 0x18;

/// **Interrupt Pending** (`IMAN.IP`, xHCI specification), the bit of
/// [`INTERRUPTER_MANAGEMENT_REGISTER`] that says an interrupt is waiting; write one to clear.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `IMAN_PENDING` (the register spelled out).
#[doc(alias = "IMAN.IP")]
pub const INTERRUPTER_MANAGEMENT_PENDING: u32 = 1 << 0;
/// **Interrupt Enable** (`IMAN.IE`, xHCI specification), the bit of
/// [`INTERRUPTER_MANAGEMENT_REGISTER`] that lets this interrupter interrupt.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `IMAN_ENABLE` (the register spelled out).
#[doc(alias = "IMAN.IE")]
pub const INTERRUPTER_MANAGEMENT_ENABLE: u32 = 1 << 1;
/// **Event Handler Busy** (`ERDP.EHB`, xHCI specification), the bit of
/// [`EVENT_RING_DEQUEUE_POINTER_REGISTER`] the controller sets when it raises an interrupt; write
/// one to clear.
///
/// Name: ratified 2026-11-08 (UTC), on #3330. Refused `ERDP_BUSY` (the register spelled out).
#[doc(alias = "ERDP.EHB")]
pub const EVENT_RING_DEQUEUE_POINTER_BUSY: u64 = 1 << 3;

/// Extended capability id: USB Legacy Support, the firmware handoff (xHCI 1.2 section 7.1).
///
/// Name: ratified 2026-10-25 (#2720).
pub const EXTENDED_LEGACY_SUPPORT: u8 = 1;
/// `USBLEGSUP` HC BIOS Owned Semaphore.
///
/// Name: ratified 2026-10-25 (#2720).
pub const LEGACY_BIOS_OWNED: u32 = 1 << 16;
/// `USBLEGSUP` HC OS Owned Semaphore.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `LEGACY_OS_OWNED` (os spelled out, as the
/// tree-wide ruling asks).
pub const LEGACY_OPERATING_SYSTEM_OWNED: u32 = 1 << 24;
/// `USBLEGCTLSTS`, the dword after `USBLEGSUP`: the SMI enables (bits 0, 4, 13, 14, 15) to clear,
/// and the write-one-to-clear SMI events (29, 30, 31), so firmware stops being told about USB.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `LEGACY_SMI_ENABLES` (`smi` spelled out, as
/// the acronym test asks).
pub const LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_ENABLES: u32 = 1 | 1 << 4 | 0b111 << 13;

/// The three SMI-enable bits share no bit (0, 4 and 13:15), which the compiler checks here; the
/// mutation exclusions for this file's constants name this constant. The constant-assertion lint
/// is off on purpose: the assertion is constant by construction, and the compiler checking it is
/// its job.
///
/// Name: ratified 2026-11-03 (UTC), #3117. The compile-time check that the named bit groups do
/// not overlap.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
pub const LEGACY_ENABLE_BITS_DO_NOT_OVERLAP: () =
    assert!(1 + (1 << 4) + (0b111 << 13) == LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_ENABLES);

/// The scratchpad count's two fields share no bit (the high field shifted to 5:9, the low field
/// at 0:4), which the compiler checks here; the mutation exclusion for
/// [`Capabilities::decode`] names this constant. The constant-assertion lint is off on purpose:
/// the assertion is constant by construction, and the compiler checking it is its job.
///
/// Name: ratified 2026-11-03 (UTC), #3117. The compile-time check that the named bit groups do
/// not overlap.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
pub const SCRATCHPAD_FIELDS_DO_NOT_OVERLAP: () = assert!(((0x1fu32 << 5) & 0x1f) == 0);

/// The write-one-to-clear event bits of `USBLEGCTLSTS`.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `LEGACY_SMI_EVENTS` (`smi` spelled out, as
/// the acronym test asks).
pub const LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_EVENTS: u32 = 0b111 << 29;

/// **The facts the capability registers state**, decoded.
///
/// Name: ratified 2026-10-25 (#2720).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// `CAPLENGTH`: where the operational registers start.
    pub operational: u64,
    /// `HCIVERSION`, in binary-coded decimal (0x0100 for 1.0).
    pub version: u16,
    /// How many device slots the controller has.
    pub maximum_slots: u8,
    /// How many interrupters.
    pub maximum_interrupters: u16,
    /// How many root hub ports.
    pub maximum_ports: u8,
    /// How many scratchpad pages the controller wants for itself.
    pub scratchpads: u16,
    /// How many bytes one context is: 32, or 64 when `HCCPARAMS1.CSZ` is set.
    pub context_bytes: u64,
    /// Where the extended capabilities list starts, in bytes, or 0 for none.
    pub extended: u64,
    /// Where the doorbell array starts.
    pub doorbells: u64,
    /// Where the runtime registers start.
    pub runtime: u64,
}

impl Capabilities {
    /// Decode the six capability registers, read as dwords at [`CAPABILITY_REGISTERS_LENGTH_BYTES_REGISTER`],
    /// [`STRUCTURAL_PARAMETERS_1_REGISTER`], [`STRUCTURAL_PARAMETERS_2_REGISTER`],
    /// [`CAPABILITY_PARAMETERS_1_REGISTER`], [`DOORBELL_OFFSET_REGISTER`] and
    /// [`RUNTIME_REGISTER_SPACE_OFFSET_REGISTER`]. Total, because every one of them
    /// is the hardware's word and a value this driver cannot use is caught by whoever uses it.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn decode(
        capability_registers_length: u32,
        structural_parameters_1: u32,
        structural_parameters_2: u32,
        capability_parameters_1: u32,
        doorbell_offset: u32,
        runtime_register_space_offset: u32,
    ) -> Capabilities {
        Capabilities {
            operational: (capability_registers_length & 0xff) as u64,
            version: (capability_registers_length >> 16) as u16,
            maximum_slots: structural_parameters_1 as u8,
            maximum_interrupters: ((structural_parameters_1 >> 8) & 0x7ff) as u16,
            maximum_ports: (structural_parameters_1 >> 24) as u8,
            scratchpads: (((structural_parameters_2 >> 21) & 0x1f) << 5
                | (structural_parameters_2 >> 27)) as u16,
            context_bytes: if capability_parameters_1 & (1 << 2) != 0 {
                64
            } else {
                32
            },
            extended: ((capability_parameters_1 >> 16) as u64) * 4,
            doorbells: (doorbell_offset & !0x3) as u64,
            runtime: (runtime_register_space_offset & !0x1f) as u64,
        }
    }

    /// Port `port_number`'s **Port Status and Control** register (`PORTSC`, xHCI specification),
    /// `port_number` from 1, as an offset into the register file.
    ///
    /// Name: ratified 2026-11-08 (UTC), on #3330. Refused `portsc` (the specification's mnemonic,
    /// kept as the alias).
    #[doc(alias = "PORTSC")]
    pub const fn port_status_and_control(&self, port_number: u8) -> u64 {
        self.operational
            + FIRST_PORT_STATUS_AND_CONTROL_REGISTER
            + 0x10 * (port_number as u64).saturating_sub(1)
    }

    /// Interrupter 0's register at `register_offset` ([`INTERRUPTER_MANAGEMENT_REGISTER`],
    /// [`EVENT_RING_DEQUEUE_POINTER_REGISTER`], ...), as an offset into the file.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn interrupter(&self, register_offset: u64) -> u64 {
        self.runtime + INTERRUPTER_0_REGISTER + register_offset
    }

    /// Slot `slot`'s doorbell (0 for the command ring), as an offset into the file.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn doorbell(&self, slot: u8) -> u64 {
        self.doorbells + 4 * slot as u64
    }
}

/// **Find an extended capability** with id `id`, walking the list from `first` (the byte offset
/// [`Capabilities::extended`] gives) through a `read` of dwords at byte offsets. `None` when the
/// list has no such entry, ends, or leaves `limit` bytes.
///
/// Bounded at 64 hops and by `limit`, for `pci::msix_capability`'s reason: the list is a linked list in
/// a device's register file, so a broken controller must end the walk, not hang the kernel that is
/// walking it.
///
/// Name: ratified 2026-10-25 (#2720).
pub fn find_extended(
    first: u64,
    limit: u64,
    id: u8,
    read: &mut dyn FnMut(u64) -> u32,
) -> Option<u64> {
    let mut at = first;
    for _ in 0..64 {
        if at == 0 || at.checked_add(4)? > limit {
            return None;
        }
        let head = read(at);
        if head as u8 == id {
            return Some(at);
        }
        let next = u64::from((head >> 8) & 0xff) * 4;
        if next == 0 {
            return None;
        }
        at = at.checked_add(next)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qemus_capability_registers_decode() {
        let capabilities =
            Capabilities::decode(0x0100_0040, 0x0800_0840, 0, 0x0008_7000, 0x2000, 0x1000);
        assert_eq!(capabilities.operational, 0x40);
        assert_eq!(capabilities.version, 0x0100);
        assert_eq!(capabilities.maximum_slots, 64);
        assert_eq!(capabilities.maximum_interrupters, 8);
        assert_eq!(capabilities.maximum_ports, 8);
        assert_eq!(capabilities.scratchpads, 0);
        assert_eq!(capabilities.context_bytes, 32);
        assert_eq!(capabilities.extended, 0x20); // the dword count in HCCPARAMS1's high half, times four
        assert_eq!(capabilities.port_status_and_control(1), 0x440);
        assert_eq!(capabilities.port_status_and_control(8), 0x4b0);
        assert_eq!(
            capabilities.interrupter(EVENT_RING_DEQUEUE_POINTER_REGISTER),
            0x1038
        );
        // Slot 0 is the command ring's doorbell, so the stride is read at 0 as well as at 1.
        assert_eq!(capabilities.doorbell(0), 0x2000);
        assert_eq!(capabilities.doorbell(1), 0x2004);
        assert_eq!(capabilities.doorbell(2), 0x2008);
    }

    /// The scratchpad count is split across two fields, high bits first in the register but low
    /// bits first in meaning: getting the order wrong gives a controller 32 times too few pages.
    #[test]
    fn the_scratchpad_count_joins_its_two_fields() {
        let high = 0b00001 << 21; // one 32
        let low = 0b00011 << 27; // plus three
        let capabilities = Capabilities::decode(0x40, 0, high | low, 1 << 2, 0, 0);
        assert_eq!(capabilities.scratchpads, 35);
        assert_eq!(capabilities.context_bytes, 64);
    }

    /// The `USBCMD`, `USBSTS`, `IMAN`, `ERDP` and legacy support bits, pinned against xHCI 1.2
    /// sections 5.4.1, 5.4.2, 5.5.2 and 7.1, so a shift written the wrong way round fails here.
    #[test]
    fn the_register_bits_are_where_the_specification_puts_them() {
        assert_eq!([COMMAND_RUN, COMMAND_RESET, COMMAND_INTERRUPTS], [1, 2, 4]);
        assert_eq!(
            [
                STATUS_HALTED,
                STATUS_HOST_SYSTEM_ERROR,
                STATUS_EVENT_INTERRUPT,
                STATUS_PORT_CHANGE
            ],
            [1, 4, 8, 0x10]
        );
        assert_eq!([STATUS_NOT_READY, STATUS_CONTROLLER_ERROR], [0x800, 0x1000]);
        assert_eq!(
            [
                INTERRUPTER_MANAGEMENT_PENDING,
                INTERRUPTER_MANAGEMENT_ENABLE
            ],
            [1, 2]
        );
        assert_eq!(EVENT_RING_DEQUEUE_POINTER_BUSY, 8);
        assert_eq!(
            [LEGACY_BIOS_OWNED, LEGACY_OPERATING_SYSTEM_OWNED],
            [0x1_0000, 0x100_0000]
        );
        assert_eq!(LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_ENABLES, 0xe011);
        assert_eq!(LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_EVENTS, 0xe000_0000);
    }

    /// The next pointer counts dwords, and an entry whose last byte is the limit's is still read.
    #[test]
    fn the_next_pointer_is_in_dwords_and_the_limit_is_inclusive() {
        let mut read = |at: u64| match at {
            0x100 => 2 | 1 << 8,
            0x104 => 1,
            _ => 0,
        };
        assert_eq!(find_extended(0x100, 0x1000, 1, &mut read), Some(0x104));
        assert_eq!(find_extended(0x100, 0x108, 1, &mut read), Some(0x104));
    }

    #[test]
    fn the_extended_list_is_walked_and_bounded() {
        // Two entries: id 2 (supported protocol) at 0x100 pointing 0x10 on, id 1 at 0x110.
        let mut read = |at: u64| match at {
            0x100 => 2 | 4 << 8,
            0x110 => 1,
            _ => 0,
        };
        assert_eq!(find_extended(0x100, 0x1000, 1, &mut read), Some(0x110));
        assert_eq!(find_extended(0x100, 0x1000, 10, &mut read), None);
        assert_eq!(find_extended(0x100, 0x110, 1, &mut read), None);
        // A self-loop (next = 0 hops is "end", so a loop needs a cycle of two).
        let mut looping = |at: u64| {
            if at == 0x100 {
                2 | 1 << 8
            } else {
                2 | 0xff << 8
            }
        };
        assert_eq!(find_extended(0x100, u64::MAX, 1, &mut looping), None);
    }
}
