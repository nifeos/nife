//! **The inert-configuration contract** (§111 (inert configuration is a read-only page)): the
//! read-only page that carries `TZ`, `LANG` and `TERM` into a process, defined once for whoever
//! assembles it (init, or a kernel test harness) and whoever reads it (the `std` PAL). A value must
//! parse as a member of a known-safe [`domain`] to reach the page, enforced by [`PageBuilder`]'s
//! setters, because once a value is bytes on a page nothing can tell a password from a timezone.
//! The page is assembled before it is mapped read-only, so it needs no seqlock, and [`MAGIC`] is
//! checked first, so a zeroed or foreign frame answers `None`. See notes/env-config.md.

//! # BUGS
//!
//! - The domains are curated and short, not the IANA, glibc or terminfo lists, which would be a
//!   dependency decision (§46 (thin primitives or whole subsystems)). They grow on demand.
//!   `KNOWN_LOCALES` spells the codeset `UTF-8`, as glibc's `LANG` does, while `locale -a` prints
//!   `utf8`; whether `setlocale` normalizes the two is unverified.
//! - Every domain member fits its field's maximum by construction, and a `const` assertion fails the
//!   build when a list grows past it, so the page assembler never truncates a value.
//! - The reader answers only a member of its key's domain, so a page some other writer produced
//!   cannot hand a program a value outside the closed set. A zone, locale or terminal type missing
//!   from the lists is added by a pull request that names the real IANA, glibc or terminfo entry.
//!   Compiling the full tables in is a dependency decision (§46) and stays refused.
//!
//! Name: ratified 2026-08-23 (calef), renamed from `env_proto` to spell out the contraction;
//! `_contract` per calef's 2026-10-18 ruling.

#![cfg_attr(not(test), no_std)]

/// The curated, validated domains each declared key is checked against.
///
/// See the crate's own `BUGS` section for what "curated" means here: every value listed is real
/// and checkable, and the lists are short by design rather than by neglect.
pub mod domain {
    /// A non-exhaustive but real subset of IANA timezone identifiers. Grown on demand; see the
    /// crate's `BUGS` section.
    ///
    /// Name: ratified 2026-10-27 (#2797).
    #[doc(alias = "TZ")]
    pub const KNOWN_TIME_ZONES: &[&str] = &[
        "UTC",
        "America/New_York",
        "America/Chicago",
        "America/Denver",
        "America/Los_Angeles",
        "America/Anchorage",
        "America/Sao_Paulo",
        "Europe/London",
        "Europe/Paris",
        "Europe/Berlin",
        "Europe/Moscow",
        "Africa/Cairo",
        "Africa/Johannesburg",
        "Asia/Tokyo",
        "Asia/Shanghai",
        "Asia/Kolkata",
        "Asia/Dubai",
        "Asia/Singapore",
        "Australia/Sydney",
        "Pacific/Auckland",
    ];

    /// A non-exhaustive but real subset of POSIX/glibc locale identifiers.
    ///
    /// Name: ratified 2026-10-27 (#2797).
    #[doc(alias = "LANG")]
    pub const KNOWN_LOCALES: &[&str] = &[
        "C",
        "POSIX",
        "en_US",
        "en_US.UTF-8",
        "en_GB",
        "en_GB.UTF-8",
        "de_DE.UTF-8",
        "fr_FR.UTF-8",
        "es_ES.UTF-8",
        "ja_JP.UTF-8",
        "zh_CN.UTF-8",
        "pt_BR.UTF-8",
    ];

    /// A non-exhaustive but real subset of terminfo terminal type names.
    ///
    /// Name: ratified 2026-10-27 (#2797).
    #[doc(alias = "TERM")]
    pub const KNOWN_TERMINAL_TYPES: &[&str] = &[
        "dumb",
        "vt100",
        "vt220",
        "ansi",
        "linux",
        "xterm",
        "xterm-256color",
        "screen",
        "screen-256color",
        "tmux",
        "tmux-256color",
    ];

    /// Whether `s` is a member of [`KNOWN_TIME_ZONES`].
    pub fn is_valid_tz(s: &str) -> bool {
        KNOWN_TIME_ZONES.contains(&s)
    }

