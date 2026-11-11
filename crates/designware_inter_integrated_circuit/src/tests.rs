//! The sequence against a simulated controller and one target with a register file: what is
//! written in which order, what a read returns, and that every stuck wait ends at its bound.
//!
//! Name: provisional.

use super::*;

/// A simulated controller wired to one target, which holds 256 one-byte registers.
struct Simulated {
    identity: u32,
    enabled: u32,
    /// The enable status never follows the enable word.
    enable_stuck: bool,
    /// The transfer aborts with this source instead of stopping.
    abort: Option<u32>,
    /// The stop condition never arrives.
    never_stops: bool,
    /// Every write, in order, as (offset, value).
    writes: Vec<(u32, u32)>,
    /// The target's registers, and the register its next byte is.
    target: [u8; 256],
    pointer: Option<u8>,
    /// Bytes read back, waiting at [`DATA_COMMAND`].
    received: Vec<u8>,
    stopped: bool,
}

impl Simulated {
    fn new() -> Self {
        Self {
            identity: COMPONENT_TYPE_DESIGNWARE,
            enabled: 0,
            enable_stuck: false,
            abort: None,
            never_stops: false,
            writes: Vec::new(),
            target: [0; 256],
            pointer: None,
            received: Vec::new(),
            stopped: false,
        }
    }

    fn data_commands(&self) -> Vec<u32> {
        self.writes
            .iter()
            .filter(|(o, _)| *o == DATA_COMMAND)
            .map(|(_, v)| *v)
            .collect()
    }
}

impl Registers for Simulated {
    fn read(&mut self, offset: u32) -> u32 {
        match offset {
            COMPONENT_TYPE => self.identity,
            ENABLE_STATUS => {
                if self.enable_stuck {
                    self.enabled ^ 1
                } else {
                    self.enabled
                }
            }
            STATUS => {
                let mut w = STATUS_TRANSMIT_NOT_FULL | STATUS_TRANSMIT_EMPTY;
                if !self.received.is_empty() {
                    w |= STATUS_RECEIVE_NOT_EMPTY;
                }
                w
            }
            DATA_COMMAND => u32::from(self.received.remove(0)),
            RAW_INTERRUPT_STATUS => {
                if self.abort.is_some() {
                    INTERRUPT_TRANSMIT_ABORT
                } else if self.stopped && !self.never_stops {
                    INTERRUPT_STOP_DETECTED
                } else {
                    0
                }
            }
            TRANSMIT_ABORT_SOURCE => self.abort.unwrap_or(0),
            _ => 0,
        }
    }

    fn write(&mut self, offset: u32, value: u32) {
        self.writes.push((offset, value));
        match offset {
            ENABLE => self.enabled = value & 1,
            DATA_COMMAND => {
                if value & READ_COMMAND != 0 {
                    let register = self.pointer.unwrap_or(0);
                    self.received.push(self.target[usize::from(register)]);
                    self.pointer = Some(register.wrapping_add(1));
                } else {
                    let byte = value as u8;
                    match self.pointer {
                        None => self.pointer = Some(byte),
                        Some(register) => {
                            self.target[usize::from(register)] = byte;
                            self.pointer = Some(register.wrapping_add(1));
                        }
                    }
                }
                if value & STOP_COMMAND != 0 {
                    self.stopped = true;
                }
            }
            _ => {}
        }
    }
}

const TIMING: Timing = Timing {
    control: 0x63,
    high_count: 0xd8,
    low_count: 0xfd,
    data_hold: 0x0f,
};

fn controller(sim: Simulated) -> Controller<Simulated> {
    Controller::new(sim, 64)
}

#[test]
fn a_register_read_writes_its_number_then_one_read_with_the_stop_on_it() {
    let mut sim = Simulated::new();
    sim.target[0x32] = 0x24;
    let mut c = controller(sim);
    assert_eq!(c.read_register(&TIMING, 0x36, 0x32), Ok(0x24));
    let sim = c.into_registers();
    assert_eq!(sim.data_commands(), [0x32, READ_COMMAND | STOP_COMMAND]);
    assert!(sim.writes.contains(&(TARGET_ADDRESS, 0x36)));
}

