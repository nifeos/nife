//! **The management data input/output (MDIO) bus, through the MAC's own two registers**, and the
//! IEEE 802.3 clause 22 registers every PHY (physical layer device) has. IEEE 802.3 clause 22
//! names the bus "management data input/output"; `MDIO` is its abbreviation there.
//!
//! The command word and the busy-bit poll are OpenBSD's `dwqe_mii_readreg` and
//! `dwqe_mii_writereg` (`sys/dev/ic/dwqe.c`, ISC license, notice in the crate root): the data
//! first on a write, then the address with the busy bit set, then wait for the controller to clear
//! it. The clause 22 register numbers and bits are the standard's, named as Linux's
//! `include/uapi/linux/mii.h` names them. Each constant keeps its source's name without the
//! family prefix, `GMAC_MAC_MDIO_ADDR_` or `MII_`, and its documentation opens with its full name
//! and its source's spelling.
//!
//! Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, IEEE 802.3's
//! phrase, replacing `mdio`.

use crate::{Hardware, registers};

/// **CSR clock range shift** (`GMAC_MAC_MDIO_ADDR_CR_SHIFT`, `dwqereg.h`), the MDC clock range
/// field in [`registers::MAC_MDIO_ADDR`].
#[doc(alias = "csr clock range shift")]
pub const CR_SHIFT: u32 = 8;
/// **MDIO clock range for a 500 to 800 MHz CSR clock** (`GMAC_MAC_MDIO_ADDR_CR_500_800`,
/// `dwqereg.h`), the range this driver always programs: `0b0111`, which divides the CSR clock by
/// 324.
///
/// OpenBSD reads the `stmmaceth` clock's rate and picks the matching range. This kernel has no
/// clock framework to ask, and the choice only has to keep MDC at or under the standard's 2.5 MHz:
/// the largest divider does that for every CSR clock up to 810 MHz and merely runs MDC slower
/// below, which MDIO tolerates. So the driver needs no rate at all, at the cost of slower PHY
/// register access during bring-up (tens of microseconds per access rather than a few), which is
/// off every path that repeats.
#[doc(alias = "mdio clock range")]
pub const CR_500_800: u32 = 7;
/// **MDIO physical layer device address shift** (`GMAC_MAC_MDIO_ADDR_PA_SHIFT`, `dwqereg.h`).
#[doc(alias = "mdio physical layer device address shift")]
pub const PA_SHIFT: u32 = 21;
/// **MDIO register address shift** (`GMAC_MAC_MDIO_ADDR_RDA_SHIFT`, `dwqereg.h`).
#[doc(alias = "mdio register address shift")]
pub const RDA_SHIFT: u32 = 16;
/// **GMII operation command, read** (`GMAC_MAC_MDIO_ADDR_GOC_READ`, `dwqereg.h`).
#[doc(alias = "gmii operation command, read")]
pub const GOC_READ: u32 = 3 << 2;
/// **GMII operation command, write** (`GMAC_MAC_MDIO_ADDR_GOC_WRITE`, `dwqereg.h`).
#[doc(alias = "gmii operation command, write")]
pub const GOC_WRITE: u32 = 1 << 2;
/// **GMII busy** (`GMAC_MAC_MDIO_ADDR_GB`, `dwqereg.h`): set by the driver to start a command,
/// cleared by the controller when it is done.
#[doc(alias = "gmii busy")]
pub const GB: u32 = 1 << 0;

/// How long one MDIO command may take before it is a timeout: OpenBSD's 2000 polls 10 µs apart.
pub const COMMAND_LIMIT_MICROSECONDS: u64 = 20_000;
/// The poll interval.
const POLL_MICROSECONDS: u64 = 10;