    /// Whether `s` is a member of [`KNOWN_LOCALES`].
    pub fn is_valid_lang(s: &str) -> bool {
        KNOWN_LOCALES.contains(&s)
    }

    /// Whether `s` is a member of [`KNOWN_TERMINAL_TYPES`].
    pub fn is_valid_term(s: &str) -> bool {
        KNOWN_TERMINAL_TYPES.contains(&s)
    }
}

/// The page's first eight bytes, so a reader can tell an assembled config page from a zeroed
/// frame or from somebody else's page. ASCII, ends unpadded at 8 bytes so the ASCII reads
/// straight in a hex dump the way `clock_contract::MAGIC` does.
pub const MAGIC: [u8; 8] = *b"ENVCONF1";

/// The page layout's version, the digit [`MAGIC`] carries. A reader's check is the magic itself;
/// a break bumps this and re-mints the magic in one change, with the dated ledger entry the
/// interface-stability discipline owes.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 13, naming the inert-configuration
/// page. The number is this interface's for good: no other interface takes it, and a format change
/// is a new version, never a new number. Milestone 956 (the linked interfaces carry checkable
/// versions) holds the table of numbers, and `script/lint` holds the tree to it. A program declares
/// the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 13;

/// The most bytes a `TZ` value may occupy. Longer than every entry in [`domain::KNOWN_TIME_ZONES`]
/// today (the longest canonical IANA names run to 30 bytes, and the longest backward link,
/// `America/Argentina/ComodRivadavia`, to 32), so a curated list can grow toward the real database's longer names without a
/// layout change.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "TZ")]
pub const MAXIMUM_TIME_ZONE_BYTES: usize = 40;

/// The most bytes a `LANG` value may occupy. Covers every entry in [`domain::KNOWN_LOCALES`] with
/// headroom for a codeset suffix this crate does not carry yet (`en_US.ISO8859-1` is 16 bytes).
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "LANG")]
pub const MAXIMUM_LOCALE_BYTES: usize = 24;

/// The most bytes a `TERM` value may occupy. Covers every entry in [`domain::KNOWN_TERMINAL_TYPES`]
/// (`screen-256color` is the longest at 15) with headroom to grow.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "TERM")]
pub const MAXIMUM_TERMINAL_TYPE_BYTES: usize = 24;

const MAGIC_OFFSET_BYTES: usize = 0;
const TIME_ZONE_LENGTH_OFFSET_BYTES: usize = MAGIC_OFFSET_BYTES + 8;
const TIME_ZONE_OFFSET_BYTES: usize = TIME_ZONE_LENGTH_OFFSET_BYTES + 1;
const LOCALE_LENGTH_OFFSET_BYTES: usize = TIME_ZONE_OFFSET_BYTES + MAXIMUM_TIME_ZONE_BYTES;
const LOCALE_OFFSET_BYTES: usize = LOCALE_LENGTH_OFFSET_BYTES + 1;
const TERMINAL_TYPE_LENGTH_OFFSET_BYTES: usize = LOCALE_OFFSET_BYTES + MAXIMUM_LOCALE_BYTES;
const TERMINAL_TYPE_OFFSET_BYTES: usize = TERMINAL_TYPE_LENGTH_OFFSET_BYTES + 1;

/// The whole page's size in bytes: magic, then one length-prefixed field per declared key. Well
/// under one frame (4096 bytes), which is the unit both the kernel and the `std` PAL map this
/// as.
pub const PAGE_BYTES: usize = TERMINAL_TYPE_OFFSET_BYTES + MAXIMUM_TERMINAL_TYPE_BYTES;

/// Why [`PageBuilder`] refused a value: it does not parse as a member of its key's [`domain`].
///
/// Not an errno space, the same way `clock_contract::status` is not one: this is a build-time
/// refusal, not a runtime failure a process observes. Whoever assembles a page (init, or a test
/// harness standing in for it) handles this the way a manifest mismatch is handled elsewhere in
/// this tree, as a refusal at the point of assembly rather than a value silently dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// The `TZ` value is not in [`domain::KNOWN_TIME_ZONES`].
    UnknownTz,
    /// The `LANG` value is not in [`domain::KNOWN_LOCALES`].
    UnknownLang,
    /// The `TERM` value is not in [`domain::KNOWN_TERMINAL_TYPES`].
    UnknownTerm,
}

