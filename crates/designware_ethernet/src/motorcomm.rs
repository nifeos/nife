//! **The Motorcomm YT8531, the PHY on both of radon's ports**, configured for the board it is on.
//!
//! The sequence is OpenBSD's `ytphy_yt8521_init`, `ytphy_yt8531_init` and `ytphy_yt8521_update`
//! (`sys/dev/mii/ytphy.c`, revision 1.6 of 2024-03-12, commit `9d7a05f92003`, ISC license, notice
//! in the crate root). [`EXT_SLEEP_CONTROL1`] and [`EXT_CLOCK_GATING`] come from Linux's
//! `drivers/net/phy/motorcomm.c`, read as facts about the part; why this driver writes them is at
//! each. The vendor tree states register values and mainline states physical quantities, and for
//! `gmac0` they agree. [`Settings::FALLBACK_GMAC0`] is used only when a tree states neither, and
//! [`Settings::source`] says which happened.

use crate::mdio::{self, Mdio};

/// **The YT8531's identifier**, `PHYSID1 << 16 | PHYSID2` (Linux's `PHY_ID_YT8531`, matched
/// exactly there; OpenBSD matches the OUI and model).
pub const YT8531_ID: u32 = 0x4f51_e91b;

/// The extended-register address port: write an extended register's number here.
pub(crate) const EXT_ADDR: u8 = 0x1e;
/// The extended-register data port: then read or write its value here.
pub(crate) const EXT_DATA: u8 = 0x1f;

/// Extended: clock gating.
pub(crate) const EXT_CLOCK_GATING: u16 = 0x000c;
/// **Clear to keep the receive clock running with no cable.** Linux's `YT8521_CGR_RX_CLK_EN`,
/// cleared for `motorcomm,keep-pll-enabled`. Cleared here unconditionally, because the controller's
/// software reset does not complete without a receive clock from the PHY (Linux's stmmac carries
/// `phylink_rx_clk_stop_block` for the same reason), and a board booted with its cable out must
/// still reset cleanly rather than refuse.
pub(crate) const EXT_CLOCK_GATING_RX_CLK_EN: u16 = 1 << 12;
/// Extended: sleep control.
pub(crate) const EXT_SLEEP_CONTROL1: u16 = 0x0027;
/// **Clear to stop the PHY sleeping when the cable is out**, which also stops its clocks. Linux's
/// `YT8521_ESC1R_SLEEP_SW`, cleared for `motorcomm,auto-sleep-disabled`; cleared here for
/// [`EXT_CLOCK_GATING_RX_CLK_EN`]'s reason. The cost is the PHY's idle power with no cable.
pub(crate) const EXT_SLEEP_CONTROL1_SLEEP_SW: u16 = 1 << 15;
/// Extended: chip configuration.
pub(crate) const EXT_CHIP_CONFIG: u16 = 0xa001;
/// The receive clock's extra fixed delay (about 1.9 ns) is enabled.
pub(crate) const CHIP_CONFIG_RXC_DLY_EN: u16 = 1 << 8;
/// The RGMII I/O voltage the PHY's LDO is strapped to.
pub(crate) const CHIP_CONFIG_LDO_MASK: u16 = 0x3 << 4;
/// 3.3 V.
pub(crate) const CHIP_CONFIG_LDO_3V3: u16 = 0;
/// 2.5 V.
pub(crate) const CHIP_CONFIG_LDO_2V5: u16 = 1 << 4;
/// Extended: RGMII configuration 1.
pub(crate) const EXT_RGMII_CONFIG1: u16 = 0xa003;
/// The transmit clock is inverted.
pub(crate) const RGMII_CONFIG1_TX_CLK_SEL: u16 = 1 << 14;
/// Receive delay select, in 150 ps steps.
pub(crate) const RGMII_CONFIG1_RX_DELAY_SHIFT: u16 = 10;
/// Transmit delay select, in 150 ps steps.
pub(crate) const RGMII_CONFIG1_TX_DELAY_SHIFT: u16 = 0;
/// Extended: pad drive strength.
pub(crate) const EXT_PAD_DRIVE: u16 = 0xa010;
/// The receive clock pad's drive strength field.
pub(crate) const PAD_DRIVE_RXC_SHIFT: u16 = 13;
/// The receive data pads' drive strength: its low two bits are bits 5:4, its high bit is bit 12.
pub(crate) const PAD_DRIVE_RXD_MASK: u16 = (1 << 12) | (0x3 << 4);
/// Extended: the synchronous Ethernet clock output.
pub(crate) const EXT_SYNCE_CFG: u16 = 0xa012;
/// The clock output is enabled. Neither JH7110 tree asks for it, so it is cleared, as OpenBSD
/// and Linux both do when `motorcomm,clk-out-frequency-hz` is absent.
pub(crate) const SYNCE_CFG_EN: u16 = 1 << 6;

