//! The JH7110 Ethernet port's two processes and what the kernel tells each: the control process's
//! windows, spawn words ([`control_spawn_words`]) and report ([`ControlRecord`]), and the data plane's
//! windows and spawn words ([`Handoff`]). The kernel maps every window and lists it on the
//! startup page. See notes/designware-ethernet.md.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves.

/// The driver crate itself, so the kernel names the tree's types without depending on it.
pub use ::designware_ethernet as device;
use address_space_map::{PAGE, pair_page};
use designware_ethernet::bring_up::{Account, AddressSource, Error, Inputs};
use designware_ethernet::cache_coherence::{Observation, Payload, Verdict};
use designware_ethernet::motorcomm_physical_layer_device::{Link, Settings, SettingsSource, Speed};
use designware_ethernet::{
    management_data_input_output as mdio, network_interface_controller as controller,
};
use startup_page_contract::WindowKind;

/// The DMA region, `designware_ethernet::layout::DIRECT_MEMORY_ACCESS_REGION_PAGES` pages, normal memory. The same address as
/// the virtio and `e1000e` servers' DMA windows; a `net_stack` drives one NIC, never two.
/// The spawner maps it here and lists it on the startup page, where the server reads it.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DATA_PLANE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DATA_PLANE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0090_0000);
/// The controller's DMA (direct memory access) register page
/// (`designware_ethernet::registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE`), device memory.
/// The spawner maps it here and lists it on the startup page, where the server reads it.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `dma` spelled out as `direct_memory_access`; this
/// lane added `REGISTER` to match the crate's `DIRECT_MEMORY_ACCESS_REGISTER_PAGE`, replacing
/// `DIRECT_MEMORY_ACCESS_PAGE_VIRTUAL_ADDRESS`.
#[doc(alias = "dma_page")]
pub const DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS: u64 =
    pair_page(0x0000_0000_009c_0000);

/// [`DATA_PLANE_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const DATA_PLANE_KIND: WindowKind = WindowKind(6);
/// [`DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const DIRECT_MEMORY_ACCESS_REGISTER_PAGE_KIND: WindowKind = WindowKind(7);

/// Where the control process finds the controller's first two register pages, device-typed: the
/// MAC's page and the DMA page (`designware_ethernet::registers` puts every register it drives
/// below `0x2000`).
///
/// Name: provisional.
pub const CONTROL_REGISTER_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0098_0000);
/// How many pages [`CONTROL_REGISTER_VIRTUAL_ADDRESS`] maps.
///
/// Name: provisional.
pub const CONTROL_REGISTER_PAGES: u64 = 2;
/// Where the control process finds the always-on syscon's page, device-typed: the word that selects
/// the port's interface is there.
///
/// Name: provisional.
pub const ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009a_0000);

const _: () = assert!(
    DATA_PLANE_VIRTUAL_ADDRESS
        + designware_ethernet::layout::DIRECT_MEMORY_ACCESS_REGION_PAGES * PAGE
        <= CONTROL_REGISTER_VIRTUAL_ADDRESS
);
const _: () = assert!(
    CONTROL_REGISTER_VIRTUAL_ADDRESS + CONTROL_REGISTER_PAGES * PAGE
        <= ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS
);

/// [`CONTROL_REGISTER_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const CONTROL_REGISTER_KIND: WindowKind = WindowKind(24);
/// [`ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const ALWAYS_ON_SYSTEM_CONTROLLER_KIND: WindowKind = WindowKind(25);

/// The control process's slot for the endpoint it sends its [`ControlRecord`]s on.
///
/// Name: provisional.
pub const CONTROL_REPORT_SLOT: u64 = 0;

/// `arg2`'s bit saying the tree names a station in its low 48 bits.
const TREE_NAMES_STATION: u64 = 1 << 48;

/// **What the kernel hands the control process at spawn**, in its three argument words: the
/// settings word, the region base with the PHY's MDIO address in its low bits, and the tree's
/// station address. `None` when the settings do not fit their word ([`Settings::to_word`]).
///
/// Name: provisional.
pub fn control_spawn_words(inputs: &Inputs) -> Option<[u64; 3]> {
    let station = match inputs.station_address {
        Some(a) => station_word(a) | TREE_NAMES_STATION,
        None => 0,
    };
    Some([
        inputs.physical_layer_device.to_word()?,
        inputs.region_physical_address | u64::from(inputs.physical_layer_device_address & 0x1f),
        station,
    ])
}