/// Assemble an inert-configuration page. Each setter validates against its key's [`domain`]
/// before accepting the value, so a page this type builds can never carry a value outside its
/// key's closed set. A key never given a value stays absent: [`ConfigPage`] reads it back as
/// `None`, not as an empty string.
///
/// # Examples
///
/// Assembling is validate-then-write; a value outside the domain is refused before it reaches the page:
///
/// ```
/// use environment_contract::{ConfigPage, PageBuilder, Refused};
///
/// let page = PageBuilder::new()
///     .tz("America/Los_Angeles").unwrap()
///     .lang("en_US.UTF-8").unwrap()
///     .term("xterm-256color").unwrap()
///     .build();
///
/// // SAFETY: `page` is a live, aligned buffer of exactly `PAGE_BYTES`, alive for this block.
/// let read = unsafe { ConfigPage::new(page.as_ptr() as u64) };
/// assert_eq!(read.tz(), Some("America/Los_Angeles"));
/// assert_eq!(read.lang(), Some("en_US.UTF-8"));
/// assert_eq!(read.term(), Some("xterm-256color"));
///
/// // A value outside the domain is refused when the page is built, not carried through
/// // disguised as configuration.
/// assert_eq!(
///     PageBuilder::new().tz("definitely-not-a-timezone"),
///     Err(Refused::UnknownTz),
/// );
/// ```
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PageBuilder<'a> {
    tz: Option<&'a str>,
    lang: Option<&'a str>,
    term: Option<&'a str>,
}

impl<'a> PageBuilder<'a> {
    /// A builder with every key undeclared.
    pub const fn new() -> Self {
        PageBuilder {
            tz: None,
            lang: None,
            term: None,
        }
    }

    /// Declare `TZ`. Refused if `value` is not a member of [`domain::KNOWN_TIME_ZONES`].
    pub fn tz(mut self, value: &'a str) -> Result<Self, Refused> {
        if domain::is_valid_tz(value) {
            self.tz = Some(value);
            Ok(self)
        } else {
            Err(Refused::UnknownTz)
        }
    }

    /// Declare `LANG`. Refused if `value` is not a member of [`domain::KNOWN_LOCALES`].
    pub fn lang(mut self, value: &'a str) -> Result<Self, Refused> {
        if domain::is_valid_lang(value) {
            self.lang = Some(value);
            Ok(self)
        } else {
            Err(Refused::UnknownLang)
        }
    }

    /// Declare `TERM`. Refused if `value` is not a member of [`domain::KNOWN_TERMINAL_TYPES`].
    pub fn term(mut self, value: &'a str) -> Result<Self, Refused> {
        if domain::is_valid_term(value) {
            self.term = Some(value);
            Ok(self)
        } else {
            Err(Refused::UnknownTerm)
        }
    }

    /// Write the declared keys into a fresh page buffer. Bytes for a key never declared stay
    /// zero, which is what [`ConfigPage`] reads as absent.
    pub fn build(&self) -> [u8; PAGE_BYTES] {
        let mut page = [0u8; PAGE_BYTES];
        page[MAGIC_OFFSET_BYTES..MAGIC_OFFSET_BYTES + 8].copy_from_slice(&MAGIC);
        write_field(
            &mut page,
            TIME_ZONE_LENGTH_OFFSET_BYTES,
            TIME_ZONE_OFFSET_BYTES,
            MAXIMUM_TIME_ZONE_BYTES,
            self.tz,
        );
        write_field(
            &mut page,
            LOCALE_LENGTH_OFFSET_BYTES,
            LOCALE_OFFSET_BYTES,
            MAXIMUM_LOCALE_BYTES,
            self.lang,
        );
        write_field(
            &mut page,
            TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
            TERMINAL_TYPE_OFFSET_BYTES,
            MAXIMUM_TERMINAL_TYPE_BYTES,
            self.term,
        );
        page
    }
}

/// Whether every member of `list` fits in `maximum_bytes`, usable in a `const` assertion.
const fn every_member_fits(list: &[&str], maximum_bytes: usize) -> bool {
    let mut index = 0;
    while index < list.len() {
        if list[index].len() > maximum_bytes {
            return false;
        }
        index += 1;
    }
    true
}

