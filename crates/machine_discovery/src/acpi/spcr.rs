//! **The SPCR**: where the firmware's console UART is, and which interrupt it raises.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// **The Serial Port Console Redirection table** (signature `SPCR`): where the firmware's console
/// is and which interrupt it raises.
///
/// It is the only fixed table that names a UART, and on an ACPI-only machine it is therefore the
/// only place that fact exists outside AML. SBBR requires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spcr {
    /// The interface type byte: see [`SPCR_PL011`] and [`SPCR_SBSA_UART`].
    pub interface: u8,
    /// The register block's address, in whatever space `address_space` names.
    pub address: u64,
    /// 0 for system memory, 1 for system I/O. An Arm machine says 0.
    pub address_space: u8,
    /// The GSIV the port raises, absolute like every GSIV, or `None` when the table states only a
    /// PC-AT IRQ (an x86 spelling an Arm machine does not use).
    pub interrupt: Option<u32>,
}

/// [`Spcr::interface`] for a PL011, which is what QEMU `virt` and most Arm machines present.
pub const SPCR_PL011: u8 = 0x03;
/// [`Spcr::interface`] for the SBSA generic UART, a PL011 subset with no DMA and no modem lines.
pub const SPCR_SBSA_UART: u8 = 0x0e;

/// The bit of SPCR's interrupt-type byte that says a GSIV is stated rather than a PC-AT IRQ.
const SPCR_INTERRUPT_TYPE_GSIV: u8 = 1 << 3;

/// The bytes of an SPCR body this decoder reads.
const SPCR_FIXED_LEN: usize = 22;

/// Decode the SPCR. `body` begins after the SDT header.
pub fn parse_spcr(body: &[u8]) -> Result<Spcr, AcpiError> {
    if body.len() < SPCR_FIXED_LEN {
        return Err(AcpiError::Truncated);
    }
    // The base address is a Generic Address Structure at offset 4: space id, width, offset, access
    // size, then the 64-bit address. Only the space id and the address decide anything here.
    let gsiv = u32(body, 18);
    Ok(Spcr {
        interface: body[0],
        address_space: body[4],
        address: u64(body, 8),
        // A GSIV of zero is how a table that has no interrupt to state spells it; interrupt 0 is
        // an SGI and no UART raises one, so zero is unambiguous rather than a value to defend.
        interrupt: (body[16] & SPCR_INTERRUPT_TYPE_GSIV != 0 && gsiv != 0).then_some(gsiv),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::every_short_prefix_is_refused;

    /// The SPCR names a PL011 at QEMU `virt`'s address on GSIV 33, which is the INTID the kernel
    /// has hardcoded since milestone 19 (run a real workload) and now reads instead.
    #[test]
    fn the_spcr_names_a_pl011_and_its_global_interrupt() {
        let mut body = [0u8; SPCR_FIXED_LEN];
        body[0] = SPCR_PL011;
        body[4] = 0; // system memory
        body[8..16].copy_from_slice(&0x0900_0000u64.to_le_bytes());
        body[16] = SPCR_INTERRUPT_TYPE_GSIV;
        body[18..22].copy_from_slice(&33u32.to_le_bytes());
        assert_eq!(
            parse_spcr(&body).expect("a full-length SPCR"),
            Spcr {
                interface: SPCR_PL011,
                address: 0x0900_0000,
                address_space: 0,
                interrupt: Some(33),
            }
        );
        every_short_prefix_is_refused(&body, SPCR_FIXED_LEN, parse_spcr);
    }

    /// **A PC-AT IRQ is not a GSIV**, and a table that states only the former answers `None` rather
    /// than handing back an ISA interrupt number as though it were a GIC INTID. That is the same
    /// class of mistake `interrupt_id`'s bank bases exist to prevent, one table over.
    #[test]
    fn an_spcr_that_states_only_a_pc_at_irq_names_no_global_interrupt() {
        let mut body = [0u8; SPCR_FIXED_LEN];
        body[0] = SPCR_PL011;
        body[16] = 1; // PC-AT compatible, not a GSIV
        body[17] = 4;
        body[18..22].copy_from_slice(&33u32.to_le_bytes());
        assert_eq!(parse_spcr(&body).expect("well formed").interrupt, None);
    }
}