/// Clause 22 vendor register: the PHY's resolved link.
pub const SPECIFIC_STATUS: u8 = 0x11;
/// The speed and duplex have been resolved.
pub const STATUS_RESOLVED: u16 = 1 << 11;
/// Real-time link.
pub const STATUS_LINK: u16 = 1 << 10;
/// Full duplex.
pub const STATUS_DUPLEX: u16 = 1 << 13;
/// The speed field, bits 15:14: 0 is 10, 1 is 100, 2 is 1000 Mbit/s.
pub const STATUS_SPEED_SHIFT: u16 = 14;

/// A link speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// 10 Mbit/s.
    Ten,
    /// 100 Mbit/s.
    Hundred,
    /// 1000 Mbit/s.
    Thousand,
}

impl Speed {
    /// Megabits per second.
    pub const fn mbps(self) -> u32 {
        match self {
            Speed::Ten => 10,
            Speed::Hundred => 100,
            Speed::Thousand => 1000,
        }
    }
}

/// A resolved link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    /// The speed negotiated.
    pub speed: Speed,
    /// Full duplex when true.
    pub full_duplex: bool,
}

/// **Decode the vendor status register.** `None` until the PHY has a link and has resolved its
/// speed and duplex, and for the speed encodings a gigabit part never reports.
pub const fn link_from_status(status: u16) -> Option<Link> {
    if status & STATUS_LINK == 0 || status & STATUS_RESOLVED == 0 {
        return None;
    }
    let speed = match (status >> STATUS_SPEED_SHIFT) & 0x3 {
        0 => Speed::Ten,
        1 => Speed::Hundred,
        2 => Speed::Thousand,
        _ => return None,
    };
    Some(Link {
        speed,
        full_duplex: status & STATUS_DUPLEX != 0,
    })
}

/// A pad drive strength, as a tree states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    /// The register code itself, 0 to 7 (the vendor tree's `rgmii_sw_dr*`).
    Code(u8),
    /// A current in microamps (mainline's `motorcomm,*-drv-microamp`), which means a different
    /// code at each I/O voltage and so is converted only once the PHY has said which it runs at.
    Microamps(u32),
}

/// Where a [`Settings`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The vendor U-Boot's register-value properties.
    VendorTree,
    /// Mainline Linux's picosecond and microamp properties.
    MainlineTree,
    /// Neither: [`Settings::FALLBACK_GMAC0`].
    Fallback,
}

/// **What the board wants of this PHY.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The receive delay select, 0 to 15, in 150 ps steps.
    pub rx_delay: u8,
    /// Whether the receive clock's extra fixed delay is on as well.
    pub rx_fixed_delay: bool,
    /// The transmit delay select, 0 to 15.
    pub tx_delay: u8,
    /// The receive clock pad's drive.
    pub rx_clock_drive: Drive,
    /// The receive data pads' drive.
    pub rx_data_drive: Drive,
    /// Whether to invert the transmit clock at 10, 100 and 1000 Mbit/s, in that order.
    pub tx_clock_inverted: [bool; 3],
    /// Which spelling this was read from.
    pub source: Source,
}

/// The receive delay at or above which the fixed extra delay is used (OpenBSD's `1900`).
const FIXED_DELAY_PS: u32 = 1900;
/// The delay select's step.
const STEP_PS: u32 = 150;

impl Settings {
    /// **radon's `gmac0`, from the table in this module's header.** Both trees give these values,
    /// so a tree that names neither spelling still gets the board it is on, and says so.
    pub(crate) const FALLBACK_GMAC0: Settings = Settings {
        rx_delay: 0xa,
        rx_fixed_delay: false,
        tx_delay: 0xa,
        rx_clock_drive: Drive::Code(6),
        rx_data_drive: Drive::Code(3),
        tx_clock_inverted: [true, true, true],
        source: Source::Fallback,
    };

