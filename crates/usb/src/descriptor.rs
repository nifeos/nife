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

/// How many bytes a device descriptor holds (USB 2.0 Table 9-8).
///
/// Name: provisional.
pub const DEVICE_DESCRIPTOR_BYTES: usize = 18;
/// How many bytes a configuration descriptor's own header holds, before its interfaces (Table
/// 9-10). Not the configuration's whole length, which is its `wTotalLength`.
///
/// Name: provisional.
pub const CONFIGURATION_HEADER_BYTES: usize = 9;
/// How many bytes an interface descriptor holds (Table 9-12).
///
/// Name: provisional.
pub const INTERFACE_DESCRIPTOR_BYTES: usize = 9;
/// How many bytes an endpoint descriptor holds (Table 9-13). USB 3 appends a companion descriptor rather
/// than lengthening this one, so 7 holds at every speed.
///
/// Name: provisional.
pub const ENDPOINT_DESCRIPTOR_BYTES: usize = 7;

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
        maximum_packet_bytes: u16,
    },
}

/// **The device descriptor**, the fields a driver uses from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// `bcdUSB`: the USB revision the device claims, in binary-coded decimal (0x0200 for 2.0).
    #[doc(alias = "bcdUSB")]
    pub usb_release_binary_coded_decimal: u16,
    /// `bDeviceClass`. Zero means "each interface says for itself", which is what keyboards send.
    pub class: u8,
    /// (`control_packet_size_code`, `bMaxPacketSize0` in USB 2.0 Table 9-8): the control endpoint's
    /// packet size in bytes below `SuperSpeed`, and the exponent of it at `SuperSpeed` and above, so
    /// a code to interpret and carrying no unit. [`control_maximum_packet_bytes`] interprets it.
    #[doc(alias = "bMaxPacketSize0")]
    pub control_packet_size_code: u8,
    /// `idVendor`.
    pub vendor: u16,
    /// `idProduct`.
    pub product: u16,
    /// `bNumConfigurations`.
    pub configurations: u8,
}

impl DeviceDescriptor {
    /// Parse a device descriptor. [`Refusal::Truncated`] for fewer than [`DEVICE_DESCRIPTOR_BYTES`]
    /// bytes, and [`Refusal::NotThatDescriptor`] for the wrong type or a `bLength` shorter than
    /// the layout.
    pub fn parse(bytes: &[u8]) -> Result<DeviceDescriptor, Refusal> {
        if bytes.len() < DEVICE_DESCRIPTOR_BYTES {
            return Err(Refusal::Truncated);
        }
        if (bytes[0] as usize) < DEVICE_DESCRIPTOR_BYTES || bytes[1] != DEVICE {
            return Err(Refusal::NotThatDescriptor);
        }
        Ok(DeviceDescriptor {
            usb_release_binary_coded_decimal: u16::from_le_bytes([bytes[2], bytes[3]]),
            class: bytes[4],
            control_packet_size_code: bytes[7],
            vendor: u16::from_le_bytes([bytes[8], bytes[9]]),
            product: u16::from_le_bytes([bytes[10], bytes[11]]),
            configurations: bytes[17],
        })
    }
}

/// **The speed a device runs at**, in the USB specification's terms. It is USB vocabulary and not
/// a host controller's numbering: a host controller driver maps its own port speed onto it.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// Low speed, 1.5 Mb/s (USB 2.0).
    Low,
    /// Full speed, 12 Mb/s (USB 2.0).
    Full,
    /// High speed, 480 Mb/s (USB 2.0).
    High,
    /// `SuperSpeed` or faster (USB 3.2), every speed at which `bMaxPacketSize0` is an exponent.
    Super,
}

