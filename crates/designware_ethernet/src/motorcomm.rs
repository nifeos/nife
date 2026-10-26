//! **The Motorcomm YT8531, the PHY on both of radon's ports**, configured for the board it is on.
//!
//! The sequence is OpenBSD's `ytphy_yt8521_init`, `ytphy_yt8531_init` and `ytphy_yt8521_update`
//! (`sys/dev/mii/ytphy.c`, revision 1.6 of 2024-03-12, commit `9d7a05f92003`, ISC license, notice
//! in the crate root). [`EXTREG_SLEEP_CONTROL1_REG`], [`CLOCK_GATING_REG`], [`PHY_ID_YT8531`],
//! [`SPECIFIC_STATUS_REG`] and its `SSR_` bits come from Linux's `drivers/net/phy/motorcomm.c`,
//! read as facts about the part; why this driver writes them is at each. Each constant keeps its
//! source's name without the part prefix (`YT8511_`, `YT8521_`, `YT8531_` or `YTPHY_`), and its
//! documentation opens with its full name and its source's spelling. The vendor tree states
//! register values and mainline states physical quantities, and for `gmac0` they agree.
//! [`Settings::FALLBACK_GMAC0`] applies only when a tree states neither, and [`Settings::source`]
//! says which happened.

use crate::management_data_input_output::{self, ManagementDataInputOutput};

/// **The YT8531's PHY identifier** (`PHY_ID_YT8531`, `motorcomm.c`), `PHYSID1 << 16 | PHYSID2`,
/// matched exactly there; OpenBSD matches the OUI and model.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `YT8531_ID`.
#[doc(alias = "yt8531 identifier")]
pub const PHY_ID_YT8531: u32 = 0x4f51_e91b;

/// **Extended register address port** (`YT8511_REG_ADDR`, `ytphy.c`): write an extended register's
/// number here.
#[doc(alias = "extended register address port")]
pub(crate) const REG_ADDR: u8 = 0x1e;
/// **Extended register data port** (`YT8511_REG_DATA`, `ytphy.c`): then read or write its value
/// here.
#[doc(alias = "extended register data port")]
pub(crate) const REG_DATA: u8 = 0x1f;