    /// **From mainline's quantities**, by OpenBSD's arithmetic: a receive delay at or past 1.9 ns
    /// that the fixed delay plus whole steps can make uses the fixed delay; both selects round to
    /// the nearest step. A select past the 4-bit field saturates rather than spilling into the
    /// neighboring field, which OpenBSD's shift would do. `tx_inverted` is `None` when the tree
    /// does not say `motorcomm,tx-clk-adj-enabled`, which means "leave the bit alone" there and
    /// "not inverted" here: the bit is written either way, so it is decided rather than inherited.
    pub(crate) fn from_mainline(
        rx_ps: u32,
        tx_ps: u32,
        rx_clock_ua: u32,
        rx_data_ua: u32,
        tx_inverted: Option<[bool; 3]>,
    ) -> Settings {
        let (rx_ps, rx_fixed_delay) =
            if rx_ps >= FIXED_DELAY_PS && (rx_ps - FIXED_DELAY_PS).is_multiple_of(STEP_PS) {
                (rx_ps - FIXED_DELAY_PS, true)
            } else {
                (rx_ps, false)
            };
        let sel = |ps: u32| ((ps.saturating_add(STEP_PS / 2)) / STEP_PS).min(0xf) as u8;
        Settings {
            rx_delay: sel(rx_ps),
            rx_fixed_delay,
            tx_delay: sel(tx_ps),
            rx_clock_drive: Drive::Microamps(rx_clock_ua),
            rx_data_drive: Drive::Microamps(rx_data_ua),
            tx_clock_inverted: tx_inverted.unwrap_or([false; 3]),
            source: Source::MainlineTree,
        }
    }
}

/// **The pad drive code for a current at a voltage**, OpenBSD's `ytphy_yt8531_ds_map`. Unknown
/// pairs, which includes everything at 2.5 V, get the part's default of 3, as there.
pub(crate) fn drive_code(volt_mv: u32, d: Drive) -> u8 {
    const MAP: [(u32, u32, u8); 16] = [
        (1800, 1200, 0),
        (1800, 2100, 1),
        (1800, 2700, 2),
        (1800, 2910, 3),
        (1800, 3110, 4),
        (1800, 3600, 5),
        (1800, 3970, 6),
        (1800, 4350, 7),
        (3300, 3070, 0),
        (3300, 4080, 1),
        (3300, 4370, 2),
        (3300, 4680, 3),
        (3300, 5020, 4),
        (3300, 5450, 5),
        (3300, 5740, 6),
        (3300, 6140, 7),
    ];
    match d {
        Drive::Code(c) => c & 0x7,
        Drive::Microamps(ua) => MAP
            .iter()
            .find(|&&(v, a, _)| v == volt_mv && a == ua)
            .map_or(3, |&(_, _, c)| c),
    }
}

/// The I/O voltage the chip configuration word's LDO field states, in millivolts.
pub(crate) const fn volt_mv(chip_config: u16) -> u32 {
    match chip_config & CHIP_CONFIG_LDO_MASK {
        CHIP_CONFIG_LDO_3V3 => 3300,
        CHIP_CONFIG_LDO_2V5 => 2500,
        _ => 1800,
    }
}

/// Read extended register `ext`.
pub fn read_ext(bus: &mut impl Mdio, phy: u8, ext: u16) -> Result<u16, mdio::Error> {
    bus.write(phy, EXT_ADDR, ext)?;
    bus.read(phy, EXT_DATA)
}

/// Write extended register `ext`.
pub fn write_ext(bus: &mut impl Mdio, phy: u8, ext: u16, v: u16) -> Result<(), mdio::Error> {
    bus.write(phy, EXT_ADDR, ext)?;
    bus.write(phy, EXT_DATA, v)
}

/// Read, change and write back extended register `ext`, clearing `clear` then setting `set`.
fn modify_ext(
    bus: &mut impl Mdio,
    phy: u8,
    ext: u16,
    clear: u16,
    set: u16,
) -> Result<u16, mdio::Error> {
    let v = (read_ext(bus, phy, ext)? & !clear) | set;
    write_ext(bus, phy, ext, v)?;
    Ok(v)
}

/// What [`configure`] found and wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// The I/O voltage the PHY is strapped to.
    pub volt_mv: u32,
    /// RGMII configuration 1 after the write.
    pub rgmii_config1: u16,
    /// The pad drive word after the write.
    pub pad_drive: u16,
}

