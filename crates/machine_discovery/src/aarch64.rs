//! What aarch64 machine is this? Decoded from the ID registers, because the CPU says so itself.
//!
//! The kernel reads the registers and hands the raw words here, so this half is a decoder, not a
//! parser. The one exception is [`Psci`], which parses `/psci`: the firmware conduit depends on
//! what runs above EL1, which no ID register can say. The three granule fields spend their
//! encodings three different ways, so each is decoded on its own line. See
//! `notes/isa-discovery.md` and `notes/machine-discovery.md` ("aarch64").

//! # BUGS
//!
//! - `VARange` reporting 52 does not mean the kernel could use 52. ARMv8.2-LVA needs a 64 KiB
//!   granule and ARMv8.7-LPA2 is a feature bit this record does not read; the field is reported
//!   because it is what the machine says.
//! - `TGran16` spells "supported" `0b0001` and "not" `0b0000`, inverted against `TGran4` and
//!   `TGran64`; a uniform decoder reports 16 KiB backwards on every part.
//! - `TGran4` has two "yes" encodings: `0b0001` (52-bit with `FEAT_LPA2`) is a stronger yes than
//!   `0b0000`, and reading it as reserved refuses the boot on `FEAT_LPA2` parts.

/// Which translation granules the machine supports at stage 1.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Granules {
    /// The one this kernel's page tables are built on. Its absence stops the boot.
    pub k4: bool,
    pub k16: bool,
    pub k64: bool,
}

/// **What this aarch64 machine is.** One record, populated once at boot, printed at boot.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Isa {
    /// `MIDR_EL1.Implementer`, an ARM-assigned vendor code. See [`Isa::implementer_name`].
    pub implementer: u8,
    /// `MIDR_EL1.PartNum`. Vendor-specific, so it prints as a number: `0xd08` is a Cortex-A72 to
    /// somebody with the vendor's list, and a guess to us.
    pub part: u16,
    /// `MIDR_EL1.Variant` and `.Revision`, which ARM writes as `r{variant}p{revision}` and which is
    /// how every errata document identifies a part.
    pub variant: u8,
    pub revision: u8,
    /// `ID_AA64MMFR0_EL1.PARange`, kept in its **raw encoding** because that is what `TCR_EL1.IPS`
    /// takes. [`Isa::pa_bits`] decodes it for the boot line.
    pub pa_range: u8,
    /// `ID_AA64MMFR0_EL1.ASIDBits`, decoded to 8 or 16. The aarch64 twin of RISC-V's `satp.ASID`
    /// probe, and the reason that probe had to exist: ARM **mandates** one of two values here, so
    /// the number is architected and readable, while RISC-V permits any width including zero and
    /// publishes nothing, so it has to be measured. `crates/address_space_identifier` assumes at least 8 either way.
    pub asid_bits: u8,
    pub granules: Granules,
    /// `ID_AA64MMFR2_EL1.VARange`, decoded to 48 or 52. See BUGS: 52 is a claim about the part, not
    /// a configuration this kernel could adopt by flipping one field.
    pub va_bits: u8,
    /// `ID_AA64ISAR0_EL1.RNDR`, milestone 162: does this part implement `FEAT_RNG`, the `RNDR`/
    /// `RNDRRS` random-number instructions? Read here for the same reason `PARange` and
    /// `ASIDBits` are: the field is architected and readable at EL1, so there is no excuse to
    /// assume it. Unlike those two, a `false` here is not fatal to the boot; it only means the
    /// entropy service's instruction backend stays unoffered (`kernel/src/user/entropy_service.rs`),
    /// the same way a missing virtio-rng device does.
    pub rndr: bool,
}

/// What a machine is missing that this kernel needs.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Missing {
    /// No 4 KiB granule at stage 1. Every page table in `crates/paging` is 4 KiB.
    pub granule_4k: bool,
    /// Fewer than 8 ASID bits. Unreachable on a conforming part (ARM's only two encodings are 8 and
    /// 16), which is exactly why it is worth checking: a reserved encoding decodes here as 0 and
    /// would otherwise be a silently aliasing TLB.
    pub asid_bits: bool,
}

