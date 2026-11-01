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
//!   Version 2 is [`members`]; the chooser still reads only version 1 until it adopts
//!   [`members::parse`].
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

/// The largest value either field holds, and the ceiling both setters saturate at. Both the
/// priority and the tries field are four bits wide.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "nibble")]
pub const MAXIMUM_FIELD_VALUE: u8 = 15;

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
    /// Both arguments saturate at [`MAXIMUM_FIELD_VALUE`] rather than wrapping, because a wrapped
    /// priority of 16 is a priority of 0, which is the one value that means "never boot this".
    pub const fn on_trial(priority: u8, tries: u8) -> State {
        State {
            priority: if priority > MAXIMUM_FIELD_VALUE {
                MAXIMUM_FIELD_VALUE
            } else {
                priority
            },
            tries: if tries > MAXIMUM_FIELD_VALUE {
                MAXIMUM_FIELD_VALUE
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

/// The version 1 layout: one image behind [`SlotHeader`]. [`SlotHeader::decode`] reads only this
/// one; [`members::parse`] reads it and version 2 ([`members::MEMBERS_VERSION`]), and the updater
/// writes only version 2.
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
    ///
    /// Name: ratified 2026-10-27 (#2797).
    pub const MAXIMUM_BYTES: usize = KEY.len() + 1;

    /// Write `boot-slot=N` into `out`, returning its length, or **0 when it does not fit or `slot`
    /// is not a single digit**.
    ///
    /// Zero rather than a panic or a truncation: the caller is a bootloader with no console left,
    /// and a line it could not write is a boot that does not confirm, which is the direction this
    /// whole feature fails in anyway.
    #[must_use]
    pub fn encode(slot: u8, out: &mut [u8]) -> usize {
        if slot > 9 || out.len() < MAXIMUM_BYTES {
            return 0;
        }
        out[..KEY.len()].copy_from_slice(KEY.as_bytes());
        out[KEY.len()] = b'0' + slot;
        MAXIMUM_BYTES
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
    /// let mut out = [0u8; cmdline::MAXIMUM_BYTES];
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
/// let mut out = [0u8; medium::MAXIMUM_BYTES];
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
    ///
    /// Name: ratified 2026-10-27 (#2797).
    pub const MAXIMUM_BYTES: usize = NVME.len();

    /// Write [`NVME`] into `out`, returning its length, or 0 when it does not fit.
    #[must_use]
    pub fn encode_nvme(out: &mut [u8]) -> usize {
        if out.len() < MAXIMUM_BYTES {
            return 0;
        }
        out[..MAXIMUM_BYTES].copy_from_slice(NVME.as_bytes());
        MAXIMUM_BYTES
    }

    /// **Did this boot come from the NVMe disk?** True only for the exact word: a malformed one is
    /// a stick as far as the kernel is concerned, because that is the direction that writes nothing.
    #[must_use]
    pub fn is_nvme(cmdline: &str) -> bool {
        cmdline.split_ascii_whitespace().any(|w| w == NVME)
    }
}

/// **The version 2 slot header: a slot that names its members.** Version 1 describes one image.
/// Version 2 describes three files, the kernel, the composed archive and the system manifest, each
/// by offset, length and SHA-256 digest, so an updater can write each from the package that
/// produced it. The layout was ruled on 2026-11-01 (UTC); see
/// `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md`, question 3. Pure
/// computation over byte slices, no allocation and no I/O.
///
/// | offset | size | |
/// |---|---|---|
/// | 0 | 8 | [`SLOT_MAGIC`] |
/// | 8 | 4 | [`members::MEMBERS_VERSION`], little-endian |
/// | 12 | 4 | reserved, zero |
/// | 16 | 56 each | three entries: tag 8, offset 8, length 8, SHA-256 digest 32 |
/// | 184 | 4 | CRC-32 of bytes 0 to 183, little-endian |
/// | 188 | 3908 | zero, to the end of the [`SLOT_IMAGE_OFFSET`] block |
///
/// Members start at multiples of [`SLOT_IMAGE_OFFSET`], in table order, without overlap, inside
/// the partition. Every refusal is a [`members::Refusal`] with a fixed phrase for the chooser to print.
///
/// # BUGS
///
/// - The parser checks structure, not content. Whether a member's bytes hash to its digest is the
///   caller's check, after it reads the member; this crate links no hash function.
/// - A zero-length member is refused as a layout break. The ruling does not say; refusing is the
///   stricter reading, and allowing one later is compatible.
/// - A member may not start inside the header block (offset below 4096). The ruling's "without
///   overlap" covers it; it is stated here because the header is the first thing in the table.
/// - [`members::Refusal::MissingTag`] cannot occur for a three-entry table: with no unknown tag and no
///   duplicate, all three tags are present. The check stays so that a fourth tag, added behind a
///   new version, cannot silently pass a table without it.
/// - A fuzz target for [`members::parse`] is not added: `.github/ci-job-budgets` holds the fuzz job at
///   eighteen targets, and a nineteenth means splitting the job into shards. The byte-flip tests
///   in this crate cover every byte of the 188 and a sample of the padding on the host.
///
/// Name: provisional. The module, [`members::parse`], [`members::ParsedHeader`], [`members::MemberHeader`], [`members::Member`],
/// [`members::MemberTag`], [`members::Refusal`] and the tag spellings are all unratified.
pub mod members {
    use globally_unique_identifier_partition_table::crc;

    use crate::{SLOT_IMAGE_OFFSET, SLOT_MAGIC, SLOT_VERSION, SlotHeader};

    /// The layout version this module writes, and the one it reads beside version 1.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    pub const MEMBERS_VERSION: u32 = 2;

    /// Bytes of the version 2 header that carry anything; the rest of the block is zero.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    pub const MEMBERS_HEADER_BYTES: usize = 188;

    /// How many members a version 2 header holds. There is no count field: the number is the layout.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    pub const MEMBER_COUNT: usize = 3;

    /// Bytes of one table entry: tag 8, offset 8, length 8, digest 32.
    const ENTRY_BYTES: usize = 56;
    /// Where the table starts.
    const TABLE_START: usize = 16;
    /// Where the CRC-32 sits; it covers everything before it.
    const CRC_START: usize = TABLE_START + MEMBER_COUNT * ENTRY_BYTES;
    /// Bytes of the reserved field (12 to 15).
    const RESERVED: core::ops::Range<usize> = 12..16;
    /// The block size as a `usize`, for slicing.
    const BLOCK: usize = SLOT_IMAGE_OFFSET as usize;

    const _: () = assert!(CRC_START == 184 && CRC_START + 4 == MEMBERS_HEADER_BYTES);

    /// **Which file a table entry names.** A closed set; any other 8 bytes is [`Refusal::UnknownTag`].
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum MemberTag {
        /// The kernel image.
        Kernel,
        /// The composed archive of base programs.
        Archive,
        /// The system manifest, which lists each package by digest.
        Manifest,
    }

    impl MemberTag {
        /// All three, in the order the updater writes them.
        ///
        /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
        pub const ALL: [MemberTag; MEMBER_COUNT] =
            [MemberTag::Kernel, MemberTag::Archive, MemberTag::Manifest];

        /// The 8 ASCII bytes in the table, zero-padded on the right. Provisional spellings.
        ///
        /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
        pub const fn bytes(self) -> [u8; 8] {
            match self {
                MemberTag::Kernel => *b"KERNEL\0\0",
                MemberTag::Archive => *b"ARCHIVE\0",
                MemberTag::Manifest => *b"MANIFEST",
            }
        }

        /// The tag these 8 bytes spell, or `None` for any other value.
        ///
        /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
        pub fn from_bytes(bytes: [u8; 8]) -> Option<MemberTag> {
            MemberTag::ALL.into_iter().find(|t| t.bytes() == bytes)
        }
    }

    /// **One file inside the slot**: where it starts, how long it is, and the SHA-256 of its bytes.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Member {
        /// Which file this is.
        pub tag: MemberTag,
        /// Byte offset from the start of the partition; a multiple of 4096.
        pub offset: u64,
        /// Length in bytes.
        pub length: u64,
        /// SHA-256 of exactly those bytes.
        pub digest: [u8; 32],
    }

    /// **The decoded version 2 header**: the three members in table order.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct MemberHeader {
        /// The table, in the order the entries sit in the header (and so in the partition).
        pub members: [Member; MEMBER_COUNT],
    }

    /// **A header this module reads**: version 1's single image or version 2's three members.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum ParsedHeader {
        /// A version 1 header, as [`SlotHeader::decode`] reads it.
        One(SlotHeader),
        /// A version 2 header.
        Two(MemberHeader),
    }

    /// **Why a slot header was refused.** The chooser skips the slot for that boot and prints
    /// [`Refusal::phrase`]. The phrases are fixed text, so a line in a console log names its cause.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Refusal {
        /// The magic is absent or the buffer is too short to hold it: an empty or foreign partition.
        NoHeader,
        /// The magic and version are present but the buffer is shorter than that version's header.
        Truncated,
        /// The magic matches and the version is neither 1 nor 2.
        UnknownVersion,
        /// A table entry's tag is not KERNEL, ARCHIVE or MANIFEST.
        UnknownTag,
        /// Two table entries carry the same tag.
        DuplicateTag,
        /// A tag of the closed set has no table entry.
        MissingTag,
        /// A member breaks the layout: an offset that is not a multiple of 4096 or that falls inside
        /// the header block, an overlap, an entry out of table order, a zero length, or a member that
        /// ends past the partition.
        LayoutBreak,
        /// A reserved or padding byte is not zero.
        NonzeroReservedBytes,
        /// The header's CRC-32 does not match its bytes.
        CrcMismatch,
    }

    impl Refusal {
        /// **The phrase the chooser prints** for this refusal. Fixed text; provisional wording.
        ///
        /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
        pub const fn phrase(self) -> &'static str {
            match self {
                Refusal::NoHeader => "no slot header",
                Refusal::Truncated => "slot header truncated",
                Refusal::UnknownVersion => "unknown slot header version",
                Refusal::UnknownTag => "unknown member tag",
                Refusal::DuplicateTag => "duplicate member tag",
                Refusal::MissingTag => "missing member tag",
                Refusal::LayoutBreak => "member layout break",
                Refusal::NonzeroReservedBytes => "nonzero reserved or padding bytes",
                Refusal::CrcMismatch => "slot header checksum mismatch",
            }
        }
    }

    fn read_u32(bytes: &[u8], at: usize) -> u32 {
        let mut word = [0u8; 4];
        word.copy_from_slice(&bytes[at..at + 4]);
        u32::from_le_bytes(word)
    }

    fn read_u64(bytes: &[u8], at: usize) -> u64 {
        let mut word = [0u8; 8];
        word.copy_from_slice(&bytes[at..at + 8]);
        u64::from_le_bytes(word)
    }

    /// The tag check on a list of raw tags: unknown first, then duplicate, then missing. Takes a
    /// slice so that the missing case, unreachable for three entries, stays testable.
    fn check_tags(tags: &[[u8; 8]]) -> Result<(), Refusal> {
        for tag in tags {
            if MemberTag::from_bytes(*tag).is_none() {
                return Err(Refusal::UnknownTag);
            }
        }
        for (i, tag) in tags.iter().enumerate() {
            if tags[..i].contains(tag) {
                return Err(Refusal::DuplicateTag);
            }
        }
        for wanted in MemberTag::ALL {
            if !tags.contains(&wanted.bytes()) {
                return Err(Refusal::MissingTag);
            }
        }
        Ok(())
    }

    /// The layout check: aligned, past the header block, in table order, no overlap, inside the
    /// partition. Arithmetic is checked, so an offset near `u64::MAX` is a break and not a wrap.
    fn check_layout(members: &[Member], partition_bytes: u64) -> Result<(), Refusal> {
        let mut floor = SLOT_IMAGE_OFFSET;
        for m in members {
            if m.offset % SLOT_IMAGE_OFFSET != 0 || m.offset < floor || m.length == 0 {
                return Err(Refusal::LayoutBreak);
            }
            let end = m.offset.checked_add(m.length).ok_or(Refusal::LayoutBreak)?;
            if end > partition_bytes {
                return Err(Refusal::LayoutBreak);
            }
            floor = end;
        }
        Ok(())
    }

    impl MemberHeader {
        /// **Write the header into the start of a slot's header block**, zeroing all 4096 bytes first
        /// so the padding is zero by construction. Refuses, writing nothing, a table that
        /// [`parse`] would refuse, and a buffer shorter than one block ([`Refusal::Truncated`]).
        ///
        /// This is the only writer: the updater writes version 2 and nothing else.
        ///
        /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
        pub fn encode(&self, out: &mut [u8], partition_bytes: u64) -> Result<(), Refusal> {
            if out.len() < BLOCK {
                return Err(Refusal::Truncated);
            }
            let tags = self.members.map(|m| m.tag.bytes());
            check_tags(&tags)?;
            check_layout(&self.members, partition_bytes)?;
            out[..BLOCK].fill(0);
            out[0..8].copy_from_slice(&SLOT_MAGIC);
            out[8..12].copy_from_slice(&MEMBERS_VERSION.to_le_bytes());
            for (i, m) in self.members.iter().enumerate() {
                let at = TABLE_START + i * ENTRY_BYTES;
                out[at..at + 8].copy_from_slice(&m.tag.bytes());
                out[at + 8..at + 16].copy_from_slice(&m.offset.to_le_bytes());
                out[at + 16..at + 24].copy_from_slice(&m.length.to_le_bytes());
                out[at + 24..at + 56].copy_from_slice(&m.digest);
            }
            let own = crc::crc32(&out[0..CRC_START]);
            out[CRC_START..CRC_START + 4].copy_from_slice(&own.to_le_bytes());
            Ok(())
        }
    }

    /// **Read a slot's header block**, version 1 or 2, given the partition's length in bytes.
    ///
    /// `bytes` is the start of the partition; a version 2 header needs the whole 4096-byte block.
    /// The checks run in this order, and the first failure is the refusal: magic, version, length,
    /// CRC-32, reserved and padding bytes, tags, layout. A version 1 header is read exactly as
    /// [`SlotHeader::decode`] reads it; its only refusals are the first four.
    ///
    /// # Examples
    ///
    /// ```
    /// use boot_slot::members::{Member, MemberHeader, MemberTag, ParsedHeader, Refusal, parse};
    ///
    /// let members = MemberTag::ALL.map(|tag| Member {
    ///     tag,
    ///     offset: 4096 * (1 + tag as u64),
    ///     length: 100,
    ///     digest: [7; 32],
    /// });
    /// let header = MemberHeader { members };
    /// let mut block = [0u8; 4096];
    /// header.encode(&mut block, 1 << 20).unwrap();
    /// assert_eq!(parse(&block, 1 << 20), Ok(ParsedHeader::Two(header)));
    ///
    /// // A partition too small for the last member refuses the slot with a printable reason.
    /// let refusal = parse(&block, 4096 * 3).unwrap_err();
    /// assert_eq!(refusal, Refusal::LayoutBreak);
    /// assert_eq!(refusal.phrase(), "member layout break");
    /// ```
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4a-the-version-2-slot-header.
    pub fn parse(bytes: &[u8], partition_bytes: u64) -> Result<ParsedHeader, Refusal> {
        if bytes.len() < 12 || bytes[0..8] != SLOT_MAGIC {
            return Err(Refusal::NoHeader);
        }
        match read_u32(bytes, 8) {
            SLOT_VERSION => parse_one(bytes),
            MEMBERS_VERSION => parse_two(bytes, partition_bytes),
            _ => Err(Refusal::UnknownVersion),
        }
    }

    fn parse_one(bytes: &[u8]) -> Result<ParsedHeader, Refusal> {
        if bytes.len() < crate::SLOT_HEADER_BYTES {
            return Err(Refusal::Truncated);
        }
        match SlotHeader::decode(bytes) {
            Some(header) => Ok(ParsedHeader::One(header)),
            None => Err(Refusal::CrcMismatch),
        }
    }

    fn parse_two(bytes: &[u8], partition_bytes: u64) -> Result<ParsedHeader, Refusal> {
        if bytes.len() < BLOCK {
            return Err(Refusal::Truncated);
        }
        if read_u32(bytes, CRC_START) != crc::crc32(&bytes[0..CRC_START]) {
            return Err(Refusal::CrcMismatch);
        }
        if bytes[RESERVED].iter().any(|b| *b != 0)
            || bytes[MEMBERS_HEADER_BYTES..BLOCK].iter().any(|b| *b != 0)
        {
            return Err(Refusal::NonzeroReservedBytes);
        }
        let mut tags = [[0u8; 8]; MEMBER_COUNT];
        for (i, tag) in tags.iter_mut().enumerate() {
            let at = TABLE_START + i * ENTRY_BYTES;
            tag.copy_from_slice(&bytes[at..at + 8]);
        }
        check_tags(&tags)?;
        let mut members = [Member {
            tag: MemberTag::Kernel,
            offset: 0,
            length: 0,
            digest: [0; 32],
        }; MEMBER_COUNT];
        for (i, m) in members.iter_mut().enumerate() {
            let at = TABLE_START + i * ENTRY_BYTES;
            m.tag = MemberTag::from_bytes(tags[i]).ok_or(Refusal::UnknownTag)?;
            m.offset = read_u64(bytes, at + 8);
            m.length = read_u64(bytes, at + 16);
            m.digest.copy_from_slice(&bytes[at + 24..at + 56]);
        }
        check_layout(&members, partition_bytes)?;
        Ok(ParsedHeader::Two(MemberHeader { members }))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const PARTITION: u64 = 64 << 20;

        fn sample() -> MemberHeader {
            MemberHeader {
                members: [
                    Member {
                        tag: MemberTag::Kernel,
                        offset: 4096,
                        length: 5000,
                        digest: [1; 32],
                    },
                    Member {
                        tag: MemberTag::Archive,
                        offset: 12288,
                        length: 4096,
                        digest: [2; 32],
                    },
                    Member {
                        tag: MemberTag::Manifest,
                        offset: 16384,
                        length: 1,
                        digest: [3; 32],
                    },
                ],
            }
        }

        fn block_of(header: &MemberHeader) -> [u8; BLOCK] {
            let mut block = [0u8; BLOCK];
            header.encode(&mut block, PARTITION).unwrap();
            block
        }

        /// Recompute the CRC after a hand edit, so a test reaches the check after the CRC.
        fn reseal(block: &mut [u8; BLOCK]) {
            let own = crc::crc32(&block[0..CRC_START]);
            block[CRC_START..CRC_START + 4].copy_from_slice(&own.to_le_bytes());
        }

        #[test]
        fn a_version_two_header_round_trips_and_fills_exactly_188_bytes() {
            let header = sample();
            let block = block_of(&header);
            assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
            assert!(block[MEMBERS_HEADER_BYTES..].iter().all(|b| *b == 0));
            assert_ne!(
                &block[MEMBERS_HEADER_BYTES - 4..MEMBERS_HEADER_BYTES],
                &[0; 4]
            );
            assert_eq!(&block[0..8], b"NIFESLOT");
            assert_eq!(&block[8..12], &2u32.to_le_bytes());
            assert_eq!(&block[16..24], b"KERNEL\0\0");
            assert_eq!(&block[72..80], b"ARCHIVE\0");
            assert_eq!(&block[128..136], b"MANIFEST");
        }

        #[test]
        fn the_table_may_list_the_tags_in_any_order() {
            let mut header = sample();
            header.members.reverse();
            for (i, m) in header.members.iter_mut().enumerate() {
                m.offset = 4096 * (1 + 2 * i as u64);
            }
            let block = block_of(&header);
            assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
        }

        #[test]
        fn a_version_one_header_still_reads_as_before() {
            let image = b"an image";
            let v1 = SlotHeader::of(image);
            let mut block = [0u8; BLOCK];
            assert!(v1.encode(&mut block));
            assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::One(v1)));
            assert_eq!(SlotHeader::decode(&block), Some(v1));
            // Just the 32 bytes also reads, as it did.
            assert_eq!(parse(&block[..32], PARTITION), Ok(ParsedHeader::One(v1)));
            block[20] ^= 1;
            assert_eq!(parse(&block, PARTITION), Err(Refusal::CrcMismatch));
            assert_eq!(parse(&block[..20], PARTITION), Err(Refusal::Truncated));
        }

        #[test]
        fn an_empty_or_foreign_partition_has_no_header() {
            assert_eq!(parse(&[0u8; BLOCK], PARTITION), Err(Refusal::NoHeader));
            assert_eq!(parse(&[], PARTITION), Err(Refusal::NoHeader));
            assert_eq!(parse(b"NIFESLOT", PARTITION), Err(Refusal::NoHeader));
        }

        #[test]
        fn an_unknown_version_is_refused_once_the_magic_matches() {
            for version in [0u32, 3, 0x0200_0000, u32::MAX] {
                let mut block = block_of(&sample());
                block[8..12].copy_from_slice(&version.to_le_bytes());
                assert_eq!(parse(&block, PARTITION), Err(Refusal::UnknownVersion));
            }
        }

        #[test]
        fn a_short_version_two_buffer_is_truncated() {
            let block = block_of(&sample());
            assert_eq!(
                parse(&block[..BLOCK - 1], PARTITION),
                Err(Refusal::Truncated)
            );
            assert_eq!(parse(&block[..12], PARTITION), Err(Refusal::Truncated));
        }

        #[test]
        fn a_wrong_crc_is_refused() {
            let mut block = block_of(&sample());
            block[CRC_START] ^= 0xFF;
            assert_eq!(parse(&block, PARTITION), Err(Refusal::CrcMismatch));
        }

        #[test]
        fn nonzero_reserved_bytes_are_refused() {
            for at in RESERVED {
                let mut block = block_of(&sample());
                block[at] = 1;
                reseal(&mut block);
                assert_eq!(parse(&block, PARTITION), Err(Refusal::NonzeroReservedBytes));
            }
        }

        #[test]
        fn nonzero_padding_is_refused_wherever_it_sits() {
            for at in [MEMBERS_HEADER_BYTES, 189, 1000, BLOCK - 1] {
                let mut block = block_of(&sample());
                block[at] = 1;
                assert_eq!(
                    parse(&block, PARTITION),
                    Err(Refusal::NonzeroReservedBytes),
                    "{at}"
                );
            }
        }

        #[test]
        fn an_unknown_tag_is_refused() {
            for bad in [*b"KERNEL\0X", *b"kernel\0\0", [0; 8], *b"INITRD\0\0"] {
                let mut block = block_of(&sample());
                block[16..24].copy_from_slice(&bad);
                reseal(&mut block);
                assert_eq!(parse(&block, PARTITION), Err(Refusal::UnknownTag));
            }
        }

        #[test]
        fn a_duplicate_tag_is_refused() {
            let mut block = block_of(&sample());
            block[72..80].copy_from_slice(b"KERNEL\0\0");
            reseal(&mut block);
            assert_eq!(parse(&block, PARTITION), Err(Refusal::DuplicateTag));
        }

        #[test]
        fn a_missing_tag_is_refused_by_the_tag_check() {
            // Three entries cannot reach this, so the check is held on its own.
            let two = [MemberTag::Kernel.bytes(), MemberTag::Archive.bytes()];
            assert_eq!(check_tags(&two), Err(Refusal::MissingTag));
            assert_eq!(check_tags(&[]), Err(Refusal::MissingTag));
            let all = MemberTag::ALL.map(MemberTag::bytes);
            assert_eq!(check_tags(&all), Ok(()));
        }

        #[test]
        fn every_layout_break_is_refused() {
            // (name, member index, offset, length) replacing that member of `sample()`.
            let breaks: [(&str, usize, u64, u64); 10] = [
                ("unaligned offset", 0, 4097, 5000),
                ("inside the header block", 0, 0, 5000),
                ("overlap", 1, 8192, 4096),
                ("out of table order", 1, 4096, 4096),
                ("past the partition", 2, 16384, PARTITION),
                ("offset past the partition", 2, PARTITION, 1),
                ("length overflows", 2, 16384, u64::MAX),
                ("zero length", 1, 12288, 0),
                ("offset overflows", 2, u64::MAX - 4095, 1),
                (
                    "ends one byte past the partition",
                    2,
                    16384,
                    PARTITION - 16383,
                ),
            ];
            for (name, index, offset, length) in breaks {
                let mut header = sample();
                header.members[index].offset = offset;
                header.members[index].length = length;
                // The writer refuses it ...
                let mut out = [0u8; BLOCK];
                assert_eq!(
                    header.encode(&mut out, PARTITION),
                    Err(Refusal::LayoutBreak),
                    "{name}"
                );
                assert!(
                    out.iter().all(|b| *b == 0),
                    "{name}: the writer wrote nothing"
                );
                // ... and so does the parser, given the same table with a correct CRC.
                let mut block = block_of(&sample());
                for (i, m) in header.members.iter().enumerate() {
                    let at = TABLE_START + i * ENTRY_BYTES;
                    block[at + 8..at + 16].copy_from_slice(&m.offset.to_le_bytes());
                    block[at + 16..at + 24].copy_from_slice(&m.length.to_le_bytes());
                }
                reseal(&mut block);
                assert_eq!(
                    parse(&block, PARTITION),
                    Err(Refusal::LayoutBreak),
                    "{name}"
                );
            }
        }

        #[test]
        fn a_gap_between_members_and_a_member_ending_at_the_partition_end_are_legal() {
            let mut header = sample();
            header.members[2].offset = 4096 * 50;
            header.members[2].length = PARTITION - header.members[2].offset;
            let block = block_of(&header);
            assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
        }

        #[test]
        fn the_writer_refuses_a_duplicate_tag_and_a_short_buffer() {
            let mut header = sample();
            header.members[1].tag = MemberTag::Kernel;
            assert_eq!(
                header.encode(&mut [0u8; BLOCK], PARTITION),
                Err(Refusal::DuplicateTag)
            );
            assert_eq!(
                sample().encode(&mut [0u8; BLOCK - 1], PARTITION),
                Err(Refusal::Truncated)
            );
        }

        #[test]
        fn the_writer_zeroes_stale_bytes_in_the_block() {
            let mut block = [0xAAu8; BLOCK + 10];
            sample().encode(&mut block, PARTITION).unwrap();
            assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(sample())));
            assert!(block[MEMBERS_HEADER_BYTES..BLOCK].iter().all(|b| *b == 0));
            assert!(
                block[BLOCK..].iter().all(|b| *b == 0xAA),
                "bytes past the block are not touched"
            );
        }

        #[test]
        fn flipping_any_byte_of_the_188_is_refused_or_changes_the_content_legally() {
            let original = sample();
            let block = block_of(&original);
            for at in 0..MEMBERS_HEADER_BYTES {
                for mask in 1..=255u8 {
                    let mut flipped = block;
                    flipped[at] ^= mask;
                    match parse(&flipped, PARTITION) {
                        Err(_) => {}
                        Ok(ParsedHeader::Two(header)) => {
                            assert_ne!(header, original, "byte {at} mask {mask:#x}");
                            panic!("a CRC-32 misses no single-byte flip: byte {at} mask {mask:#x}");
                        }
                        Ok(ParsedHeader::One(_)) => {
                            // Only a flip of the version byte can reach version 1, and its own CRC
                            // then has to hold over bytes it was not computed for.
                            assert_eq!(at, 8, "byte {at} mask {mask:#x}");
                        }
                    }
                }
            }
        }

        #[test]
        fn flipping_any_padding_byte_is_refused() {
            let block = block_of(&sample());
            for at in MEMBERS_HEADER_BYTES..BLOCK {
                let mut flipped = block;
                flipped[at] ^= 0x01;
                assert_eq!(
                    parse(&flipped, PARTITION),
                    Err(Refusal::NonzeroReservedBytes),
                    "{at}"
                );
            }
        }

        #[test]
        fn every_refusal_has_its_own_fixed_phrase() {
            let all = [
                Refusal::NoHeader,
                Refusal::Truncated,
                Refusal::UnknownVersion,
                Refusal::UnknownTag,
                Refusal::DuplicateTag,
                Refusal::MissingTag,
                Refusal::LayoutBreak,
                Refusal::NonzeroReservedBytes,
                Refusal::CrcMismatch,
            ];
            for (i, a) in all.iter().enumerate() {
                assert!(!a.phrase().is_empty());
                for b in &all[i + 1..] {
                    assert_ne!(a.phrase(), b.phrase());
                }
            }
            assert_eq!(Refusal::LayoutBreak.phrase(), "member layout break");
        }
    }
}