/// **Configure the PHY for the board**, OpenBSD's order: the receive delay enable, both delay
/// selects, the drive strengths at the voltage the PHY reports, the clock output off; then this
/// driver's two additions, the clocks kept running with no cable. The extended-address port is
/// restored at the end, as OpenBSD does, so a firmware access interleaved before or after finds
/// it where it left it.
pub fn configure(bus: &mut impl Mdio, phy: u8, s: &Settings) -> Result<Applied, mdio::Error> {
    let saved = bus.read(phy, EXT_ADDR)?;
    let delay_en = if s.rx_fixed_delay {
        CHIP_CONFIG_RXC_DLY_EN
    } else {
        0
    };
    let chip = modify_ext(bus, phy, EXT_CHIP_CONFIG, CHIP_CONFIG_RXC_DLY_EN, delay_en)?;
    let volt = volt_mv(chip);
    let delays = u16::from(s.rx_delay & 0xf) << RGMII_CONFIG1_RX_DELAY_SHIFT
        | u16::from(s.tx_delay & 0xf) << RGMII_CONFIG1_TX_DELAY_SHIFT;
    let rgmii_config1 = modify_ext(
        bus,
        phy,
        EXT_RGMII_CONFIG1,
        0xf << RGMII_CONFIG1_RX_DELAY_SHIFT | 0xf << RGMII_CONFIG1_TX_DELAY_SHIFT,
        delays,
    )?;
    let rxc = u16::from(drive_code(volt, s.rx_clock_drive));
    let rxd = u16::from(drive_code(volt, s.rx_data_drive));
    let pad_drive = modify_ext(
        bus,
        phy,
        EXT_PAD_DRIVE,
        0x7 << PAD_DRIVE_RXC_SHIFT | PAD_DRIVE_RXD_MASK,
        rxc << PAD_DRIVE_RXC_SHIFT | (rxd & 0x3) << 4 | (rxd >> 2) << 12,
    )?;
    modify_ext(bus, phy, EXT_SYNCE_CFG, SYNCE_CFG_EN, 0)?;
    modify_ext(bus, phy, EXT_CLOCK_GATING, EXT_CLOCK_GATING_RX_CLK_EN, 0)?;
    modify_ext(bus, phy, EXT_SLEEP_CONTROL1, EXT_SLEEP_CONTROL1_SLEEP_SW, 0)?;
    bus.write(phy, EXT_ADDR, saved)?;
    Ok(Applied {
        volt_mv: volt,
        rgmii_config1,
        pad_drive,
    })
}

/// **Set the transmit clock's inversion for the speed just negotiated**, OpenBSD's
/// `ytphy_yt8521_update`. Returns the new RGMII configuration word.
pub fn apply_speed(
    bus: &mut impl Mdio,
    phy: u8,
    s: &Settings,
    speed: Speed,
) -> Result<u16, mdio::Error> {
    let inverted = match speed {
        Speed::Ten => s.tx_clock_inverted[0],
        Speed::Hundred => s.tx_clock_inverted[1],
        Speed::Thousand => s.tx_clock_inverted[2],
    };
    let saved = bus.read(phy, EXT_ADDR)?;
    let set = if inverted {
        RGMII_CONFIG1_TX_CLK_SEL
    } else {
        0
    };
    let v = modify_ext(bus, phy, EXT_RGMII_CONFIG1, RGMII_CONFIG1_TX_CLK_SEL, set)?;
    bus.write(phy, EXT_ADDR, saved)?;
    Ok(v)
}

