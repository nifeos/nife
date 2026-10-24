//! **Asking the emulator for a screen**: QEMU's `fw_cfg` interface, wire side, as QEMU's
//! `docs/specs/fw_cfg.rst` specifies it. QEMU's `virt` boards are entered from `-kernel` with no
//! firmware that lit a display, but they have **`ramfb`**, a device whose contract is that the guest
//! supplies the memory and tells the emulator where it is, through `fw_cfg`'s DMA interface (the
//! only one that can write). This crate is that conversation's wire format and nothing else, so the
//! kernel driver is a `write_volatile` and a poll. **Every multi-byte field is big-endian**, even on
//! little-endian machines, so every encoder takes native values and returns bytes, never a struct
//! anybody could `transmute`. See notes/serial-less-output/ramfb.md.

//! # BUGS
//!
//! - Nothing checks that the device is real: the trust boundary is the device tree, as for the UART.
//! - The directory is walked, `O(files)` reads, once per boot: `ramfb`'s key moves as devices are
//!   added.
//! - `ramfb` is QEMU's, not a standard; the boards' answer is milestone 157 (real display output on
//!   the board) and U-Boot's `simple-framebuffer` handoff.
//!
//! Name: provisional. Refused `fw_cfg` (an abbreviation only its author can
//! expand), `qemu_firmware_config` (a vendor in an interface's name, and still abbreviated) and
//! `firmware_config` (still abbreviated).

#![no_std]

use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

/// The key the file directory is published at. Fixed by the interface: every other key is assigned.
pub const DIRECTORY_KEY: u16 = 0x0019;

/// The name of the `ramfb` file, as it appears in the directory.
pub const RAMFB: &[u8] = b"etc/ramfb";

/// One directory entry's length on the wire, including the 56-byte name.
pub const ENTRY_LEN: usize = 64;

/// The `ramfb` configuration's length on the wire.
pub const RAMFB_LEN: usize = 28;

/// A `DmaCommand` on the wire.
pub const COMMAND_LEN: usize = 16;

/// **What the device is being asked to do**, the low bits of a command's control word.
///
/// Named here rather than left as literals at the call site because the device reports back through
/// the same word: it clears the ones it has honoured and sets [`control::ERROR`] if it refused, so a driver
/// polling for completion is reading these same bits in the other direction.
pub mod control {
    /// The device refused. Set by the device, never by the guest.
    pub const ERROR: u32 = 0x01;
    /// Copy from the device into the buffer.
    pub const READ: u32 = 0x02;
    /// Advance the read position without copying.
    pub const SKIP: u32 = 0x04;
    /// The upper sixteen bits name a key to select before doing anything else.
    pub const SELECT: u32 = 0x08;
    /// Copy from the buffer into the device.
    pub const WRITE: u32 = 0x10;
}

/// **One transaction**, as the device reads it out of guest memory.
///
/// The guest writes the *physical address of one of these* to the interface's DMA register; the
/// device then reads the sixteen bytes there, performs the transfer they describe, and writes the
/// [`control`] word back with the requested bits cleared, which is how the guest knows it is done.
///
/// **The address inside is physical**, and there is nothing here that could check that: a kernel
/// that hands over a virtual one has the emulator read sixteen bytes of somebody else's memory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DmaCommand {
    /// The [`control`] bits, with any selected key already folded into the upper half.
    pub control: u32,
    /// How many bytes to move.
    pub length: u32,
    /// The **physical** address of the guest's buffer.
    pub address: u64,
}

impl DmaCommand {
    /// Select `key` and read `length` bytes into the buffer at `address`.
    #[must_use]
    pub const fn read(key: u16, length: u32, address: u64) -> Self {
        Self {
            control: ((key as u32) << 16) | control::SELECT | control::READ,
            length,
            address,
        }
    }

    /// Read `length` more bytes from wherever the last command left the read position.
    ///
    /// The device keeps a position per selected key, which is what makes walking the file directory
    /// a first [`read`](Self::read) for the count and then one of these per entry, rather than one
    /// enormous buffer the caller has to size in advance.
    #[must_use]
    pub const fn read_more(length: u32, address: u64) -> Self {
        Self {
            control: control::READ,
            length,
            address,
        }
    }

