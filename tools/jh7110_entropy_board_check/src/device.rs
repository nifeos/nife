//! The TRNG's register window: volatile 32-bit accesses at the base the device tree names, or the
//! software model in [`crate::model`] when the `model` feature is on.
//!
//! Every offset comes from `jh7110_entropy::registers`, so the checks touch only what the crate under
//! test names. [`Trng::snapshot`] reads `STAT`, `ISTAT` and `STAT` again, in that order, because a
//! single `(STAT, ISTAT)` pair is two reads and a bit can change between them.
//!
//! Name: provisional.

use jh7110_entropy::registers::{
    CONTROL_REGISTER, INTERRUPT_STATUS_REGISTER, RANDOM_NUMBER_0_REGISTER, STATUS_REGISTER,
};

/// The register window under test.
pub(crate) struct Trng {
    #[cfg_attr(feature = "model", allow(dead_code))]
    base: u64,
    #[cfg(feature = "model")]
    model: crate::model::Model,
}

/// Two reads of `STAT` with one of `ISTAT` between them.
#[derive(Clone, Copy)]
pub(crate) struct Snapshot {
    pub(crate) status_before: u32,
    pub(crate) interrupt_status: u32,
    pub(crate) status_after: u32,
}

impl Trng {
    /// The window at physical address `base`, which the hart reaches with paging off.
    pub(crate) fn new(base: u64) -> Self {
        Trng {
            base,
            #[cfg(feature = "model")]
            model: crate::model::Model::new(),
        }
    }

    /// Reads the 32-bit register at `offset`.
    pub(crate) fn read(&mut self, offset: u64) -> u32 {
        #[cfg(feature = "model")]
        {
            self.model.read(offset)
        }
        #[cfg(not(feature = "model"))]
        {
            // SAFETY: `base` is the TRNG window from the device tree (or the binding's address),
            // reached physically, and `offset` is one of the crate's register offsets, inside the
            // window's 0x4000 bytes.
            unsafe { core::ptr::read_volatile((self.base + offset) as *const u32) }
        }
    }

    /// Writes `value` to the 32-bit register at `offset`.
    pub(crate) fn write(&mut self, offset: u64, value: u32) {
        #[cfg(feature = "model")]
        {
            self.model.write(offset, value);
        }
        #[cfg(not(feature = "model"))]
        {
            // SAFETY: as in `read`.
            unsafe { core::ptr::write_volatile((self.base + offset) as *mut u32, value) }
        }
    }

    /// `STAT`, then `ISTAT`, then `STAT`.
    pub(crate) fn snapshot(&mut self) -> Snapshot {
        let status_before = self.read(STATUS_REGISTER);
        let interrupt_status = self.read(INTERRUPT_STATUS_REGISTER);
        let status_after = self.read(STATUS_REGISTER);
        Snapshot {
            status_before,
            interrupt_status,
            status_after,
        }
    }

    /// The eight `RAND` words, word 0 first.
    pub(crate) fn random_numbers(&mut self) -> [u32; 8] {
        let mut words = [0u32; 8];
        for (i, word) in words.iter_mut().enumerate() {
            *word = self.read(RANDOM_NUMBER_0_REGISTER + 4 * i as u64);
        }
        words
    }

    /// Writes `bits` to `ISTAT`, which is write-1-to-clear: it clears exactly those bits.
    pub(crate) fn acknowledge(&mut self, bits: u32) {
        self.write(INTERRUPT_STATUS_REGISTER, bits);
    }

    /// Writes `command` to `CTRL`.
    pub(crate) fn command(&mut self, command: u32) {
        self.write(CONTROL_REGISTER, command);
    }
}