/// **Clock gating register** (`YT8521_CLOCK_GATING_REG`, `motorcomm.c`).
#[doc(alias = "clock gating register")]
pub(crate) const CLOCK_GATING_REG: u16 = 0x000c;
/// **Clock gating register, receive clock enable** (`YT8521_CGR_RX_CLK_EN`, `motorcomm.c`). Clear
/// it to keep the receive clock running with no cable; Linux clears it for
/// `motorcomm,keep-pll-enabled`. Cleared here unconditionally, because the controller's
/// software reset does not complete without a receive clock from the PHY (Linux's stmmac carries
/// `phylink_rx_clk_stop_block` for the same reason), and a board booted with its cable out must
/// still reset cleanly rather than refuse.
#[doc(alias = "receive clock enable")]
pub(crate) const CGR_RX_CLK_EN: u16 = 1 << 12;
/// **Extended register, sleep control 1** (`YT8521_EXTREG_SLEEP_CONTROL1_REG`, `motorcomm.c`).
#[doc(alias = "extended register, sleep control 1")]
pub(crate) const EXTREG_SLEEP_CONTROL1_REG: u16 = 0x0027;
/// **Sleep control 1, sleep switch** (`YT8521_ESC1R_SLEEP_SW`, `motorcomm.c`). Clear it to stop the
/// PHY sleeping when the cable is out, which also stops its clocks. Linux clears it for
/// `motorcomm,auto-sleep-disabled`; this driver clears it for
/// [`CGR_RX_CLK_EN`]'s reason. The cost is the PHY's idle power with no cable.
#[doc(alias = "sleep switch")]
pub(crate) const ESC1R_SLEEP_SW: u16 = 1 << 15;
/// **Extended register, chip configuration** (`YT8521_EXT_CHIP_CONFIG`, `ytphy.c`).
#[doc(alias = "extended register, chip configuration")]
pub(crate) const EXT_CHIP_CONFIG: u16 = 0xa001;
/// **Receive clock delay enable** (`YT8521_RXC_DLY_EN`, `ytphy.c`). The receive clock's extra fixed
/// delay (about 1.9 ns) is enabled.
#[doc(alias = "receive clock delay enable")]
pub(crate) const RXC_DLY_EN: u16 = 1 << 8;
/// **Chip configuration, low-dropout regulator voltage field** (`YT8521_CFG_LDO_MASK`, `ytphy.c`).
/// The RGMII I/O voltage the PHY's LDO is strapped to.
#[doc(alias = "chip configuration, low-dropout regulator voltage field")]
pub(crate) const CFG_LDO_MASK: u16 = 0x3 << 4;
/// **Low-dropout regulator at 3.3 V** (`YT8521_CFG_LDO_3V3`, `ytphy.c`).
#[doc(alias = "low-dropout regulator at 3.3 v")]
pub(crate) const CFG_LDO_3V3: u16 = 0;
/// **Low-dropout regulator at 2.5 V** (`YT8521_CFG_LDO_2V5`, `ytphy.c`).
#[doc(alias = "low-dropout regulator at 2.5 v")]
pub(crate) const CFG_LDO_2V5: u16 = 1 << 4;
/// **Extended register, RGMII configuration 1** (`YT8521_EXT_RGMII_CONFIG1`, `ytphy.c`).
#[doc(alias = "extended register, rgmii configuration 1")]
pub(crate) const EXT_RGMII_CONFIG1: u16 = 0xa003;
/// **Transmit clock select** (`YT8521_TX_CLK_SEL`, `ytphy.c`): set, the transmit clock is inverted.
#[doc(alias = "transmit clock select")]
pub(crate) const TX_CLK_SEL: u16 = 1 << 14;
/// **Receive delay select shift** (`YT8521_RX_DELAY_SEL_SHIFT`, `ytphy.c`), in 150 ps steps.
#[doc(alias = "receive delay select shift")]
pub(crate) const RX_DELAY_SEL_SHIFT: u16 = 10;
/// **Transmit delay select shift** (`YT8521_TX_DELAY_SEL_SHIFT`, `ytphy.c`), in 150 ps steps.
#[doc(alias = "transmit delay select shift")]
pub(crate) const TX_DELAY_SEL_SHIFT: u16 = 0;
/// **Extended register, pad drive strength** (`YT8521_EXT_PAD_DRIVE_STRENGTH`, `ytphy.c`).
#[doc(alias = "extended register, pad drive strength")]
pub(crate) const EXT_PAD_DRIVE_STRENGTH: u16 = 0xa010;
/// **RGMII receive clock drive strength shift** (`YT8531_RGMII_RXC_DS_SHIFT`, `ytphy.c`).
#[doc(alias = "rgmii receive clock drive strength shift")]
pub(crate) const RGMII_RXC_DS_SHIFT: u16 = 13;
/// **RGMII receive data drive strength mask** (`YT8531_RGMII_RXD_DS_MASK`, `ytphy.c`): its low two
/// bits are bits 5:4, its high bit is bit 12.
#[doc(alias = "rgmii receive data drive strength mask")]
pub(crate) const RGMII_RXD_DS_MASK: u16 = (1 << 12) | (0x3 << 4);
/// **Extended register, synchronous Ethernet configuration** (`YT8521_EXT_SYNCE_CFG`, `ytphy.c`).
#[doc(alias = "extended register, synchronous ethernet configuration")]
pub(crate) const EXT_SYNCE_CFG: u16 = 0xa012;
/// **Synchronous Ethernet clock output enable** (`YT8531_EN_SYNC_E`, `ytphy.c`). Neither JH7110
/// tree asks for it, so it is cleared, as OpenBSD and Linux both do when
/// `motorcomm,clk-out-frequency-hz` is absent.
#[doc(alias = "synchronous ethernet clock output enable")]
pub(crate) const EN_SYNC_E: u16 = 1 << 6;