/// **The control endpoint's packet size in bytes**, from `control_packet_size_code`, the
/// `bMaxPacketSize0` the device sent in the first eight bytes of its device descriptor. `None` for
/// a code the specification does not allow at `speed`.
///
/// Below `SuperSpeed` the code is the size (USB 2.0 section 9.6.1), and section 5.5.3 fixes which
/// sizes each speed allows: 8 at low speed, 8, 16, 32 or 64 at full speed, and 64 at high speed.
/// At `SuperSpeed` it is an exponent and must be 9, for 512 bytes (USB 3.2 section 9.6.1).
/// Refusing anything else matters because the host controller is told this number and sizes every
/// control transfer by it. Linux's hub driver also refuses a low-speed device that does not declare
/// 8, and accepts 8 to 64 at high speed with a warning (`drivers/usb/core/hub.c` at
/// `torvalds/linux@23c62304dde29abe92013d85910d03a77dada35d`); this function refuses that, as the
/// specification does.
///
/// Name: provisional.
pub fn control_maximum_packet_bytes(control_packet_size_code: u8, speed: Speed) -> Option<u16> {
    match (speed, control_packet_size_code) {
        (Speed::Low, 8) | (Speed::Full, 8 | 16 | 32 | 64) | (Speed::High, 64) => {
            Some(u16::from(control_packet_size_code))
        }
        (Speed::Super, 9) => Some(512),
        _ => None,
    }
}

/// **The configuration descriptor's header**: how long the whole configuration is, and the value
/// `SET_CONFIGURATION` selects it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationHeader {
    /// (`total_length`, `wTotalLength` in USB 2.0 Table 9-10): the bytes of the header plus every
    /// descriptor beneath it.
    #[doc(alias = "wTotalLength")]
    pub total_length: u16,
    /// `bNumInterfaces`.
    pub interfaces: u8,
    /// `bConfigurationValue`, the argument to `SET_CONFIGURATION`.
    pub value: u8,
}

