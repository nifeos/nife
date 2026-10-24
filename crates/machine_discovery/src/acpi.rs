//! ACPI: the machine description x86 has instead of a device tree, and every aarch64 server too.
//!
//! A linked structure of independent tables, each with its own signature and checksum, reached
//! from the RSDP through the RSDT or XSDT. One private module per table (`sdt`, `root`, `madt`,
//! `mcfg`, `dmar`, `gtdt`, `spcr`, `fadt`) is re-exported here, beside the public [`ivrs`]. Every
//! table's checksum is checked: the RSDP is found by scanning memory for a string. AML is not
//! decoded. See `notes/x86-port/acpi-and-pci.md` and `notes/machine-discovery.md` ("ACPI").

//! # BUGS
//!
//! - Nothing checks that tables do not overlap or that a length suits where a table sits.
//! - The MADT's `PCAT_COMPAT` (8259s present) is reported; whoever brings the APIC up masks them.
//! - A DMAR's scopes and RMRRs fill fixed arrays, and overflow sets [`DmarUnits::truncated`];
//!   other scope kinds and structures (ANDD among them) are not decoded. See the note's "ACPI".
//! - `Dmar::flags`' `INTR_REMAP` is reported ([`DmarUnits::platform_interrupt_remapping`]) and
//!   never acted on; this kernel does not build interrupt remapping.
//! - An MCFG window with `end_bus` below `start_bus` is reported as written; [`McfgEntry::size`]
//!   answers zero.

mod bytes;
mod dmar;
mod fadt;
mod gtdt;
mod madt;
mod mcfg;
mod root;
mod sdt;
mod spcr;
#[cfg(test)]
mod tests;

// The IVRS, AMD's counterpart, is a module of its own beside the DMAR's: a different table with
// a different idea of how a device is named (by id, where the DMAR names it by path).
pub mod ivrs;

use bytes::{u16, u32, u64};
pub use dmar::{
    DeviceScope, Dmar, DmarEntry, DmarStructures, DmarUnits, Drhd, InterruptRemappingClaims,
    MAX_DRHDS, MAX_RMRR_SCOPES, MAX_RMRRS, MAX_SCOPE_PATH, MAX_SCOPES, Ownership, ReservedRegion,
    SCOPE_PCI_ENDPOINT, SCOPE_PCI_SUBHIERARCHY, dmar_structures, first_drhd, parse_dmar,
};
pub use fadt::{ArmBoot, ResetRegister, ResetSpace, parse_arm_boot, parse_fadt_reset};
pub use gtdt::{GTDT_ACTIVE_LOW, GTDT_EDGE_TRIGGERED, Gtdt, parse_gtdt};
pub use madt::{
    INTI_POLARITY_MASK, INTI_TRIGGER_MASK, ISA_BUS, ISA_IRQ_COUNT, IsaIrqRouting, MADT_PCAT_COMPAT,
    Madt, MadtEntries, MadtEntry, MsiSpis, isa_irq_table, madt_entries, parse_madt,
};
pub use mcfg::{McfgEntry, mcfg_entry};
pub use root::{RSDP_SIGNATURE, Rsdp, parse_rsdp, root_entry, root_entry_count};
pub use sdt::{AcpiError, SDT_HEADER_LEN, SdtHeader, is_checksum_ok, parse_sdt_header};
pub use spcr::{SPCR_PL011, SPCR_SBSA_UART, Spcr, parse_spcr};