/// The inverse of [`control_spawn_words`], refusing settings no word carries, a zero base and any
/// bit no field owns.
///
/// Name: provisional.
pub fn control_inputs(words: [u64; 3]) -> Option<Inputs> {
    let [settings, base, station] = words;
    let physical_layer_device = Settings::from_word(settings)?;
    let region_physical_address = base & !(PAGE - 1);
    if region_physical_address == 0 || base & (PAGE - 1) > 0x1f {
        return None;
    }
    let tree_station = match station {
        0 => None,
        s if s & !(TREE_NAMES_STATION | ((1 << 48) - 1)) == 0 && s & TREE_NAMES_STATION != 0 => {
            Some(station_from_word(s))
        }
        _ => return None,
    };
    Some(Inputs {
        region_physical_address,
        station_address: tree_station,
        physical_layer_device_address: (base & 0x1f) as u8,
        physical_layer_device,
    })
}

/// **The control process's report, one message at a time.** It sends [`ACCOUNT_RECORDS`] parts
/// of its account in order, then [`ControlRecord::Done`], and exits.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRecord {
    /// Part `index` of the packed account: words `2 * index` and `2 * index + 1` of
    /// [`ControlReport::to_words`].
    Part {
        /// Which part.
        index: u8,
        /// Its two words.
        words: [u64; 2],
    },
    /// The outcome: the port is programmed, and started when it has a link; or it was refused.
    Done(Result<bool, Error>),
}

/// The tag every record's first word carries in its low half.
const RECORD_TAG: u64 = 0xd3c0;
/// [`ControlRecord::Done`]'s index, past every part's.
const DONE_INDEX: u64 = 0xff;
/// How many words a packed [`ControlReport`] is.
const ACCOUNT_WORDS: usize = 10;
/// How many [`ControlRecord::Part`]s carry a [`ControlReport`].
///
/// Name: provisional.
pub const ACCOUNT_RECORDS: usize = ACCOUNT_WORDS / 2;

impl ControlRecord {
    /// The three words the control process sends.
    ///
    /// Name: provisional.
    pub fn pack(&self) -> [u64; 3] {
        match *self {
            ControlRecord::Part { index, words } => {
                [RECORD_TAG | u64::from(index) << 16, words[0], words[1]]
            }
            ControlRecord::Done(outcome) => {
                let (code, detail) = outcome_code(outcome);
                [RECORD_TAG | DONE_INDEX << 16, code, detail]
            }
        }
    }

    /// The inverse of [`ControlRecord::pack`]; `None` for a word no control process sends.
    ///
    /// Name: provisional.
    pub fn unpack(words: [u64; 3]) -> Option<ControlRecord> {
        if words[0] & 0xffff != RECORD_TAG || words[0] >> 24 != 0 {
            return None;
        }
        match (words[0] >> 16) & 0xff {
            DONE_INDEX => Some(ControlRecord::Done(outcome_from_code(words[1], words[2])?)),
            index if (index as usize) < ACCOUNT_RECORDS => Some(ControlRecord::Part {
                index: index as u8,
                words: [words[1], words[2]],
            }),
            _ => None,
        }
    }
}

/// **The control process's account of the bring-up**: the driver crate's [`Account`] and the
/// interface select's word, before and after.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlReport {
    /// What the bring-up saw.
    pub account: Account,
    /// The always-on system controller's interface-select word, before and after the select.
    #[doc(alias = "syscon")]
    pub system_controller: (u32, u32),
}

const fn verdict_code(v: Verdict) -> u64 {
    match v {
        Verdict::Coherent => 1,
        Verdict::DeviceCannotSeeCpuWrites => 2,
        Verdict::CpuCannotSeeDeviceWrites => 3,
        Verdict::ReceivedPayloadStale => 4,
        Verdict::TransmittedPayloadStale => 5,
        Verdict::BusError => 6,
        Verdict::Inconclusive => 7,
    }
}

