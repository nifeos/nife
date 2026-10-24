//! **Two boot slots, and the rule that stops a bad upgrade bricking the machine.** An upgrade
//! writes the slot that is not running and puts it on trial; if the trial image never comes up,
//! the machine goes back to the previous one by itself. This crate touches no disk; see
//! notes/boot-slots.md. `system_installer` writes the state and `uefi_loader`'s chooser acts on it.
//!
//! The state is bits 48 to 56 ([`PRIORITY_SHIFT`], [`TRIES_SHIFT`], [`SUCCESSFUL_BIT`]) of a slot
//! partition's GPT attributes, which UEFI 2.11 section 5.3.3 gives to the owner of the type,
//! `NIFE_BOOT`; every other bit is preserved. The layout is ChromeOS's (ratified by calef on
//! 2026-09-21). **The chooser decrements `tries` before it hands off**, at a disk write per trial
//! boot, or a machine that hangs before userspace would retry the same bad image forever.

//! # BUGS
//!
//! - The layout and [`SlotHeader`] are ratified; change them through the header's version field.
//! - Confirmation proves nothing off the boot path, and a machine that never confirms rolls back a
//!   working upgrade, which fails safe. See notes/boot-slots.md, "What marks a trial boot
//!   successful".
//! - [`select`] takes any number of slots; two is `system_installer`'s choice, not this crate's.
//! - An exhausted slot's priority is not zeroed, unlike ChromeOS: [`select`] already skips it.
//! - A slot is 64 MiB of disk that is usually a duplicate.
//!
//! Name: ratified 2026-09-21 by calef. Refused `ab_boot` (it says two when nothing here does),
//! `boot_slot_attributes` (the byte layout, not the concept); `slot` alone is ambiguous in a
//! capability system.

#![no_std]

use globally_unique_identifier_partition_table::crc;

/// The lowest bit of the four-bit priority field: bits 48 to 51.
pub const PRIORITY_SHIFT: u32 = 48;
/// The lowest bit of the four-bit tries field: bits 52 to 55.
pub const TRIES_SHIFT: u32 = 52;
/// The confirmed-boot bit: bit 56.
pub const SUCCESSFUL_BIT: u32 = 56;

/// The largest value either four-bit field holds, and the ceiling both setters saturate at.
pub const MAX_NIBBLE: u8 = 15;

/// **The priority `system_installer` gives the slot it writes at install time.**
///
/// Not 15, deliberately. An upgrade has to be able to outrank the running image without first
/// lowering it, and a first install that took the top of the range would force every upgrader to
/// begin by rewriting a partition entry it is not otherwise touching.
pub const INSTALLED_PRIORITY: u8 = 2;

/// Mask of the bits this crate owns: 48 through 56. Everything else in an attribute word is
/// somebody else's and is preserved.
const OWNED: u64 = (0xF << PRIORITY_SHIFT) | (0xF << TRIES_SHIFT) | (1 << SUCCESSFUL_BIT);

/// **One slot's boot state**, as the three fields the attribute bits carry.
///
/// A plain struct rather than a wrapped `u64` so that a caller reading a disk in a debugger sees
/// three numbers, and so that the encode/decode round trip is something the tests can state.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct State {
    /// 0 to 15. **Zero means never select this slot**, whatever else is set.
    pub priority: u8,
    /// 0 to 15. Boot attempts left before the slot stops being bootable.
    pub tries: u8,
    /// This image has booted and something confirmed it, so [`Self::tries`] no longer matters.
    pub successful: bool,
}

impl State {
    /// A slot that holds nothing and will never be chosen. What `system_installer` writes into the spare
    /// slot, so that an upgrader finds a partition already laid out and does not have to repartition
    /// a running machine to install an update.
    pub const EMPTY: State = State {
        priority: 0,
        tries: 0,
        successful: false,
    };

    /// **The state of the slot an install just wrote**: confirmed, because the image in it is the
    /// one that is running at that moment and has therefore already booted this machine once.
    ///
    /// That claim is worth being precise about, because it is the strongest thing said anywhere in
    /// this crate. The bytes written into slot 0 by an install are a copy of the file the firmware
    /// started a few seconds earlier, on this machine, with this firmware. Marking it good is a
    /// record of an observation rather than an assumption.
    pub const fn installed() -> State {
        State {
            priority: INSTALLED_PRIORITY,
            tries: 0,
            successful: true,
        }
    }