/// **Basic mode control register** (`MII_BMCR`, `mii.h`), clause 22.
#[doc(alias = "basic mode control register")]
pub(crate) const BMCR: u8 = 0x00;
/// **Autonegotiation restart** (`BMCR_ANRESTART`, `mii.h`), self-clearing.
#[doc(alias = "autonegotiation restart")]
pub(crate) const BMCR_ANRESTART: u16 = 1 << 9;
/// **Autonegotiation enable** (`BMCR_ANENABLE`, `mii.h`).
#[doc(alias = "autonegotiation enable")]
pub(crate) const BMCR_ANENABLE: u16 = 1 << 12;
/// **Reset to the default state** (`BMCR_RESET`, `mii.h`): a software reset of the PHY,
/// self-clearing.
#[doc(alias = "reset to the default state")]
pub const BMCR_RESET: u16 = 1 << 15;
/// **Basic mode status register** (`MII_BMSR`, `mii.h`), clause 22.
#[doc(alias = "basic mode status register")]
pub(crate) const BMSR: u8 = 0x01;
/// **Link status** (`BMSR_LSTATUS`, `mii.h`), latched low: a link that dropped since the last read
/// reads zero once, so the current state is the second of two reads.
#[doc(alias = "link status")]
pub(crate) const BMSR_LSTATUS: u16 = 1 << 2;
/// **Isolate** (`BMCR_ISO`, OpenBSD `sys/dev/mii/mii.h`): the PHY is electrically isolated from
/// the MAC.
#[doc(alias = "isolate")]
pub(crate) const BMCR_ISO: u16 = 0x0400;
/// **Set duplex mode** (`BMCR_FDX`, OpenBSD `sys/dev/mii/mii.h`): full duplex when forced.
#[doc(alias = "full duplex")]
pub(crate) const BMCR_FDX: u16 = 0x0100;
/// **100 Mb/s** (`BMCR_S100`, OpenBSD `sys/dev/mii/mii.h`): the forced speed's low selection bit.
#[doc(alias = "forced 100 mbit/s")]
pub(crate) const BMCR_S100: u16 = 0x2000;
/// **1000 Mb/s** (`BMCR_S1000`, OpenBSD `sys/dev/mii/mii.h`): the forced speed's high selection
/// bit.
#[doc(alias = "forced 1000 mbit/s")]
pub(crate) const BMCR_S1000: u16 = 0x0040;
/// **Autonegotiation advertisement register** (`MII_ANAR`, OpenBSD `sys/dev/mii/mii.h`), clause 28.
/// The crate drops the `MII_` prefix, as it does for `BMCR`.
#[doc(alias = "autonegotiation advertisement")]
pub(crate) const ANAR: u8 = 0x04;
/// **Autonegotiation link partner abilities register** (`MII_ANLPAR`, OpenBSD
/// `sys/dev/mii/mii.h`), clause 28.
#[doc(alias = "autonegotiation link partner abilities")]
pub(crate) const ANLPAR: u8 = 0x05;
/// **Link partner supports 100BASE-T4** (`ANLPAR_T4`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "100base-t4")]
pub(crate) const ANLPAR_T4: u16 = 0x0200;
/// **Link partner supports 100BASE-TX full duplex** (`ANLPAR_TX_FD`, OpenBSD
/// `sys/dev/mii/mii.h`).
#[doc(alias = "100base-tx full duplex")]
pub(crate) const ANLPAR_TX_FD: u16 = 0x0100;
/// **Link partner supports 100BASE-TX** (`ANLPAR_TX`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "100base-tx")]
pub(crate) const ANLPAR_TX: u16 = 0x0080;
/// **Link partner supports 10BASE-T full duplex** (`ANLPAR_10_FD`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "10base-t full duplex")]
pub(crate) const ANLPAR_10_FD: u16 = 0x0040;
/// **Link partner supports 10BASE-T** (`ANLPAR_10`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "10base-t")]
pub(crate) const ANLPAR_10: u16 = 0x0020;
/// **1000BASE-T control register** (`MII_100T2CR`, OpenBSD `sys/dev/mii/mii.h`), clause 40. The
/// name keeps `MII_` because an identifier cannot begin with a digit.
#[doc(alias = "1000base-t control")]
pub(crate) const MII_100T2CR: u8 = 0x09;
/// **Advertise 1000BASE-T full duplex** (`GTCR_ADV_1000TFDX`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "advertise 1000base-t full duplex")]
pub(crate) const GTCR_ADV_1000TFDX: u16 = 0x0200;
/// **Advertise 1000BASE-T half duplex** (`GTCR_ADV_1000THDX`, OpenBSD `sys/dev/mii/mii.h`).
#[doc(alias = "advertise 1000base-t half duplex")]
pub(crate) const GTCR_ADV_1000THDX: u16 = 0x0100;
/// **1000BASE-T status register** (`MII_100T2SR`, OpenBSD `sys/dev/mii/mii.h`), clause 40.
#[doc(alias = "1000base-t status")]
pub(crate) const MII_100T2SR: u8 = 0x0a;
/// **Link partner 1000BASE-T full duplex capable** (`GTSR_LP_1000TFDX`, OpenBSD
/// `sys/dev/mii/mii.h`).
#[doc(alias = "link partner 1000base-t full duplex")]
pub(crate) const GTSR_LP_1000TFDX: u16 = 0x0800;
/// **Link partner 1000BASE-T half duplex capable** (`GTSR_LP_1000THDX`, OpenBSD
/// `sys/dev/mii/mii.h`).
#[doc(alias = "link partner 1000base-t half duplex")]
pub(crate) const GTSR_LP_1000THDX: u16 = 0x0400;
/// **Autonegotiation complete** (`BMSR_ANEGCOMPLETE`, `mii.h`).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `BMSR_ANEG_COMPLETE`.
#[doc(alias = "autonegotiation complete")]
pub const BMSR_ANEGCOMPLETE: u16 = 1 << 5;
/// **Physical layer device identifier 1** (`MII_PHYSID1`, `mii.h`), clause 22: the
/// identifier's high half.
#[doc(alias = "physical layer device identifier 1")]
pub(crate) const PHYSID1: u8 = 0x02;
/// **Physical layer device identifier 2** (`MII_PHYSID2`, `mii.h`), clause 22: the
/// identifier's low half (the low four bits are the revision).
#[doc(alias = "physical layer device identifier 2")]
pub(crate) const PHYSID2: u8 = 0x03;

