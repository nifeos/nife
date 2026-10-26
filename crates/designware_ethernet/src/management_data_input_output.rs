//! **The management data input/output (MDIO) bus, through the MAC's own two registers**, and the
//! IEEE 802.3 clause 22 registers every PHY (physical layer device) has. IEEE 802.3 clause 22
//! names the bus "management data input/output"; `MDIO` is its abbreviation there.
//!
//! The command word and the busy-bit poll are OpenBSD's `dwqe_mii_readreg` and
//! `dwqe_mii_writereg` (`sys/dev/ic/dwqe.c`, ISC license, notice in the crate root): the data
//! first on a write, then the address with the busy bit set, then wait for the controller to clear
//! it. The clause 22 register numbers and bits are the standard's.

use crate::{Hardware, registers};

/// The MDC clock range field's shift in [`registers::MAC_MDIO_ADDR`].
pub const CR_SHIFT: u32 = 8;
/// **The clock range this driver always programs**: `0b0111`, "CSR clock 500 to 800 MHz",
/// which divides the CSR clock by 324.
///
/// OpenBSD reads the `stmmaceth` clock's rate and picks the matching range. This kernel has no
/// clock framework to ask, and the choice only has to keep MDC at or under the standard's 2.5 MHz:
/// the largest divider does that for every CSR clock up to 810 MHz and merely runs MDC slower
/// below, which MDIO tolerates. So the driver needs no rate at all, at the cost of slower PHY
/// register access during bring-up (tens of microseconds per access rather than a few), which is
/// off every path that repeats.
pub const CR_500_800: u32 = 7;
/// The PHY address field's shift.
pub const PA_SHIFT: u32 = 21;
/// The register address field's shift.
pub const RDA_SHIFT: u32 = 16;
/// Command: read.
pub const GOC_READ: u32 = 3 << 2;
/// Command: write.
pub const GOC_WRITE: u32 = 1 << 2;
/// Busy: set by the driver to start a command, cleared by the controller when it is done.
pub const GB: u32 = 1 << 0;

/// How long one MDIO command may take before it is a timeout: OpenBSD's 2000 polls 10 µs apart.
pub const COMMAND_LIMIT_US: u64 = 20_000;
/// The poll interval.
const POLL_US: u64 = 10;

/// Clause 22: basic mode control.
pub(crate) const BMCR: u8 = 0x00;
/// Restart autonegotiation; self-clearing.
pub(crate) const BMCR_ANRESTART: u16 = 1 << 9;
/// Autonegotiation enable.
pub(crate) const BMCR_ANENABLE: u16 = 1 << 12;
/// Software reset of the PHY; self-clearing.
pub const BMCR_RESET: u16 = 1 << 15;
/// Clause 22: basic mode status.
pub(crate) const BMSR: u8 = 0x01;
/// Link status. **Latched low**: a link that dropped since the last read reads zero once, so the
/// current state is the second of two reads.
pub(crate) const BMSR_LINK: u16 = 1 << 2;
/// Autonegotiation complete.
pub const BMSR_ANEG_COMPLETE: u16 = 1 << 5;
/// Clause 22: PHY identifier, high half.
pub(crate) const PHYSID1: u8 = 0x02;
/// Clause 22: PHY identifier, low half (the low four bits are the revision).
pub(crate) const PHYSID2: u8 = 0x03;

/// Why an MDIO command did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The busy bit stayed set for [`COMMAND_LIMIT_US`]: the MAC's CSR clock is off, or the
    /// command never started.
    Timeout {
        /// The PHY addressed.
        physical_layer_device: u8,
        /// The register addressed.
        register: u8,
    },
}

/// **An MDIO command word**: the clock range, the PHY and register addresses, the operation, and
/// the busy bit that starts it. Out-of-range addresses are reduced to the field (5 bits each)
/// rather than spilling into the neighbors.
pub const fn command(physical_layer_device: u8, register: u8, write: bool) -> u32 {
    (CR_500_800 << CR_SHIFT)
        | (((physical_layer_device & 0x1f) as u32) << PA_SHIFT)
        | (((register & 0x1f) as u32) << RDA_SHIFT)
        | if write { GOC_WRITE } else { GOC_READ }
        | GB
}