const fn verdict_from_code(c: u64) -> Option<Verdict> {
    Some(match c {
        1 => Verdict::Coherent,
        2 => Verdict::DeviceCannotSeeCpuWrites,
        3 => Verdict::CpuCannotSeeDeviceWrites,
        4 => Verdict::ReceivedPayloadStale,
        5 => Verdict::TransmittedPayloadStale,
        6 => Verdict::BusError,
        7 => Verdict::Inconclusive,
        _ => return None,
    })
}

const fn source_code(s: SettingsSource) -> u64 {
    match s {
        SettingsSource::VendorTree => 0,
        SettingsSource::MainlineTree => 1,
        SettingsSource::Fallback => 2,
    }
}

const fn source_from_code(c: u64) -> Option<SettingsSource> {
    Some(match c {
        0 => SettingsSource::VendorTree,
        1 => SettingsSource::MainlineTree,
        2 => SettingsSource::Fallback,
        _ => return None,
    })
}

const fn speed_from_code(c: u64) -> Option<Speed> {
    Some(match c {
        0 => Speed::Ten,
        1 => Speed::Hundred,
        2 => Speed::Thousand,
        _ => return None,
    })
}

const fn station_word(a: [u8; 6]) -> u64 {
    u64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], 0, 0])
}

const fn station_from_word(w: u64) -> [u8; 6] {
    let b = w.to_le_bytes();
    [b[0], b[1], b[2], b[3], b[4], b[5]]
}

/// An outcome's two words: a code, then what it carries.
fn outcome_code(outcome: Result<bool, Error>) -> (u64, u64) {
    match outcome {
        Ok(linked) => (0, u64::from(linked)),
        Err(Error::Silent { version }) => (1, u64::from(version)),
        Err(Error::NoStationAddress) => (2, 0),
        Err(Error::ManagementDataInputOutput(mdio::Error::Timeout {
            physical_layer_device_address,
            register,
        })) => (
            3,
            u64::from(physical_layer_device_address) | u64::from(register) << 8,
        ),
        Err(Error::UnknownPhysicalLayerDevice { id }) => (4, u64::from(id)),
        Err(Error::Controller(controller::Error::ResetTimeout)) => (5, 0),
        Err(Error::Controller(controller::Error::RegionOutOfReach {
            direct_memory_access_address_bits,
        })) => (6, u64::from(direct_memory_access_address_bits)),
        Err(Error::Controller(controller::Error::FlushTimeout)) => (7, 0),
        Err(Error::NotCoherent(v)) => (8, verdict_code(v)),
    }
}

fn outcome_from_code(code: u64, detail: u64) -> Option<Result<bool, Error>> {
    // The detail word, refused when it has bits past `bits`; a refusal that carries nothing
    // refuses any nonzero detail.
    let narrow = |bits: u32| (detail.checked_shr(bits).unwrap_or(0) == 0).then_some(detail);
    Some(match code {
        0 => Ok(narrow(1)? == 1),
        1 => Err(Error::Silent {
            version: narrow(32)? as u32,
        }),
        2 => narrow(0).map(|_| Err(Error::NoStationAddress))?,
        3 => {
            let d = narrow(16)?;
            Err(Error::ManagementDataInputOutput(mdio::Error::Timeout {
                physical_layer_device_address: d as u8,
                register: (d >> 8) as u8,
            }))
        }
        4 => Err(Error::UnknownPhysicalLayerDevice {
            id: narrow(32)? as u32,
        }),
        5 => narrow(0).map(|_| Err(Error::Controller(controller::Error::ResetTimeout)))?,
        6 => Err(Error::Controller(controller::Error::RegionOutOfReach {
            direct_memory_access_address_bits: narrow(8)? as u8,
        })),
        7 => narrow(0).map(|_| Err(Error::Controller(controller::Error::FlushTimeout)))?,
        8 => Err(Error::NotCoherent(verdict_from_code(detail)?)),
        _ => return None,
    })
}

// Word layout of a packed report. Presence bits sit in word 0 above the version and the width.
const HAS_PHYSICAL_LAYER_DEVICE: u64 = 1 << 40;
const HAS_STATION: u64 = 1 << 41;
const HAS_COHERENCE: u64 = 1 << 42;
const HAS_LINK: u64 = 1 << 43;