    /// Select `key` and write `length` bytes from the buffer at `address` into it.
    #[must_use]
    pub const fn write(key: u16, length: u32, address: u64) -> Self {
        Self {
            control: ((key as u32) << 16) | control::SELECT | control::WRITE,
            length,
            address,
        }
    }

    /// The sixteen bytes the device will read, big-endian throughout.
    #[must_use]
    pub const fn encode(self) -> [u8; COMMAND_LEN] {
        let c = self.control.to_be_bytes();
        let l = self.length.to_be_bytes();
        let a = self.address.to_be_bytes();
        [
            c[0], c[1], c[2], c[3], l[0], l[1], l[2], l[3], a[0], a[1], a[2], a[3], a[4], a[5],
            a[6], a[7],
        ]
    }

    /// Read a control word the device wrote back.
    ///
    /// `Ok(true)` means the transfer finished, `Ok(false)` that it has not yet, and [`Refused`]
    /// that the device set [`control::ERROR`]. Three states rather than two because a driver that
    /// treated the error as "not finished yet" would spin forever on a refusal, which is the one
    /// failure this interface can produce that looks exactly like a slow machine.
    ///
    /// # Errors
    ///
    /// When the device set [`control::ERROR`].
    pub const fn settled(word: u32) -> Result<bool, Refused> {
        if word & control::ERROR != 0 {
            return Err(Refused);
        }
        Ok(word == 0)
    }
}

/// The device set [`control::ERROR`] on a transfer.
///
/// A type of its own rather than `()`, so that a caller matching on the outcome has a name to match
/// against and a reader of the signature is told what went wrong rather than only that something
/// did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Refused;

/// **One file the device publishes**, as it appears in the directory at [`DIRECTORY_KEY`].
///
/// # Examples
///
/// The key to select is found by walking the directory the device publishes at a fixed key:
///
/// ```
/// use firmware_configuration::{DirectoryEntry, ENTRY_LEN, RAMFB};
///
/// let mut entry = [0u8; ENTRY_LEN];
/// entry[..4].copy_from_slice(&28u32.to_be_bytes());
/// entry[4..6].copy_from_slice(&0x0021u16.to_be_bytes());
/// entry[8..8 + RAMFB.len()].copy_from_slice(RAMFB);
///
/// let parsed = DirectoryEntry::parse(&entry).expect("a well-formed entry");
/// assert_eq!(parsed.size, 28);
/// assert_eq!(parsed.select, 0x0021);
/// assert!(parsed.is(RAMFB));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DirectoryEntry {
    /// How many bytes the file holds.
    pub size: u32,
    /// The key to select to reach it. **Not fixed**: QEMU assigns these in the order devices are
    /// attached, so it must be read rather than remembered.
    pub select: u16,
    /// The name, NUL-padded to 56 bytes. Kept raw rather than as a `&str` because a device is free
    /// to publish bytes that are not UTF-8 and refusing the whole directory over one of them would
    /// lose the entry being looked for.
    pub name: [u8; 56],
}

impl DirectoryEntry {
    /// Decode one entry. `None` when `bytes` is not [`ENTRY_LEN`] long.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; ENTRY_LEN] = bytes.try_into().ok()?;
        let mut name = [0u8; 56];
        name.copy_from_slice(&bytes[8..64]);
        Some(Self {
            size: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            select: u16::from_be_bytes([bytes[4], bytes[5]]),
            name,
        })
    }

    /// Whether this entry is the file called `wanted`.
    ///
    /// A whole-name comparison against the NUL-padded field rather than a prefix match, because
    /// `etc/ramfb` and a hypothetical `etc/ramfb2` would both pass a prefix test and writing 28
    /// bytes into the wrong file is not a diagnosable failure.
    #[must_use]
    pub fn is(&self, wanted: &[u8]) -> bool {
        self.name.len() > wanted.len()
            && &self.name[..wanted.len()] == wanted
            && self.name[wanted.len()] == 0
    }
}