/// **A management data input/output (MDIO) bus**: a clause 22 PHY register file, reached somehow.
/// The MAC's MDIO pair is one implementation ([`Bus`]); a test's simulated PHY is another.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing
/// `Mdio`.
#[doc(alias = "mdio")]
pub trait ManagementDataInputOutput {
    /// Read register `register` of the PHY at `physical_layer_device`.
    fn read(&mut self, physical_layer_device: u8, register: u8) -> Result<u16, Error>;
    /// Write `v` to register `register` of the PHY at `physical_layer_device`.
    fn write(&mut self, physical_layer_device: u8, register: u8, v: u16) -> Result<(), Error>;
}

/// The MAC's MDIO registers, through whatever [`Hardware`] reaches the controller.
pub struct Bus<'a, H: Hardware>(pub &'a mut H);

impl<H: Hardware> Bus<'_, H> {
    fn wait(&mut self, physical_layer_device: u8, register: u8) -> Result<(), Error> {
        let mut waited = 0;
        loop {
            if self.0.read(registers::MAC_MDIO_ADDR) & GB == 0 {
                return Ok(());
            }
            if waited >= COMMAND_LIMIT_US {
                return Err(Error::Timeout {
                    physical_layer_device,
                    register,
                });
            }
            self.0.delay_us(POLL_US);
            waited += POLL_US;
        }
    }
}

impl<H: Hardware> ManagementDataInputOutput for Bus<'_, H> {
    fn read(&mut self, physical_layer_device: u8, register: u8) -> Result<u16, Error> {
        // A command still running from someone else (firmware) would be overwritten mid-flight.
        self.wait(physical_layer_device, register)?;
        self.0.write(
            registers::MAC_MDIO_ADDR,
            command(physical_layer_device, register, false),
        );
        self.wait(physical_layer_device, register)?;
        Ok(self.0.read(registers::MAC_MDIO_DATA) as u16)
    }

    fn write(&mut self, physical_layer_device: u8, register: u8, v: u16) -> Result<(), Error> {
        self.wait(physical_layer_device, register)?;
        self.0.write(registers::MAC_MDIO_DATA, u32::from(v));
        self.0.write(
            registers::MAC_MDIO_ADDR,
            command(physical_layer_device, register, true),
        );
        self.wait(physical_layer_device, register)
    }
}

/// **The PHY's identifier**, `PHYSID1 << 16 | PHYSID2`, revision included.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `phy` spelled out as IEEE 802.3's
/// `physical_layer_device`, replacing `phy_id`.
#[doc(alias = "phy")]
pub fn physical_layer_device_id(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device: u8,
) -> Result<u32, Error> {
    let high = bus.read(physical_layer_device, PHYSID1)?;
    let low = bus.read(physical_layer_device, PHYSID2)?;
    Ok(u32::from(high) << 16 | u32::from(low))
}

/// **Is the link up right now?** Two reads of the latched-low status bit, so a drop since the last
/// look does not read as down forever and a link that is up now reads as up.
pub fn link_is_up(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device: u8,
) -> Result<bool, Error> {
    let _ = bus.read(physical_layer_device, BMSR)?;
    Ok(bus.read(physical_layer_device, BMSR)? & BMSR_LINK != 0)
}

