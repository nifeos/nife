//! A software stand-in for the TRNG, for rehearsing the run under QEMU `virt`.
//!
//! It exists to exercise the payload, the log and the checker end to end on a machine with no
//! JH7110, and it decides nothing about the silicon. Its behavior is this file's choice wherever the
//! sources are silent: a command completes after a few register reads, the block resets unseeded and
//! in 128-bit mode, `RAND_RDY` stays latched until acknowledged, an `AUTO_RQSTS` of 1 raises the
//! request alarm on the next generation, an `AUTO_AGE` of 1 raises the age alarm after a few reads,
//! and the 5000th generation raises `LFSR_LOCKUP`, which a reseed leaves standing and an
//! acknowledgement clears. The output is xorshift64*, which passes the statistics on purpose so the
//! rehearsal's statistics rows pass. A run under the model prints `model=1` on its `begin` record,
//! and `results.py check` refuses such a log as a board result.
//!
//! Name: provisional.

use jh7110_entropy::registers::{
    AUTOMATIC_RESEED_AGE_REGISTER, AUTOMATIC_RESEED_REQUESTS_REGISTER,
    BUILD_CONFIGURATION_REGISTER, CONTROL_REGISTER, FEATURES_REGISTER, INTERRUPT_ENABLE_REGISTER,
    INTERRUPT_STATUS_REGISTER, MODE_REGISTER, RANDOM_NUMBER_0_REGISTER, RANDOM_NUMBER_7_REGISTER,
    STATUS_MODE_REGISTER, STATUS_REGISTER,
};
use jh7110_entropy::{
    CONTROL_COMMAND_GENERATE, CONTROL_COMMAND_RESEED, INTERRUPT_STATUS_AGE_ALARM,
    INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP, INTERRUPT_STATUS_RANDOM_NUMBER_READY,
    INTERRUPT_STATUS_REQUEST_ALARM, INTERRUPT_STATUS_SEED_DONE, MODE_256_BIT_OUTPUT,
    STATUS_256_BIT_OUTPUT, STATUS_GENERATING, STATUS_LAST_RESEED_MASK, STATUS_MISSION_MODE,
    STATUS_RESEEDING, STATUS_SEEDED,
};

/// How many register reads a command takes to complete.
const GENERATE_READS: u32 = 3;
const RESEED_READS: u32 = 6;
/// Which generation raises `LFSR_LOCKUP`.
const LOCKUP_AT_GENERATION: u64 = 5000;

/// The modeled block's state.
pub(crate) struct Model {
    status: u32,
    mode: u32,
    interrupt_status: u32,
    interrupt_enable: u32,
    random_numbers: [u32; 8],
    automatic_requests: u32,
    automatic_age: u32,
    pending: Option<(u32, u32)>,
    generations: u64,
    reads_since_age_set: u32,
    state: u64,
}

impl Model {
    /// The block as it leaves reset: unseeded, `LAST_RESEED` 7, mission mode, 128-bit output.
    pub(crate) fn new() -> Self {
        Model {
            status: STATUS_LAST_RESEED_MASK | STATUS_MISSION_MODE,
            mode: 0,
            interrupt_status: 0,
            interrupt_enable: 0,
            random_numbers: [0; 8],
            automatic_requests: 0,
            automatic_age: 0,
            pending: None,
            generations: 0,
            reads_since_age_set: 0,
            state: 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_word(&mut self) -> u32 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        (self.state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    /// Advances a command in flight by one read, completing it when its count runs out.
    fn tick(&mut self) {
        if self.automatic_age != 0 {
            self.reads_since_age_set += 1;
            if self.reads_since_age_set == 8 {
                self.interrupt_status |= INTERRUPT_STATUS_AGE_ALARM;
            }
        }
        let Some((command, remaining)) = self.pending else {
            return;
        };
        if remaining > 1 {
            self.pending = Some((command, remaining - 1));
            return;
        }
        self.pending = None;
        if command == CONTROL_COMMAND_GENERATE {
            self.status &= !STATUS_GENERATING;
            for i in 0..8 {
                self.random_numbers[i] = self.next_word();
            }
            self.generations += 1;
            self.interrupt_status |= INTERRUPT_STATUS_RANDOM_NUMBER_READY;
            if self.generations == LOCKUP_AT_GENERATION {
                self.interrupt_status |= INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP;
            }
            if self.automatic_requests != 0 {
                self.interrupt_status |= INTERRUPT_STATUS_REQUEST_ALARM;
            }
        } else {
            self.status &= !(STATUS_RESEEDING | STATUS_LAST_RESEED_MASK);
            self.status |= STATUS_SEEDED;
            self.interrupt_status |= INTERRUPT_STATUS_SEED_DONE;
        }
    }

    /// A read of the register at `offset`.
    pub(crate) fn read(&mut self, offset: u64) -> u32 {
        self.tick();
        match offset {
            STATUS_REGISTER => self.status,
            MODE_REGISTER => self.mode,
            STATUS_MODE_REGISTER => STATUS_MISSION_MODE,
            INTERRUPT_ENABLE_REGISTER => self.interrupt_enable,
            INTERRUPT_STATUS_REGISTER => self.interrupt_status,
            FEATURES_REGISTER => 0b1101,
            RANDOM_NUMBER_0_REGISTER..=RANDOM_NUMBER_7_REGISTER => {
                self.random_numbers[((offset - RANDOM_NUMBER_0_REGISTER) / 4) as usize]
            }
            AUTOMATIC_RESEED_REQUESTS_REGISTER => self.automatic_requests,
            AUTOMATIC_RESEED_AGE_REGISTER => self.automatic_age,
            BUILD_CONFIGURATION_REGISTER => 0b11_0100,
            _ => 0,
        }
    }

    /// A write of `value` to the register at `offset`.
    pub(crate) fn write(&mut self, offset: u64, value: u32) {
        match offset {
            CONTROL_REGISTER if value == CONTROL_COMMAND_GENERATE && self.pending.is_none() => {
                self.status |= STATUS_GENERATING;
                self.pending = Some((value, GENERATE_READS));
            }
            CONTROL_REGISTER if value == CONTROL_COMMAND_RESEED && self.pending.is_none() => {
                self.status |= STATUS_RESEEDING;
                self.pending = Some((value, RESEED_READS));
            }
            MODE_REGISTER => {
                self.mode = value;
                self.status &= !STATUS_256_BIT_OUTPUT;
                if value & MODE_256_BIT_OUTPUT != 0 {
                    self.status |= STATUS_256_BIT_OUTPUT;
                }
            }
            INTERRUPT_ENABLE_REGISTER => self.interrupt_enable = value,
            INTERRUPT_STATUS_REGISTER => self.interrupt_status &= !value,
            AUTOMATIC_RESEED_REQUESTS_REGISTER => self.automatic_requests = value,
            AUTOMATIC_RESEED_AGE_REGISTER => {
                self.automatic_age = value;
                self.reads_since_age_set = 0;
            }
            _ => {}
        }
    }
}