impl ControlReport {
    /// **The report as [`ACCOUNT_RECORDS`] pairs of words**: the version, address width and
    /// presence bits; the PHY; the station address; both resets; the probe's two words; the link;
    /// and the syscon word's two values.
    ///
    /// Name: provisional.
    pub fn to_words(&self) -> [u64; ACCOUNT_WORDS] {
        let a = &self.account;
        let mut w = [0u64; ACCOUNT_WORDS];
        w[0] = u64::from(a.version) | u64::from(a.direct_memory_access_address_bits) << 32;
        w[1] = u64::from(a.physical_layer_device_id);
        if let Some((millivolts, source)) = a.physical_layer_device {
            w[0] |= HAS_PHYSICAL_LAYER_DEVICE;
            w[1] |= u64::from(millivolts & 0xff_ffff) << 32 | source_code(source) << 56;
        }
        if let Some((station, source)) = a.station_address {
            w[0] |= HAS_STATION;
            w[2] = station_word(station) | u64::from(source == AddressSource::Registers) << 48;
        }
        w[3] = a.resets_microseconds[0];
        w[4] = a.resets_microseconds[1];
        if let Some((verdict, o)) = a.coherence {
            w[0] |= HAS_COHERENCE;
            let payload = match o.payload {
                Payload::Matches => 0,
                Payload::Poison => 1,
                Payload::Other => 2,
            };
            w[5] = verdict_code(verdict) | payload << 8 | u64::from(o.status) << 32;
            w[6] = u64::from(o.receive_descriptor_word_3)
                | u64::from(o.transmit_descriptor_word_3) << 32;
        }
        if let Some(l) = a.link {
            w[0] |= HAS_LINK;
            let speed = match l.speed {
                Speed::Ten => 0,
                Speed::Hundred => 1,
                Speed::Thousand => 2,
            };
            w[7] = speed | u64::from(l.full_duplex) << 8;
        }
        w[8] = a.link_wait_milliseconds;
        w[9] = u64::from(self.system_controller.0) | u64::from(self.system_controller.1) << 32;
        w
    }

    /// The inverse of [`ControlReport::to_words`]; `None` for a code no field has.
    ///
    /// Name: provisional.
    pub fn from_words(w: &[u64; ACCOUNT_WORDS]) -> Option<ControlReport> {
        let mut a = Account {
            version: w[0] as u32,
            direct_memory_access_address_bits: (w[0] >> 32) as u8,
            physical_layer_device_id: w[1] as u32,
            resets_microseconds: [w[3], w[4]],
            link_wait_milliseconds: w[8],
            ..Account::default()
        };
        if w[0] & HAS_PHYSICAL_LAYER_DEVICE != 0 {
            a.physical_layer_device = Some((
                ((w[1] >> 32) & 0xff_ffff) as u32,
                source_from_code(w[1] >> 56)?,
            ));
        }
        if w[0] & HAS_STATION != 0 {
            let source = match w[2] >> 48 {
                0 => AddressSource::Tree,
                1 => AddressSource::Registers,
                _ => return None,
            };
            a.station_address = Some((station_from_word(w[2]), source));
        }
        if w[0] & HAS_COHERENCE != 0 {
            let payload = match (w[5] >> 8) & 0xff {
                0 => Payload::Matches,
                1 => Payload::Poison,
                2 => Payload::Other,
                _ => return None,
            };
            a.coherence = Some((
                verdict_from_code(w[5] & 0xff)?,
                Observation {
                    receive_descriptor_word_3: w[6] as u32,
                    transmit_descriptor_word_3: (w[6] >> 32) as u32,
                    status: (w[5] >> 32) as u32,
                    payload,
                },
            ));
        }
        if w[0] & HAS_LINK != 0 {
            a.link = Some(Link {
                speed: speed_from_code(w[7] & 0xff)?,
                full_duplex: (w[7] >> 8) & 1 != 0,
            });
        }
        Some(ControlReport {
            account: a,
            system_controller: (w[9] as u32, (w[9] >> 32) as u32),
        })
    }
}

/// The role word's low half when `arg0` is this server's handoff. Disjoint from `net_stack`'s
/// existing roles: 0 is the virtio server, small integers are socket clients, `0xe1e0` is
/// `e1000e`'s.
///
/// Name: provisional. No ruling covers it.
pub const ROLE_TAG: u64 = 0xd3e0;

