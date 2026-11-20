//! **The root port's bus numbers and forwarding windows**, written through configuration space
//! once the bridge decodes it.
//!
//! On a machine whose firmware enumerates PCI, the root port arrives with its buses and windows
//! set. On the JH7110 nothing has run before the kernel, so the root port at `00:00.0` reads zero
//! bus numbers and forwards nothing, and the function behind it at bus 1 is invisible until
//! [`configure`] runs. See notes/jh7110-pci-express.md.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
//! calef has not named it.

use pci_configuration_space::{
    BUS_NUMBERS_REGISTER, BusDeviceFunction, COMMAND_BUS_MASTER_ENABLE,
    COMMAND_MEMORY_SPACE_ENABLE, COMMAND_REGISTER, HEADER_TYPE_BRIDGE, HEADER_TYPE_REGISTER,
};

use crate::Window;

/// The root port's address: bus 0, device 0, function 0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const LOCATION: BusDeviceFunction = BusDeviceFunction {
    bus: 0,
    device: 0,
    function: 0,
};

/// The bus [`configure`] puts behind the root port.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const SECONDARY_BUS: u8 = 1;

/// The type 1 header's I/O base and limit word (the high half is the secondary status).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCI_IO_BASE")]
pub const INPUT_OUTPUT_BASE_AND_LIMIT: u64 = 0x1c;
/// The type 1 header's memory base and limit word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCI_MEMORY_BASE")]
pub const MEMORY_BASE_AND_LIMIT: u64 = 0x20;
/// The type 1 header's prefetchable memory base and limit word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCI_PREF_MEMORY_BASE")]
pub const PREFETCH_BASE_AND_LIMIT: u64 = 0x24;

/// A base above its limit, which forwards nothing: base `0xf0`, limit `0x00` in the I/O word.
/// The secondary status half is written zero, which clears nothing, as its bits clear on a one.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const INPUT_OUTPUT_CLOSED: u32 = 0x0000_00f0;
/// A base above its limit for the prefetchable word: base `0xfff0_0000`, limit below it.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const PREFETCH_CLOSED: u32 = 0x0000_fff0;

/// **The memory base and limit word that forwards exactly `window`**, or `None` when the window
/// is empty, crosses 4 GiB, or is not aligned to the 1 MiB granule the word counts in.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[must_use]
pub const fn memory_base_and_limit(window: Window) -> Option<u32> {
    const GRANULE: u64 = 0x10_0000;
    let base = window.physical_address;
    let Some(end) = base.checked_add(window.size_bytes) else {
        return None;
    };
    if window.size_bytes == 0
        || end > 0x1_0000_0000
        || !base.is_multiple_of(GRANULE)
        || !window.size_bytes.is_multiple_of(GRANULE)
    {
        return None;
    }
    let last = end - 1;
    Some((((last >> 16) as u32 & 0xfff0) << 16) | ((base >> 16) as u32 & 0xfff0))
}

/// **The bus numbers word**: primary 0, secondary and subordinate [`SECONDARY_BUS`], keeping the
/// secondary latency timer the port reported.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[must_use]
pub const fn bus_numbers(before: u32) -> u32 {
    (before & 0xff00_0000) | ((SECONDARY_BUS as u32) << 16) | ((SECONDARY_BUS as u32) << 8)
}

/// What [`configure`] found and wrote.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Configured {
    /// The port now forwards bus [`SECONDARY_BUS`] and the memory window.
    Forwarding {
        /// The bus numbers word as read back.
        bus_numbers: u32,
        /// The memory base and limit word as read back.
        memory_base_and_limit: u32,
    },
    /// `00:00.0` answered no vendor: configuration space is not decoding.
    Absent,
    /// `00:00.0` is not a type 1 header, so it is not the root port this driver configures.
    NotABridge {
        /// The header type, multifunction bit masked.
        header_type: u8,
    },
    /// The memory window cannot be expressed in the 32-bit memory base and limit word.
    WindowUnusable,
}

/// **Configure the root port to forward bus [`SECONDARY_BUS`] and `memory`**, through `read32` and
/// `write32`, the caller's configuration accessors (`BusDeviceFunction`, byte offset of an aligned word).
///
/// Writes the bus numbers, the memory window, closes the I/O and prefetchable windows, and turns on
/// memory decoding and bus mastering, so the BARs the kernel places behind it are reachable and
/// the controller behind it can reach memory. Idempotent.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub fn configure(
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    write32: &mut dyn FnMut(BusDeviceFunction, u64, u32),
    memory: Window,
) -> Configured {
    if read32(LOCATION, 0) & 0xffff == 0xffff {
        return Configured::Absent;
    }
    let header_type = ((read32(LOCATION, HEADER_TYPE_REGISTER & !3) >> 16) & 0x7f) as u8;
    if header_type != HEADER_TYPE_BRIDGE {
        return Configured::NotABridge { header_type };
    }
    let Some(memory_word) = memory_base_and_limit(memory) else {
        return Configured::WindowUnusable;
    };
    let before = read32(LOCATION, BUS_NUMBERS_REGISTER);
    write32(LOCATION, BUS_NUMBERS_REGISTER, bus_numbers(before));
    write32(LOCATION, MEMORY_BASE_AND_LIMIT, memory_word);
    write32(LOCATION, INPUT_OUTPUT_BASE_AND_LIMIT, INPUT_OUTPUT_CLOSED);
    write32(LOCATION, PREFETCH_BASE_AND_LIMIT, PREFETCH_CLOSED);
    let command = read32(LOCATION, COMMAND_REGISTER) & 0xffff;
    write32(
        LOCATION,
        COMMAND_REGISTER,
        command | u32::from(COMMAND_MEMORY_SPACE_ENABLE | COMMAND_BUS_MASTER_ENABLE),
    );
    Configured::Forwarding {
        bus_numbers: read32(LOCATION, BUS_NUMBERS_REGISTER),
        memory_base_and_limit: read32(LOCATION, MEMORY_BASE_AND_LIMIT),
    }
}