const _: () = assert!(
    every_member_fits(domain::KNOWN_TIME_ZONES, MAXIMUM_TIME_ZONE_BYTES),
    "a time zone in KNOWN_TIME_ZONES is longer than MAXIMUM_TIME_ZONE_BYTES"
);
const _: () = assert!(
    every_member_fits(domain::KNOWN_LOCALES, MAXIMUM_LOCALE_BYTES),
    "a locale in KNOWN_LOCALES is longer than MAXIMUM_LOCALE_BYTES"
);
const _: () = assert!(
    every_member_fits(domain::KNOWN_TERMINAL_TYPES, MAXIMUM_TERMINAL_TYPE_BYTES),
    "a terminal type in KNOWN_TERMINAL_TYPES is longer than MAXIMUM_TERMINAL_TYPE_BYTES"
);

/// Write one length-prefixed field. A value longer than `maximum_bytes` is a defect in a domain
/// list, which the `const` assertion above refuses to compile, so this asserts instead of cutting
/// a real value short.
fn write_field(
    page: &mut [u8],
    length_offset: usize,
    value_offset: usize,
    maximum_bytes: usize,
    value: Option<&str>,
) {
    let Some(value) = value else { return };
    let bytes = value.as_bytes();
    assert!(bytes.len() <= maximum_bytes);
    let n = bytes.len();
    page[length_offset] = n as u8;
    page[value_offset..value_offset + n].copy_from_slice(bytes);
}

/// The bytes of one length-prefixed field. `None` for a zero length (the key was never declared)
/// or for a length past the field's maximum, which [`write_field`] never writes: the bytes beyond the
/// field belong to the next field or lie off the page, and a foreign or corrupt page might claim
/// them. Never panics, whatever the page and the length byte hold.
fn field_bytes(
    page: &[u8],
    length_offset: usize,
    value_offset: usize,
    maximum_bytes: usize,
) -> Option<&[u8]> {
    let n = *page.get(length_offset)? as usize;
    if n == 0 || n > maximum_bytes {
        return None;
    }
    page.get(value_offset..value_offset.checked_add(n)?)
}

/// Read one length-prefixed field back as a `&str`. `None` where [`field_bytes`] is `None`, or for
/// bytes that do not decode as UTF-8 (a page [`PageBuilder`] wrote never produces this; a foreign
/// or corrupt page might, and the honest answer is absence rather than a panic).
fn read_field(
    page: &[u8],
    length_offset: usize,
    value_offset: usize,
    maximum_bytes: usize,
) -> Option<&str> {
    core::str::from_utf8(field_bytes(
        page,
        length_offset,
        value_offset,
        maximum_bytes,
    )?)
    .ok()
}

/// **The inert-configuration page**, as seen through one process's read-only mapping of it. It
/// needs no seqlock: the page has exactly one writer, which finishes before it has a second reader.
///
/// # Examples
///
/// An undeclared key reads as absent, and an unassembled frame as no configuration at all:
///
/// ```
/// use environment_contract::{ConfigPage, PageBuilder};
///
/// let page = PageBuilder::new().tz("UTC").unwrap().build(); // LANG and TERM left undeclared
/// // SAFETY: `page` is a live, aligned buffer of exactly `PAGE_BYTES`, alive for this block.
/// let read = unsafe { ConfigPage::new(page.as_ptr() as u64) };
/// assert_eq!(read.tz(), Some("UTC"));
/// assert_eq!(read.lang(), None);
/// assert_eq!(read.term(), None);
///
/// let zeroed = [0u8; environment_contract::PAGE_BYTES];
/// // SAFETY: as above.
/// let nothing = unsafe { ConfigPage::new(zeroed.as_ptr() as u64) };
/// assert_eq!(nothing.tz(), None);
/// assert_eq!(nothing.lang(), None);
/// assert_eq!(nothing.term(), None);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ConfigPage {
    base: *const u8,
}

