//! **The FADT, read two ways**: an Arm machine's PSCI boot flags, and an x86 machine's reset
//! register. Nothing else in the table is decoded.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// **The two facts the FADT holds that only an Arm machine has**: whether firmware implements PSCI,
/// and which instruction calls it.
///
/// They are two bits of the "ARM Boot Architecture Flags" word, which ACPI 5.1 added at a fixed
/// offset and which is meaningless on every other architecture. A device tree states the same pair
/// as `/psci`'s `compatible` and `method`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmBoot {
    /// Firmware implements the PSCI interface, so cores can be started with `CPU_ON`.
    pub psci: bool,
    /// The call is `hvc` rather than `smc`. Meaningless when [`ArmBoot::psci`] is false.
    pub hvc: bool,
}

/// Where the ARM Boot Architecture Flags sit **in the FADT body**, which is 36 bytes after the
/// offset ACPI's own table states (129).
const FADT_ARM_BOOT_AT: usize = 129 - SDT_HEADER_LEN;

/// Decode the FADT's Arm boot flags. `body` begins after the SDT header.
///
/// A body too short to hold the word is an ACPI 5.0-or-earlier FADT, which predates PSCI's
/// existence in this table; that is [`AcpiError::Truncated`] rather than "no PSCI", because the two
/// are different claims and only the caller knows what to do with the first.
pub fn parse_arm_boot(body: &[u8]) -> Result<ArmBoot, AcpiError> {
    if body.len() < FADT_ARM_BOOT_AT + 2 {
        return Err(AcpiError::Truncated);
    }
    let flags = u16(body, FADT_ARM_BOOT_AT);
    Ok(ArmBoot {
        psci: flags & 1 != 0,
        hvc: flags & 2 != 0,
    })
}

/// **Where the FADT says to write to reset the machine** (milestone 249 (the boot lottery is sampled by a person walking to the board)'s `x86_64` half).
///
/// ACPI 2.0 added a Generic Address Structure (`RESET_REG`) and a byte (`RESET_VALUE`) to the FADT:
/// write the byte to the register and the platform performs a reset. It is the route firmware
/// vouches for, which is why the kernel tries it before the two legacy ports it falls back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetRegister {
    /// Which address space [`ResetRegister::address`] is in.
    pub space: ResetSpace,
    /// The register's width in bits, as the table states it. ACPI requires 8 for this register;
    /// kept rather than assumed so a caller can refuse anything else out loud.
    pub bit_width: u8,
    /// The register's address in [`ResetRegister::space`]. For [`ResetSpace::PciConfig`] this is
    /// ACPI's packed form: device in bits 32..48, function in 16..32, register offset in 0..16, on
    /// bus 0.
    pub address: u64,
    /// The byte to write.
    pub value: u8,
}

/// The address spaces a reset register can be in. ACPI permits exactly three for this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetSpace {
    /// System memory: an MMIO store.
    Memory,
    /// System I/O: an `out` to a port. Every PC chipset this project has seen puts it at `0xCF9`.
    Io,
    /// PCI configuration space on bus 0.
    PciConfig,
    /// Anything else, which ACPI does not permit here; carried so the caller can say what it saw.
    Other(u8),
}

/// The FADT flag bit that says the reset register is implemented (`RESET_REG_SUP`, bit 10).
const FADT_RESET_REG_SUP: u32 = 1 << 10;
/// Where the FADT's `Flags` word sits **in the body** (ACPI's own offset is 112).
const FADT_FLAGS_AT: usize = 112 - SDT_HEADER_LEN;
/// Where `RESET_REG` sits in the body (ACPI offset 116); a 12-byte Generic Address Structure.
const FADT_RESET_REG_AT: usize = 116 - SDT_HEADER_LEN;
/// Where `RESET_VALUE` sits in the body (ACPI offset 128).
const FADT_RESET_VALUE_AT: usize = 128 - SDT_HEADER_LEN;