impl ConfigurationHeader {
    /// Parse the first [`CONFIGURATION_HEADER_BYTES`] bytes of a configuration descriptor.
    pub fn parse(bytes: &[u8]) -> Result<ConfigurationHeader, Refusal> {
        if bytes.len() < CONFIGURATION_HEADER_BYTES {
            return Err(Refusal::Truncated);
        }
        if (bytes[0] as usize) < CONFIGURATION_HEADER_BYTES || bytes[1] != CONFIGURATION {
            return Err(Refusal::NotThatDescriptor);
        }
        Ok(ConfigurationHeader {
            total_length: u16::from_le_bytes([bytes[2], bytes[3]]),
            interfaces: bytes[4],
            value: bytes[5],
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
    /// The interrupt IN endpoint's number and direction (`0x81` for endpoint 1 IN). The
    /// specification's reserved bits 6:4 of `bEndpointAddress` are cleared.
    pub endpoint: u8,
    /// The endpoint's packet size in bytes (`wMaxPacketSize` bits 10:0); at least 8, or the walk
    /// refused it.
    pub maximum_packet_bytes: u16,
    /// `bInterval` as the device sent it. What it means depends on the speed, which is the host
    /// controller driver's business.
    pub interval: u8,
}

/// **Find the boot keyboard in a configuration**: the first interface (alternate setting 0) of
/// class 3, subclass 1, protocol 1, and the first interrupt IN endpoint beneath it.
///
/// The walk begins after the configuration header's own `bLength`, so a header longer than nine
/// bytes skips what the device says it carries.
///
/// `configuration` is the configuration as the device sent it, possibly cut short by the buffer it was
/// read into: the walk stops at the lesser of `wTotalLength` and `configuration.len()`, so a truncated
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
/// let configuration = [
///     9, 2, 34, 0, 1, 1, 0, 0xa0, 50, // configuration 1, 34 bytes, one interface
///     9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0: HID, boot, keyboard, one endpoint
///     9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0, // the HID descriptor, which the walk skips
///     7, 5, 0x81, 3, 8, 0, 10, // endpoint 1 IN, interrupt, 8 bytes, every 10 frames
/// ];
/// let keyboard = find_boot_keyboard(&configuration).unwrap();
/// assert_eq!(keyboard.configuration, 1);
/// assert_eq!(keyboard.interface, 0);
/// assert_eq!(keyboard.endpoint, 0x81);
/// assert_eq!(keyboard.maximum_packet_bytes, 8);
/// assert_eq!(keyboard.interval, 10);
/// ```
pub fn find_boot_keyboard(configuration: &[u8]) -> Result<BootKeyboard, Refusal> {
    let header = ConfigurationHeader::parse(configuration)?;
    let total = usize::from(header.total_length);
    let end = configuration.len().min(total);

    // The interface currently being read, when it is a boot keyboard: its number. `None` while
    // the walk is inside some other interface.
    let mut candidate: Option<u8> = None;
    // The last boot keyboard interface that ended with no interrupt IN, for the refusal.
    let mut without_endpoint: Option<u8> = None;
    // An endpoint refused for its packet size, for the refusal, if nothing better turns up.
    let mut too_small: Option<u16> = None;

    let mut at = configuration[0] as usize;
    while at + 2 <= end {
        let len = configuration[at] as usize;
        if len < 2 || at + len > total {
            return Err(Refusal::Malformed {
                at: u16::try_from(at).unwrap_or(u16::MAX),
            });
        }
        if at + len > configuration.len() {
            // Well formed by the device's own total, and cut off by the buffer it was read into.
            break;
        }
        let d = &configuration[at..at + len];
        match d[1] {
            // A descriptor of a type this walk reads, too short for that type's layout, is
            // refused: skipping it would leave the previous interface open, and the endpoint
            // that follows would be read as that interface's.
            INTERFACE if len < INTERFACE_DESCRIPTOR_BYTES => {
                return Err(Refusal::Malformed {
                    at: u16::try_from(at).unwrap_or(u16::MAX),
                });
            }
            ENDPOINT if len < ENDPOINT_DESCRIPTOR_BYTES => {
                return Err(Refusal::Malformed {
                    at: u16::try_from(at).unwrap_or(u16::MAX),
                });
            }
            INTERFACE => {
                if let Some(interface) = candidate {
                    without_endpoint = Some(interface);
                }
                let is_boot_keyboard = d[3] == 0 // alternate setting 0, the one SET_CONFIGURATION selects
                    && d[5] == CLASS_HID
                    && d[6] == SUBCLASS_BOOT
                    && d[7] == PROTOCOL_KEYBOARD;
                candidate = is_boot_keyboard.then_some(d[2]);
            }
            ENDPOINT => {
                if let Some(interface) = candidate {
                    let endpoint_number_and_direction = d[2] & 0x8f;
                    let interrupt = d[3] & 0x3 == 0x3;
                    let inbound = endpoint_number_and_direction & 0x80 != 0;
                    let number = endpoint_number_and_direction & 0x0f;
                    let maximum_packet_bytes = u16::from_le_bytes([d[4], d[5]]) & 0x07ff;
                    if interrupt && inbound && number != 0 {
                        if maximum_packet_bytes < crate::boot_keyboard::REPORT_BYTES as u16 {
                            too_small = Some(maximum_packet_bytes);
                        } else {
                            return Ok(BootKeyboard {
                                configuration: header.value,
                                interface,
                                endpoint: endpoint_number_and_direction,
                                maximum_packet_bytes,
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
    if let Some(maximum_packet_bytes) = too_small {
        return Err(Refusal::PacketTooSmall {
            maximum_packet_bytes,
        });
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
    const QEMU_USB_KEYBOARD: [u8; 34] = [
        9, 2, 34, 0, 1, 1, 6, 0xa0, 50, // configuration
        9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0, HID boot keyboard
        9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0, // HID descriptor
        7, 5, 0x81, 3, 8, 0, 7, // endpoint 1 IN, interrupt, 8 bytes, bInterval 7
    ];

    #[test]
    fn qemus_keyboard_is_found() {
        assert_eq!(
            find_boot_keyboard(&QEMU_USB_KEYBOARD),
            Ok(BootKeyboard {
                configuration: 1,
                interface: 0,
                endpoint: 0x81,
                maximum_packet_bytes: 8,
                interval: 7,
            })
        );
    }

    /// **A composite keyboard**, the common real shape: interface 0 is the boot keyboard, interface
    /// 1 is a media-key HID interface with no boot subclass, each with its own endpoint. The walk
    /// must take interface 0's endpoint and not the last one it saw.
    #[test]
    fn a_composite_keyboard_answers_its_boot_interface() {
        let configuration = [
            9, 2, 59, 0, 2, 1, 0, 0xa0, 50, // configuration, two interfaces
            9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0: boot keyboard
            9, 0x21, 0x11, 1, 0, 1, 0x22, 65, 0, //
            7, 5, 0x81, 3, 8, 0, 10, // endpoint 1 IN
            9, 4, 1, 0, 1, 3, 0, 0, 0, // interface 1: HID, no boot subclass
            9, 0x21, 0x11, 1, 0, 1, 0x22, 50, 0, //
            7, 5, 0x82, 3, 16, 0, 10, // endpoint 2 IN
        ];
        let k = find_boot_keyboard(&configuration).unwrap();
        assert_eq!((k.interface, k.endpoint), (0, 0x81));
    }

    /// The boot interface second, behind a mouse: the walk keeps looking.
    #[test]
    fn a_keyboard_behind_another_interface_is_found() {
        let configuration = [
            9, 2, 41, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 1, 3, 1, 2, 0, // interface 0: boot MOUSE (protocol 2)
            7, 5, 0x81, 3, 4, 0, 10, //
            9, 4, 1, 0, 1, 3, 1, 1, 0, // interface 1: boot keyboard
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&configuration).unwrap();
        assert_eq!((k.interface, k.endpoint), (1, 0x82));
    }

    /// A mass-storage stick is not a keyboard, and says so rather than failing somewhere later.
    #[test]
    fn a_storage_device_is_refused_as_not_a_keyboard() {
        let configuration = [
            9, 2, 32, 0, 1, 1, 0, 0x80, 50, //
            9, 4, 0, 0, 2, 8, 6, 0x50, 0, // mass storage, SCSI, bulk-only
            7, 5, 0x81, 2, 0, 2, 0, // bulk IN
            7, 5, 0x02, 2, 0, 2, 0, // bulk OUT
        ];
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::NoBootKeyboard)
        );
    }

    /// An OUT endpoint, or a bulk one, under a keyboard interface is not where reports arrive.
    #[test]
    fn only_an_interrupt_in_endpoint_counts() {
        let configuration = [
            9, 2, 32, 0, 1, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 2, 3, 1, 1, 0, //
            7, 5, 0x01, 3, 8, 0, 10, // interrupt OUT (the LED endpoint some keyboards have)
            7, 5, 0x82, 2, 8, 0, 10, // bulk IN
        ];
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
    }

    /// Endpoint number 0 is the control endpoint, which no endpoint descriptor may name: an
    /// interrupt IN descriptor claiming it is not where reports arrive.
    #[test]
    fn an_endpoint_numbered_zero_is_not_the_report_endpoint() {
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[29] = 0x80; // bEndpointAddress: IN, number 0
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
    }

    /// **A zero length is the infinite loop**, and a length past the end is the overrun. Both stop
    /// the walk and name the offset.
    #[test]
    fn a_zero_or_overlong_length_is_malformed_and_never_loops() {
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[9] = 0; // the interface descriptor's bLength
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::Malformed { at: 9 })
        );
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[27] = 200; // the endpoint descriptor claims to run far past the end
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::Malformed { at: 27 })
        );
    }

    /// A defect after the keyboard was found does not lose the keyboard.
    #[test]
    fn a_trailing_defect_after_the_keyboard_is_tolerated() {
        let mut configuration = [0u8; 36];
        configuration[..34].copy_from_slice(&QEMU_USB_KEYBOARD);
        configuration[2] = 36; // wTotalLength covers the junk
        configuration[34] = 0; // a zero-length descriptor
        assert!(find_boot_keyboard(&configuration).is_ok());
    }

    /// `wTotalLength` larger than what was read is a truncated read, which is a shorter walk.
    #[test]
    fn a_truncated_read_is_a_shorter_walk_not_an_overrun() {
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[2] = 0xff;
        configuration[3] = 0xff;
        assert!(find_boot_keyboard(&configuration).is_ok());
        assert_eq!(
            find_boot_keyboard(&configuration[..20]),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
        assert_eq!(
            find_boot_keyboard(&configuration[..4]),
            Err(Refusal::Truncated)
        );
    }

    #[test]
    fn a_device_descriptor_parses_and_a_short_one_does_not() {
        // QEMU usb-kbd's device descriptor.
        let d = [
            18, 1, 0x00, 0x02, 0, 0, 0, 8, 0x27, 0x06, 0x01, 0x00, 0, 0, 1, 4, 11, 1,
        ];
        let device = DeviceDescriptor::parse(&d).unwrap();
        assert_eq!(device.usb_release_binary_coded_decimal, 0x0200);
        assert_eq!(device.control_packet_size_code, 8);
        assert_eq!((device.vendor, device.product), (0x0627, 0x0001));
        assert_eq!(device.configurations, 1);
        assert_eq!(DeviceDescriptor::parse(&d[..8]), Err(Refusal::Truncated));
        let mut wrong = d;
        wrong[1] = CONFIGURATION;
        assert_eq!(
            DeviceDescriptor::parse(&wrong),
            Err(Refusal::NotThatDescriptor)
        );
    }

    /// A device descriptor whose `bLength` is shorter than the layout is refused, however many
    /// bytes the buffer holds; a longer one (a future revision's) is read.
    #[test]
    fn a_device_descriptor_reads_its_own_length() {
        let mut d = [
            18, 1, 0x00, 0x02, 0, 0, 0, 8, 0x27, 0x06, 0x01, 0x00, 0, 0, 1, 4, 11, 1,
        ];
        d[0] = 17;
        assert_eq!(
            DeviceDescriptor::parse(&d),
            Err(Refusal::NotThatDescriptor)
        );
        d[0] = 20;
        assert!(DeviceDescriptor::parse(&d).is_ok());
    }

    /// Exactly the nine header bytes parse; a short `bLength` or a different type is not this
    /// descriptor.
    #[test]
    fn a_configuration_header_is_nine_bytes_of_type_two() {
        let header = ConfigurationHeader::parse(&QEMU_USB_KEYBOARD[..9]).unwrap();
        assert_eq!((header.total_length, header.value), (34, 1));
        let mut short = QEMU_USB_KEYBOARD;
        short[0] = 8;
        assert_eq!(
            ConfigurationHeader::parse(&short),
            Err(Refusal::NotThatDescriptor)
        );
        let mut long = QEMU_USB_KEYBOARD;
        long[0] = 10;
        assert!(ConfigurationHeader::parse(&long).is_ok());
        let mut other = QEMU_USB_KEYBOARD;
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
        let configuration = [
            9, 2, 41, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 1, 3, 1, 2, 0, // interface 0: boot mouse
            7, 5, 0x81, 3, 8, 0, 10, // 8 bytes, as wide as a keyboard report
            9, 4, 1, 0, 1, 3, 1, 1, 0, // interface 1: boot keyboard
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&configuration).unwrap();
        assert_eq!((k.interface, k.endpoint), (1, 0x82));
    }

    /// A descriptor of type interface or endpoint too short for its layout is refused with its
    /// offset: skipping it would leave the previous interface open and hand the next interface's
    /// endpoint to it. A two-byte vendor descriptor is well formed and skipped.
    #[test]
    fn a_short_interface_or_endpoint_is_malformed_and_two_bytes_is_enough() {
        let configuration = [
            9, 2, 35, 0, 1, 1, 0, 0xa0, 50, //
            2, 0xff, // a vendor descriptor of length and type alone
            9, 4, 0, 0, 1, 3, 1, 1, 0, // the boot keyboard
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        let k = find_boot_keyboard(&configuration).unwrap();
        assert_eq!((k.interface, k.endpoint), (0, 0x82));

        let short_interface = [
            9, 2, 39, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 0, 3, 1, 1, 0, // boot keyboard with no endpoint
            4, 4, 1, 0, // an interface cut short
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        assert_eq!(
            find_boot_keyboard(&short_interface),
            Err(Refusal::Malformed { at: 18 })
        );
        let short_endpoint = [
            9, 2, 29, 0, 1, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 1, 3, 1, 1, 0, //
            4, 5, 0x81, 3, // an endpoint cut short
        ];
        assert_eq!(
            find_boot_keyboard(&short_endpoint),
            Err(Refusal::Malformed { at: 18 })
        );
    }

    /// A boot interface with no endpoint, then a non-boot HID interface with one: the walk
    /// refuses with the boot interface's number, and does not take the other's endpoint.
    #[test]
    fn a_boot_interface_without_an_endpoint_is_refused_by_number() {
        let configuration = [
            9, 2, 41, 0, 2, 1, 0, 0xa0, 50, //
            9, 4, 0, 0, 0, 3, 1, 1, 0, // interface 0: boot keyboard, zero endpoints
            9, 4, 1, 0, 1, 3, 0, 0, 0, // interface 1: HID, not boot
            7, 5, 0x82, 3, 8, 0, 10, //
        ];
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::NoInterruptIn { interface: 0 })
        );
    }

    /// The reserved bits of `bEndpointAddress` do not reach the caller.
    #[test]
    fn the_endpoint_carries_only_its_number_and_direction() {
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[29] = 0xf1;
        assert_eq!(find_boot_keyboard(&configuration).unwrap().endpoint, 0x81);
    }

    /// Each speed allows only its own sizes: a low-speed keyboard that declares 64, or a
    /// high-speed one that declares 8, is refused.
    #[test]
    fn the_control_packet_size_is_checked_against_the_speed() {
        assert_eq!(control_maximum_packet_bytes(8, Speed::Low), Some(8));
        assert_eq!(control_maximum_packet_bytes(64, Speed::Low), None);
        for size in [8, 16, 32, 64] {
            assert_eq!(
                control_maximum_packet_bytes(size, Speed::Full),
                Some(u16::from(size))
            );
        }
        assert_eq!(control_maximum_packet_bytes(64, Speed::High), Some(64));
        assert_eq!(control_maximum_packet_bytes(8, Speed::High), None);
        assert_eq!(control_maximum_packet_bytes(9, Speed::Super), Some(512));
        assert_eq!(control_maximum_packet_bytes(9, Speed::Full), None);
        assert_eq!(control_maximum_packet_bytes(0, Speed::Full), None);
        assert_eq!(control_maximum_packet_bytes(64, Speed::Super), None);
    }

    /// A keyboard whose endpoint cannot carry one report in a packet is refused by name.
    #[test]
    fn an_endpoint_smaller_than_a_report_is_refused() {
        let mut configuration = QEMU_USB_KEYBOARD;
        configuration[31] = 4; // wMaxPacketSize
        assert_eq!(
            find_boot_keyboard(&configuration),
            Err(Refusal::PacketTooSmall {
                maximum_packet_bytes: 4
            })
        );
    }
}