impl Missing {
    pub const fn any(self) -> bool {
        self.granule_4k || self.asid_bits
    }
}

impl Isa {
    /// Decode the four registers the kernel reads. Pure: no `mrs` here, so this is host-testable
    /// against words captured from real parts.
    ///
    /// The field positions are the ARM ARM's (D19.2 in DDI 0487): `MIDR_EL1` is
    /// `implementer` 31:24, `variant` 23:20, `architecture` 19:16, `partnum` 15:4, `revision` 3:0,
    /// the `ID_AA64MMFR*` fields are 4 bits each at the offsets named below, and
    /// `ID_AA64ISAR0_EL1.RNDR` is bits 63:60 (`0b0001` implemented, `0b0000` not; every other
    /// encoding is reserved and decoded as not implemented, the same conservative default
    /// `ASIDBits`' reserved encodings use).
    pub fn decode(midr_el1: u64, mmfr0_el1: u64, mmfr2_el1: u64, isar0_el1: u64) -> Isa {
        let f = |reg: u64, shift: u32| ((reg >> shift) & 0xf) as u8;

        Isa {
            implementer: (midr_el1 >> 24) as u8,
            part: ((midr_el1 >> 4) & 0xfff) as u16,
            variant: f(midr_el1, 20),
            revision: f(midr_el1, 0),
            pa_range: f(mmfr0_el1, 0),
            asid_bits: match f(mmfr0_el1, 4) {
                0b0000 => 8,
                0b0010 => 16,
                // Reserved. Reported as zero rather than rounded up to 8, so
                // `missing_requirements` refuses the boot instead of assuming the kind answer.
                _ => 0,
            },
            granules: Granules {
                // 0b0000 supported, 0b0001 supported with 52-bit input and output addresses
                // (FEAT_LPA2), 0b1111 not. The LPA2 value is a stronger yes, not a different
                // answer: this kernel's 4 KiB, 48-bit tables work on it unchanged. Any other value
                // is reserved and read as absent.
                k4: matches!(f(mmfr0_el1, 28), 0b0000 | 0b0001),
                // 0b0001 supported, 0b0010 supported with FEAT_LPA2, 0b0000 not: inverted against
                // its two siblings.
                k16: matches!(f(mmfr0_el1, 20), 0b0001 | 0b0010),
                // 0b0000 supported, 0b1111 not. No LPA2 value: 64 KiB reaches 52 bits through
                // FEAT_LVA and FEAT_LPA, which this field does not report.
                k64: f(mmfr0_el1, 24) == 0b0000,
            },
            va_bits: match f(mmfr2_el1, 16) {
                0b0001 => 52,
                _ => 48,
            },
            rndr: f(isar0_el1, 60) == 0b0001,
        }
    }

    /// **Can this machine run us?** The aarch64 twin of
    /// [`riscv64::Isa::missing_requirements`](crate::riscv64::Isa::missing_requirements), and the
    /// only verb here a call site is meant to branch on.
    pub fn missing_requirements(&self) -> Missing {
        Missing {
            granule_4k: !self.granules.k4,
            asid_bits: self.asid_bits < 8,
        }
    }

    /// How many physical address bits [`pa_range`](Isa::pa_range) encodes.
    ///
    /// Zero for a reserved encoding, which is honest rather than useful: the kernel writes the raw
    /// field into `TCR_EL1.IPS` regardless, because ARM's rule is that a value *larger* than the
    /// implementation supports is UNPREDICTABLE, and the machine's own encoding cannot be larger
    /// than itself.
    pub fn pa_bits(&self) -> u8 {
        match self.pa_range {
            0b0000 => 32,
            0b0001 => 36,
            0b0010 => 40,
            0b0011 => 42,
            0b0100 => 44,
            0b0101 => 48,
            0b0110 => 52,
            0b0111 => 56,
            _ => 0,
        }
    }

    /// The vendor name for [`implementer`](Isa::implementer), or `None` for a code ARM has not
    /// published. Printing the number is better than printing a guess.
    pub fn implementer_name(&self) -> Option<&'static str> {
        let mut i = 0;
        while i < IMPLEMENTERS.len() {
            if IMPLEMENTERS[i].code == self.implementer {
                return Some(IMPLEMENTERS[i].name);
            }
            i += 1;
        }
        None
    }
}