/// **The link, if there is one**, from the vendor status register.
pub fn link(bus: &mut impl Mdio, phy: u8) -> Result<Option<Link>, mdio::Error> {
    Ok(link_from_status(bus.read(phy, SPECIFIC_STATUS)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Phy;

    #[test]
    fn the_two_trees_spell_the_same_registers_for_radons_gmac0() {
        let mainline = Settings::from_mainline(1500, 1500, 3970, 2910, Some([false, true, true]));
        let fallback = Settings::FALLBACK_GMAC0;
        assert_eq!(mainline.rx_delay, fallback.rx_delay);
        assert_eq!(mainline.tx_delay, fallback.tx_delay);
        assert_eq!(mainline.rx_fixed_delay, fallback.rx_fixed_delay);
        assert_eq!(drive_code(1800, mainline.rx_clock_drive), 6);
        assert_eq!(drive_code(1800, mainline.rx_data_drive), 3);
        assert_eq!(drive_code(1800, fallback.rx_clock_drive), 6);
    }

    #[test]
    fn a_receive_delay_the_fixed_delay_can_make_uses_it() {
        // 1.9 ns + 2 steps: OpenBSD's rule takes the fixed delay and leaves 300 ps, select 2.
        let s = Settings::from_mainline(2200, 0, 0, 0, None);
        assert!(s.rx_fixed_delay);
        assert_eq!(s.rx_delay, 2);
        // 2000 ps is not 1900 plus whole steps, so it is all select: (2000 + 75) / 150 = 13.
        let s = Settings::from_mainline(2000, 0, 0, 0, None);
        assert!(!s.rx_fixed_delay);
        assert_eq!(s.rx_delay, 13);
    }

    #[test]
    fn a_delay_too_long_for_the_field_saturates_rather_than_spilling() {
        let s = Settings::from_mainline(10_000, 10_000, 0, 0, None);
        assert_eq!((s.rx_delay, s.tx_delay), (15, 15));
    }

    #[test]
    fn an_unknown_current_gets_the_parts_default_drive() {
        assert_eq!(drive_code(2500, Drive::Microamps(3970)), 3);
        assert_eq!(drive_code(1800, Drive::Microamps(1)), 3);
        assert_eq!(drive_code(3300, Drive::Microamps(6140)), 7);
    }

    #[test]
    fn the_status_register_resolves_only_with_link_and_resolution() {
        let up_1000_full = STATUS_LINK | STATUS_RESOLVED | STATUS_DUPLEX | 2 << 14;
        assert_eq!(
            link_from_status(up_1000_full),
            Some(Link {
                speed: Speed::Thousand,
                full_duplex: true
            })
        );
        assert_eq!(link_from_status(up_1000_full & !STATUS_RESOLVED), None);
        assert_eq!(link_from_status(up_1000_full & !STATUS_LINK), None);
        assert_eq!(
            link_from_status(STATUS_LINK | STATUS_RESOLVED | 3 << 14),
            None
        );
        assert_eq!(
            link_from_status(STATUS_LINK | STATUS_RESOLVED | 1 << 14),
            Some(Link {
                speed: Speed::Hundred,
                full_duplex: false
            })
        );
    }

    #[test]
    fn configure_writes_radons_registers_and_restores_the_address_port() {
        let mut phy = Phy::yt8531_at_1v8();
        phy.write_raw(EXT_ADDR, 0x1234);
        let a = configure(&mut phy, 0, &Settings::FALLBACK_GMAC0).unwrap();
        assert_eq!(a.volt_mv, 1800);
        assert_eq!(phy.ext(EXT_RGMII_CONFIG1) & 0x3c0f, 0xa << 10 | 0xa);
        assert_eq!(phy.ext(EXT_PAD_DRIVE) >> 13, 6);
        assert_eq!(phy.ext(EXT_PAD_DRIVE) & PAD_DRIVE_RXD_MASK, 3 << 4);
        assert_eq!(phy.ext(EXT_CHIP_CONFIG) & CHIP_CONFIG_RXC_DLY_EN, 0);
        assert_eq!(phy.ext(EXT_SYNCE_CFG) & SYNCE_CFG_EN, 0);
        assert_eq!(phy.ext(EXT_SLEEP_CONTROL1) & EXT_SLEEP_CONTROL1_SLEEP_SW, 0);
        assert_eq!(phy.ext(EXT_CLOCK_GATING) & EXT_CLOCK_GATING_RX_CLK_EN, 0);
        assert_eq!(phy.raw(EXT_ADDR), 0x1234);
    }

    #[test]
    fn a_drive_code_with_its_high_bit_set_lands_in_bit_twelve() {
        let mut phy = Phy::yt8531_at_1v8();
        let s = Settings {
            rx_data_drive: Drive::Code(7),
            ..Settings::FALLBACK_GMAC0
        };
        configure(&mut phy, 0, &s).unwrap();
        assert_eq!(
            phy.ext(EXT_PAD_DRIVE) & PAD_DRIVE_RXD_MASK,
            1 << 12 | 3 << 4
        );
    }

    #[test]
    fn the_link_reads_none_until_a_cable_resolves_it() {
        let mut phy = Phy::yt8531_at_1v8();
        assert_eq!(link(&mut phy, 0).unwrap(), None);
        assert!(!mdio::link_is_up(&mut phy, 0).unwrap());
        let gig = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        phy.set_link(Some(gig));
        assert_eq!(link(&mut phy, 0).unwrap(), Some(gig));
        assert!(mdio::link_is_up(&mut phy, 0).unwrap());
    }

    #[test]
    fn the_transmit_clock_inverts_per_speed() {
        let mut phy = Phy::yt8531_at_1v8();
        let s = Settings {
            tx_clock_inverted: [false, false, true],
            ..Settings::FALLBACK_GMAC0
        };
        apply_speed(&mut phy, 0, &s, Speed::Thousand).unwrap();
        assert_ne!(phy.ext(EXT_RGMII_CONFIG1) & RGMII_CONFIG1_TX_CLK_SEL, 0);
        apply_speed(&mut phy, 0, &s, Speed::Hundred).unwrap();
        assert_eq!(phy.ext(EXT_RGMII_CONFIG1) & RGMII_CONFIG1_TX_CLK_SEL, 0);
    }
    /// The extended registers' bits as Linux's `motorcomm.c` names them, written out: the
    /// simulated PHY reads the same names, so only a literal tells a misplaced bit from a right one.
    #[test]
    fn the_extended_register_bits_are_motorcomms() {
        assert_eq!(
            [
                EXT_CLOCK_GATING_RX_CLK_EN,
                EXT_SLEEP_CONTROL1_SLEEP_SW,
                CHIP_CONFIG_RXC_DLY_EN,
                CHIP_CONFIG_LDO_2V5,
                PAD_DRIVE_RXD_MASK,
                SYNCE_CFG_EN,
            ],
            [0x1000, 0x8000, 0x0100, 0x0010, 0x1030, 0x0040]
        );
    }

    #[test]
    fn each_speed_is_its_megabits() {
        assert_eq!(
            [Speed::Ten, Speed::Hundred, Speed::Thousand].map(Speed::mbps),
            [10, 100, 1000]
        );
        assert_eq!(
            link_from_status(STATUS_LINK | STATUS_RESOLVED | STATUS_DUPLEX),
            Some(Link {
                speed: Speed::Ten,
                full_duplex: true
            })
        );
    }

    /// Each LDO strap, whatever the bits around the field hold.
    #[test]
    fn the_ldo_field_alone_names_the_voltage() {
        assert_eq!(volt_mv(0), 3300);
        assert_eq!(volt_mv(0xffcf), 3300);
        assert_eq!(volt_mv(0x0010), 2500);
        assert_eq!(volt_mv(0x0020), 1800);
        assert_eq!(volt_mv(0x0030), 1800);
    }

    /// A delay rounds to the nearest 150 ps step: 1450 ps is nearer 1500 than 1350.
    #[test]
    fn a_delay_rounds_to_the_nearest_step() {
        let s = Settings::from_mainline(1450, 1420, 0, 0, None);
        assert_eq!((s.rx_delay, s.tx_delay), (10, 9));
    }

    /// `configure` replaces both delay fields and leaves the rest of the RGMII word, and writes
    /// the drive codes into a pad word that held nothing, so a stray bit would show.
    #[test]
    fn configure_replaces_the_delay_and_drive_fields_and_nothing_else() {
        let mut phy = Phy::yt8531_at_1v8();
        write_ext(&mut phy, 0, EXT_RGMII_CONFIG1, 0xffff).unwrap();
        write_ext(&mut phy, 0, EXT_PAD_DRIVE, 0).unwrap();
        let s = Settings {
            rx_data_drive: Drive::Code(7),
            ..Settings::FALLBACK_GMAC0
        };
        let a = configure(&mut phy, 0, &s).unwrap();
        // 0xffff with bits 13:10 and 3:0 replaced by 0xa each.
        assert_eq!(a.rgmii_config1, 0xebfa);
        // Clock drive 6 in bits 15:13; data drive 7 as 0b11 in bits 5:4 and 0b1 in bit 12.
        assert_eq!(a.pad_drive, 0xd030);
        assert_eq!(phy.ext(EXT_PAD_DRIVE), 0xd030);
    }
}