// SAFETY: the whole point of the page is that the process that assembled it and the process
// that reads it are different address spaces; every access is a plain read of immutable bytes
// (see the crate docs), so there is no mutable aliasing to protect against.
unsafe impl Send for ConfigPage {}
// SAFETY: as `Send` above.
unsafe impl Sync for ConfigPage {}

impl ConfigPage {
    /// Name the config page mapped at `va`.
    ///
    /// # Safety
    ///
    /// `va` must be a mapped, byte-aligned buffer of at least [`PAGE_BYTES`] bytes (either an
    /// assembled page, or a zeroed frame, which reads as "no configuration"), and it must stay
    /// mapped and unwritten by this process for as long as this value is used. The page is
    /// read-only by construction on the kernel side (the same `PageFrame` `READ`-only shape the
    /// clock page uses), so a caller with a correctly typed capability cannot violate this by
    /// accident.
    pub const unsafe fn new(va: u64) -> Self {
        ConfigPage {
            base: va as *const u8,
        }
    }

    /// # Safety
    ///
    /// Upheld by [`new`](Self::new)'s contract: `base` names at least `PAGE_BYTES` mapped,
    /// stable bytes.
    fn bytes(&self) -> &[u8] {
        // SAFETY: `new`'s contract.
        unsafe { core::slice::from_raw_parts(self.base, PAGE_BYTES) }
    }

    fn is_recognized(&self) -> bool {
        self.bytes()[MAGIC_OFFSET_BYTES..MAGIC_OFFSET_BYTES + 8] == MAGIC
    }

    /// `TZ`, or `None` if this process holds no config page, the page is unrecognized, `TZ` was
    /// never declared onto it, or the bytes are not a member of [`domain::KNOWN_TIME_ZONES`]. A
    /// reader never sees a value outside that set, whoever wrote the page.
    pub fn tz(&self) -> Option<&str> {
        self.is_recognized()
            .then(|| {
                read_field(
                    self.bytes(),
                    TIME_ZONE_LENGTH_OFFSET_BYTES,
                    TIME_ZONE_OFFSET_BYTES,
                    MAXIMUM_TIME_ZONE_BYTES,
                )
            })
            .flatten()
            .filter(|value| domain::is_valid_tz(value))
    }

    /// `LANG`, or `None` for the same reasons [`tz`](Self::tz) can be `None`, with
    /// [`domain::KNOWN_LOCALES`] as the set.
    pub fn lang(&self) -> Option<&str> {
        self.is_recognized()
            .then(|| {
                read_field(
                    self.bytes(),
                    LOCALE_LENGTH_OFFSET_BYTES,
                    LOCALE_OFFSET_BYTES,
                    MAXIMUM_LOCALE_BYTES,
                )
            })
            .flatten()
            .filter(|value| domain::is_valid_lang(value))
    }

    /// `TERM`, or `None` for the same reasons [`tz`](Self::tz) can be `None`, with
    /// [`domain::KNOWN_TERMINAL_TYPES`] as the set.
    pub fn term(&self) -> Option<&str> {
        self.is_recognized()
            .then(|| {
                read_field(
                    self.bytes(),
                    TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
                    TERMINAL_TYPE_OFFSET_BYTES,
                    MAXIMUM_TERMINAL_TYPE_BYTES,
                )
            })
            .flatten()
            .filter(|value| domain::is_valid_term(value))
    }
}

impl core::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Refused::UnknownTz => "TZ is not a member of KNOWN_TIME_ZONES",
            Refused::UnknownLang => "LANG is not a member of KNOWN_LOCALES",
            Refused::UnknownTerm => "TERM is not a member of KNOWN_TERMINAL_TYPES",
        })
    }
}

impl core::error::Error for Refused {}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests {
    use super::*;

    /// `buffer` read as a mapped [`ConfigPage`] for the length of `f`, and the one place these tests
    /// argue `ConfigPage::new`'s contract. The page is lent to `f` by reference; a test that copied it
    /// out would break the argument, and none does.
    fn with_page<R>(buffer: &[u8; PAGE_BYTES], f: impl FnOnce(&ConfigPage) -> R) -> R {
        // SAFETY: `buffer` is live, sized and aligned for the page by its type, and borrowed for
        // the whole call, so it outlives every use of the page inside `f`.
        f(&unsafe { ConfigPage::new(buffer.as_ptr() as u64) })
    }

