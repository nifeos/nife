//! **The port's whole bring-up, in order**: identify the controller, take the station address,
//! configure the PHY, reset the controller, run the coherence probe, reset again, program both
//! rings, wait for a link and start at its speed. A [`Port`] supplies the register window, the DMA
//! region and a clock; this module supplies the order and every decision, so the process that
//! drives the silicon and the host tests that drive the simulation run the same code.
//!
//! The clocks, the resets and the board's interface select come first and are not here: they sit
//! in windows the controller does not own.
//!
//! Name: provisional.

use crate::cache_coherence::{self, Observation, Verdict};
use crate::motorcomm_physical_layer_device::{self, Link, Settings, SettingsSource, Speed};
use crate::network_interface_controller::{self as controller, Identity};
use crate::{
    DataPlane, DirectMemoryAccessRegion, Hardware, Tails, jh7110, layout,
    management_data_input_output, registers,
};

/// **What the bring-up drives**: the controller's register window (the [`Hardware`] it is), its
/// DMA region with the tail registers beside it, and a clock.
///
/// Name: provisional.
pub trait Port: Hardware {
    /// The CPU's handle on the DMA region.
    ///
    /// Name: provisional.
    type Region<'a>: DirectMemoryAccessRegion
    where
        Self: 'a;
    /// The channel's tail registers.
    ///
    /// Name: provisional.
    type Tails<'a>: Tails
    where
        Self: 'a;
    /// Run `f` with the region and the tail registers, which alias the window and so cannot be
    /// borrowed beside it.
    ///
    /// Name: provisional. `with`, as `simulator`'s handle and `LocalKey::with` spell it.
    fn with_region<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Region<'_>, &mut Self::Tails<'_>) -> R,
    ) -> R;
    /// A monotonic clock, in microseconds.
    ///
    /// Name: provisional.
    fn now_microseconds(&mut self) -> u64;
}

/// **What the board and its tree say about this port**, everything the bring-up needs that the
/// controller cannot tell it.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// The DMA region's physical base, which descriptors name.
    pub region_physical_address: u64,
    /// (`station_address`, the IEEE 802.3 standard's name for a MAC address) The tree's
    /// `local-mac-address`, if it has one.
    #[doc(alias = "mac_address")]
    pub station_address: Option<[u8; 6]>,
    /// (`physical_layer_device_address`, the IEEE 802.3 standard's PHY address) The PHY's MDIO
    /// address.
    #[doc(alias = "phy")]
    #[doc(alias = "phy_address")]
    pub physical_layer_device_address: u8,
    /// The PHY's board settings.
    #[doc(alias = "phy")]
    pub physical_layer_device: Settings,
}

/// How long the bring-up waits for the PHY to resolve a link after autonegotiation restarts. A
/// gigabit negotiation takes about three seconds; five is slack, not a guess at a slow switch.
///
/// Name: provisional.
pub const LINK_WAIT_MILLISECONDS: u64 = 5000;

/// How often the link wait asks the PHY.
const LINK_POLL_MICROSECONDS: u64 = 10_000;

/// Why the bring-up refused the port.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The controller's version register read as nothing (zero or all ones): its clocks or resets
    /// are not up, or the window is wrong.
    Silent {
        /// What the version register read.
        version: u32,
    },
    /// (`NoStationAddress`, after the IEEE 802.3 standard's station address) The station address
    /// is in neither the tree nor the controller's own registers.
    #[doc(alias = "no_mac_address")]
    NoStationAddress,
    /// An MDIO command did not complete.
    #[doc(alias = "mdio")]
    ManagementDataInputOutput(management_data_input_output::Error),
    /// The PHY is not the YT8531 these board settings are for.
    UnknownPhysicalLayerDevice {
        /// What it answered.
        id: u32,
    },
    /// A controller sequence refused (a reset that did not complete, a region out of reach).
    Controller(controller::Error),
    /// The coherence probe found device DMA not coherent with the caches.
    NotCoherent(Verdict),
}

/// (`AddressSource`, after the IEEE 802.3 standard's station address) Where the station address
/// came from.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "mac_address_source")]
pub enum AddressSource {
    /// The tree's `local-mac-address`, which U-Boot writes from the board's EEPROM.
    Tree,
    /// The controller's address registers, as firmware left them.
    Registers,
}

