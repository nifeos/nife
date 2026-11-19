//! **The PLDA `XpressRICH` bridge**: the registers in the controller's `apb` window, and the address
//! translation (ATR) table that maps the processor's windows onto PCI Express addresses.
//!
//! Transcribed from Linux's `drivers/pci/controller/plda/pcie-plda.h` and `pcie-plda-host.c`
//! (see the crate documentation for the revision). No BSD drives this controller, so each
//! constant carries the Linux spelling as its alias.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex,
//! after the controller's product name. calef has not named it.

/// The general settings word. Bit 0 makes the controller a root port.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "GEN_SETTINGS")]
pub const GENERAL_SETTINGS: u32 = 0x80;
/// [`GENERAL_SETTINGS`]: the controller is a root port.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "RP_ENABLE")]
pub const ROOT_PORT_ENABLE: u32 = 1 << 0;

/// The root port's identification word: class in bits 31:8, revision in 7:0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_PCI_IDS_DW1")]
pub const IDENTIFICATION_CLASS_AND_REVISION: u32 = 0x9c;
/// [`IDENTIFICATION_CLASS_AND_REVISION`]: the revision field, which the bring-up keeps.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "REVISION_ID_MASK")]
pub const REVISION_MASK: u32 = 0xff;
/// The class and subclass a PCI-to-PCI bridge reports, `0x0604`, shifted to where
/// [`IDENTIFICATION_CLASS_AND_REVISION`] holds it (programming interface 0).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCI_CLASS_BRIDGE_PCI")]
pub const CLASS_PCI_TO_PCI_BRIDGE: u32 = 0x0604 << 16;

/// The miscellaneous word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCI_MISC")]
pub const MISCELLANEOUS: u32 = 0xb4;
/// [`MISCELLANEOUS`]: disable the physical function the `StarFive` glue currently selects.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PHY_FUNCTION_DIS")]
pub const PHYSICAL_FUNCTION_DISABLE: u32 = 1 << 15;

/// The window capabilities word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_WINROM")]
pub const WINDOW_CAPABILITIES: u32 = 0xfc;
/// [`WINDOW_CAPABILITIES`]: the prefetchable window takes 64-bit addresses.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PREF_MEM_WIN_64_SUPPORT")]
pub const PREFETCHABLE_WINDOW_64_BIT: u32 = 1 << 3;

/// The message reception word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PMSG_SUPPORT_RX")]
pub const MESSAGE_RECEPTION: u32 = 0x3f0;
/// [`MESSAGE_RECEPTION`]: forward latency tolerance reporting messages. The bring-up clears it,
/// because the forwarding address is uninitialized and a forwarded message wedges the bus.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PMSG_LTR_SUPPORT")]
pub const LATENCY_TOLERANCE_REPORTING_RECEPTION: u32 = 1 << 2;

/// The local interrupt mask. A set bit lets the source interrupt the processor.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "IMASK_LOCAL")]
pub const LOCAL_INTERRUPT_MASK: u32 = 0x180;
/// The local interrupt status. Write a set bit to clear it.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ISTATUS_LOCAL")]
pub const LOCAL_INTERRUPT_STATUS: u32 = 0x184;
/// The address a device writes a message-signalled interrupt to, inside this window.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "IMSI_ADDR")]
pub const MESSAGE_SIGNALLED_INTERRUPT_ADDRESS: u32 = 0x190;
/// The message-signalled interrupt status: one bit per vector, 32 vectors.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ISTATUS_MSI")]
pub const MESSAGE_SIGNALLED_INTERRUPT_STATUS: u32 = 0x194;
/// [`LOCAL_INTERRUPT_STATUS`]: the four legacy interrupt lines, INTA to INTD, bits 24 to 27.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PM_MSI_INT_INTX_MASK")]
pub const LEGACY_INTERRUPTS: u32 = 0xf << 24;
/// [`LOCAL_INTERRUPT_STATUS`]: a message-signalled interrupt arrived.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PM_MSI_INT_MSI")]
pub const MESSAGE_SIGNALLED_INTERRUPT: u32 = 1 << 28;

/// The root port's own type 1 configuration header, at this offset inside the bridge window.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "CONFIG_SPACE_ADDR_OFFSET")]
pub const ROOT_PORT_CONFIGURATION: u32 = 0x1000;
/// The root port's BAR0, inside the bridge window. The bring-up writes zero to it and to the word
/// after it.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const ROOT_PORT_BAR0: u32 = ROOT_PORT_CONFIGURATION + 0x10;
/// The root port's BAR1, inside the bridge window.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const ROOT_PORT_BAR1: u32 = ROOT_PORT_CONFIGURATION + 0x14;

