//! **Descriptors: what a device says about itself** (USB 2.0 section 9.6, HID 1.11 section 6.2).
//!
//! A descriptor is a run of bytes whose first byte is its own length and whose second is its type.
//! A configuration descriptor is followed, in one transfer, by every interface and endpoint
//! descriptor beneath it, concatenated, so reading one is a walk over a linked structure whose
//! links are lengths **the device chose**. That is the whole hazard: a zero length loops forever,
//! a length past the end reads past the end, and a walk that trusts either is a driver a cheap
//! keyboard can crash. [`find_boot_keyboard`] checks both before every step.

/// `bDescriptorType` of a device descriptor.
pub const DEVICE: u8 = 1;
/// `bDescriptorType` of a configuration descriptor.
pub const CONFIGURATION: u8 = 2;
/// `bDescriptorType` of an interface descriptor.
pub const INTERFACE: u8 = 4;
/// `bDescriptorType` of an endpoint descriptor.
pub const ENDPOINT: u8 = 5;

/// How long a device descriptor is (USB 2.0 Table 9-8).
pub const DEVICE_LEN: usize = 18;
/// How long a configuration descriptor's own header is, before its interfaces (Table 9-10).
pub const CONFIGURATION_LEN: usize = 9;
/// How long an interface descriptor is (Table 9-12).
pub const INTERFACE_LEN: usize = 9;
/// How long an endpoint descriptor is (Table 9-13). USB 3 appends a companion descriptor rather
/// than lengthening this one, so 7 holds at every speed.
pub const ENDPOINT_LEN: usize = 7;

/// The HID interface class (HID 1.11 section 4.1).
pub const CLASS_HID: u8 = 3;
/// The HID boot interface subclass (HID 1.11 section 4.2): the device promises the fixed boot report.
pub const SUBCLASS_BOOT: u8 = 1;
/// The boot keyboard protocol (HID 1.11 section 4.3).
pub const PROTOCOL_KEYBOARD: u8 = 1;

/// **Why a descriptor was refused.** Each names the fact a person at the bench needs, because "the
/// keyboard does not work" is the report this driver would otherwise produce for all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer bytes than the descriptor's fixed header needs.
    Truncated,
    /// The bytes are not the descriptor they were asked for: the wrong type, or a length field
    /// shorter than that type's fixed layout.
    NotThatDescriptor,
    /// A descriptor inside a configuration has a length of zero or one, or runs past the end of
    /// what the device sent. `at` is its byte offset, for the bench transcript.
    Malformed {
        /// Byte offset of the bad descriptor within the configuration.
        at: u16,
    },
    /// The configuration has no interface of class 3, subclass 1, protocol 1. The device is not a
    /// keyboard, or is one that does not offer the boot protocol.
    NoBootKeyboard,
    /// A boot keyboard interface with no interrupt IN endpoint, which no conforming keyboard sends.
    NoInterruptIn {
        /// The interface that had none.
        interface: u8,
    },
    /// The interrupt endpoint's packets are smaller than one eight-byte boot report, so a report
    /// would arrive in pieces this driver does not reassemble.
    PacketTooSmall {
        /// What the endpoint descriptor declared.
        max_packet: u16,
    },
}

/// **The device descriptor**, the fields a driver uses from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// `bcdUSB`: the USB revision the device claims, in binary-coded decimal (0x0200 for 2.0).
    pub usb: u16,
    /// `bDeviceClass`. Zero means "each interface says for itself", which is what keyboards send.
    pub class: u8,
    /// `bMaxPacketSize0`: the control endpoint's packet size in bytes, or, for a `SuperSpeed`
    /// device, the exponent of it ([`control_max_packet`] says which).
    pub max_packet0: u8,
    /// `idVendor`.
    pub vendor: u16,
    /// `idProduct`.
    pub product: u16,
    /// `bNumConfigurations`.
    pub configurations: u8,
}

impl DeviceDescriptor {
    /// Parse a device descriptor. `None` for fewer than [`DEVICE_LEN`] bytes, the wrong type, or a
    /// `bLength` shorter than the layout.
    pub fn parse(b: &[u8]) -> Option<DeviceDescriptor> {
        if b.len() < DEVICE_LEN || (b[0] as usize) < DEVICE_LEN || b[1] != DEVICE {
            return None;
        }
        Some(DeviceDescriptor {
            usb: u16::from_le_bytes([b[2], b[3]]),
            class: b[4],
            max_packet0: b[7],
            vendor: u16::from_le_bytes([b[8], b[9]]),
            product: u16::from_le_bytes([b[10], b[11]]),
            configurations: b[17],
        })
    }
}

