//! **Register offsets and the capability registers' facts** (xHCI 1.2 section 5).
//!
//! The register file has four parts, each found from the one before: the **capability** registers
//! at offset 0 (read-only facts about this controller), the **operational** registers at
//! `CAPLENGTH` (run, reset, ring bases, then one register set per port at 0x400), the **runtime**
//! registers at `RTSOFF` (one set per interrupter, from 0x20), and the **doorbell array** at
//! `DBOFF` (one 32-bit doorbell per device slot, slot 0 being the command ring's).

/// Capability register: `CAPLENGTH` in the low byte, `HCIVERSION` in the high half.
pub const CAPLENGTH: u64 = 0x00;
/// Capability register: slots, interrupters, ports.
pub const HCSPARAMS1: u64 = 0x04;
/// Capability register: scratchpad count, among others.
pub const HCSPARAMS2: u64 = 0x08;
/// Capability register: context size, extended capabilities pointer, among others.
pub const HCCPARAMS1: u64 = 0x10;
/// Capability register: the doorbell array's offset.
pub const DBOFF: u64 = 0x14;
/// Capability register: the runtime registers' offset.
pub const RTSOFF: u64 = 0x18;

/// Operational register (from `CAPLENGTH`): `USBCMD`.
pub const USBCMD: u64 = 0x00;
/// Operational register: `USBSTS`.
pub const USBSTS: u64 = 0x04;
/// Operational register: `PAGESIZE`, a bitmap of page sizes; bit 0 is 4 KiB.
pub const PAGESIZE: u64 = 0x08;
/// Operational register: the command ring control register, its base and cycle.
pub const CRCR: u64 = 0x18;
/// Operational register: the device context base address array's pointer.
pub const DCBAAP: u64 = 0x30;
/// Operational register: `CONFIG`, whose low byte is how many slots are enabled.
pub const CONFIG: u64 = 0x38;
/// Operational register: port 1's `PORTSC`; port `n`'s is `0x10 * (n - 1)` further.
pub(crate) const PORTSC_BASE: u64 = 0x400;

/// `USBCMD` Run/Stop.
pub const CMD_RUN: u32 = 1 << 0;
/// `USBCMD` Host Controller Reset.
pub const CMD_RESET: u32 = 1 << 1;
/// `USBCMD` Interrupter Enable.
pub const CMD_INTERRUPTS: u32 = 1 << 2;

/// `USBSTS` `HCHalted`.
pub const STS_HALTED: u32 = 1 << 0;
/// `USBSTS` Host System Error.
pub const STS_HOST_SYSTEM_ERROR: u32 = 1 << 2;
/// `USBSTS` Event Interrupt, write one to clear.
pub const STS_EVENT_INTERRUPT: u32 = 1 << 3;
/// `USBSTS` Port Change Detect, write one to clear.
pub const STS_PORT_CHANGE: u32 = 1 << 4;
/// `USBSTS` Controller Not Ready.
pub const STS_NOT_READY: u32 = 1 << 11;
/// `USBSTS` Host Controller Error.
pub const STS_CONTROLLER_ERROR: u32 = 1 << 12;

/// Interrupter 0's register set, from `RTSOFF`.
pub(crate) const INTERRUPTER_0: u64 = 0x20;
/// Interrupter register: `IMAN` (pending, enable).
pub const IMAN: u64 = 0x00;
/// Interrupter register: `IMOD` (moderation interval).
pub const IMOD: u64 = 0x04;
/// Interrupter register: `ERSTSZ` (segment table size).
pub const ERSTSZ: u64 = 0x08;
/// Interrupter register: `ERSTBA` (segment table base).
pub const ERSTBA: u64 = 0x10;
/// Interrupter register: `ERDP` (dequeue pointer).
pub const ERDP: u64 = 0x18;

/// `IMAN` Interrupt Pending, write one to clear.
pub const IMAN_PENDING: u32 = 1 << 0;
/// `IMAN` Interrupt Enable.
pub const IMAN_ENABLE: u32 = 1 << 1;
/// `ERDP` Event Handler Busy, write one to clear.
pub const ERDP_BUSY: u64 = 1 << 3;

/// Extended capability id: USB Legacy Support, the firmware handoff (xHCI 1.2 section 7.1).
pub const EXTENDED_LEGACY_SUPPORT: u8 = 1;
/// `USBLEGSUP` HC BIOS Owned Semaphore.
pub const LEGACY_BIOS_OWNED: u32 = 1 << 16;
/// `USBLEGSUP` HC OS Owned Semaphore.
pub const LEGACY_OS_OWNED: u32 = 1 << 24;
/// `USBLEGCTLSTS`, the dword after `USBLEGSUP`: the SMI enables (bits 0, 4, 13, 14, 15) to clear,
/// and the write-one-to-clear SMI events (29, 30, 31), so firmware stops being told about USB.
pub const LEGACY_SMI_ENABLES: u32 = 1 | 1 << 4 | 0b111 << 13;
/// The write-one-to-clear event bits of `USBLEGCTLSTS`.
pub const LEGACY_SMI_EVENTS: u32 = 0b111 << 29;