    /// Every curated domain member fits under its field's maximum. The `const` assertion above
    /// already fails the build for a list grown past it; this names the offender at run time.
    #[test]
    fn every_domain_member_fits_its_field() {
        for tz in domain::KNOWN_TIME_ZONES {
            assert!(
                tz.len() <= MAXIMUM_TIME_ZONE_BYTES,
                "{tz:?} is {} bytes, over MAXIMUM_TIME_ZONE_BYTES",
                tz.len()
            );
        }
        for lang in domain::KNOWN_LOCALES {
            assert!(
                lang.len() <= MAXIMUM_LOCALE_BYTES,
                "{lang:?} is {} bytes, over MAXIMUM_LOCALE_BYTES",
                lang.len()
            );
        }
        for term in domain::KNOWN_TERMINAL_TYPES {
            assert!(
                term.len() <= MAXIMUM_TERMINAL_TYPE_BYTES,
                "{term:?} is {} bytes, over MAXIMUM_TERMINAL_TYPE_BYTES",
                term.len()
            );
        }
    }

    /// The domains are real (not synthesized) and this is what makes them worth being closed:
    /// every UTC-adjacent name a reader would type is one of the recognized ones, and one that
    /// looks plausible but is not (a made-up city, a common typo) is refused.
    #[test]
    fn domain_checks_accept_real_values_and_refuse_invented_ones() {
        assert!(domain::is_valid_tz("UTC"));
        assert!(domain::is_valid_tz("America/Los_Angeles"));
        assert!(!domain::is_valid_tz("Mars/Olympus_Mons"));
        assert!(!domain::is_valid_tz("america/los_angeles")); // case matters, like the real db

        assert!(domain::is_valid_lang("en_US.UTF-8"));
        assert!(!domain::is_valid_lang("klingon"));

        assert!(domain::is_valid_term("xterm-256color"));
        assert!(!domain::is_valid_term("my-cool-terminal"));
    }

    /// [`PageBuilder`]'s setters refuse before they ever touch a page, and the refusal names
    /// which key.
    #[test]
    fn the_builder_refuses_a_value_outside_its_domain() {
        assert_eq!(
            PageBuilder::new().tz("not-a-real-zone"),
            Err(Refused::UnknownTz)
        );
        assert_eq!(
            PageBuilder::new().lang("not-a-real-locale"),
            Err(Refused::UnknownLang)
        );
        assert_eq!(
            PageBuilder::new().term("not-a-real-terminal"),
            Err(Refused::UnknownTerm)
        );
    }

    /// The full round trip: build with all three keys, read every one back through a page built
    /// from a raw pointer the way a real mapped frame would be named.
    #[test]
    fn a_fully_declared_page_round_trips() {
        let bytes = PageBuilder::new()
            .tz("Europe/Berlin")
            .unwrap()
            .lang("de_DE.UTF-8")
            .unwrap()
            .term("screen-256color")
            .unwrap()
            .build();
        with_page(&bytes, |page| {
            assert_eq!(page.tz(), Some("Europe/Berlin"));
            assert_eq!(page.lang(), Some("de_DE.UTF-8"));
            assert_eq!(page.term(), Some("screen-256color"));
        });
    }

    /// A key never declared reads back absent, distinct from an empty string: this is the
    /// property that makes a page carrying only `TZ` legible as "no LANG was granted" rather
    /// than "LANG was granted the empty string".
    #[test]
    fn an_undeclared_key_reads_as_absent_not_empty() {
        let bytes = PageBuilder::new().tz("UTC").unwrap().build();
        with_page(&bytes, |page| {
            assert_eq!(page.tz(), Some("UTC"));
            assert_eq!(page.lang(), None);
            assert_eq!(page.term(), None);
        });
    }

    /// A page nobody has ever built into (a zeroed frame, the state a fresh page starts in
    /// before `init` maps something over it) reads as "no configuration" for every key, never
    /// as a fabricated default. This is DECISIONS §42's rule applied to configuration instead of
    /// to the clock.
    #[test]
    fn a_zeroed_frame_carries_no_configuration() {
        let zeroed = [0u8; PAGE_BYTES];
        with_page(&zeroed, |page| {
            assert_eq!(page.tz(), None);
            assert_eq!(page.lang(), None);
            assert_eq!(page.term(), None);
        });
    }