/// **Everything the bring-up saw**, filled as far as it got, so a refusal still says where it
/// stopped.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Account {
    /// The controller's version register.
    pub version: u32,
    /// (`direct_memory_access_address_bits`, the databook's `ADDR64` field) The DMA's address
    /// width.
    #[doc(alias = "addr64")]
    pub direct_memory_access_address_bits: u8,
    /// The PHY's identifier.
    #[doc(alias = "phy")]
    pub physical_layer_device_id: u32,
    /// The PHY's I/O voltage, and which tree spelling its settings came from, once configured.
    #[doc(alias = "phy")]
    pub physical_layer_device: Option<(u32, SettingsSource)>,
    /// (`station_address`, the IEEE 802.3 standard's name for a MAC address) The station address
    /// and where it came from.
    #[doc(alias = "mac_address")]
    pub station_address: Option<([u8; 6], AddressSource)>,
    /// How long each software reset took, in microseconds: the probe's, then the real one's.
    pub resets_microseconds: [u64; 2],
    /// The coherence probe's verdict and what it saw.
    pub coherence: Option<(Verdict, Observation)>,
    /// The negotiated link, if one resolved.
    pub link: Option<Link>,
    /// How long the link took to resolve, or how long was waited for it.
    pub link_wait_milliseconds: u64,
}