/// **PHY-specific status register** (`YTPHY_SPECIFIC_STATUS_REG`, `motorcomm.c`). Clause 22 vendor
/// register: the PHY's resolved link.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `SPECIFIC_STATUS`.
#[doc(alias = "phy-specific status register")]
pub const SPECIFIC_STATUS_REG: u8 = 0x11;
/// **Specific status, speed and duplex resolved** (`YTPHY_SSR_SPEED_DUPLEX_RESOLVED`,
/// `motorcomm.c`). The speed and duplex have been resolved.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `STATUS_RESOLVED`.
#[doc(alias = "specific status, speed and duplex resolved")]
pub const SSR_SPEED_DUPLEX_RESOLVED: u16 = 1 << 11;
/// **Specific status, link** (`YTPHY_SSR_LINK`, `motorcomm.c`). Real-time link.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `STATUS_LINK`.
#[doc(alias = "specific status, link")]
pub const SSR_LINK: u16 = 1 << 10;
/// **Specific status, duplex** (`YTPHY_SSR_DUPLEX`, `motorcomm.c`). Full duplex.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `STATUS_DUPLEX`.
#[doc(alias = "specific status, duplex")]
pub const SSR_DUPLEX: u16 = 1 << 13;
/// **Specific status, speed field shift**: bits 15:14, where 0 is 10, 1 is 100 and 2 is 1000
/// Mbit/s. This crate's name, in the `YTPHY_SSR_` family of `motorcomm.c`, which states the field
/// through `YTPHY_SSR_SPEED_MASK` and has no shift of its own.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c; the source has no such constant, so
/// this crate names it in the source's `YTPHY_SSR_` family, replacing `STATUS_SPEED_SHIFT`.
#[doc(alias = "speed shift")]
pub const SSR_SPEED_SHIFT: u16 = 14;

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
    if status & SSR_LINK == 0 || status & SSR_SPEED_DUPLEX_RESOLVED == 0 {
        return None;
    }
    let speed = match (status >> SSR_SPEED_SHIFT) & 0x3 {
        0 => Speed::Ten,
        1 => Speed::Hundred,
        2 => Speed::Thousand,
        _ => return None,
    };
    Some(Link {
        speed,
        full_duplex: status & SSR_DUPLEX != 0,
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
    pub receive_delay: u8,
    /// Whether the receive clock's extra fixed delay is on as well.
    pub receive_fixed_delay: bool,
    /// The transmit delay select, 0 to 15.
    pub transmit_delay: u8,
    /// The receive clock pad's drive.
    pub receive_clock_drive: Drive,
    /// The receive data pads' drive.
    pub receive_data_drive: Drive,
    /// Whether to invert the transmit clock at 10, 100 and 1000 Mbit/s, in that order.
    pub transmit_clock_inverted: [bool; 3],
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
    ///
    /// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the
    /// device-tree walk that compares against it left the crate.
    pub const FALLBACK_GMAC0: Settings = Settings {
        receive_delay: 0xa,
        receive_fixed_delay: false,
        transmit_delay: 0xa,
        receive_clock_drive: Drive::Code(6),
        receive_data_drive: Drive::Code(3),
        transmit_clock_inverted: [true, true, true],
        source: Source::Fallback,
    };

    /// **From mainline's quantities**, by OpenBSD's arithmetic: a receive delay at or past 1.9 ns
    /// that the fixed delay plus whole steps can make uses the fixed delay; both selects round to
    /// the nearest step. A select past the 4-bit field saturates rather than spilling into the
    /// neighboring field, which OpenBSD's shift would do. `transmit_inverted` is `None` when the
    /// tree does not say `motorcomm,tx-clk-adj-enabled`, which means "leave the bit alone" there
    /// and "not inverted" here: the bit is written either way, so it is decided rather than
    /// inherited.
    pub(crate) fn from_mainline(
        receive_ps: u32,
        transmit_ps: u32,
        receive_clock_ua: u32,
        receive_data_ua: u32,
        transmit_inverted: Option<[bool; 3]>,
    ) -> Settings {
        let (receive_ps, receive_fixed_delay) = if receive_ps >= FIXED_DELAY_PS
            && (receive_ps - FIXED_DELAY_PS).is_multiple_of(STEP_PS)
        {
            (receive_ps - FIXED_DELAY_PS, true)
        } else {
            (receive_ps, false)
        };
        let sel = |ps: u32| ((ps.saturating_add(STEP_PS / 2)) / STEP_PS).min(0xf) as u8;
        Settings {
            receive_delay: sel(receive_ps),
            receive_fixed_delay,
            transmit_delay: sel(transmit_ps),
            receive_clock_drive: Drive::Microamps(receive_clock_ua),
            receive_data_drive: Drive::Microamps(receive_data_ua),
            transmit_clock_inverted: transmit_inverted.unwrap_or([false; 3]),
            source: Source::MainlineTree,
        }
    }
}

/// **The pad drive code for a current at a voltage**, OpenBSD's `ytphy_yt8531_ds_map`. Unknown
/// pairs, which includes everything at 2.5 V, get the part's default of 3, as there.
pub(crate) fn drive_code(voltage_mv: u32, d: Drive) -> u8 {
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
            .find(|&&(v, a, _)| v == voltage_mv && a == ua)
            .map_or(3, |&(_, _, c)| c),
    }
}

