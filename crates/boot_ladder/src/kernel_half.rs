//! **The kernel's half of the self-test, as the page the kernel hands the progenitor**
//! (milestone 1063 (the boot self-test's spawning checks run from userspace)). The kernel runs the
//! checks of its own internals, prints their lines and writes this page; the progenitor maps it,
//! read-only, into `boot_self_test`, which runs the checks that need a user program and prints the
//! one verdict.
//!
//! The page carries the kernel's tally, the facts the moved checks need and cannot learn at user
//! level (a kernel address to fault on, what the JH7110 bring-up answered), and whether the build
//! injects a failure. Two programs agree on it, so it is code here and not prose (§311 (a contract
//! is code, not prose)). See notes/self-test-checks.md, "The two halves".
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`, for whose half the page carries.

use crate::verdict::Tally;

/// The page's first word: `NIFESTK1` as little-endian bytes. A page without it is not this page.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub const MAGIC: u64 = u64::from_le_bytes(*b"NIFESTK1");

/// **Where each fact sits in the page**, in 64-bit words from its start.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod word {
    /// [`super::MAGIC`].
    ///
    /// Name: provisional.
    pub const MAGIC: usize = 0;
    /// The kernel's tally: passed, failed, absent (`verdict::Tally::to_words`).
    ///
    /// Name: provisional.
    pub const TALLY: usize = 1;
    /// [`super::flag`] bits.
    ///
    /// Name: provisional.
    pub const FLAGS: usize = 4;
    /// A kernel address a user program may not read: the `userspace` check faults on it.
    ///
    /// Name: provisional.
    pub const KERNEL_ADDRESS: usize = 5;
    /// The JH7110 TRNG's register window, physical.
    ///
    /// Name: provisional.
    pub const TRNG_REGISTERS: usize = 6;
    /// The TRNG service's three-word readiness report (`entropy_contract`).
    ///
    /// Name: provisional.
    pub const TRNG_REPORT: usize = 7;
    /// Counter ticks from the start of the TRNG's bring-up to its readiness report.
    ///
    /// Name: provisional.
    pub const TRNG_BRING_UP_TICKS: usize = 10;
    /// The JH7110 clock-and-reset window, physical.
    ///
    /// Name: provisional.
    pub const CLOCK_WINDOW: usize = 11;
    /// The two recorded clock words before the bring-up wrote them, the first in the low half.
    ///
    /// Name: provisional.
    pub const CLOCK_BEFORE: usize = 12;
    /// The same two words read back after.
    ///
    /// Name: provisional.
    pub const CLOCK_AFTER: usize = 13;
    /// How many status reads the reset's release took.
    ///
    /// Name: provisional.
    pub const CLOCK_POLLS: usize = 14;
    /// The number of words in use.
    ///
    /// Name: provisional.
    pub const COUNT: usize = 15;
}

/// **The [`word::FLAGS`] bits.**
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod flag {
    /// The kernel was built with `self_test_injection`: one check in each half fails on purpose.
    ///
    /// Name: provisional.
    pub const INJECT: u64 = 1 << 0;
    /// The device tree names a JH7110 TRNG.
    ///
    /// Name: provisional.
    pub const TRNG_FOUND: u64 = 1 << 1;
    /// The tree marks that TRNG `okay`.
    ///
    /// Name: provisional.
    pub const TRNG_STATUS_OKAY: u64 = 1 << 2;
    /// The archive had no `jh7110_entropy` program, so nothing was wired.
    ///
    /// Name: provisional.
    pub const TRNG_PROGRAM_MISSING: u64 = 1 << 3;
    /// The TRNG service was wired; the progenitor holds its request endpoint.
    ///
    /// Name: provisional.
    pub const TRNG_WIRED: u64 = 1 << 4;
    /// The service sent its readiness report before the kernel's deadline.
    ///
    /// Name: provisional.
    pub const TRNG_ANSWERED: u64 = 1 << 5;
    /// The bring-up had a clock-and-reset window to work on, so [`super::word::CLOCK_BEFORE`] and
    /// the words after it mean something.
    ///
    /// Name: provisional.
    pub const CLOCK_REPORTED: u64 = 1 << 6;
    /// Every recorded clock read back enabled.
    ///
    /// Name: provisional.
    pub const CLOCK_RUNNING: u64 = 1 << 7;
    /// The TRNG's reset was released.
    ///
    /// Name: provisional.
    pub const CLOCK_RELEASED: u64 = 1 << 8;
    /// The firmware had already enabled the clocks and released the reset.
    ///
    /// Name: provisional.
    pub const CLOCK_ALREADY_UP: u64 = 1 << 9;
    /// The clock window's address came from the device tree, not the published constant.
    ///
    /// Name: provisional.
    pub const CLOCK_FROM_TREE: u64 = 1 << 10;
    /// The TRNG's status register says it gives 256-bit answers.
    ///
    /// Name: provisional.
    pub const TRNG_256_BIT: u64 = 1 << 11;
    /// The progenitor holds disk 0's virtio-mmio trio for the `blockdev` check.
    ///
    /// Name: provisional.
    pub const DISK_MMIO: u64 = 1 << 12;
    /// The progenitor holds disk 0's virtio-pci trio for the `blockdev` check.
    ///
    /// Name: provisional.
    pub const DISK_PCI: u64 = 1 << 13;
}

/// **The kernel's half, read out of the page**: the tally and the facts, by name.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelHalf {
    /// What the kernel's checks came to.
    pub tally: Tally,
    /// [`flag`] bits.
    pub flags: u64,
    /// [`word`]'s facts, in order, from [`word::KERNEL_ADDRESS`] on.
    pub words: [u64; word::COUNT],
}

impl KernelHalf {
    /// Whether `bit`, one of [`flag`]'s, is set.
    ///
    /// Name: provisional.
    pub const fn has(&self, bit: u64) -> bool {
        self.flags & bit != 0
    }

    /// The word at `index`, one of [`word`]'s.
    ///
    /// Name: provisional.
    pub const fn word(&self, index: usize) -> u64 {
        self.words[index]
    }

    /// The page's words, ready to copy into a zeroed page.
    ///
    /// Name: provisional.
    pub fn to_words(&self) -> [u64; word::COUNT] {
        let mut w = self.words;
        w[word::MAGIC] = MAGIC;
        let [p, f, a] = self.tally.to_words();
        w[word::TALLY] = p;
        w[word::TALLY + 1] = f;
        w[word::TALLY + 2] = a;
        w[word::FLAGS] = self.flags;
        w
    }

    /// The half from a page's words, or `None` when the magic is wrong, which a reader must treat
    /// as a kernel that ran none of its checks.
    ///
    /// Name: provisional.
    pub fn from_words(w: &[u64; word::COUNT]) -> Option<Self> {
        (w[word::MAGIC] == MAGIC).then(|| Self {
            tally: Tally::from_words([w[word::TALLY], w[word::TALLY + 1], w[word::TALLY + 2]]),
            flags: w[word::FLAGS],
            words: *w,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::{Outcome, check};

    /// What the kernel writes is what the program reads, and a page without the magic reads as
    /// nothing.
    #[test]
    fn the_half_round_trips_through_its_words() {
        let mut tally = Tally::new();
        tally.record(check("timer"), Outcome::Passed);
        let mut half = KernelHalf {
            tally,
            flags: flag::INJECT | flag::DISK_PCI,
            ..KernelHalf::default()
        };
        half.words[word::KERNEL_ADDRESS] = 0xffff_0000_1234_0000;
        let words = half.to_words();
        let back = KernelHalf::from_words(&words).expect("the magic is there");
        assert_eq!(back.tally, tally);
        assert!(back.has(flag::INJECT) && back.has(flag::DISK_PCI) && !back.has(flag::DISK_MMIO));
        assert_eq!(back.word(word::KERNEL_ADDRESS), 0xffff_0000_1234_0000);
        assert_eq!(KernelHalf::from_words(&[0; word::COUNT]), None);
    }
}