    /// A slot holding a freshly written image that has never booted: outranking whatever is there,
    /// and with `tries` attempts to prove itself before the machine gives up on it.
    ///
    /// Both arguments saturate at [`MAX_NIBBLE`] rather than wrapping, because a wrapped priority
    /// of 16 is a priority of 0, which is the one value that means "never boot this".
    pub const fn on_trial(priority: u8, tries: u8) -> State {
        State {
            priority: if priority > MAX_NIBBLE {
                MAX_NIBBLE
            } else {
                priority
            },
            tries: if tries > MAX_NIBBLE {
                MAX_NIBBLE
            } else {
                tries
            },
            successful: false,
        }
    }

    /// **Can this slot be chosen?** Priority above zero, and either confirmed or with tries left.
    /// The same two clauses ChromeOS's firmware checks, in the same order.
    pub const fn is_bootable(&self) -> bool {
        self.priority > 0 && (self.successful || self.tries > 0)
    }

    /// **The state to write before handing off to this slot**: one try spent.
    ///
    /// A confirmed slot is returned unchanged, so a machine running a good image writes nothing to
    /// its disk on an ordinary boot. That is not only tidiness: a write on every boot is a write
    /// that can be interrupted on every boot.
    pub const fn attempted(&self) -> State {
        if self.successful || self.tries == 0 {
            *self
        } else {
            State {
                tries: self.tries - 1,
                ..*self
            }
        }
    }

    /// **The state of a slot that has proved itself**: confirmed, and with its tries returned to
    /// zero because they no longer mean anything.
    ///
    /// Priority is left alone. A trial slot was written with a priority above the running one, so
    /// confirming it is enough to keep it chosen; raising it further would spend range for nothing.
    ///
    /// # Examples
    ///
    /// The same upgrade when it works:
    ///
    /// ```
    /// # use boot_slot::{State, select};
    /// let mut slots = [State::installed(), State::on_trial(3, 3)];
    /// slots[1] = select(&slots).map(|i| slots[i].attempted()).unwrap();
    /// slots[1] = slots[1].confirmed();
    /// assert!(slots[1].successful);
    /// assert_eq!(select(&slots), Some(1));
    /// ```
    pub const fn confirmed(&self) -> State {
        State {
            priority: self.priority,
            tries: 0,
            successful: true,
        }
    }

    /// **The state of a slot whose image cannot be read at all**: given up on immediately rather
    /// than after its remaining tries.
    ///
    /// This is the one place nife writes a zero priority, and the reason is that the failure is
    /// one the chooser can see for itself. Tries exist to bound a failure that happens *after* the
    /// handoff, where nothing is watching. A header that does not decode, or an image whose
    /// checksum is wrong, is a failure the chooser observed with its own eyes, and spending three
    /// reboots to re-observe it would be three reboots of a machine somebody is waiting on.
    /// ChromeOS does the same thing for an invalid kernel header.
    pub const fn unreadable(&self) -> State {
        State {
            priority: 0,
            tries: 0,
            successful: false,
        }
    }

    /// Decode the three fields out of a GPT entry's attribute word.
    ///
    /// Total: every `u64` decodes, because every bit pattern of four bits is a number from 0 to 15
    /// and every bit is a boolean. There is no such thing as a malformed slot state, only one that
    /// is not bootable, which is the same layering
    /// [`globally_unique_identifier_partition_table::Entry::decode`] keeps.
    pub const fn from_attributes(attributes: u64) -> State {
        State {
            priority: ((attributes >> PRIORITY_SHIFT) & 0xF) as u8,
            tries: ((attributes >> TRIES_SHIFT) & 0xF) as u8,
            successful: (attributes >> SUCCESSFUL_BIT) & 1 == 1,
        }
    }

    /// The attribute word to write back, **preserving every bit this crate does not own**: the
    /// specification's bits 0 to 47 and the reserved 57 to 63.
    ///
    /// That preservation is the specification's own instruction ("They must be preserved if Bits
    /// 0-47 are modified", read in both directions) and it is why this takes the previous word
    /// rather than building one from nothing.
    pub const fn into_attributes(self, previous: u64) -> u64 {
        let priority = (self.priority & 0xF) as u64;
        let tries = (self.tries & 0xF) as u64;
        let successful = self.successful as u64;
        (previous & !OWNED)
            | (priority << PRIORITY_SHIFT)
            | (tries << TRIES_SHIFT)
            | (successful << SUCCESSFUL_BIT)
    }
}

