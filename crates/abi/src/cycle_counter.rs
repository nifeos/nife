/// What a cycle-counter read counts. The discriminant is the word a program sends when it
/// reports a reading, and zero is deliberately not a meaning, so a zeroed message word reads
/// as "no meaning reported" rather than as one of these.
///
/// Name: provisional, minted by milestone 353's lane on 2026-10-07 (UTC). A noun for what a
/// reading means. Considered and refused: `CycleKind` (a kind of cycle says nothing about
/// which privilege levels were counted, the half that differs between architectures),
/// `CounterSource` (names the register, where a reader needs the quantity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum CycleMeaning {
    /// **aarch64: core cycles at EL0 and EL1, not EL2.** `PMCCNTR_EL0` under the
    /// `PMCCFILTR_EL0` value of zero the kernel writes on every core.
    /// User plus kernel, the quantity the other two architectures count.
    CoreCyclesUserAndKernel = 1,
    /// **riscv64: core cycles in every privilege mode**, M-mode firmware included. The
    /// `cycle` CSR, the user-mode shadow of `mcycle`, opened by `scounteren.CY`.
    ///
    /// **Not necessarily the counter the kernel's own bench probe reads.** The kernel asks
    /// firmware for a cycle counter through SBI PMU, and firmware may answer with a
    /// programmable one (`hpmcounter3` on QEMU's `rva23s64` model) rather than `mcycle`.
    /// Both count core cycles, but a program comparing its own reads with the kernel's
    /// `bench-probe` lines cannot tell from this value whether they are one counter. Saying
    /// so needs the kernel to tell the process, which no channel does today.
    CoreCyclesEveryMode = 2,
    /// **`x86_64`: constant-rate reference cycles, not core cycles.** The TSC, read with
    /// `rdtsc`, ambient in ring 3 (who may read the cycle counter is a separate question of
    /// authority). It ticks at a fixed rate whatever the
    /// core's frequency, so a difference of two reads is time in reference cycles, and a
    /// figure in core cycles from either other architecture is a different unit. Reading core
    /// cycles from ring 3 would need `rdpmc` and `CR4.PCE`, which was declined on 2026-10-07.
    ConstantRateReferenceCycles = 3,
}

impl CycleMeaning {
    /// The word this meaning travels as in a message.
    #[must_use]
    pub const fn to_word(self) -> u64 {
        self as u64
    }

    /// The meaning a message word names, or `None` for a word that names none (zero
    /// included, which is what a sender that reported nothing leaves).
    #[must_use]
    pub const fn from_word(word: u64) -> Option<Self> {
        match word {
            1 => Some(Self::CoreCyclesUserAndKernel),
            2 => Some(Self::CoreCyclesEveryMode),
            3 => Some(Self::ConstantRateReferenceCycles),
            _ => None,
        }
    }

    /// Whether a difference of two reads is a count of the core's own clock cycles, as
    /// opposed to a fixed-rate reference clock's.
    #[must_use]
    pub const fn is_core_cycles(self) -> bool {
        !matches!(self, Self::ConstantRateReferenceCycles)
    }
}

#[cfg(test)]
mod tests {
    use super::CycleMeaning;

    /// Every meaning survives the trip through a message word, and zero names none. The
    /// kernel's test decodes the word a program sent, so a variant that did not round-trip
    /// would fail there as "no meaning reported" and send a reader looking in the wrong place.
    #[test]
    fn every_meaning_round_trips_and_zero_is_none() {
        for m in [
            CycleMeaning::CoreCyclesUserAndKernel,
            CycleMeaning::CoreCyclesEveryMode,
            CycleMeaning::ConstantRateReferenceCycles,
        ] {
            assert_eq!(CycleMeaning::from_word(m.to_word()), Some(m));
        }
        assert_eq!(CycleMeaning::from_word(0), None);
        assert_eq!(CycleMeaning::from_word(4), None);
    }

    /// Two meanings count the core's own clock and the third a fixed-rate reference, which is
    /// the one a reader must not compare with the others as the same unit.
    #[test]
    fn only_the_reference_clock_is_not_core_cycles() {
        assert!(CycleMeaning::CoreCyclesUserAndKernel.is_core_cycles());
        assert!(CycleMeaning::CoreCyclesEveryMode.is_core_cycles());
        assert!(!CycleMeaning::ConstantRateReferenceCycles.is_core_cycles());
    }
}