/// The first outbound translation entry: the processor's AXI slave 0 window 0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR0_AXI4_SLV0_SRCADDR_PARAM")]
pub const OUTBOUND_SOURCE_ADDRESS_AND_SIZE: u32 = 0x800;
/// The entry's source address, bits 63:32.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR0_AXI4_SLV0_SRC_ADDR")]
pub const OUTBOUND_SOURCE_ADDRESS_HIGH: u32 = 0x804;
/// The entry's translated (PCI Express) address, bits 31:0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR0_AXI4_SLV0_TRSL_ADDR_LSB")]
pub const OUTBOUND_TRANSLATED_ADDRESS_LOW: u32 = 0x808;
/// The entry's translated address, bits 63:32.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR0_AXI4_SLV0_TRSL_ADDR_UDW")]
pub const OUTBOUND_TRANSLATED_ADDRESS_HIGH: u32 = 0x80c;
/// The entry's interface: [`INTERFACE_TRANSMIT_RECEIVE`] or [`INTERFACE_CONFIGURATION`].
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR0_AXI4_SLV0_TRSL_PARAM")]
pub const OUTBOUND_INTERFACE: u32 = 0x810;
/// Bytes between one translation entry and the next.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR_ENTRY_SIZE")]
pub const TRANSLATION_ENTRY_BYTES: u32 = 32;
/// How many outbound entries the AXI slave 0 table holds.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const OUTBOUND_ENTRIES: u32 = 8;

/// [`OUTBOUND_SOURCE_ADDRESS_AND_SIZE`]: the entry is in use.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR_IMPL_ENABLE")]
pub const TRANSLATION_ENABLE: u32 = 1 << 0;
/// [`OUTBOUND_SOURCE_ADDRESS_AND_SIZE`]: the size field, bits 6:1, holding `log2(size) - 1`.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "ATR_SIZE_MASK")]
pub const TRANSLATION_SIZE_MASK: u32 = 0x3f << 1;
/// [`OUTBOUND_INTERFACE`]: memory reads and writes.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_TX_RX_INTERFACE")]
pub const INTERFACE_TRANSMIT_RECEIVE: u32 = 0;
/// [`OUTBOUND_INTERFACE`]: configuration reads and writes, addressed as ECAM is.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_CONFIG_INTERFACE")]
pub const INTERFACE_CONFIGURATION: u32 = 1;

/// The smallest window an entry translates: 4 KiB, the granule its source address keeps.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const MINIMUM_TRANSLATION_BYTES: u64 = 0x1000;

/// **One outbound translation entry's five words**, as `(offset, value)` pairs in the order Linux's
/// `plda_pcie_setup_window` writes them, for entry `index` mapping `size_bytes` of the processor's
/// address space at `processor_address` onto `pci_address`.
///
/// Entry 0 carries configuration requests and every other entry memory requests, as Linux assigns
/// them. Refuses (`None`) an index past the table, a size that is not a power of two of at least
/// [`MINIMUM_TRANSLATION_BYTES`].
/// The source address keeps its low 12 bits clear, as the hardware reads them as the size and
/// enable fields.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[must_use]
pub const fn outbound_window(
    index: u32,
    processor_address: u64,
    pci_address: u64,
    size_bytes: u64,
) -> Option<[(u32, u32); 5]> {
    if index >= OUTBOUND_ENTRIES
        || size_bytes < MINIMUM_TRANSLATION_BYTES
        || !size_bytes.is_power_of_two()
    {
        return None;
    }
    // At most 62, since `size_bytes` is a power of two of at least 4 KiB in a u64: always inside
    // the 6-bit field.
    let size_field = size_bytes.trailing_zeros() - 1;
    let base = index * TRANSLATION_ENTRY_BYTES;
    let interface = if index == 0 {
        INTERFACE_CONFIGURATION
    } else {
        INTERFACE_TRANSMIT_RECEIVE
    };
    let source_low = (processor_address as u32) & !(MINIMUM_TRANSLATION_BYTES as u32 - 1);
    Some([
        (base + OUTBOUND_INTERFACE, interface),
        (
            base + OUTBOUND_SOURCE_ADDRESS_AND_SIZE,
            source_low | ((size_field << 1) & TRANSLATION_SIZE_MASK) | TRANSLATION_ENABLE,
        ),
        (
            base + OUTBOUND_SOURCE_ADDRESS_HIGH,
            (processor_address >> 32) as u32,
        ),
        (base + OUTBOUND_TRANSLATED_ADDRESS_LOW, pci_address as u32),
        (
            base + OUTBOUND_TRANSLATED_ADDRESS_HIGH,
            (pci_address >> 32) as u32,
        ),
    ])
}

/// **The window an entry's source word describes**: `Some(size_bytes)` when the entry is enabled
/// and its size field names a window a `u64` can count (field 63 would be 2^64 bytes).
/// The inverse of [`outbound_window`]'s size encoding, for a reader that checks what was
/// programmed.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[must_use]
pub const fn translated_size_bytes(source_address_and_size: u32) -> Option<u64> {
    if source_address_and_size & TRANSLATION_ENABLE == 0 {
        return None;
    }
    let field = (source_address_and_size & TRANSLATION_SIZE_MASK) >> 1;
    1u64.checked_shl(field + 1)
}