/// **A screen the guest owns, described to `ramfb`.**
///
/// A newtype over the boot handoff's own [`Framebuffer`] rather than a second set of five fields,
/// so that the thing the device is told about and the thing `screen_console` paints into cannot
/// drift apart. What it adds is the one thing the handoff does not carry: the DRM format code the
/// device wants instead of a byte order.
///
/// # Examples
///
/// Point a `ramfb` at a framebuffer the guest owns: a 28-byte configuration and a 16-byte command
/// that delivers it.
///
/// ```
/// use firmware_configuration::{DmaCommand, RamFramebuffer};
/// use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
///
/// let screen = Framebuffer {
///     base: 0x4020_0000,
///     width: 800,
///     height: 600,
///     stride: 3200,
///     order: PixelOrder::Bgrx,
/// };
///
/// let config = RamFramebuffer(screen).encode();
/// // The address leads, big-endian, and the format is DRM's `XR24` spelled as four ASCII bytes.
/// assert_eq!(&config[..8], &0x4020_0000u64.to_be_bytes());
/// assert_eq!(&config[8..12], b"\x34\x32\x52\x58");
///
/// // Selecting the file and writing those 28 bytes is one command.
/// let command = DmaCommand::write(0x0021, config.len() as u32, 0x4000_1000).encode();
/// assert_eq!(&command[..4], &0x0021_0018u32.to_be_bytes());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RamFramebuffer(pub Framebuffer);

impl RamFramebuffer {
    /// The DRM `fourcc` for this screen's byte order.
    ///
    /// DRM names a format by the little-endian **word**, so its `XRGB8888` (`XR24`) is the
    /// memory order B, G, R, unused, which is [`PixelOrder::Bgrx`]. Getting this pair the wrong
    /// way round produces a picture with red and blue exchanged and nothing else wrong, which is
    /// why the mapping is stated here with the reasoning attached rather than inlined.
    #[must_use]
    pub const fn fourcc(self) -> u32 {
        match self.0.order {
            // 'X', 'R', '2', '4'
            PixelOrder::Bgrx => 0x3432_5258,
            // 'X', 'B', '2', '4'
            PixelOrder::Rgbx => 0x3432_4258,
        }
    }