    /// A page with a recognized magic but a length byte past its field reads that field as
    /// absent, for every field and every oversized length, without a panic. `TERM` is the last
    /// field, so a length of 255 there runs off the end of the page, and a length one past a
    /// field's maximum in the time zone or locale field would otherwise read into the next field.
    #[test]
    fn a_length_past_its_field_reads_that_field_as_absent() {
        let fields = [
            (TIME_ZONE_LENGTH_OFFSET_BYTES, MAXIMUM_TIME_ZONE_BYTES),
            (LOCALE_LENGTH_OFFSET_BYTES, MAXIMUM_LOCALE_BYTES),
            (
                TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
                MAXIMUM_TERMINAL_TYPE_BYTES,
            ),
        ];
        for (corrupted, (length_offset, maximum_bytes)) in fields.into_iter().enumerate() {
            for length in (maximum_bytes + 1)..=usize::from(u8::MAX) {
                let mut bytes = PageBuilder::new()
                    .tz("UTC")
                    .unwrap()
                    .lang("C")
                    .unwrap()
                    .term("xterm")
                    .unwrap()
                    .build();
                bytes[length_offset] = length as u8;
                with_page(&bytes, |page| {
                    let read = [page.tz(), page.lang(), page.term()];
                    for (i, value) in read.iter().enumerate() {
                        assert_eq!(
                            value.is_none(),
                            i == corrupted,
                            "field {i}, length {length}"
                        );
                    }
                });
            }
        }
    }

    /// No domain member holds a zero byte, so a length that runs past a member into the zeroes after
    /// it never spells another member. The Kani harness that bounds its length byte relies on this.
    #[test]
    fn no_domain_member_holds_a_zero_byte() {
        for list in [
            domain::KNOWN_TIME_ZONES,
            domain::KNOWN_LOCALES,
            domain::KNOWN_TERMINAL_TYPES,
        ] {
            assert!(list.iter().all(|value| !value.as_bytes().contains(&0)));
        }
    }

    /// A recognized page whose field carries bytes that are not a member of the key's domain
    /// (a writer other than [`PageBuilder`], or a zone name with trailing zero bytes) reads that
    /// key as absent, and leaves the other keys alone.
    #[test]
    fn a_recognized_page_carrying_a_non_member_reads_that_key_as_absent() {
        let mut bytes = PageBuilder::new()
            .tz("UTC")
            .unwrap()
            .lang("C")
            .unwrap()
            .term("xterm")
            .unwrap()
            .build();
        // `UTC` followed by a NUL, one byte longer than the member.
        bytes[TIME_ZONE_LENGTH_OFFSET_BYTES] = 4;
        with_page(&bytes, |page| {
            assert_eq!(page.tz(), None);
            assert_eq!(page.lang(), Some("C"));
            assert_eq!(page.term(), Some("xterm"));
        });
        // A well-formed UTF-8 value the lists do not hold.
        let mut bytes = PageBuilder::new().lang("C").unwrap().build();
        bytes[LOCALE_OFFSET_BYTES] = b'X';
        with_page(&bytes, |page| assert_eq!(page.lang(), None));
        let mut bytes = PageBuilder::new().term("xterm").unwrap().build();
        bytes[TERMINAL_TYPE_OFFSET_BYTES] = b'X';
        with_page(&bytes, |page| assert_eq!(page.term(), None));
    }

    /// A refusal prints and converts into a boxed error, naming the domain constant.
    #[test]
    fn a_refusal_displays_and_is_an_error() {
        let all = [
            Refused::UnknownTz,
            Refused::UnknownLang,
            Refused::UnknownTerm,
        ];
        let names = ["KNOWN_TIME_ZONES", "KNOWN_LOCALES", "KNOWN_TERMINAL_TYPES"];
        for (refused, name) in all.into_iter().zip(names) {
            assert!(refused.to_string().contains(name));
            let boxed: Box<dyn core::error::Error> = Box::new(refused);
            assert!(boxed.to_string().contains(name));
        }
    }