/// **Restart autonegotiation**, advertising whatever the PHY's own defaults advertise (every speed
/// on a gigabit part).
pub fn restart_autonegotiation(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device: u8,
) -> Result<(), Error> {
    let bmcr = bus.read(physical_layer_device, BMCR)?;
    bus.write(
        physical_layer_device,
        BMCR,
        bmcr | BMCR_ANENABLE | BMCR_ANRESTART,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_command_carries_the_largest_divider_the_addresses_and_the_busy_bit() {
        let c = command(0, PHYSID1, false);
        assert_eq!(c, 7 << 8 | 2 << 16 | 3 << 2 | 1);
        let w = command(31, 31, true);
        assert_eq!(w, 7 << 8 | 31 << 21 | 31 << 16 | 1 << 2 | 1);
    }

    use std::vec::Vec;

    /// The MAC's two MDIO registers and nothing else: every write is logged in order, a command's
    /// busy bit stays set for `busy_reads` reads of the address register, and a read command
    /// leaves `answer` in the data register when it completes.
    struct Pair {
        address: u32,
        data: u32,
        answer: u16,
        busy_reads: u32,
        stuck: bool,
        writes: Vec<(u32, u32)>,
        waited_us: u64,
    }

    impl Pair {
        fn new(answer: u16, busy_reads: u32) -> Pair {
            Pair {
                address: 0,
                data: 0,
                answer,
                busy_reads,
                stuck: false,
                writes: Vec::new(),
                waited_us: 0,
            }
        }
    }

    impl Hardware for Pair {
        fn read(&mut self, offset: u32) -> u32 {
            match offset {
                registers::MAC_MDIO_ADDR => {
                    if self.address & GB != 0 && !self.stuck {
                        if self.busy_reads == 0 {
                            self.address &= !GB;
                            if self.address & (0x3 << 2) == GOC_READ {
                                self.data = u32::from(self.answer);
                            }
                        } else {
                            self.busy_reads -= 1;
                        }
                    }
                    self.address
                }
                registers::MAC_MDIO_DATA => self.data,
                _ => panic!("MDIO touched {offset:#x}, outside its two registers"),
            }
        }
        fn write(&mut self, offset: u32, v: u32) {
            self.writes.push((offset, v));
            match offset {
                registers::MAC_MDIO_ADDR => self.address = v,
                registers::MAC_MDIO_DATA => self.data = v,
                _ => panic!("MDIO wrote {offset:#x}, outside its two registers"),
            }
        }
        fn delay_us(&mut self, us: u64) {
            self.waited_us += us;
        }
    }

    #[test]
    fn a_read_issues_one_read_command_and_returns_the_data_register() {
        let mut hardware = Pair::new(0xbeef, 3);
        assert_eq!(Bus(&mut hardware).read(4, BMSR), Ok(0xbeef));
        assert_eq!(
            hardware.writes,
            [(registers::MAC_MDIO_ADDR, command(4, BMSR, false))]
        );
        assert_eq!(
            hardware.waited_us,
            3 * POLL_US,
            "it waited out the busy bit, poll by poll"
        );
    }

    #[test]
    fn a_write_puts_the_data_down_before_the_command_that_sends_it() {
        // The other order would send whatever the data register held before.
        let mut hardware = Pair::new(0, 2);
        Bus(&mut hardware).write(1, BMCR, 0x1200).unwrap();
        assert_eq!(
            hardware.writes,
            [
                (registers::MAC_MDIO_DATA, 0x1200),
                (registers::MAC_MDIO_ADDR, command(1, BMCR, true))
            ]
        );
    }

    #[test]
    fn a_command_still_busy_from_someone_else_is_waited_out_not_overwritten() {
        let mut hardware = Pair::new(0x1234, 2);
        hardware.address = command(7, 9, false); // firmware's read, in flight
        assert_eq!(Bus(&mut hardware).read(0, PHYSID1), Ok(0x1234));
        // Nothing was written until the firmware's command had cleared its busy bit.
        assert_eq!(hardware.writes.len(), 1);
        assert!(hardware.waited_us >= 2 * POLL_US);
    }

    #[test]
    fn a_busy_bit_that_never_clears_is_a_bounded_timeout_naming_the_access() {
        let mut hardware = Pair::new(0, 0);
        hardware.stuck = true;
        hardware.address = GB;
        assert_eq!(
            Bus(&mut hardware).read(3, BMSR),
            Err(Error::Timeout {
                physical_layer_device: 3,
                register: BMSR
            })
        );
        assert_eq!(hardware.waited_us, COMMAND_LIMIT_US);
        assert!(
            hardware.writes.is_empty(),
            "a bus held busy was never written"
        );
        let mut hardware = Pair::new(0, 0);
        hardware.stuck = true;
        assert_eq!(
            Bus(&mut hardware).write(2, BMCR, 1),
            Err(Error::Timeout {
                physical_layer_device: 2,
                register: BMCR
            })
        );
        assert_eq!(hardware.waited_us, COMMAND_LIMIT_US);
    }

    /// A clause 22 register file with a latched-low link bit, for the helpers above the bus.
    struct Latched {
        registers: [u16; 32],
        link_now: bool,
        dropped_since_read: bool,
    }

    impl ManagementDataInputOutput for Latched {
        fn read(&mut self, _physical_layer_device: u8, register: u8) -> Result<u16, Error> {
            if register == BMSR {
                let up = self.link_now && !self.dropped_since_read;
                self.dropped_since_read = false;
                return Ok(if up { BMSR_LINK } else { 0 });
            }
            Ok(self.registers[register as usize])
        }
        fn write(&mut self, _physical_layer_device: u8, register: u8, v: u16) -> Result<(), Error> {
            self.registers[register as usize] = v;
            Ok(())
        }
    }

    #[test]
    fn a_link_that_dropped_and_came_back_reads_up_on_the_second_read() {
        let mut physical_layer_device = Latched {
            registers: [0; 32],
            link_now: true,
            dropped_since_read: true,
        };
        assert!(link_is_up(&mut physical_layer_device, 0).unwrap());
        physical_layer_device.link_now = false;
        assert!(!link_is_up(&mut physical_layer_device, 0).unwrap());
    }

    #[test]
    fn the_identifier_is_the_high_register_then_the_low() {
        let mut physical_layer_device = Latched {
            registers: [0; 32],
            link_now: false,
            dropped_since_read: false,
        };
        physical_layer_device.registers[PHYSID1 as usize] = 0x4f51;
        physical_layer_device.registers[PHYSID2 as usize] = 0xe91b;
        assert_eq!(
            physical_layer_device_id(&mut physical_layer_device, 0),
            Ok(0x4f51_e91b)
        );
    }

    #[test]
    fn restarting_autonegotiation_keeps_the_other_control_bits() {
        let mut physical_layer_device = Latched {
            registers: [0; 32],
            link_now: false,
            dropped_since_read: false,
        };
        physical_layer_device.registers[BMCR as usize] = 0x0100; // full duplex, set by someone else
        restart_autonegotiation(&mut physical_layer_device, 0).unwrap();
        assert_eq!(
            physical_layer_device.registers[BMCR as usize],
            0x0100 | BMCR_ANENABLE | BMCR_ANRESTART
        );
    }

    #[test]
    fn an_address_too_wide_for_its_field_does_not_spill_into_the_next() {
        // PHY 33 is PHY 1; it must not set bit 26, which is outside the 5-bit field.
        assert_eq!(command(33, 0, false), command(1, 0, false));
        assert_eq!(command(0, 0x3f, false) & (1 << PA_SHIFT), 0);
    }

    /// A restart asked of a PHY already negotiating (both bits read set) leaves both set rather
    /// than toggling them off.
    #[test]
    fn restarting_a_negotiation_already_running_keeps_it_running() {
        let mut physical_layer_device = Latched {
            registers: [0; 32],
            link_now: false,
            dropped_since_read: false,
        };
        physical_layer_device.registers[BMCR as usize] = 0x1300;
        restart_autonegotiation(&mut physical_layer_device, 0).unwrap();
        assert_eq!(physical_layer_device.registers[BMCR as usize], 0x1300);
    }

    /// Clause 22's bits as IEEE 802.3 numbers them (22.2.4.1 and 22.2.4.2), written out: the
    /// simulated PHY reads the same names, so only a literal tells a misplaced bit from a right
    /// one.
    #[test]
    fn the_clause_22_bits_are_the_standards() {
        assert_eq!(
            [
                BMCR_ANRESTART,
                BMCR_ANENABLE,
                BMCR_RESET,
                BMSR_LINK,
                BMSR_ANEG_COMPLETE
            ],
            [0x0200, 0x1000, 0x8000, 0x0004, 0x0020]
        );
    }
}