#[test]
fn the_timing_and_the_target_are_programmed_while_the_controller_is_disabled() {
    let mut c = controller(Simulated::new());
    c.write_register(&TIMING, 0x36, 0x32, 0x64).unwrap();
    let sim = c.into_registers();
    let at = |offset| sim.writes.iter().position(|(o, _)| *o == offset).unwrap();
    let disabled = sim.writes.iter().position(|w| *w == (ENABLE, 0)).unwrap();
    let enabled = sim.writes.iter().position(|w| *w == (ENABLE, 1)).unwrap();
    for offset in [
        CONTROL,
        STANDARD_SPEED_CLOCK_HIGH_COUNT,
        STANDARD_SPEED_CLOCK_LOW_COUNT,
        DATA_HOLD,
        TARGET_ADDRESS,
    ] {
        assert!(disabled < at(offset) && at(offset) < enabled, "{offset:#x}");
    }
    assert!(sim.writes.contains(&(STANDARD_SPEED_CLOCK_HIGH_COUNT, 0xd8)));
    assert!(sim.writes.contains(&(DATA_HOLD, 0x0f)));
    // Disabled again once the stop arrived.
    assert_eq!(sim.writes.last(), Some(&(ENABLE, 0)));
}

#[test]
fn a_register_write_sends_both_bytes_with_the_stop_on_the_second() {
    let mut c = controller(Simulated::new());
    c.write_register(&TIMING, 0x36, 0x32, 0x64).unwrap();
    let sim = c.into_registers();
    assert_eq!(sim.data_commands(), [0x32, 0x64 | STOP_COMMAND]);
    assert_eq!(sim.target[0x32], 0x64);
}

#[test]
fn a_read_modify_write_keeps_every_other_bit() {
    assert_eq!(with_bit(0x24, 6), 0x64);
    assert_eq!(with_bit(0x64, 6), 0x64);
    assert_eq!(with_bit(0x00, 7), 0x80);
}

#[test]
fn a_page_that_is_not_the_controller_is_refused_before_anything_is_written() {
    let mut sim = Simulated::new();
    sim.identity = 0;
    let mut c = controller(sim);
    assert_eq!(
        c.read_register(&TIMING, 0x36, 0x32),
        Err(Failure::NotADesignWare(0))
    );
    assert!(c.into_registers().writes.is_empty());
}

#[test]
fn a_controller_that_never_takes_the_enable_word_fails_at_the_bound() {
    let mut sim = Simulated::new();
    sim.enable_stuck = true;
    let mut c = controller(sim);
    assert!(matches!(
        c.write_register(&TIMING, 0x36, 0x32, 0x64),
        Err(Failure::EnableStuck(_))
    ));
}

#[test]
fn an_aborted_transfer_names_its_source_and_disables_the_controller() {
    let mut sim = Simulated::new();
    sim.abort = Some(1 << 7);
    let mut c = controller(sim);
    let failure = c.write_register(&TIMING, 0x36, 0x32, 0x64).unwrap_err();
    assert_eq!(
        failure,
        Failure::TransmitAborted {
            source: 1 << 7,
            raw: INTERRUPT_TRANSMIT_ABORT
        }
    );
    assert_eq!(failure.kind(), 7);
    assert_eq!(failure.word(), 1 << 7);
    assert_eq!(c.into_registers().writes.last(), Some(&(ENABLE, 0)));
}

#[test]
fn a_stop_that_never_arrives_fails_at_the_bound_and_disables_the_controller() {
    let mut sim = Simulated::new();
    sim.never_stops = true;
    let mut c = controller(sim);
    assert_eq!(
        c.write_register(&TIMING, 0x36, 0x32, 0x64),
        Err(Failure::StopNeverDetected(0))
    );
    assert_eq!(c.into_registers().writes.last(), Some(&(ENABLE, 0)));
}

#[test]
fn each_failure_kind_has_its_own_number() {
    let kinds = [
        Failure::NotADesignWare(0).kind(),
        Failure::EnableStuck(0).kind(),
        Failure::BusBusy(0).kind(),
        Failure::TransmitterStuck(0).kind(),
        Failure::ReceiverStuck(0).kind(),
        Failure::StopNeverDetected(0).kind(),
        Failure::TransmitAborted { source: 0, raw: 0 }.kind(),
    ];
    assert_eq!(kinds, [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn the_register_map_is_vendor_u_boots() {
    assert_eq!(
        [CONTROL, TARGET_ADDRESS, DATA_COMMAND, RAW_INTERRUPT_STATUS, ENABLE, STATUS],
        [0x00, 0x04, 0x10, 0x34, 0x6c, 0x70]
    );
    assert_eq!([ENABLE_STATUS, COMPONENT_TYPE], [0x9c, 0xfc]);
    assert_eq!(COMPONENT_TYPE_DESIGNWARE, 0x4457_0140);
}