/// **The control endpoint's packet size**, from `bMaxPacketSize0` as the device sent it in the
/// first eight bytes of its device descriptor. `None` for a value the specification does not
/// allow at that speed.
///
/// Below `SuperSpeed` the byte is the size and must be 8, 16, 32 or 64 (USB 2.0 section 9.6.1). At
/// `SuperSpeed` it is an exponent and must be 9, for 512 bytes (USB 3.2 section 9.6.1). Refusing anything
/// else matters because the host controller is told this number and sizes every control transfer
/// by it.
pub fn control_max_packet(b_max_packet0: u8, superspeed: bool) -> Option<u16> {
    match (superspeed, b_max_packet0) {
        (true, 9) => Some(512),
        (false, 8 | 16 | 32 | 64) => Some(u16::from(b_max_packet0)),
        _ => None,
    }
}

/// **The configuration descriptor's header**: how long the whole configuration is, and the value
/// `SET_CONFIGURATION` selects it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationHeader {
    /// `wTotalLength`: the header plus every descriptor beneath it.
    pub total_length: u16,
    /// `bNumInterfaces`.
    pub interfaces: u8,
    /// `bConfigurationValue`, the argument to `SET_CONFIGURATION`.
    pub value: u8,
}

impl ConfigurationHeader {
    /// Parse the first [`CONFIGURATION_LEN`] bytes of a configuration descriptor.
    pub fn parse(b: &[u8]) -> Result<ConfigurationHeader, Refusal> {
        if b.len() < CONFIGURATION_LEN {
            return Err(Refusal::Truncated);
        }
        if (b[0] as usize) < CONFIGURATION_LEN || b[1] != CONFIGURATION {
            return Err(Refusal::NotThatDescriptor);
        }
        Ok(ConfigurationHeader {
            total_length: u16::from_le_bytes([b[2], b[3]]),
            interfaces: b[4],
            value: b[5],
        })
    }
}

/// **A keyboard that speaks the boot protocol, and where its reports arrive.** Everything the
/// driver needs to configure the device and the host controller's endpoint, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootKeyboard {
    /// The `bConfigurationValue` to select with `SET_CONFIGURATION`.
    pub configuration: u8,
    /// The interface number, for the class requests `SET_PROTOCOL` and `SET_IDLE`.
    pub interface: u8,
    /// The interrupt IN endpoint's address, direction bit included (`0x81` for endpoint 1 IN).
    pub endpoint: u8,
    /// The endpoint's packet size in bytes; at least 8, or the walk refused it.
    pub max_packet: u16,
    /// `bInterval` as the device sent it. What it means depends on the speed, which is the host
    /// controller driver's business.
    pub interval: u8,
}