/// The I/O voltage the chip configuration word's LDO field states, in millivolts.
pub(crate) const fn voltage_mv(chip_configuration: u16) -> u32 {
    match chip_configuration & CFG_LDO_MASK {
        CFG_LDO_3V3 => 3300,
        CFG_LDO_2V5 => 2500,
        _ => 1800,
    }
}

/// Read extended register `extended`.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing
/// `read_ext`.
pub fn read_extended(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
    extended: u16,
) -> Result<u16, management_data_input_output::Error> {
    bus.write(physical_layer_device_address, REG_ADDR, extended)?;
    bus.read(physical_layer_device_address, REG_DATA)
}

/// Write extended register `extended`.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing
/// `write_ext`.
pub fn write_extended(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
    extended: u16,
    v: u16,
) -> Result<(), management_data_input_output::Error> {
    bus.write(physical_layer_device_address, REG_ADDR, extended)?;
    bus.write(physical_layer_device_address, REG_DATA, v)
}

/// Read, change and write back extended register `extended`, clearing `clear` then setting `set`.
fn modify_extended(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
    extended: u16,
    clear: u16,
    set: u16,
) -> Result<u16, management_data_input_output::Error> {
    let v = (read_extended(bus, physical_layer_device_address, extended)? & !clear) | set;
    write_extended(bus, physical_layer_device_address, extended, v)?;
    Ok(v)
}