/// Why an MDIO command did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The busy bit stayed set for [`COMMAND_LIMIT_MICROSECONDS`]: the MAC's CSR clock is off, or the
    /// command never started.
    Timeout {
        /// The PHY (physical layer device) addressed, by its bus address.
        ///
        /// Name: ratified 2026-10-26 (#2762), Decision 50: a `u8` PHY bus address is
        /// `physical_layer_device_address`, replacing `phy`.
        #[doc(alias = "phy")]
        physical_layer_device_address: u8,
        /// The register addressed.
        register: u8,
    },
}

/// **An MDIO command word**: the clock range, the PHY and register addresses, the operation, and
/// the busy bit that starts it. Out-of-range addresses are reduced to the field (5 bits each)
/// rather than spilling into the neighbors.
pub const fn command(physical_layer_device_address: u8, register: u8, write: bool) -> u32 {
    (CR_500_800 << CR_SHIFT)
        | (((physical_layer_device_address & 0x1f) as u32) << PA_SHIFT)
        | (((register & 0x1f) as u32) << RDA_SHIFT)
        | if write { GOC_WRITE } else { GOC_READ }
        | GB
}

// The command word's five fields share no bit, so its `|`s are its `^`s: the clock range, the PHY
// and register addresses (five bits each, reduced by mask), the operation, and the busy bit. Each
// operand enters through its own field mask, of which this is the pairwise disjointness.
const _: () = assert!(
    (CR_500_800 << CR_SHIFT) | (0x1f << PA_SHIFT) | (0x1f << RDA_SHIFT) | (0x3 << 2) | GB
        == (CR_500_800 << CR_SHIFT) ^ (0x1f << PA_SHIFT) ^ (0x1f << RDA_SHIFT) ^ (0x3 << 2) ^ GB
);

/// **A management data input/output (MDIO) bus**: a clause 22 PHY register file, reached somehow.
/// The MAC's MDIO pair is one implementation ([`Bus`]); a test's simulated PHY is another.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing
/// `Mdio`.
#[doc(alias = "mdio")]
pub trait ManagementDataInputOutput {
    /// Read register `register` of the PHY at `physical_layer_device_address`.
    fn read(&mut self, physical_layer_device_address: u8, register: u8) -> Result<u16, Error>;
    /// Write `v` to register `register` of the PHY at `physical_layer_device_address`.
    fn write(
        &mut self,
        physical_layer_device_address: u8,
        register: u8,
        v: u16,
    ) -> Result<(), Error>;
}

/// The MAC's MDIO registers, through whatever [`Hardware`] reaches the controller.
pub struct Bus<'a, H: Hardware>(pub &'a mut H);

impl<H: Hardware> Bus<'_, H> {
    fn wait(&mut self, physical_layer_device_address: u8, register: u8) -> Result<(), Error> {
        let mut waited = 0;
        loop {
            if self.0.read(registers::MAC_MDIO_ADDR) & GB == 0 {
                return Ok(());
            }
            if waited >= COMMAND_LIMIT_MICROSECONDS {
                return Err(Error::Timeout {
                    physical_layer_device_address,
                    register,
                });
            }
            self.0.delay_microseconds(POLL_MICROSECONDS);
            waited += POLL_MICROSECONDS;
        }
    }
}