/// **The facts the capability registers state**, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// `CAPLENGTH`: where the operational registers start.
    pub operational: u64,
    /// `HCIVERSION`, in binary-coded decimal (0x0100 for 1.0).
    pub version: u16,
    /// How many device slots the controller has.
    pub max_slots: u8,
    /// How many interrupters.
    pub max_interrupters: u16,
    /// How many root hub ports.
    pub max_ports: u8,
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
    /// Decode the six capability registers, read as dwords at [`CAPLENGTH`], [`HCSPARAMS1`],
    /// [`HCSPARAMS2`], [`HCCPARAMS1`], [`DBOFF`] and [`RTSOFF`]. Total, because every one of them
    /// is the hardware's word and a value this driver cannot use is caught by whoever uses it.
    pub const fn decode(
        caplength: u32,
        hcsparams1: u32,
        hcsparams2: u32,
        hccparams1: u32,
        dboff: u32,
        rtsoff: u32,
    ) -> Capabilities {
        Capabilities {
            operational: (caplength & 0xff) as u64,
            version: (caplength >> 16) as u16,
            max_slots: hcsparams1 as u8,
            max_interrupters: ((hcsparams1 >> 8) & 0x7ff) as u16,
            max_ports: (hcsparams1 >> 24) as u8,
            scratchpads: (((hcsparams2 >> 21) & 0x1f) << 5 | (hcsparams2 >> 27)) as u16,
            context_bytes: if hccparams1 & (1 << 2) != 0 { 64 } else { 32 },
            extended: ((hccparams1 >> 16) as u64) * 4,
            doorbells: (dboff & !0x3) as u64,
            runtime: (rtsoff & !0x1f) as u64,
        }
    }

    /// Port `n`'s `PORTSC`, `n` from 1, as an offset into the register file.
    pub const fn portsc(&self, n: u8) -> u64 {
        self.operational + PORTSC_BASE + 0x10 * (n as u64).saturating_sub(1)
    }

    /// Interrupter 0's register `reg` ([`IMAN`], [`ERDP`], ...), as an offset into the file.
    pub const fn interrupter(&self, reg: u64) -> u64 {
        self.runtime + INTERRUPTER_0 + reg
    }

    /// Slot `slot`'s doorbell (0 for the command ring), as an offset into the file.
    pub const fn doorbell(&self, slot: u8) -> u64 {
        self.doorbells + 4 * slot as u64
    }
}

/// **Find an extended capability** with id `id`, walking the list from `first` (the byte offset
/// [`Capabilities::extended`] gives) through a `read` of dwords at byte offsets. `None` when the
/// list has no such entry, ends, or leaves `limit` bytes.
///
/// Bounded at 64 hops and by `limit`, for `pci::msix_cap`'s reason: the list is a linked list in
/// a device's register file, so a broken controller must end the walk, not hang the kernel that is
/// walking it.
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
        let caps = Capabilities::decode(0x0100_0040, 0x0800_0840, 0, 0x0008_7000, 0x2000, 0x1000);
        assert_eq!(caps.operational, 0x40);
        assert_eq!(caps.version, 0x0100);
        assert_eq!(caps.max_slots, 64);
        assert_eq!(caps.max_interrupters, 8);
        assert_eq!(caps.max_ports, 8);
        assert_eq!(caps.scratchpads, 0);
        assert_eq!(caps.context_bytes, 32);
        assert_eq!(caps.extended, 0x20); // the dword count in HCCPARAMS1's high half, times four
        assert_eq!(caps.portsc(1), 0x440);
        assert_eq!(caps.portsc(8), 0x4b0);
        assert_eq!(caps.interrupter(ERDP), 0x1038);
        assert_eq!(caps.doorbell(1), 0x2004);
    }

    /// The scratchpad count is split across two fields, high bits first in the register but low
    /// bits first in meaning: getting the order wrong gives a controller 32 times too few pages.
    #[test]
    fn the_scratchpad_count_joins_its_two_fields() {
        let hi = 0b00001 << 21; // one 32
        let lo = 0b00011 << 27; // plus three
        let caps = Capabilities::decode(0x40, 0, hi | lo, 1 << 2, 0, 0);
        assert_eq!(caps.scratchpads, 35);
        assert_eq!(caps.context_bytes, 64);
    }

    /// The `USBCMD`, `USBSTS`, `IMAN`, `ERDP` and legacy support bits, pinned against xHCI 1.2
    /// sections 5.4.1, 5.4.2, 5.5.2 and 7.1, so a shift written the wrong way round fails here.
    #[test]
    fn the_register_bits_are_where_the_specification_puts_them() {
        assert_eq!([CMD_RUN, CMD_RESET, CMD_INTERRUPTS], [1, 2, 4]);
        assert_eq!(
            [
                STS_HALTED,
                STS_HOST_SYSTEM_ERROR,
                STS_EVENT_INTERRUPT,
                STS_PORT_CHANGE
            ],
            [1, 4, 8, 0x10]
        );
        assert_eq!([STS_NOT_READY, STS_CONTROLLER_ERROR], [0x800, 0x1000]);
        assert_eq!([IMAN_PENDING, IMAN_ENABLE], [1, 2]);
        assert_eq!(ERDP_BUSY, 8);
        assert_eq!([LEGACY_BIOS_OWNED, LEGACY_OS_OWNED], [0x1_0000, 0x100_0000]);
        assert_eq!(LEGACY_SMI_ENABLES, 0xe011);
        assert_eq!(LEGACY_SMI_EVENTS, 0xe000_0000);
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