/// What [`configure`] found and wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// The I/O voltage the PHY is strapped to.
    pub voltage_mv: u32,
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
pub fn configure(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
    s: &Settings,
) -> Result<Applied, management_data_input_output::Error> {
    let saved = bus.read(physical_layer_device_address, REG_ADDR)?;
    let delay_en = if s.receive_fixed_delay { RXC_DLY_EN } else { 0 };
    let chip = modify_extended(
        bus,
        physical_layer_device_address,
        EXT_CHIP_CONFIG,
        RXC_DLY_EN,
        delay_en,
    )?;
    let volt = voltage_mv(chip);
    let delays = u16::from(s.receive_delay & 0xf) << RX_DELAY_SEL_SHIFT
        | u16::from(s.transmit_delay & 0xf) << TX_DELAY_SEL_SHIFT;
    let rgmii_config1 = modify_extended(
        bus,
        physical_layer_device_address,
        EXT_RGMII_CONFIG1,
        0xf << RX_DELAY_SEL_SHIFT | 0xf << TX_DELAY_SEL_SHIFT,
        delays,
    )?;
    let rxc = u16::from(drive_code(volt, s.receive_clock_drive));
    let rxd = u16::from(drive_code(volt, s.receive_data_drive));
    let pad_drive = modify_extended(
        bus,
        physical_layer_device_address,
        EXT_PAD_DRIVE_STRENGTH,
        0x7 << RGMII_RXC_DS_SHIFT | RGMII_RXD_DS_MASK,
        rxc << RGMII_RXC_DS_SHIFT | (rxd & 0x3) << 4 | (rxd >> 2) << 12,
    )?;
    modify_extended(
        bus,
        physical_layer_device_address,
        EXT_SYNCE_CFG,
        EN_SYNC_E,
        0,
    )?;
    modify_extended(
        bus,
        physical_layer_device_address,
        CLOCK_GATING_REG,
        CGR_RX_CLK_EN,
        0,
    )?;
    modify_extended(
        bus,
        physical_layer_device_address,
        EXTREG_SLEEP_CONTROL1_REG,
        ESC1R_SLEEP_SW,
        0,
    )?;
    bus.write(physical_layer_device_address, REG_ADDR, saved)?;
    Ok(Applied {
        voltage_mv: volt,
        rgmii_config1,
        pad_drive,
    })
}

/// **Set the transmit clock's inversion for the speed just negotiated**, OpenBSD's
/// `ytphy_yt8521_update`. Returns the new RGMII configuration word.
pub fn apply_speed(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
    s: &Settings,
    speed: Speed,
) -> Result<u16, management_data_input_output::Error> {
    let inverted = match speed {
        Speed::Ten => s.transmit_clock_inverted[0],
        Speed::Hundred => s.transmit_clock_inverted[1],
        Speed::Thousand => s.transmit_clock_inverted[2],
    };
    let saved = bus.read(physical_layer_device_address, REG_ADDR)?;
    let set = if inverted { TX_CLK_SEL } else { 0 };
    let v = modify_extended(
        bus,
        physical_layer_device_address,
        EXT_RGMII_CONFIG1,
        TX_CLK_SEL,
        set,
    )?;
    bus.write(physical_layer_device_address, REG_ADDR, saved)?;
    Ok(v)
}