/// **Bring the port up**, recording into `account` as it goes. On success the controller runs at
/// the link's speed with both rings programmed and empty, and the answer is the station address
/// and the link; with no link the rings are programmed and the controller is not started.
///
/// # Errors
///
/// The first [`Error`] the sequence meets; `account` holds what it saw until then.
///
/// Name: provisional.
pub fn bring_up(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<([u8; 6], Option<Link>), Error> {
    let id = Identity::read(port);
    account.version = id.version;
    account.direct_memory_access_address_bits = id.direct_memory_access_address_bits();
    if id.version == 0 || id.version == u32::MAX {
        return Err(Error::Silent {
            version: id.version,
        });
    }

    // The address before the reset, which returns the registers to all ones.
    let station = match inputs.station_address {
        Some(m) => (m, AddressSource::Tree),
        None => controller::station_address(
            port.read(registers::MAC_ADDRESS_0_HIGH_REGISTER),
            port.read(registers::MAC_ADDRESS_0_LOW_REGISTER),
        )
        .map(|m| (m, AddressSource::Registers))
        .ok_or(Error::NoStationAddress)?,
    };
    account.station_address = Some(station);
    let station = station.0;

    bring_up_physical_layer_device(port, inputs, account)?;

    // The PHY keeps its receive clock running from here on, which the reset needs.
    account.resets_microseconds[0] = controller::soft_reset(port).map_err(Error::Controller)?;

    let verdict = probe(port, &id, inputs.region_physical_address, station, account)?;
    if !matches!(verdict, Verdict::Coherent | Verdict::Inconclusive) {
        return Err(Error::NotCoherent(verdict));
    }

    // The probe left the controller in loopback with a used ring: reset it and start clean.
    let _ = controller::stop(port);
    account.resets_microseconds[1] = controller::soft_reset(port).map_err(Error::Controller)?;
    zero(port);
    controller::configure(
        port,
        &id,
        &jh7110::BUS_CONFIGURATION,
        inputs.region_physical_address,
        station,
    )
    .map_err(Error::Controller)?;

    let link = wait_for_link(port, inputs, account)?;
    Ok((station, link))
}

/// Identify the PHY, refuse any other part, and configure it for the board.
fn bring_up_physical_layer_device(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<(), Error> {
    let device = inputs.physical_layer_device_address;
    account.physical_layer_device_id = management_data_input_output::physical_layer_device_id(
        &mut management_data_input_output::Bus(port),
        device,
    )
    .map_err(Error::ManagementDataInputOutput)?;
    if account.physical_layer_device_id
        != motorcomm_physical_layer_device::MOTORCOMM_YT8531_PHYSICAL_LAYER_DEVICE_ID
    {
        return Err(Error::UnknownPhysicalLayerDevice {
            id: account.physical_layer_device_id,
        });
    }
    let applied = motorcomm_physical_layer_device::configure(
        &mut management_data_input_output::Bus(port),
        device,
        &inputs.physical_layer_device,
    )
    .map_err(Error::ManagementDataInputOutput)?;
    account.physical_layer_device = Some((
        applied.voltage_millivolts,
        inputs.physical_layer_device.source,
    ));
    Ok(())
}

/// Restart autonegotiation, wait up to [`LINK_WAIT_MILLISECONDS`] for a link, and start the
/// controller at its speed. `None` when no link came up.
fn wait_for_link(
    port: &mut impl Port,
    inputs: &Inputs,
    account: &mut Account,
) -> Result<Option<Link>, Error> {
    let device = inputs.physical_layer_device_address;
    management_data_input_output::restart_autonegotiation(
        &mut management_data_input_output::Bus(&mut *port),
        device,
    )
    .map_err(Error::ManagementDataInputOutput)?;
    let started = port.now_microseconds();
    let link = loop {
        if let Some(l) = motorcomm_physical_layer_device::read_link(
            &mut management_data_input_output::Bus(&mut *port),
            device,
        )
        .map_err(Error::ManagementDataInputOutput)?
        {
            break Some(l);
        }
        if port.now_microseconds() - started >= LINK_WAIT_MILLISECONDS * 1000 {
            break None;
        }
        port.delay_microseconds(LINK_POLL_MICROSECONDS);
    };
    account.link_wait_milliseconds = (port.now_microseconds() - started) / 1000;
    account.link = link;
    if let Some(l) = link {
        motorcomm_physical_layer_device::apply_speed(
            &mut management_data_input_output::Bus(&mut *port),
            device,
            &inputs.physical_layer_device,
            l.speed,
        )
        .map_err(Error::ManagementDataInputOutput)?;
        controller::start(port, l, false);
    }
    Ok(link)
}

/// **The coherence probe** (`cache_coherence`'s header): MAC loopback, one frame, the CPU's
/// account against the device's.
fn probe(
    port: &mut impl Port,
    id: &Identity,
    region: u64,
    station: [u8; 6],
    account: &mut Account,
) -> Result<Verdict, Error> {
    zero(port);
    controller::configure(port, id, &jh7110::BUS_CONFIGURATION, region, station)
        .map_err(Error::Controller)?;
    let loopback = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };
    controller::start(port, loopback, true);
    let mut data_plane = DataPlane::new(region);
    // A probe frame is a fixed 128 bytes, so `transmit` cannot refuse it for its length, and the
    // ring was just emptied, so it cannot be full; an error here would leave nothing to observe,
    // which `classify` reports as inconclusive.
    let _ = port.with_region(|r, t| cache_coherence::prepare(&mut data_plane, r, t, station));
    let started = port.now_microseconds();
    while !port.with_region(|r, _| cache_coherence::is_delivered(r))
        && port.now_microseconds() - started < cache_coherence::WAIT_MICROSECONDS
    {
        port.delay_microseconds(1);
    }
    let status = port.read(registers::CHANNEL_STATUS_REGISTER);
    let o = port.with_region(|r, _| cache_coherence::observe(r, station, status));
    let v = cache_coherence::classify(&o);
    account.coherence = Some((v, o));
    Ok(v)
}

/// Zero the region. A stale descriptor from the probe, or a previous wiring, would otherwise read
/// as a received frame.
fn zero(port: &mut impl Port) {
    port.with_region(|r, _| {
        for offset in (0..layout::DIRECT_MEMORY_ACCESS_REGION_BYTES).step_by(8) {
            r.write_u64(offset, 0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management_data_input_output::PHYSICAL_LAYER_DEVICE_ID_1_REGISTER;
    use crate::simulator::{Memory, NetworkInterfaceController};

    const REGION: u64 = 0x4800_0000;
    const STATION: [u8; 6] = [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c];
    const GIGABIT: Link = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };

    fn inputs() -> Inputs {
        Inputs {
            region_physical_address: REGION,
            station_address: Some(STATION),
            physical_layer_device_address: 0,
            physical_layer_device: Settings::FALLBACK_GMAC0,
        }
    }

    fn port(memory: Memory, link: Option<Link>) -> NetworkInterfaceController {
        let mut p = NetworkInterfaceController::new(REGION, memory);
        p.physical_layer_device.set_link(link);
        p
    }

    /// The whole sequence on the machine every source says the JH7110 is: it ends started at the
    /// link's speed, out of loopback, with the probe's verdict and both resets on the account.
    #[test]
    fn a_coherent_port_with_a_link_comes_up_started_and_out_of_loopback() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        assert_eq!(
            bring_up(&mut p, &inputs(), &mut account),
            Ok((STATION, Some(GIGABIT)))
        );
        assert!(matches!(account.coherence, Some((Verdict::Coherent, _))));
        assert_eq!(
            account.station_address,
            Some((STATION, AddressSource::Tree))
        );
        assert_eq!(
            account.physical_layer_device,
            Some((1800, SettingsSource::Fallback))
        );
        assert_eq!(account.link, Some(GIGABIT));
        let configuration = p.register(registers::MAC_CONFIGURATION_REGISTER);
        assert_ne!(
            configuration & registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
            0
        );
        assert_eq!(
            configuration & registers::MAC_CONFIGURATION_LOOPBACK_MODE,
            0
        );
        // The probe's frame went round the loopback, never onto the wire.
        assert!(p.wire_out().is_empty());
    }

    /// No link: the rings are programmed and the controller is not started, after the whole wait.
    #[test]
    fn a_port_with_no_link_waits_the_bound_and_is_not_started() {
        let mut p = port(Memory::coherent(REGION), None);
        let mut account = Account::default();
        assert_eq!(
            bring_up(&mut p, &inputs(), &mut account),
            Ok((STATION, None))
        );
        assert!(account.link_wait_milliseconds >= LINK_WAIT_MILLISECONDS);
        assert_eq!(
            p.register(registers::MAC_CONFIGURATION_REGISTER)
                & registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
            0
        );
    }

    /// A machine whose DMA does not see the CPU's writes is refused at the probe, and the account
    /// says what the probe saw.
    #[test]
    fn a_port_that_cannot_see_the_cpus_writes_is_refused_at_the_probe() {
        let mut p = port(Memory::write_back_unsnooped(REGION), Some(GIGABIT));
        let mut account = Account::default();
        assert_eq!(
            bring_up(&mut p, &inputs(), &mut account),
            Err(Error::NotCoherent(Verdict::DeviceCannotSeeCpuWrites))
        );
        assert!(account.coherence.is_some());
        assert_eq!(account.link, None);
    }

    /// No tree address and registers reset to all ones: there is no address to take.
    #[test]
    fn with_no_station_named_anywhere_the_port_is_refused_before_the_physical_layer_device() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        let mut account = Account::default();
        let no_tree = Inputs {
            station_address: None,
            ..inputs()
        };
        assert_eq!(
            bring_up(&mut p, &no_tree, &mut account),
            Err(Error::NoStationAddress)
        );
        assert_eq!(account.physical_layer_device_id, 0);
    }

    /// Another PHY is refused before its registers are written.
    #[test]
    fn another_physical_layer_device_is_refused_and_named() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.physical_layer_device
            .write_raw(PHYSICAL_LAYER_DEVICE_ID_1_REGISTER, 0x1234);
        let mut account = Account::default();
        assert!(matches!(
            bring_up(&mut p, &inputs(), &mut account),
            Err(Error::UnknownPhysicalLayerDevice { .. })
        ));
        assert_eq!(account.physical_layer_device, None);
    }

    /// A PHY that drives no receive clock leaves the reset hung, and the refusal comes after the
    /// PHY was configured, which the account shows.
    #[test]
    fn a_reset_with_no_receive_clock_is_refused_after_the_physical_layer_device() {
        let mut p = port(Memory::coherent(REGION), Some(GIGABIT));
        p.stop_receive_clock();
        let mut account = Account::default();
        assert_eq!(
            bring_up(&mut p, &inputs(), &mut account),
            Err(Error::Controller(controller::Error::ResetTimeout))
        );
        assert!(account.physical_layer_device.is_some());
        assert_eq!(account.coherence, None);
    }
}