/// **Find the boot keyboard in a configuration**: the first interface (alternate setting 0) of
/// class 3, subclass 1, protocol 1, and the first interrupt IN endpoint beneath it.
///
/// `config` is the configuration as the device sent it, possibly cut short by the buffer it was
/// read into: the walk stops at the lesser of `wTotalLength` and `config.len()`, so a truncated
/// read is a shorter walk rather than an overrun. A malformed descriptor (a length of 0 or 1, or
/// one that runs past `wTotalLength`) is refused with its offset. The walk answers at the first
/// keyboard endpoint it finds, so a defect after that is never read: the bytes the keyboard was
/// found in were well formed, and a trailing defect is common in cheap devices.
///
/// # Examples
///
/// A boot keyboard's configuration as QEMU's `usb-kbd` reports it, trimmed to what matters:
///
/// ```
/// use usb::descriptor::find_boot_keyboard;
///
/// let config = [
///     9, 2, 34, 0, 1, 1, 0, 0xa0, 50, // configuration 1, 34 bytes, one interface
///     9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0: HID, boot, keyboard, one endpoint
///     9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0, // the HID descriptor, which the walk skips
///     7, 5, 0x81, 3, 8, 0, 10, // endpoint 1 IN, interrupt, 8 bytes, every 10 frames
/// ];
/// let keyboard = find_boot_keyboard(&config).unwrap();
/// assert_eq!(keyboard.configuration, 1);
/// assert_eq!(keyboard.interface, 0);
/// assert_eq!(keyboard.endpoint, 0x81);
/// assert_eq!(keyboard.max_packet, 8);
/// assert_eq!(keyboard.interval, 10);
/// ```
pub fn find_boot_keyboard(config: &[u8]) -> Result<BootKeyboard, Refusal> {
    let header = ConfigurationHeader::parse(config)?;
    let total = usize::from(header.total_length);
    let end = config.len().min(total);

    // The interface currently being read, when it is a boot keyboard: its number. `None` while
    // the walk is inside some other interface.
    let mut candidate: Option<u8> = None;
    // The last boot keyboard interface that ended with no interrupt IN, for the refusal.
    let mut without_endpoint: Option<u8> = None;
    // An endpoint refused for its packet size, for the refusal, if nothing better turns up.
    let mut too_small: Option<u16> = None;

    let mut at = config[0] as usize;
    while at + 2 <= end {
        let len = config[at] as usize;
        if len < 2 || at + len > total {
            return Err(Refusal::Malformed {
                at: u16::try_from(at).unwrap_or(u16::MAX),
            });
        }
        if at + len > config.len() {
            // Well formed by the device's own total, and cut off by the buffer it was read into.
            break;
        }
        let d = &config[at..at + len];
        match d[1] {
            INTERFACE if len >= INTERFACE_LEN => {
                if let Some(interface) = candidate {
                    without_endpoint = Some(interface);
                }
                let is_boot_keyboard = d[3] == 0 // alternate setting 0, the one SET_CONFIGURATION selects
                    && d[5] == CLASS_HID
                    && d[6] == SUBCLASS_BOOT
                    && d[7] == PROTOCOL_KEYBOARD;
                candidate = is_boot_keyboard.then_some(d[2]);
            }
            ENDPOINT if len >= ENDPOINT_LEN => {
                if let Some(interface) = candidate {
                    let address = d[2];
                    let interrupt = d[3] & 0x3 == 0x3;
                    let inbound = address & 0x80 != 0;
                    let number = address & 0x0f;
                    let max_packet = u16::from_le_bytes([d[4], d[5]]) & 0x07ff;
                    if interrupt && inbound && number != 0 {
                        if max_packet < crate::boot_keyboard::REPORT_LEN as u16 {
                            too_small = Some(max_packet);
                        } else {
                            return Ok(BootKeyboard {
                                configuration: header.value,
                                interface,
                                endpoint: address,
                                max_packet,
                                interval: d[6],
                            });
                        }
                    }
                }
            }
            // The HID class descriptor, string and vendor descriptors, interface associations,
            // `SuperSpeed` companions: none of them changes which endpoint the reports arrive on.
            _ => {}
        }
        at += len;
    }
    if let Some(max_packet) = too_small {
        return Err(Refusal::PacketTooSmall { max_packet });
    }
    match candidate.or(without_endpoint) {
        Some(interface) => Err(Refusal::NoInterruptIn { interface }),
        None => Err(Refusal::NoBootKeyboard),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QEMU's `usb-kbd` configuration, byte for byte as `hw/usb/dev-hid.c` builds it, which is the
    /// device the end-to-end gate types on.
    const QEMU_USB_KBD: [u8; 34] = [
        9, 2, 34, 0, 1, 1, 6, 0xa0, 50, // configuration
        9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0, HID boot keyboard
        9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0, // HID descriptor
        7, 5, 0x81, 3, 8, 0, 7, // endpoint 1 IN, interrupt, 8 bytes, bInterval 7
    ];

    #[test]
    fn qemus_keyboard_is_found() {
        assert_eq!(
            find_boot_keyboard(&QEMU_USB_KBD),
            Ok(BootKeyboard {
                configuration: 1,
                interface: 0,
                endpoint: 0x81,
                max_packet: 8,
                interval: 7,
            })
        );
    }

    /// **A composite keyboard**, the common real shape: interface 0 is the boot keyboard, interface
    /// 1 is a media-key HID interface with no boot subclass, each with its own endpoint. The walk
    /// must take interface 0's endpoint and not the last one it saw.
    #[test]
    fn a_composite_keyboard_answers_its_boot_interface() {
        let config = [
            9, 2, 59, 0, 2, 1, 0, 0xa0, 50, // configuration, two interfaces
            9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0: boot keyboard
            9, 0x21, 0x11, 1, 0, 1, 0x22, 65, 0, //
            7, 5, 0x81, 3, 8, 0, 10, // endpoint 1 IN
            9, 4, 1, 0, 1, 3, 0, 0, 0, // interface 1: HID, no boot subclass
            9, 0x21, 0x11, 1, 0, 1, 0x22, 50, 0, //
            7, 5, 0x82, 3, 16, 0, 10, // endpoint 2 IN
        ];
        let k = find_boot_keyboard(&config).unwrap();
        assert_eq!((k.interface, k.endpoint), (0, 0x81));
    }

    /// The boot interface second, behind a mouse: the walk keeps looking.
    #[test]
    fn a_keyboard_behind_another_interface_is_found() {
        let config = [
            9, 2, 41, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 1, 3, 1, 2, 0, // interface 0: boot MOUSE (protocol 2)
            7, 5, 0x81, 3, 4, 0, 10, //
            9, 4, 1, 0, 1, 3, 1, 1, 0, // interface 1: boot keyboard
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&config).unwrap();
        assert_eq!((k.interface, k.endpoint), (1, 0x82));
    }

    /// A mass-storage stick is not a keyboard, and says so rather than failing somewhere later.
    #[test]
    fn a_storage_device_is_refused_as_not_a_keyboard() {
        let config = [
            9, 2, 32, 0, 1, 1, 0, 0x80, 50, //
            9, 4, 0, 0, 2, 8, 6, 0x50, 0, // mass storage, SCSI, bulk-only
            7, 5, 0x81, 2, 0, 2, 0, // bulk IN
            7, 5, 0x02, 2, 0, 2, 0, // bulk OUT
        ];
        assert_eq!(find_boot_keyboard(&config), Err(Refusal::NoBootKeyboard));
    }

    /// An OUT endpoint, or a bulk one, under a keyboard interface is not where reports arrive.
    #[test]
    fn only_an_interrupt_in_endpoint_counts() {
        let config = [
            9, 2, 32, 0, 1, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 2, 3, 1, 1, 0, //
            7, 5, 0x01, 3, 8, 0, 10, // interrupt OUT (the LED endpoint some keyboards have)
            7, 5, 0x82, 2, 8, 0, 10, // bulk IN
        ];
        assert_eq!(
            find_boot_keyboard(&config),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
    }

    /// Endpoint number 0 is the control endpoint, which no endpoint descriptor may name: an
    /// interrupt IN descriptor claiming it is not where reports arrive.
    #[test]
    fn an_endpoint_numbered_zero_is_not_the_report_endpoint() {
        let mut config = QEMU_USB_KBD;
        config[29] = 0x80; // bEndpointAddress: IN, number 0
        assert_eq!(
            find_boot_keyboard(&config),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
    }

    /// **A zero length is the infinite loop**, and a length past the end is the overrun. Both stop
    /// the walk and name the offset.
    #[test]
    fn a_zero_or_overlong_length_is_malformed_and_never_loops() {
        let mut config = QEMU_USB_KBD;
        config[9] = 0; // the interface descriptor's bLength
        assert_eq!(
            find_boot_keyboard(&config),
            Err(Refusal::Malformed { at: 9 })
        );
        let mut config = QEMU_USB_KBD;
        config[27] = 200; // the endpoint descriptor claims to run far past the end
        assert_eq!(
            find_boot_keyboard(&config),
            Err(Refusal::Malformed { at: 27 })
        );
    }

    /// A defect after the keyboard was found does not lose the keyboard.
    #[test]
    fn a_trailing_defect_after_the_keyboard_is_tolerated() {
        let mut config = [0u8; 36];
        config[..34].copy_from_slice(&QEMU_USB_KBD);
        config[2] = 36; // wTotalLength covers the junk
        config[34] = 0; // a zero-length descriptor
        assert!(find_boot_keyboard(&config).is_ok());
    }

    /// `wTotalLength` larger than what was read is a truncated read, which is a shorter walk.
    #[test]
    fn a_truncated_read_is_a_shorter_walk_not_an_overrun() {
        let mut config = QEMU_USB_KBD;
        config[2] = 0xff;
        config[3] = 0xff;
        assert!(find_boot_keyboard(&config).is_ok());
        assert_eq!(
            find_boot_keyboard(&config[..20]),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
        assert_eq!(find_boot_keyboard(&config[..4]), Err(Refusal::Truncated));
    }

    #[test]
    fn a_device_descriptor_parses_and_a_short_one_does_not() {
        // QEMU usb-kbd's device descriptor.
        let d = [
            18, 1, 0x00, 0x02, 0, 0, 0, 8, 0x27, 0x06, 0x01, 0x00, 0, 0, 1, 4, 11, 1,
        ];
        let dev = DeviceDescriptor::parse(&d).unwrap();
        assert_eq!(dev.usb, 0x0200);
        assert_eq!(dev.max_packet0, 8);
        assert_eq!((dev.vendor, dev.product), (0x0627, 0x0001));
        assert_eq!(dev.configurations, 1);
        assert_eq!(DeviceDescriptor::parse(&d[..8]), None);
        let mut wrong = d;
        wrong[1] = CONFIGURATION;
        assert_eq!(DeviceDescriptor::parse(&wrong), None);
    }

    /// A device descriptor whose `bLength` is shorter than the layout is refused, however many
    /// bytes the buffer holds; a longer one (a future revision's) is read.
    #[test]
    fn a_device_descriptor_reads_its_own_length() {
        let mut d = [
            18, 1, 0x00, 0x02, 0, 0, 0, 8, 0x27, 0x06, 0x01, 0x00, 0, 0, 1, 4, 11, 1,
        ];
        d[0] = 17;
        assert_eq!(DeviceDescriptor::parse(&d), None);
        d[0] = 20;
        assert!(DeviceDescriptor::parse(&d).is_some());
    }

    /// Exactly the nine header bytes parse; a short `bLength` or a different type is not this
    /// descriptor.
    #[test]
    fn a_configuration_header_is_nine_bytes_of_type_two() {
        let header = ConfigurationHeader::parse(&QEMU_USB_KBD[..9]).unwrap();
        assert_eq!((header.total_length, header.value), (34, 1));
        let mut short = QEMU_USB_KBD;
        short[0] = 8;
        assert_eq!(
            ConfigurationHeader::parse(&short),
            Err(Refusal::NotThatDescriptor)
        );
        let mut long = QEMU_USB_KBD;
        long[0] = 10;
        assert!(ConfigurationHeader::parse(&long).is_ok());
        let mut other = QEMU_USB_KBD;
        other[1] = INTERFACE;
        assert_eq!(
            ConfigurationHeader::parse(&other),
            Err(Refusal::NotThatDescriptor)
        );
    }

    /// A boot mouse whose endpoint is big enough for a keyboard report is still a mouse: every
    /// one of class, subclass and protocol has to match.
    #[test]
    fn a_boot_mouse_with_a_wide_endpoint_is_passed_over() {
        let config = [
            9, 2, 41, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 1, 3, 1, 2, 0, // interface 0: boot mouse
            7, 5, 0x81, 3, 8, 0, 10, // 8 bytes, as wide as a keyboard report
            9, 4, 1, 0, 1, 3, 1, 1, 0, // interface 1: boot keyboard
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&config).unwrap();
        assert_eq!((k.interface, k.endpoint), (1, 0x82));
    }

    /// Descriptors too short to be the interface or endpoint their type names are skipped, not
    /// read past their end, and a two-byte descriptor is well formed.
    #[test]
    fn a_short_descriptor_is_skipped_and_two_bytes_is_enough() {
        let config = [
            9, 2, 35, 0, 1, 1, 0, 0xa0, 50, //
            2, 0xff, // a vendor descriptor of length and type alone
            4, 4, 0, 0, // an "interface" too short to be one
            9, 4, 0, 0, 1, 3, 1, 1, 0, // the boot keyboard
            4, 5, 0x81, 3, // an "endpoint" too short to be one
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&config).unwrap();
        assert_eq!((k.interface, k.endpoint), (0, 0x82));
    }

    #[test]
    fn the_control_packet_size_is_checked_against_the_speed() {
        assert_eq!(control_max_packet(8, false), Some(8));
        assert_eq!(control_max_packet(64, false), Some(64));
        assert_eq!(control_max_packet(9, true), Some(512));
        assert_eq!(control_max_packet(9, false), None);
        assert_eq!(control_max_packet(0, false), None);
        assert_eq!(control_max_packet(64, true), None);
    }

    /// A keyboard whose endpoint cannot carry one report in a packet is refused by name.
    #[test]
    fn an_endpoint_smaller_than_a_report_is_refused() {
        let mut config = QEMU_USB_KBD;
        config[31] = 4; // wMaxPacketSize
        assert_eq!(
            find_boot_keyboard(&config),
            Err(Refusal::PacketTooSmall { max_packet: 4 })
        );
    }
}