/// **Which slot to boot**: the bootable one of highest priority, ties broken toward the lower
/// index.
///
/// `None` when nothing is bootable, and the caller's answer to that is the interesting half. On
/// this system it is not "refuse to boot": see [`select_excluding`] and `uefi_loader`'s chooser,
/// which falls back to the image in its own file.
///
/// # Examples
///
/// The life of one upgrade that never comes up, in the states the disk holds between reboots:
///
/// ```
/// use boot_slot::{State, select};
///
/// // After an install: slot 0 holds the image that was just written and is known good.
/// let mut slots = [State::installed(), State::EMPTY];
/// assert_eq!(select(&slots), Some(0));
///
/// // An upgrade writes slot 1 and puts it on trial, above slot 0.
/// slots[1] = State::on_trial(State::installed().priority + 1, 3);
/// assert_eq!(select(&slots), Some(1));
///
/// // Three boots that never come up. Each one spends a try BEFORE handing off.
/// for _ in 0..3 {
///     let chosen = select(&slots).expect("something is always bootable here");
///     slots[chosen] = slots[chosen].attempted();
/// }
///
/// // Nobody had to do anything: the machine is back on the image it came with.
/// assert_eq!(select(&slots), Some(0));
/// ```
pub fn select(slots: &[State]) -> Option<usize> {
    select_excluding(slots, 0)
}