/// Decode the FADT's reset register. `body` begins after the SDT header.
///
/// `None` is one answer to three questions, deliberately: a body too short to hold the field (an
/// ACPI 1.0 FADT, which predates it), a table whose `RESET_REG_SUP` flag is clear, and an address of
/// zero. Each means "firmware does not offer this route", and the caller's response to all three is
/// the same: fall back to the legacy ports. Unlike [`parse_arm_boot`], a short body is not an error
/// here, because on `x86_64` a FADT without the field is a machine the kernel still has to reset.
///
/// Name: provisional (milestone 249): calef names public items.
pub fn parse_fadt_reset(body: &[u8]) -> Option<ResetRegister> {
    if body.len() <= FADT_RESET_VALUE_AT {
        return None;
    }
    if u32(body, FADT_FLAGS_AT) & FADT_RESET_REG_SUP == 0 {
        return None;
    }
    let gas = &body[FADT_RESET_REG_AT..FADT_RESET_REG_AT + 12];
    let address = u64(gas, 4);
    if address == 0 {
        return None;
    }
    Some(ResetRegister {
        space: match gas[0] {
            0 => ResetSpace::Memory,
            1 => ResetSpace::Io,
            2 => ResetSpace::PciConfig,
            other => ResetSpace::Other(other),
        },
        bit_width: gas[1],
        address,
        value: body[FADT_RESET_VALUE_AT],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::every_short_prefix_is_refused;

    extern crate std;

    /// The FADT's Arm boot flags, which are two bits at a fixed offset and the only place ACPI says
    /// how to start a second core.
    #[test]
    fn the_fadt_says_whether_psci_is_there_and_which_instruction_calls_it() {
        let mut body = [0u8; FADT_ARM_BOOT_AT + 2];
        body[FADT_ARM_BOOT_AT..][..2].copy_from_slice(&0b11u16.to_le_bytes());
        assert_eq!(
            parse_arm_boot(&body).expect("a long enough FADT"),
            ArmBoot {
                psci: true,
                hvc: true
            }
        );
        body[FADT_ARM_BOOT_AT..][..2].copy_from_slice(&0b01u16.to_le_bytes());
        assert_eq!(
            parse_arm_boot(&body).expect("a long enough FADT"),
            ArmBoot {
                psci: true,
                hvc: false
            }
        );
        every_short_prefix_is_refused(&body, FADT_ARM_BOOT_AT + 2, parse_arm_boot);
    }

    /// **The reset register is read where ACPI puts it, and only when the flag says it is there.**
    /// The values are QEMU `q35`'s, as this kernel read them from its FADT on 2026-09-24 (`fadt reset
    /// register: Io 0xcf9 (8 bits) <- 0x0f`, `script/soak-test --reboot --arch x86_64`): system
    /// I/O, eight bits wide, port `0xCF9`, and the value `0x0F`.
    #[test]
    fn the_fadt_reset_register_is_read_only_when_the_flag_says_so() {
        let mut body = [0u8; FADT_RESET_VALUE_AT + 1];
        body[FADT_FLAGS_AT..][..4].copy_from_slice(&FADT_RESET_REG_SUP.to_le_bytes());
        body[FADT_RESET_REG_AT] = 1; // system I/O
        body[FADT_RESET_REG_AT + 1] = 8;
        body[FADT_RESET_REG_AT + 4..][..8].copy_from_slice(&0xcf9u64.to_le_bytes());
        body[FADT_RESET_VALUE_AT] = 0x0f;
        assert_eq!(
            parse_fadt_reset(&body),
            Some(ResetRegister {
                space: ResetSpace::Io,
                bit_width: 8,
                address: 0xcf9,
                value: 0x0f,
            })
        );

        // The same bytes with RESET_REG_SUP clear: firmware has not vouched for them.
        let mut unflagged = body;
        unflagged[FADT_FLAGS_AT..][..4].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse_fadt_reset(&unflagged), None);

        // Flagged but at address zero, which is how a table that has nothing to say spells it.
        let mut zero = body;
        zero[FADT_RESET_REG_AT + 4..][..8].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(parse_fadt_reset(&zero), None);

        // An ACPI 1.0 FADT ends before the field, at every length short of it, without panicking.
        for len in 0..body.len() {
            assert_eq!(parse_fadt_reset(&body[..len]), None, "length {len}");
        }
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// The FADT's ARM boot flags and its reset register sit at the offsets ACPI states (129, 112,
    /// 116 and 128), less the 36-byte header the body does not carry. PSCI and HVC are separate bits.
    #[test]
    fn the_fadt_fields_are_read_from_the_offsets_acpi_states() {
        let mut body = std::vec![0u8; 129 - SDT_HEADER_LEN + 2];
        body[129 - SDT_HEADER_LEN] = 0b10;
        let boot = parse_arm_boot(&body).unwrap();
        assert!(!boot.psci && boot.hvc, "flags 0b10 is HVC without PSCI");
        body[129 - SDT_HEADER_LEN] = 0b01;
        let boot = parse_arm_boot(&body).unwrap();
        assert!(boot.psci && !boot.hvc);

        let mut fadt = std::vec![0u8; 129 - SDT_HEADER_LEN];
        let flags = 112 - SDT_HEADER_LEN;
        fadt[flags..flags + 4].copy_from_slice(&(1u32 << 10).to_le_bytes());
        let reg = 116 - SDT_HEADER_LEN;
        fadt[reg] = 1;
        fadt[reg + 1] = 8;
        fadt[reg + 4..reg + 12].copy_from_slice(&0xcf9u64.to_le_bytes());
        fadt[128 - SDT_HEADER_LEN] = 0x06;
        let r = parse_fadt_reset(&fadt).expect("RESET_REG_SUP is set");
        assert_eq!(
            (r.space, r.bit_width, r.address, r.value),
            (ResetSpace::Io, 8, 0xcf9, 0x06)
        );
    }
}