/// One row of [`IMPLEMENTERS`].
pub struct Implementer {
    /// The code ARM assigned, as it appears in `MIDR_EL1[31:24]`.
    pub code: u8,
    pub name: &'static str,
}

/// The vendor codes ARM has published, from the registry in the ARM ARM's `MIDR_EL1` section.
///
/// A table rather than a `match`, so a duplicated key is the compile error asserted below rather
/// than a silently unreachable arm, and a test can walk the rows. The codes are not derivable from
/// the vendors' initials (`0x50`, `0x56`, `0xc0` are not), so they are written out.
pub const IMPLEMENTERS: [Implementer; 14] = [
    Implementer {
        code: 0x41,
        name: "Arm",
    },
    Implementer {
        code: 0x42,
        name: "Broadcom",
    },
    Implementer {
        code: 0x43,
        name: "Cavium",
    },
    Implementer {
        code: 0x44,
        name: "DEC",
    },
    Implementer {
        code: 0x46,
        name: "Fujitsu",
    },
    Implementer {
        code: 0x49,
        name: "Infineon",
    },
    Implementer {
        code: 0x4d,
        name: "Motorola/Freescale",
    },
    Implementer {
        code: 0x4e,
        name: "NVIDIA",
    },
    Implementer {
        code: 0x50,
        name: "AppliedMicro",
    },
    Implementer {
        code: 0x51,
        name: "Qualcomm",
    },
    Implementer {
        code: 0x56,
        name: "Marvell",
    },
    Implementer {
        code: 0x61,
        name: "Apple",
    },
    Implementer {
        code: 0x69,
        name: "Intel",
    },
    Implementer {
        code: 0xc0,
        name: "Ampere",
    },
];

// A code listed twice would shadow whichever row came second, and the shadowed vendor would be
// unreportable for as long as the table lived. Cheaper as a compile error than as a test.
const _: () = assert!(
    codes_are_distinct(&IMPLEMENTERS),
    "two rows of IMPLEMENTERS share a code"
);