    /// The 28 bytes the device reads, big-endian throughout.
    #[must_use]
    pub fn encode(self) -> [u8; RAMFB_LEN] {
        let mut out = [0u8; RAMFB_LEN];
        out[..8].copy_from_slice(&self.0.base.to_be_bytes());
        out[8..12].copy_from_slice(&self.fourcc().to_be_bytes());
        // Flags. No bit is defined; the field exists so that one can be.
        out[12..16].copy_from_slice(&0u32.to_be_bytes());
        out[16..20].copy_from_slice(&self.0.width.to_be_bytes());
        out[20..24].copy_from_slice(&self.0.height.to_be_bytes());
        out[24..28].copy_from_slice(&self.0.stride.to_be_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(order: PixelOrder) -> Framebuffer {
        Framebuffer {
            base: 0x4020_0000,
            width: 800,
            height: 600,
            stride: 3200,
            order,
        }
    }

    /// The whole reason this crate exists: a little-endian host writing a big-endian interface.
    /// A `transmute` of the struct would pass every other test here and fail this one.
    #[test]
    fn every_field_of_a_command_is_big_endian() {
        let bytes = DmaCommand::write(0x0021, 28, 0x4000_1000).encode();
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0x21);
        assert_eq!(bytes[2], 0x00);
        assert_eq!(bytes[3], (control::SELECT | control::WRITE) as u8);
        assert_eq!(&bytes[4..8], &28u32.to_be_bytes());
        assert_eq!(&bytes[8..16], &0x4000_1000u64.to_be_bytes());
    }

    #[test]
    fn a_read_selects_and_a_continuation_does_not() {
        let first = DmaCommand::read(DIRECTORY_KEY, 4, 0x1000);
        assert_eq!(first.control >> 16, u32::from(DIRECTORY_KEY));
        assert_ne!(first.control & control::SELECT, 0);

        let next = DmaCommand::read_more(ENTRY_LEN as u32, 0x1000);
        assert_eq!(next.control, control::READ);
        assert_eq!(next.control >> 16, 0);
    }

    /// The device answers in the control word, and "refused" must not read as "still working".
    #[test]
    fn a_refusal_is_not_a_slow_machine() {
        assert_eq!(DmaCommand::settled(0), Ok(true));
        assert_eq!(DmaCommand::settled(control::READ), Ok(false));
        assert_eq!(DmaCommand::settled(control::ERROR), Err(Refused));
        assert_eq!(
            DmaCommand::settled(control::ERROR | control::READ),
            Err(Refused)
        );
    }

    #[test]
    fn a_directory_entry_round_trips_and_names_itself() {
        let mut raw = [0u8; ENTRY_LEN];
        raw[..4].copy_from_slice(&0x1234u32.to_be_bytes());
        raw[4..6].copy_from_slice(&0x0033u16.to_be_bytes());
        raw[8..8 + RAMFB.len()].copy_from_slice(RAMFB);

        let entry = DirectoryEntry::parse(&raw).expect("64 bytes");
        assert_eq!(entry.size, 0x1234);
        assert_eq!(entry.select, 0x0033);
        assert!(entry.is(RAMFB));
        assert!(!entry.is(b"etc/e820"));
    }

    /// A prefix match would write the `ramfb` configuration into a file that merely starts the
    /// same way, which the device would accept and nothing downstream would notice.
    #[test]
    fn a_longer_name_is_not_the_file_we_want() {
        let mut raw = [0u8; ENTRY_LEN];
        raw[8..8 + RAMFB.len() + 1].copy_from_slice(b"etc/ramfb2");
        let entry = DirectoryEntry::parse(&raw).expect("64 bytes");
        assert!(!entry.is(RAMFB));
    }

    /// A name that fills the whole 56-byte field has no NUL to end it, so it is not a file called
    /// that, and asking must say no rather than index past the field. A name one byte shorter is
    /// still found.
    #[test]
    fn a_name_that_fills_the_field_is_compared_without_reading_past_it() {
        let mut raw = [0u8; ENTRY_LEN];
        raw[8..].copy_from_slice(&[b'a'; 56]);
        let full = DirectoryEntry::parse(&raw).expect("64 bytes");
        assert!(!full.is(&[b'a'; 56]));
        assert!(!full.is(&[b'a'; 55]));

        raw[63] = 0;
        let nearly = DirectoryEntry::parse(&raw).expect("64 bytes");
        assert!(nearly.is(&[b'a'; 55]));
    }

    #[test]
    fn a_short_entry_is_refused_rather_than_padded() {
        assert_eq!(DirectoryEntry::parse(&[0u8; 63]), None);
        assert_eq!(DirectoryEntry::parse(&[0u8; 65]), None);
    }

    /// DRM names a format by the little-endian word and this tree names it by the memory order, so
    /// the two spellings are mirror images and swapping them is invisible except in the picture.
    #[test]
    fn the_two_byte_orders_get_the_two_drm_codes() {
        assert_eq!(
            RamFramebuffer(screen(PixelOrder::Bgrx)).fourcc(),
            0x3432_5258
        );
        assert_eq!(
            RamFramebuffer(screen(PixelOrder::Rgbx)).fourcc(),
            0x3432_4258
        );
        assert_eq!(
            &RamFramebuffer(screen(PixelOrder::Bgrx)).encode()[8..12],
            b"\x34\x32\x52\x58",
        );
    }

    #[test]
    fn a_ramfb_configuration_lays_out_the_way_the_device_reads_it() {
        let bytes = RamFramebuffer(screen(PixelOrder::Bgrx)).encode();
        assert_eq!(bytes.len(), RAMFB_LEN);
        assert_eq!(&bytes[..8], &0x4020_0000u64.to_be_bytes());
        assert_eq!(&bytes[12..16], &0u32.to_be_bytes());
        assert_eq!(&bytes[16..20], &800u32.to_be_bytes());
        assert_eq!(&bytes[20..24], &600u32.to_be_bytes());
        // The stride, not `width * 4`, which is the same value here and is not in general.
        assert_eq!(&bytes[24..28], &3200u32.to_be_bytes());
    }
}