    /// A length exactly equal to a field's maximum is the longest legal value and reads back whole, in
    /// every field, so the check against the maximum refuses only what lies past it.
    #[test]
    fn a_length_equal_to_its_maximum_reads_the_whole_field() {
        let fields = [
            (
                TIME_ZONE_LENGTH_OFFSET_BYTES,
                TIME_ZONE_OFFSET_BYTES,
                MAXIMUM_TIME_ZONE_BYTES,
            ),
            (
                LOCALE_LENGTH_OFFSET_BYTES,
                LOCALE_OFFSET_BYTES,
                MAXIMUM_LOCALE_BYTES,
            ),
            (
                TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
                TERMINAL_TYPE_OFFSET_BYTES,
                MAXIMUM_TERMINAL_TYPE_BYTES,
            ),
        ];
        for (length_offset, value_offset, maximum_bytes) in fields {
            let mut bytes = PageBuilder::new().build();
            bytes[length_offset] = maximum_bytes as u8;
            bytes[value_offset..value_offset + maximum_bytes].fill(b'a');
            let read = field_bytes(&bytes, length_offset, value_offset, maximum_bytes).unwrap();
            assert_eq!(read.len(), maximum_bytes);
            assert!(read.iter().all(|&b| b == b'a'));
        }
    }

    /// A zero length byte after the magic reads as absent: an assembler that wrote the magic but
    /// never declared the time zone.
    #[test]
    fn a_page_with_a_zero_length_after_the_magic_reads_that_field_as_absent() {
        let mut bytes = PageBuilder::new().tz("UTC").unwrap().build();
        // Corrupt the length byte to zero after building, simulating an assembler that wrote
        // the magic but never declared the time zone.
        bytes[TIME_ZONE_LENGTH_OFFSET_BYTES] = 0;
        with_page(&bytes, |page| {
            assert_eq!(page.tz(), None);
        });
    }

    /// An unrecognized magic refuses every field, even one whose bytes look plausible: the magic
    /// check runs before any field read, so a frame that merely happens to start with the same
    /// bytes as a real `TZ` value does not get misread as a config page.
    #[test]
    fn an_unrecognized_magic_refuses_every_field() {
        let mut bytes = PageBuilder::new()
            .tz("UTC")
            .unwrap()
            .lang("C")
            .unwrap()
            .term("dumb")
            .unwrap()
            .build();
        bytes[MAGIC_OFFSET_BYTES] ^= 0xff; // flip one byte of the magic
        with_page(&bytes, |page| {
            assert_eq!(page.tz(), None);
            assert_eq!(page.lang(), None);
            assert_eq!(page.term(), None);
        });
    }

    /// The layout constants are self-consistent (no overlap, no gap that would misalign a field
    /// against what `write_field`/`read_field` compute), pinned so a mutant swapping an offset
    /// is caught here rather than by a torn read on real hardware. Asserting on constants is
    /// this test's entire purpose (the same shape `clock_contract`'s
    /// `the_sanity_window_is_where_it_says` test pins its own layout constants with), so the
    /// lint that flags a constant assertion as pointless is switched off on purpose here.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_layout_offsets_do_not_overlap() {
        assert_eq!(TIME_ZONE_LENGTH_OFFSET_BYTES, 8);
        assert_eq!(TIME_ZONE_OFFSET_BYTES, 9);
        assert_eq!(LOCALE_LENGTH_OFFSET_BYTES, 9 + MAXIMUM_TIME_ZONE_BYTES);
        assert_eq!(LOCALE_OFFSET_BYTES, LOCALE_LENGTH_OFFSET_BYTES + 1);
        assert_eq!(
            TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
            LOCALE_OFFSET_BYTES + MAXIMUM_LOCALE_BYTES
        );
        assert_eq!(
            TERMINAL_TYPE_OFFSET_BYTES,
            TERMINAL_TYPE_LENGTH_OFFSET_BYTES + 1
        );
        assert_eq!(
            PAGE_BYTES,
            TERMINAL_TYPE_OFFSET_BYTES + MAXIMUM_TERMINAL_TYPE_BYTES
        );
        assert!(PAGE_BYTES < 4096, "the page must fit in one frame");
    }
}