/// Does every row of `table` carry its own code? A function rather than a loop inside the `const`
/// above, because a check that only ever meets a good table cannot show it would refuse a bad one,
/// and this one is fed a bad one in the tests below.
const fn codes_are_distinct(table: &[Implementer]) -> bool {
    let mut i = 0;
    while i < table.len() {
        let mut j = i + 1;
        while j < table.len() {
            if table[i].code == table[j].code {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// **How a PSCI call leaves EL1.**
///
/// PSCI (Power State Coordination Interface) is ARM's firmware call standard for turning cores on
/// and off, and the instruction that reaches the firmware is not fixed: it depends on what runs
/// above the kernel. A hypervisor at EL2 serves `hvc`; secure firmware at EL3 serves `smc`. Getting
/// it wrong is not a wrong answer, it is **no answer**: an `hvc` on a machine with nothing at EL2
/// traps as an undefined instruction, and an `smc` on a machine with no EL3 does the same. So this
/// cannot be guessed, and the `/psci` node's `method` property is where the machine states it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Conduit {
    /// `"hvc"`: the call goes to a hypervisor at EL2. What QEMU `virt` states by default.
    Hvc,
    /// `"smc"`: the call goes to secure firmware at EL3. What QEMU `virt` states under
    /// `virtualization=on`, and what a board with TF-A under it states.
    Smc,
}

impl Conduit {
    /// Decode a `method` value. `None` for a spelling the binding does not define, rather than a
    /// default: there is no safe fallback here, because both wrong answers are traps.
    pub fn from_property(value: &[u8]) -> Option<Conduit> {
        match value.split(|&b| b == 0).next().unwrap_or(b"") {
            b"hvc" => Some(Conduit::Hvc),
            b"smc" => Some(Conduit::Smc),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Conduit::Hvc => "hvc",
            Conduit::Smc => "smc",
        }
    }
}

/// The `CPU_ON` function id every PSCI 0.2 and later implementation uses, in its 64-bit (SMC64)
/// form. PSCI 0.1 had no fixed ids at all and each implementation published its own in the device
/// tree, which is why [`Psci::cpu_on`] prefers the property when there is one.
pub const PSCI_CPU_ON_64: u32 = 0xC400_0003;

/// **What the machine's `/psci` node says.**
///
/// Two facts, and both are needed to start a second core: which instruction reaches the firmware,
/// and what number to put in `x0` when it does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Psci {
    /// The conduit `method` named. `None` when the node does not say, or says something the binding
    /// does not define. Not defaulted: see [`Conduit`].
    pub conduit: Option<Conduit>,
    /// The `CPU_ON` function id: the node's own `cpu_on` property when it has one, otherwise
    /// [`PSCI_CPU_ON_64`] when `compatible` claims 0.2 or later. `None` when neither holds, which is
    /// a PSCI 0.1 node that never published its ids and is therefore unusable.
    pub cpu_on: Option<u32>,
    /// Did the id come from the node's own property rather than from the standard? Worth a boot
    /// line, because it is exactly the case in which a hardcoded id would have been wrong.
    pub cpu_on_from_property: bool,
    /// Does `compatible` claim `arm,psci-0.2` or `arm,psci-1.0`? False for a bare `arm,psci`, which
    /// is 0.1 and shares nothing with the modern specification but the name.
    pub standard: bool,
}

impl Psci {
    /// Read `/psci`, found by name at the root as the binding places it.
    ///
    /// Returns `Ok(None)` when the tree has no PSCI node (spin-table bring-up, or a uniprocessor),
    /// and `Ok(Some(..))` with `None` fields for a node this decoder could not make sense of.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn f(dt: &device_tree_blob::DeviceTreeBlob<'_>) -> Result<(), device_tree_blob::Error> {
    /// use machine_discovery::aarch64::{Conduit, PSCI_CPU_ON_64, Psci};
    /// let psci = Psci::from_device_tree(dt)?.expect("QEMU virt has a /psci node");
    /// assert_eq!(psci.conduit, Some(Conduit::Hvc));
    /// assert_eq!(psci.cpu_on, Some(PSCI_CPU_ON_64));
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_device_tree(
        dt: &device_tree_blob::DeviceTreeBlob<'_>,
    ) -> Result<Option<Psci>, device_tree_blob::Error> {
        let method = dt.node_prop(b"psci", b"method")?;
        let cpu_on = dt.node_prop(b"psci", b"cpu_on")?;
        let compatible = dt.node_prop(b"psci", b"compatible")?;

        // None of the three: either there is no `/psci` node, or there is one holding nothing. The
        // two are indistinguishable through this API and mean the same thing to every caller, so
        // they get the same answer.
        if method.is_none() && cpu_on.is_none() && compatible.is_none() {
            return Ok(None);
        }

        let standard = compatible.is_some_and(|bytes| {
            bytes
                .split(|&b| b == 0)
                .any(|s| matches!(s, b"arm,psci-0.2" | b"arm,psci-1.0"))
        });
        let from_property = cpu_on.and_then(|bytes| {
            (bytes.len() >= 4).then(|| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        });

        Ok(Some(Psci {
            conduit: method.and_then(Conduit::from_property),
            // The property wins over the standard id when both are present, because a node that
            // publishes an id is telling us its own, and a 0.2 implementation publishing the
            // standard number loses nothing by being taken at its word.
            cpu_on: from_property.or(standard.then_some(PSCI_CPU_ON_64)),
            cpu_on_from_property: from_property.is_some(),
            standard,
        }))
    }

    /// Can a core actually be started with this? Both halves have to be known.
    pub fn can_start_a_cpu(&self) -> bool {
        self.conduit.is_some() && self.cpu_on.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build-time duplicate check refuses a code listed twice, apart and not adjacent, so
    /// both of its loops have to run to see it.
    #[test]
    fn the_duplicate_check_refuses_a_code_listed_twice() {
        let row = |code| Implementer { code, name: "" };
        assert!(!codes_are_distinct(&[row(0x41), row(0x42), row(0x41)]));
    }
}