/// **What the kernel hands `net_stack` at spawn**, in two of its three argument words.
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The station address the kernel programmed. The process cannot read or change it: the
    /// MAC's page is not mapped.
    #[doc(alias = "mac_address")]
    pub station_address: [u8; 6],
    /// The DMA region's physical base. Descriptors speak physical addresses (there is no IOMMU
    /// on the JH7110), so the process cannot compute them from its virtual window.
    pub data_plane_physical_address: u64,
}

impl Handoff {
    /// `[arg0, arg1]`: the tag and the MAC address in the role word, the base in the second.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn pack(&self) -> [u64; 2] {
        let mut station_address = [0u8; 8];
        station_address[..6].copy_from_slice(&self.station_address);
        [
            ROLE_TAG | u64::from_le_bytes(station_address) << 16,
            self.data_plane_physical_address,
        ]
    }

    /// The inverse of [`Handoff::pack`], refusing a role word without the tag, a base that is
    /// zero or not page aligned, and a MAC address no station can own.
    ///
    /// # Examples
    ///
    /// ```
    /// use driver_process_contract::designware_ethernet::Handoff;
    ///
    /// let h = Handoff {
    ///     station_address: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c],
    ///     data_plane_physical_address: 0x4012_3000,
    /// };
    /// let [role, phys] = h.pack();
    /// assert_eq!(Handoff::unpack(role, phys), Some(h));
    /// assert_eq!(Handoff::unpack(0, phys), None);
    /// assert_eq!(Handoff::unpack(role, phys + 8), None);
    /// ```
    ///
    /// Name: provisional. No ruling covers it.
    pub fn unpack(arg0: u64, arg1: u64) -> Option<Handoff> {
        if arg0 & 0xffff != ROLE_TAG || arg1 == 0 || !arg1.is_multiple_of(PAGE) {
            return None;
        }
        let b = (arg0 >> 16).to_le_bytes();
        let station_address = [b[0], b[1], b[2], b[3], b[4], b[5]];
        if station_address == [0; 6] || station_address[0] & 1 != 0 {
            return None;
        }
        Some(Handoff {
            station_address,
            data_plane_physical_address: arg1,
        })
    }

    /// Is `arg0` this server's role word at all? `net_stack` dispatches on this.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn is_role(arg0: u64) -> bool {
        arg0 & 0xffff == ROLE_TAG
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn inputs(station: Option<[u8; 6]>) -> Inputs {
        Inputs {
            region_physical_address: 0x4012_3000,
            station_address: station,
            physical_layer_device_address: 0,
            physical_layer_device: Settings::FALLBACK_GMAC0,
        }
    }

    /// The control process's spawn words survive their packing with and without a tree station,
    /// and a word with a bit no field owns is refused.
    #[test]
    fn the_control_spawn_words_round_trip_and_refuse_stray_bits() {
        for station in [None, Some([0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c])] {
            let i = Inputs {
                physical_layer_device_address: 7,
                ..inputs(station)
            };
            let [a0, a1, a2] =
                control_spawn_words(&i).expect("the fallback settings fit their word");
            assert_eq!(control_inputs([a0, a1, a2]), Some(i));
            assert_eq!(control_inputs([a0, a1 | 0x20, a2]), None);
            assert_eq!(control_inputs([a0, a1 & 0x1f, a2]), None);
            assert_eq!(control_inputs([a0, a1, a2 | 1 << 50]), None);
        }
        assert_eq!(
            control_inputs([0, 0x4000_0000, 1]),
            None,
            "a station with no flag"
        );
    }

    /// Every record survives its packing, every refusal keeps its detail, and a word no control
    /// process sends is refused.
    #[test]
    fn every_record_round_trips() {
        let refusals = [
            Error::Silent {
                version: 0xffff_ffff,
            },
            Error::NoStationAddress,
            Error::ManagementDataInputOutput(mdio::Error::Timeout {
                physical_layer_device_address: 3,
                register: 0x1e,
            }),
            Error::UnknownPhysicalLayerDevice { id: 0x0102_0304 },
            Error::Controller(controller::Error::ResetTimeout),
            Error::Controller(controller::Error::RegionOutOfReach {
                direct_memory_access_address_bits: 32,
            }),
            Error::Controller(controller::Error::FlushTimeout),
            Error::NotCoherent(Verdict::DeviceCannotSeeCpuWrites),
        ];
        let outcomes = [Ok(true), Ok(false)]
            .into_iter()
            .chain(refusals.into_iter().map(Err));
        for outcome in outcomes {
            let r = ControlRecord::Done(outcome);
            assert_eq!(ControlRecord::unpack(r.pack()), Some(r));
        }
        let part = ControlRecord::Part {
            index: 4,
            words: [u64::MAX, 1],
        };
        assert_eq!(ControlRecord::unpack(part.pack()), Some(part));
        let [w0, w1, w2] = part.pack();
        assert_eq!(
            ControlRecord::unpack([w0 + (1 << 16), w1, w2]),
            None,
            "past the last part"
        );
        assert_eq!(
            ControlRecord::unpack([w0 ^ 1, w1, w2]),
            None,
            "not a record"
        );
        let [d0, _, _] = ControlRecord::Done(Err(Error::NoStationAddress)).pack();
        assert_eq!(
            ControlRecord::unpack([d0, 2, 1]),
            None,
            "a detail nothing carries"
        );
    }

    /// A full report and an empty one survive their words, including every verdict.
    #[test]
    fn the_report_round_trips() {
        assert_eq!(
            ControlReport::from_words(&ControlReport::default().to_words()),
            Some(ControlReport::default())
        );
        for verdict in [
            Verdict::Coherent,
            Verdict::DeviceCannotSeeCpuWrites,
            Verdict::CpuCannotSeeDeviceWrites,
            Verdict::ReceivedPayloadStale,
            Verdict::TransmittedPayloadStale,
            Verdict::BusError,
            Verdict::Inconclusive,
        ] {
            let r = ControlReport {
                account: Account {
                    version: 0x5251,
                    direct_memory_access_address_bits: 40,
                    physical_layer_device_id: 0x4f51_e91b,
                    physical_layer_device: Some((1800, SettingsSource::MainlineTree)),
                    station_address: Some((
                        [0x6c, 0xcf, 0x39, 0, 0x47, 0x2c],
                        AddressSource::Registers,
                    )),
                    resets_microseconds: [12, 34],
                    coherence: Some((
                        verdict,
                        Observation {
                            receive_descriptor_word_3: 0x3400_0080,
                            transmit_descriptor_word_3: 0x3000_0080,
                            status: 0x0000_0441,
                            payload: Payload::Poison,
                        },
                    )),
                    link: Some(Link {
                        speed: Speed::Hundred,
                        full_duplex: true,
                    }),
                    link_wait_milliseconds: 2900,
                },
                system_controller: (0x0, 0x1),
            };
            assert_eq!(ControlReport::from_words(&r.to_words()), Some(r));
        }
    }

    #[test]
    fn the_handoff_refuses_a_multicast_or_absent_address() {
        let h = Handoff {
            station_address: [0x01, 0, 0, 0, 0, 1],
            data_plane_physical_address: 0x4000_0000,
        };
        let [r, p] = h.pack();
        assert_eq!(Handoff::unpack(r, p), None);
        let [r, p] = Handoff {
            station_address: [0; 6],
            ..h
        }
        .pack();
        assert_eq!(Handoff::unpack(r, p), None);
        assert!(!Handoff::is_role(0xe1e0));
    }

    /// A handoff survives its own packing, and each refusal holds on its own: a role word that is
    /// not this server's is refused with a good base beside it, as is a good role word with no
    /// base or a base off a page boundary.
    #[test]
    fn the_handoff_round_trips_and_each_refusal_stands_alone() {
        let h = Handoff {
            station_address: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c],
            data_plane_physical_address: 0x4800_0000,
        };
        let [r, p] = h.pack();
        assert!(Handoff::is_role(r));
        assert_eq!(Handoff::unpack(r, p), Some(h));
        assert_eq!(Handoff::unpack(r ^ 1, p), None, "not this server's role");
        assert_eq!(Handoff::unpack(r, 0), None, "no base");
        assert_eq!(Handoff::unpack(r, p + 8), None, "not page aligned");
    }
}