/// **The link, if there is one**, from the vendor status register.
pub fn link(
    bus: &mut impl ManagementDataInputOutput,
    physical_layer_device_address: u8,
) -> Result<Option<Link>, management_data_input_output::Error> {
    Ok(link_from_status(bus.read(
        physical_layer_device_address,
        SPECIFIC_STATUS_REG,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::PhysicalLayerDevice;

    #[test]
    fn the_two_trees_spell_the_same_registers_for_radons_gmac0() {
        let mainline = Settings::from_mainline(1500, 1500, 3970, 2910, Some([false, true, true]));
        let fallback = Settings::FALLBACK_GMAC0;
        assert_eq!(mainline.receive_delay, fallback.receive_delay);
        assert_eq!(mainline.transmit_delay, fallback.transmit_delay);
        assert_eq!(mainline.receive_fixed_delay, fallback.receive_fixed_delay);
        assert_eq!(drive_code(1800, mainline.receive_clock_drive), 6);
        assert_eq!(drive_code(1800, mainline.receive_data_drive), 3);
        assert_eq!(drive_code(1800, fallback.receive_clock_drive), 6);
    }

    #[test]
    fn a_receive_delay_the_fixed_delay_can_make_uses_it() {
        // 1.9 ns + 2 steps: OpenBSD's rule takes the fixed delay and leaves 300 ps, select 2.
        let s = Settings::from_mainline(2200, 0, 0, 0, None);
        assert!(s.receive_fixed_delay);
        assert_eq!(s.receive_delay, 2);
        // 2000 ps is not 1900 plus whole steps, so it is all select: (2000 + 75) / 150 = 13.
        let s = Settings::from_mainline(2000, 0, 0, 0, None);
        assert!(!s.receive_fixed_delay);
        assert_eq!(s.receive_delay, 13);
    }

    #[test]
    fn a_delay_too_long_for_the_field_saturates_rather_than_spilling() {
        let s = Settings::from_mainline(10_000, 10_000, 0, 0, None);
        assert_eq!((s.receive_delay, s.transmit_delay), (15, 15));
    }

    #[test]
    fn an_unknown_current_gets_the_parts_default_drive() {
        assert_eq!(drive_code(2500, Drive::Microamps(3970)), 3);
        assert_eq!(drive_code(1800, Drive::Microamps(1)), 3);
        assert_eq!(drive_code(3300, Drive::Microamps(6140)), 7);
    }

    #[test]
    fn the_status_register_resolves_only_with_link_and_resolution() {
        let up_1000_full = SSR_LINK | SSR_SPEED_DUPLEX_RESOLVED | SSR_DUPLEX | 2 << 14;
        assert_eq!(
            link_from_status(up_1000_full),
            Some(Link {
                speed: Speed::Thousand,
                full_duplex: true
            })
        );
        assert_eq!(
            link_from_status(up_1000_full & !SSR_SPEED_DUPLEX_RESOLVED),
            None
        );
        assert_eq!(link_from_status(up_1000_full & !SSR_LINK), None);
        assert_eq!(
            link_from_status(SSR_LINK | SSR_SPEED_DUPLEX_RESOLVED | 3 << 14),
            None
        );
        assert_eq!(
            link_from_status(SSR_LINK | SSR_SPEED_DUPLEX_RESOLVED | 1 << 14),
            Some(Link {
                speed: Speed::Hundred,
                full_duplex: false
            })
        );
    }

    #[test]
    fn configure_writes_radons_registers_and_restores_the_address_port() {
        let mut physical_layer_device = PhysicalLayerDevice::yt8531_at_1v8();
        physical_layer_device.write_raw(REG_ADDR, 0x1234);
        let a = configure(&mut physical_layer_device, 0, &Settings::FALLBACK_GMAC0).unwrap();
        assert_eq!(a.voltage_mv, 1800);
        assert_eq!(
            physical_layer_device.extended(EXT_RGMII_CONFIG1) & 0x3c0f,
            0xa << 10 | 0xa
        );
        assert_eq!(
            physical_layer_device.extended(EXT_PAD_DRIVE_STRENGTH) >> 13,
            6
        );
        assert_eq!(
            physical_layer_device.extended(EXT_PAD_DRIVE_STRENGTH) & RGMII_RXD_DS_MASK,
            3 << 4
        );
        assert_eq!(
            physical_layer_device.extended(EXT_CHIP_CONFIG) & RXC_DLY_EN,
            0
        );
        assert_eq!(physical_layer_device.extended(EXT_SYNCE_CFG) & EN_SYNC_E, 0);
        assert_eq!(
            physical_layer_device.extended(EXTREG_SLEEP_CONTROL1_REG) & ESC1R_SLEEP_SW,
            0
        );
        assert_eq!(
            physical_layer_device.extended(CLOCK_GATING_REG) & CGR_RX_CLK_EN,
            0
        );
        assert_eq!(physical_layer_device.raw(REG_ADDR), 0x1234);
    }

    #[test]
    fn a_drive_code_with_its_high_bit_set_lands_in_bit_twelve() {
        let mut physical_layer_device = PhysicalLayerDevice::yt8531_at_1v8();
        let s = Settings {
            receive_data_drive: Drive::Code(7),
            ..Settings::FALLBACK_GMAC0
        };
        configure(&mut physical_layer_device, 0, &s).unwrap();
        assert_eq!(
            physical_layer_device.extended(EXT_PAD_DRIVE_STRENGTH) & RGMII_RXD_DS_MASK,
            1 << 12 | 3 << 4
        );
    }

    #[test]
    fn the_link_reads_none_until_a_cable_resolves_it() {
        let mut physical_layer_device = PhysicalLayerDevice::yt8531_at_1v8();
        assert_eq!(link(&mut physical_layer_device, 0).unwrap(), None);
        assert!(!management_data_input_output::link_is_up(&mut physical_layer_device, 0).unwrap());
        let gig = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        physical_layer_device.set_link(Some(gig));
        assert_eq!(link(&mut physical_layer_device, 0).unwrap(), Some(gig));
        assert!(management_data_input_output::link_is_up(&mut physical_layer_device, 0).unwrap());
    }

    #[test]
    fn the_transmit_clock_inverts_per_speed() {
        let mut physical_layer_device = PhysicalLayerDevice::yt8531_at_1v8();
        let s = Settings {
            transmit_clock_inverted: [false, false, true],
            ..Settings::FALLBACK_GMAC0
        };
        apply_speed(&mut physical_layer_device, 0, &s, Speed::Thousand).unwrap();
        assert_ne!(
            physical_layer_device.extended(EXT_RGMII_CONFIG1) & TX_CLK_SEL,
            0
        );
        apply_speed(&mut physical_layer_device, 0, &s, Speed::Hundred).unwrap();
        assert_eq!(
            physical_layer_device.extended(EXT_RGMII_CONFIG1) & TX_CLK_SEL,
            0
        );
    }
    /// The extended registers' bits as Linux's `motorcomm.c` names them, written out: the simulated
    /// PHY reads the same names, so only a literal tells a misplaced bit from a right one.
    #[test]
    fn the_extended_register_bits_are_motorcomms() {
        assert_eq!(
            [
                CGR_RX_CLK_EN,
                ESC1R_SLEEP_SW,
                RXC_DLY_EN,
                CFG_LDO_2V5,
                RGMII_RXD_DS_MASK,
                EN_SYNC_E,
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
            link_from_status(SSR_LINK | SSR_SPEED_DUPLEX_RESOLVED | SSR_DUPLEX),
            Some(Link {
                speed: Speed::Ten,
                full_duplex: true
            })
        );
    }

    /// Each LDO strap, whatever the bits around the field hold.
    #[test]
    fn the_ldo_field_alone_names_the_voltage() {
        assert_eq!(voltage_mv(0), 3300);
        assert_eq!(voltage_mv(0xffcf), 3300);
        assert_eq!(voltage_mv(0x0010), 2500);
        assert_eq!(voltage_mv(0x0020), 1800);
        assert_eq!(voltage_mv(0x0030), 1800);
    }

    /// A delay rounds to the nearest 150 ps step: 1450 ps is nearer 1500 than 1350.
    #[test]
    fn a_delay_rounds_to_the_nearest_step() {
        let s = Settings::from_mainline(1450, 1420, 0, 0, None);
        assert_eq!((s.receive_delay, s.transmit_delay), (10, 9));
    }

    /// `configure` replaces both delay fields and leaves the rest of the RGMII word, and writes
    /// the drive codes into a pad word that held nothing, so a stray bit would show.
    #[test]
    fn configure_replaces_the_delay_and_drive_fields_and_nothing_else() {
        let mut physical_layer_device = PhysicalLayerDevice::yt8531_at_1v8();
        write_extended(&mut physical_layer_device, 0, EXT_RGMII_CONFIG1, 0xffff).unwrap();
        write_extended(&mut physical_layer_device, 0, EXT_PAD_DRIVE_STRENGTH, 0).unwrap();
        let s = Settings {
            receive_data_drive: Drive::Code(7),
            ..Settings::FALLBACK_GMAC0
        };
        let a = configure(&mut physical_layer_device, 0, &s).unwrap();
        // 0xffff with bits 13:10 and 3:0 replaced by 0xa each.
        assert_eq!(a.rgmii_config1, 0xebfa);
        // Clock drive 6 in bits 15:13; data drive 7 as 0b11 in bits 5:4 and 0b1 in bit 12.
        assert_eq!(a.pad_drive, 0xd030);
        assert_eq!(
            physical_layer_device.extended(EXT_PAD_DRIVE_STRENGTH),
            0xd030
        );
    }
}