impl<H: Hardware> ManagementDataInputOutput for Bus<'_, H> {
    fn read(&mut self, physical_layer_device_address: u8, register: u8) -> Result<u16, Error> {
        // A command still running from someone else (firmware) would be overwritten mid-flight.
        self.wait(physical_layer_device_address, register)?;
        self.0.write(
            registers::MAC_MDIO_ADDR,
            command(physical_layer_device_address, register, false),
        );
        self.wait(physical_layer_device_address, register)?;
        Ok(self.0.read(registers::MAC_MDIO_DATA) as u16)
    }

    fn write(
        &mut self,
        physical_layer_device_address: u8,
        register: u8,
        v: u16,
    ) -> Result<(), Error> {
        self.wait(physical_layer_device_address, register)?;
        self.0.write(registers::MAC_MDIO_DATA, u32::from(v));
        self.0.write(
            registers::MAC_MDIO_ADDR,
            command(physical_layer_device_address, register, true),
        );
        self.wait(physical_layer_device_address, register)
    }
}

/// **The PHY's identifier**, `PHYSID1 << 16 | PHYSID2`, revision included.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `phy` spelled out as IEEE 802.3's
/// `physical_layer_device_address`, replacing `phy_id`.
#[doc(alias = "phy")]
pub fn physical_layer_device_id(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
) -> Result<u32, Error> {
    let high = bus.read(physical_layer_device_address, PHYSID1)?;
    let low = bus.read(physical_layer_device_address, PHYSID2)?;
    Ok(u32::from(high) << 16 | u32::from(low))
}

// The high half is shifted above the low half's whole width, so the operands share no bit for
// every value the bus can return.
const _: () = assert!(((u16::MAX as u32) << 16) & (u16::MAX as u32) == 0);

/// **Is the link up right now?** Two reads of the latched-low status bit, so a drop since the last
/// look does not read as down forever and a link that is up now reads as up.
pub fn link_is_up(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
) -> Result<bool, Error> {
    let _ = bus.read(physical_layer_device_address, BMSR)?;
    Ok(bus.read(physical_layer_device_address, BMSR)? & BMSR_LSTATUS != 0)
}

/// **Restart autonegotiation**, advertising whatever the PHY's own defaults advertise (every speed
/// on a gigabit part).
pub fn restart_autonegotiation(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
) -> Result<(), Error> {
    let bmcr = bus.read(physical_layer_device_address, BMCR)?;
    bus.write(
        physical_layer_device_address,
        BMCR,
        bmcr | BMCR_ANENABLE | BMCR_ANRESTART,
    )
}

// The enable and restart bits share no bit, so the swap between them is `|`-versus-`^` over
// disjoint bits.
const _: () = assert!(BMCR_ANENABLE & BMCR_ANRESTART == 0);

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
        waited_microseconds: u64,
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
                waited_microseconds: 0,
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
        fn delay_microseconds(&mut self, microseconds: u64) {
            self.waited_microseconds += microseconds;
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
            hardware.waited_microseconds,
            3 * POLL_MICROSECONDS,
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
        assert!(hardware.waited_microseconds >= 2 * POLL_MICROSECONDS);
    }

    #[test]
    fn a_busy_bit_that_never_clears_is_a_bounded_timeout_naming_the_access() {
        let mut hardware = Pair::new(0, 0);
        hardware.stuck = true;
        hardware.address = GB;
        assert_eq!(
            Bus(&mut hardware).read(3, BMSR),
            Err(Error::Timeout {
                physical_layer_device_address: 3,
                register: BMSR
            })
        );
        assert_eq!(hardware.waited_microseconds, COMMAND_LIMIT_MICROSECONDS);
        assert!(
            hardware.writes.is_empty(),
            "a bus held busy was never written"
        );
        let mut hardware = Pair::new(0, 0);
        hardware.stuck = true;
        assert_eq!(
            Bus(&mut hardware).write(2, BMCR, 1),
            Err(Error::Timeout {
                physical_layer_device_address: 2,
                register: BMCR
            })
        );
        assert_eq!(hardware.waited_microseconds, COMMAND_LIMIT_MICROSECONDS);
    }

    /// A clause 22 register file with a latched-low link bit, for the helpers above the bus.
    struct Latched {
        registers: [u16; 32],
        link_now: bool,
        dropped_since_read: bool,
    }

    impl ManagementDataInputOutput for Latched {
        fn read(&mut self, _physical_layer_device_address: u8, register: u8) -> Result<u16, Error> {
            if register == BMSR {
                let up = self.link_now && !self.dropped_since_read;
                self.dropped_since_read = false;
                return Ok(if up { BMSR_LSTATUS } else { 0 });
            }
            Ok(self.registers[register as usize])
        }
        fn write(
            &mut self,
            _physical_layer_device_address: u8,
            register: u8,
            v: u16,
        ) -> Result<(), Error> {
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
                BMSR_LSTATUS,
                BMSR_ANEGCOMPLETE
            ],
            [0x0200, 0x1000, 0x8000, 0x0004, 0x0020]
        );
    }
}