/// [`select`], ignoring every slot whose bit is set in `tried`.
///
/// **The chooser retries within one boot**, which is the difference between a failure it can see
/// and one it cannot. An image whose header will not decode is a failure the chooser observed
/// before handing off, and making a person wait for a reboot to recover from it would be a worse
/// machine for no gain. An image that starts and then hangs is a failure nothing observes, and
/// that one is what [`State::attempted`] and the write-before-handoff are for.
///
/// Bit `n` of `tried` corresponds to slot `n`; slots past bit 63 cannot be excluded, which is not a
/// limit anything reaches with two.
pub fn select_excluding(slots: &[State], tried: u64) -> Option<usize> {
    slots
        .iter()
        .enumerate()
        .filter(|(i, s)| s.is_bootable() && (*i >= 64 || tried & (1 << i) == 0))
        .max_by_key(|(i, s)| (s.priority, core::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// The magic at the start of a slot partition: eight bytes a reader can find with `strings`.
pub const SLOT_MAGIC: [u8; 8] = *b"NIFESLOT";

/// The layout version this crate writes and the only one it reads.
pub const SLOT_VERSION: u32 = 1;

/// **Bytes of a slot partition reserved for [`SlotHeader`] before the image begins**: 4096, one
/// whole transfer block of `filesystem_contract::blk`.
///
/// Only the first [`SLOT_HEADER_BYTES`] carry anything. The rest is padding, and it is padding
/// rather than a smaller reservation so that **the image starts on a transfer-block boundary**.
/// `system_installer` writes whole 4096-byte blocks, and milestone 198 rung 2a already lost ten megabytes
/// to an unaligned start once; that bug is recorded in `crates/file_allocation_table` and this
/// number is what stops it happening a second time in a different file.
pub const SLOT_IMAGE_OFFSET: u64 = 4096;

/// Bytes of [`SLOT_IMAGE_OFFSET`] that [`SlotHeader::encode`] actually fills.
pub const SLOT_HEADER_BYTES: usize = 32;

/// **What is in a boot slot**, written at the very start of the partition and read by the chooser
/// before it spends a page of memory on the image. A partition has no length of its own, so the
/// image's length lives here, and its checksum makes an install that died mid-copy a refused slot
/// rather than a truncated image started.
///
/// | offset | size | |
/// |---|---|---|
/// | 0 | 8 | [`SLOT_MAGIC`] |
/// | 8 | 4 | [`SLOT_VERSION`], little-endian |
/// | 12 | 4 | reserved, zero |
/// | 16 | 8 | [`Self::image_len`], little-endian |
/// | 24 | 4 | [`Self::image_crc32`], little-endian |
/// | 28 | 4 | CRC-32 of bytes 0 to 27, little-endian |
///
/// CRC-32 is the GPT's own, `globally_unique_identifier_partition_table::crc`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotHeader {
    /// How many bytes of image follow, starting [`SLOT_IMAGE_OFFSET`] into the partition.
    pub image_len: u64,
    /// CRC-32 of exactly those bytes.
    pub image_crc32: u32,
}

impl SlotHeader {
    /// The header for an image, with its checksum computed from the bytes themselves.
    pub fn of(image: &[u8]) -> SlotHeader {
        SlotHeader {
            image_len: image.len() as u64,
            image_crc32: crc::crc32(image),
        }
    }

    /// Write the header into the start of a buffer of at least [`SLOT_HEADER_BYTES`], zeroing
    /// nothing else. `false` if the buffer is too small.
    pub fn encode(&self, out: &mut [u8]) -> bool {
        if out.len() < SLOT_HEADER_BYTES {
            return false;
        }
        out[0..8].copy_from_slice(&SLOT_MAGIC);
        out[8..12].copy_from_slice(&SLOT_VERSION.to_le_bytes());
        out[12..16].copy_from_slice(&0u32.to_le_bytes());
        out[16..24].copy_from_slice(&self.image_len.to_le_bytes());
        out[24..28].copy_from_slice(&self.image_crc32.to_le_bytes());
        let own = crc::crc32(&out[0..28]);
        out[28..32].copy_from_slice(&own.to_le_bytes());
        true
    }

    /// Read a header back. `None` for a buffer that is too short, a wrong magic, a version this
    /// build does not know, or a header whose own checksum does not hold.
    ///
    /// **An empty slot decodes as `None` rather than as a zero-length image**, because a partition
    /// that was never written is all zeros and all zeros is not the magic. That is the shape a
    /// caller wants: "there is nothing here" and "there is something broken here" both mean do not
    /// boot this slot.
    pub fn decode(bytes: &[u8]) -> Option<SlotHeader> {
        if bytes.len() < SLOT_HEADER_BYTES || bytes[0..8] != SLOT_MAGIC {
            return None;
        }
        if u32::from_le_bytes(bytes[8..12].try_into().ok()?) != SLOT_VERSION {
            return None;
        }
        if u32::from_le_bytes(bytes[28..32].try_into().ok()?) != crc::crc32(&bytes[0..28]) {
            return None;
        }
        Some(SlotHeader {
            image_len: u64::from_le_bytes(bytes[16..24].try_into().ok()?),
            image_crc32: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
        })
    }

    /// **Are these the bytes this header describes?** The check the chooser runs after reading an
    /// image and before starting it.
    pub fn matches(&self, image: &[u8]) -> bool {
        image.len() as u64 == self.image_len && crc::crc32(image) == self.image_crc32
    }
}

/// **The token that tells a booted kernel which slot it came from**, and the only thing that
/// crosses from the chooser into the running system.
///
/// A running system cannot work out its own slot: the chooser spends a try before handing off, so
/// a slot started on its last try is no longer [`State::is_bootable`], [`select`] names the other
/// one, and the machine would confirm the copy it did not boot. **This is a value two programs
/// agree on**, and the spelling was ratified by calef on 2026-09-22. The screen's token belongs to
/// `machine_discovery::framebuffer`, so no one place lists the line's vocabulary;
/// `uefi_loader::handoff::cmdline` writes all of it, and its `CMDLINE_LEN` adds up the lengths. See
/// notes/boot-slots/the-command-line.md.
pub mod cmdline {
    /// The token's key, including the `=`. Provisional: names are an architect's.
    pub const KEY: &str = "boot-slot=";

    /// The longest this token can be: the key and one decimal digit.
    ///
    /// One digit, because [`super::select_excluding`] cannot exclude past slot 63 and
    /// `system_installer` lays out two; [`encode`] refuses a wider number rather than truncating.
    pub const MAX_LEN: usize = KEY.len() + 1;

    /// Write `boot-slot=N` into `out`, returning its length, or **0 when it does not fit or `slot`
    /// is not a single digit**.
    ///
    /// Zero rather than a panic or a truncation: the caller is a bootloader with no console left,
    /// and a line it could not write is a boot that does not confirm, which is the direction this
    /// whole feature fails in anyway.
    #[must_use]
    pub fn encode(slot: u8, out: &mut [u8]) -> usize {
        if slot > 9 || out.len() < MAX_LEN {
            return 0;
        }
        out[..KEY.len()].copy_from_slice(KEY.as_bytes());
        out[KEY.len()] = b'0' + slot;
        MAX_LEN
    }

    /// **Which slot started this boot**, from the whole command line, or `None` when no word on it
    /// is this token: a stick, a `-kernel` boot and a chooser's fallback to its own image all reach
    /// a running kernel with nothing to confirm. A value that is not a single digit is `None` too,
    /// since a malformed number parsed as *some* slot would confirm the wrong partition entry,
    /// the one outcome worse than not confirming.
    ///
    /// # Examples
    ///
    /// ```
    /// use boot_slot::cmdline;
    ///
    /// let mut out = [0u8; cmdline::MAX_LEN];
    /// let n = cmdline::encode(1, &mut out);
    /// let line = core::str::from_utf8(&out[..n]).unwrap();
    /// assert_eq!(line, "boot-slot=1");
    /// assert_eq!(cmdline::parse(line), Some(1));
    ///
    /// // Beside the screen, in either order, which is what the kernel actually reads.
    /// assert_eq!(cmdline::parse("screen=0x80000000,800,600,3200,bgrx boot-slot=0"), Some(0));
    ///
    /// // A line that says nothing about slots is a boot that was not started by a chooser.
    /// assert_eq!(cmdline::parse("screen=0x80000000,800,600,3200,bgrx"), None);
    /// ```
    #[must_use]
    pub fn parse(cmdline: &str) -> Option<u8> {
        let word = cmdline
            .split_ascii_whitespace()
            .find(|w| w.starts_with(KEY))?;
        let digits = &word[KEY.len()..];
        match digits.as_bytes() {
            [d @ b'0'..=b'9'] => Some(d - b'0'),
            _ => None,
        }
    }
}

/// **Which medium this boot came from, on the kernel's command line** (the live stick proposal's
/// G2; calef ruled the token on PR #1652, 2026-10-04 UTC). `uefi_loader` writes `boot-medium=nvme`
/// when the file it was started from is on an NVMe disk, and the kernel mounts the internal NVMe
/// filesystem only then, because RedoxFS writes on mount. **Absence is the safe answer**: a stick,
/// a `-kernel` boot and an older loader all leave the internal disk alone. It sits beside
/// [`cmdline`] as the same kind of token two programs agree on (rule 7). Key and value are
/// provisional. See notes/boot-slots/the-command-line.md.
///
/// # EXAMPLES
///
/// ```
/// use boot_slot::medium;
///
/// let mut out = [0u8; medium::MAX_LEN];
/// let n = medium::encode_nvme(&mut out);
/// let line = core::str::from_utf8(&out[..n]).unwrap();
/// assert_eq!(line, "boot-medium=nvme");
/// assert!(medium::is_nvme("screen=0x80000000,800,600,3200,bgrx boot-slot=0 boot-medium=nvme"));
///
/// // A stick's line, and a line from a loader that never heard of the token.
/// assert!(!medium::is_nvme("screen=0x80000000,800,600,3200,bgrx"));
/// assert!(!medium::is_nvme("boot-medium=nvmex"));
/// ```
pub mod medium {
    /// The whole token. Provisional.
    pub const NVME: &str = "boot-medium=nvme";

    /// The longest this token can be.
    pub const MAX_LEN: usize = NVME.len();

    /// Write [`NVME`] into `out`, returning its length, or 0 when it does not fit.
    #[must_use]
    pub fn encode_nvme(out: &mut [u8]) -> usize {
        if out.len() < MAX_LEN {
            return 0;
        }
        out[..MAX_LEN].copy_from_slice(NVME.as_bytes());
        MAX_LEN
    }

    /// **Did this boot come from the NVMe disk?** True only for the exact word: a malformed one is
    /// a stick as far as the kernel is concerned, because that is the direction that writes nothing.
    #[must_use]
    pub fn is_nvme(cmdline: &str) -> bool {
        cmdline.split_ascii_whitespace().any(|w| w == NVME)
    }
}
