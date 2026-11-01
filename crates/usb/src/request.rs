//! **Setup packets: what a host asks a device** (USB 2.0 section 9.3, section 9.4; HID 1.11 section 7.2).
//!
//! Every control transfer opens with eight bytes saying who the request is for, what it is, and
//! how much data follows in which direction. A host controller carries them as the immediate data
//! of a setup-stage TRB, which is why [`SetupPacket::to_u64`] exists: the packet is one little-
//! endian quadword there.

/// `bmRequestType` bit 7: data flows device to host.
pub const DEVICE_TO_HOST: u8 = 0x80;
/// `bmRequestType` bits 6:5 = 1: a class request (here, HID's).
pub const CLASS: u8 = 0x20;
/// `bmRequestType` bits 4:0 = 1: addressed to an interface.
pub const TO_INTERFACE: u8 = 0x01;

/// `GET_DESCRIPTOR` (USB 2.0 Table 9-4).
pub const GET_DESCRIPTOR: u8 = 6;
/// `SET_CONFIGURATION` (USB 2.0 Table 9-4).
pub const SET_CONFIGURATION: u8 = 9;
/// HID `SET_IDLE` (HID 1.11 section 7.2.4).
pub const SET_IDLE: u8 = 0x0a;
/// HID `SET_PROTOCOL` (HID 1.11 section 7.2.6).
pub const SET_PROTOCOL: u8 = 0x0b;
/// `SET_PROTOCOL`'s value for the boot protocol (report protocol is 1).
pub const BOOT_PROTOCOL: u16 = 0;

/// **One setup packet.** Field names are the specification's, without the Hungarian prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupPacket {
    /// `bmRequestType`: direction, type and recipient.
    pub request_type: u8,
    /// `bRequest`.
    pub request: u8,
    /// `wValue`.
    pub value: u16,
    /// `wIndex`.
    pub index: u16,
    /// `wLength`: how many bytes the data stage carries, zero for none.
    pub length: u16,
}

impl SetupPacket {
    /// `GET_DESCRIPTOR` of `kind` (a [`crate::descriptor`] type) number `index`, at most `length`
    /// bytes. A device may answer fewer, and the host must accept a short answer.
    pub const fn get_descriptor(kind: u8, index: u8, length: u16) -> SetupPacket {
        SetupPacket {
            request_type: DEVICE_TO_HOST,
            request: GET_DESCRIPTOR,
            value: (kind as u16) << 8 | index as u16,
            index: 0,
            length,
        }
    }

    /// `SET_CONFIGURATION` to the configuration whose `bConfigurationValue` is `value`.
    pub const fn set_configuration(value: u8) -> SetupPacket {
        SetupPacket {
            request_type: 0,
            request: SET_CONFIGURATION,
            value: value as u16,
            index: 0,
            length: 0,
        }
    }

    /// HID `SET_PROTOCOL` to the boot protocol, on `interface`. HID 1.11 section 7.2.6 says a host should
    /// send it rather than assume the protocol, because every device starts in report protocol; some
    /// keyboards stall it and report boot anyway, which is why the driver tolerates a stall here.
    pub const fn set_boot_protocol(interface: u8) -> SetupPacket {
        SetupPacket {
            request_type: CLASS | TO_INTERFACE,
            request: SET_PROTOCOL,
            value: BOOT_PROTOCOL,
            index: interface as u16,
            length: 0,
        }
    }

    /// HID `SET_IDLE` with a duration of zero on `interface`: report only when something changes.
    /// What a boot keyboard defaults to in practice, asked for because a device that repeated
    /// its last report would look like a key held down to nothing at all.
    pub const fn set_idle_forever(interface: u8) -> SetupPacket {
        SetupPacket {
            request_type: CLASS | TO_INTERFACE,
            request: SET_IDLE,
            value: 0,
            index: interface as u16,
            length: 0,
        }
    }

    /// Whether a data stage, if there is one, flows device to host.
    pub const fn is_device_to_host(&self) -> bool {
        self.request_type & DEVICE_TO_HOST != 0
    }

    /// **The eight bytes as one little-endian quadword**, the order they go on the wire and the
    /// layout an xHCI setup-stage TRB carries as immediate data.
    pub const fn to_u64(&self) -> u64 {
        self.request_type as u64
            | (self.request as u64) << 8
            | (self.value as u64) << 16
            | (self.index as u64) << 32
            | (self.length as u64) << 48
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The device descriptor's first eight bytes, the first request any host sends: `80 06 00 01
    /// 00 00 08 00` on the wire.
    #[test]
    fn get_descriptor_is_the_bytes_the_wire_carries() {
        let p = SetupPacket::get_descriptor(crate::descriptor::DEVICE, 0, 8);
        assert_eq!(
            p.to_u64().to_le_bytes(),
            [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 0x08, 0x00]
        );
        assert!(p.is_device_to_host());
    }

    #[test]
    fn the_class_requests_address_the_interface() {
        assert_eq!(
            SetupPacket::set_boot_protocol(2).to_u64().to_le_bytes(),
            [0x21, 0x0b, 0, 0, 2, 0, 0, 0]
        );
        assert_eq!(
            SetupPacket::set_idle_forever(0).to_u64().to_le_bytes(),
            [0x21, 0x0a, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            SetupPacket::set_configuration(1).to_u64().to_le_bytes(),
            [0x00, 0x09, 1, 0, 0, 0, 0, 0]
        );
        assert!(!SetupPacket::set_configuration(1).is_device_to_host());
    }
}
