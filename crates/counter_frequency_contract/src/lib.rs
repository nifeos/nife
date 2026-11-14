//! **The counter frequency page**: how many counter ticks make a second, on the architectures with
//! no register that says so (aarch64 reads `CNTFRQ_EL0` and maps no page). The kernel learns the rate once at boot and writes the page
//! once before mapping it read-only into every process, so no seqlock is needed; a child space's
//! page is written by its parent, and an unknown rate is zeroes, read as `None`. This crate is the
//! page's one definition for every writer and reader.
//!
//! # Identity
//!
//! The page's identity is frozen: [`MAGIC`] is the bytes `TIMEBAS1`, [`VERSION`] is 1 and
//! [`INTERFACE_ID`] is 9. So is its layout: [`PAGE_CONTENT_BYTES`] is 16, the magic at offset 0
//! and the rate as a little-endian `u64` at offset 8, at [`PAGE_VIRTUAL_ADDRESS`]
//! `0x0000_7000_0000_0000` on `x86_64` and `0x0000_0038_0000_0000` on `riscv64`. The id stays 9 for
//! good. A layout break ships as a new magic and a new version together, never a new id, and a
//! retired value is never reused.

//! # BUGS
//!
//! - The page carries one number for the life of a process, so the contract cannot express a
//!   counter whose rate changes after the page is written. The writer establishes that its counter
//!   keeps one rate before it writes the number.
//! - A process with no mapping at the fixed address faults when it reads the page, by design.
//! - The rate is read once per process, so a later, better number cannot reach it.
//!
//! Name: ratified 2026-11-01 (#3059). The stem
//! `counter_frequency` mirrors `CNTFRQ_EL0`, the register this page
//! substitutes for. Refused `timebase_proto` (the oscillator, where the contents are the frequency),
//! `tick_rate_proto` (drops the `CNTFRQ_EL0` connection), `tsc_frequency_proto` and
//! `x86_64_timebase_proto` (one architecture's name for a neutral contract).

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// The page's first eight bytes: an assembled page from an unrecognized or zeroed frame. ASCII,
/// unpadded, the same shape the sibling shared pages' magics use.
///
/// Name: ratified 2026-11-01 (#3059). Kept, and its value `TIMEBAS1` is kept: a wire value.
pub const MAGIC: [u8; 8] = *b"TIMEBAS1";

/// The page layout's version, the digit [`MAGIC`] carries. A reader's check is the magic itself;
/// a break bumps this and re-mints the magic in one change, with a dated ledger entry.
///
/// Name: ratified 2026-11-01 (#3059). The name every contract crate uses for its layout version.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 9, naming the counter-frequency
/// page. The number is this interface's for good: no other interface takes it, and a format change
/// is a new version, never a new number. The project that links these crates keeps the table of numbers and holds it to that rule. A program declares
/// the pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-01 (#3059). The name every contract crate uses for its interface identity.
pub const INTERFACE_ID: u64 = 9;

/// **The slowest counter this crate believes**, 100 kilohertz. Far below anything real, because a
/// false refusal on somebody's silicon turns a machine that would have run into one that will not
/// boot: the VisionFive 2's 4 megahertz `time` register is the slowest counter met so far, and 100
/// kilohertz sits above the 32.768 kilohertz watch crystal on purpose. Anything under it is a failed
/// measurement, not a slow part.
///
/// Name: ratified 2026-11-06, renamed from `MINIMUM_PLAUSIBLE_HZ`. The unit is spelled out.
pub const MINIMUM_PLAUSIBLE_HERTZ: u64 = 100_000;

/// **The fastest counter this crate believes**, 100 gigahertz, and the same reasoning as
/// [`MINIMUM_PLAUSIBLE_HERTZ`] read from the other end. No part ticks a counter at 100 gigahertz
/// and none will for a long time (the fastest met so far is a few gigahertz), so a number above this came from a
/// division that went the wrong way or a register that was never written. Generous by two orders of
/// magnitude on purpose: this band exists to catch nonsense, not to police hardware.
///
/// Name: ratified 2026-11-06, renamed from `MAXIMUM_PLAUSIBLE_HZ`. The unit is spelled out.
pub const MAXIMUM_PLAUSIBLE_HERTZ: u64 = 100_000_000_000;

/// Is `hertz` a rate a real machine could plausibly have? See [`MINIMUM_PLAUSIBLE_HERTZ`] and
/// [`MAXIMUM_PLAUSIBLE_HERTZ`] for where the bounds come from and why they are so wide.
///
/// One definition rather than three: a band copied into each architecture's timer drifts, so every
/// architecture's boot path calls this where it first learns its number.
///
/// # Examples
///
/// ```
/// use counter_frequency_contract::is_plausible;
///
/// assert!(is_plausible(4_000_000)); // the VisionFive 2's `time` register
/// assert!(is_plausible(62_500_000)); // QEMU's aarch64 virtual counter under TCG
/// assert!(!is_plausible(0)); // firmware never wrote it
/// assert!(!is_plausible(1)); // firmware wrote nonsense, and `> 0` would have believed it
/// ```
///
/// Name: ratified 2026-11-01 (#3059). Kept.
pub fn is_plausible(hertz: u64) -> bool {
    (MINIMUM_PLAUSIBLE_HERTZ..=MAXIMUM_PLAUSIBLE_HERTZ).contains(&hertz)
}

const MAGIC_OFFSET_BYTES: usize = 0;
const HERTZ_OFFSET_BYTES: usize = MAGIC_OFFSET_BYTES + 8;

/// **The size of the page's content** in bytes: the magic, then one little-endian `u64`. This is
/// not a page size. The mapped page is one whole frame (4096 bytes), and this is how much of it
/// carries data; the rest is zeroes.
///
/// Name: ratified 2026-11-01 (#3059), renamed from `PAGE_BYTES`.
pub const PAGE_CONTENT_BYTES: usize = HERTZ_OFFSET_BYTES + 8;

/// **The fixed virtual address the kernel maps this page at**, in every `x86_64` and every
/// `riscv64` process, shared by the writer and the reader through this crate. `riscv64` has its own,
/// because Sv39's low half ends at `0x40_0000_0000`. This one sits at seven-eighths of `x86_64`'s
/// low half, far above every band a program's own address space uses, because the page is mapped
/// into every process unconditionally.
///
/// Name: ratified 2026-11-01 (#3059), renamed from `PAGE_VA`.
#[cfg(not(target_arch = "riscv64"))]
pub const PAGE_VIRTUAL_ADDRESS: u64 = 0x0000_7000_0000_0000;

/// The same page's fixed address under Sv39, where the low half ends at `0x40_0000_0000` and the
/// value above is not addressable. `0x38_0000_0000` is seven-eighths of that ceiling, which is the
/// same rule the `x86_64` arm follows one address width down: 224 GiB in, with 8 GiB of headroom
/// below the boundary and every other user address a program is handed (the highest is a 2 GiB
/// code-model ceiling, below which the whole map lies) more than two hundred gigabytes below it.
///
/// The host build of this crate takes the `x86_64` arm, which is harmless: nothing in the `cfg(test)`
/// suite below depends on the value, only on the layout.
///
/// Name: ratified 2026-11-01 (#3059), renamed from `PAGE_VA`.
#[cfg(target_arch = "riscv64")]
pub const PAGE_VIRTUAL_ADDRESS: u64 = 0x0000_0038_0000_0000;

// A target whose pointers are narrower than the fixed address would truncate it to a wrong page in
// `CounterFrequencyPage::new`; refuse to build there instead.
const _: () = assert!(PAGE_VIRTUAL_ADDRESS <= usize::MAX as u64);

/// Build a counter frequency page's bytes for `hertz`, the counter's rate in hertz. The writer puts
/// the result into a fresh frame before mapping it read-only into any process; nothing else ever
/// constructs one of these. The number is written as given: the writer applies [`is_plausible`]
/// first.
///
/// # Examples
///
/// ```
/// use counter_frequency_contract::{CounterFrequencyPage, build_page};
///
/// let bytes = build_page(1_000_000_000);
/// // SAFETY: `bytes` is a live, byte-aligned buffer of exactly `PAGE_CONTENT_BYTES` for this block.
/// let page = unsafe { CounterFrequencyPage::new(bytes.as_ptr() as u64) };
/// assert_eq!(page.hertz(), Some(1_000_000_000));
/// ```
///
/// Name: ratified 2026-11-01 (#3059). Kept.
pub fn build_page(hertz: u64) -> [u8; PAGE_CONTENT_BYTES] {
    let mut page = [0u8; PAGE_CONTENT_BYTES];
    page[MAGIC_OFFSET_BYTES..MAGIC_OFFSET_BYTES + 8].copy_from_slice(&MAGIC);
    page[HERTZ_OFFSET_BYTES..HERTZ_OFFSET_BYTES + 8].copy_from_slice(&hertz.to_le_bytes());
    page
}

/// **The counter frequency page**, as seen through one process's read-only mapping of it at [`PAGE_VIRTUAL_ADDRESS`].
///
/// Name: ratified 2026-11-01 (#3059), renamed from `TimebasePage`.
#[derive(Debug, Clone, Copy)]
pub struct CounterFrequencyPage {
    base: *const u8,
}

// SAFETY: the page has one writer (the kernel, before the page is ever mapped into a second
// address space) and every reader after that sees only immutable bytes, so there is no mutable
// aliasing across the `Send`/`Sync` boundary to protect against.
unsafe impl Send for CounterFrequencyPage {}
// SAFETY: as `Send` above.
unsafe impl Sync for CounterFrequencyPage {}

impl CounterFrequencyPage {
    /// Name the counter frequency page mapped at `virtual_address`.
    ///
    /// # Safety
    ///
    /// `virtual_address` must be a mapped, byte-aligned buffer of at least [`PAGE_CONTENT_BYTES`]
    /// bytes (either a page [`build_page`] wrote, or a zeroed frame, which reads as "unknown"), and
    /// it must stay mapped and unwritten by this process for as long as this value is used. The
    /// address must also be representable as a pointer on the target; a target whose pointers are
    /// too narrow for [`PAGE_VIRTUAL_ADDRESS`] fails to compile. The kernel maps exactly such a page,
    /// read-only, at [`PAGE_VIRTUAL_ADDRESS`], into every `x86_64` and every `riscv64` process; see
    /// the crate's own `BUGS` section for a process with no such mapping.
    ///
    /// Name: ratified 2026-11-01 (#3059). Kept.
    pub const unsafe fn new(virtual_address: u64) -> Self {
        CounterFrequencyPage {
            base: virtual_address as *const u8,
        }
    }

    /// # Safety
    ///
    /// Upheld by [`new`](Self::new)'s contract: `base` names at least `PAGE_CONTENT_BYTES` mapped,
    /// stable bytes.
    fn bytes(&self) -> &[u8] {
        // SAFETY: `new`'s contract.
        unsafe { core::slice::from_raw_parts(self.base, PAGE_CONTENT_BYTES) }
    }

    /// The counter's rate in hertz, or `None` if this page is unrecognized (a zeroed frame, the
    /// state before calibration has run, or a page belonging to something else). Never a fabricated
    /// default: a parent that does not know the rate writes zeroes.
    ///
    /// The rate comes back exactly as the page carries it, so a page built for 0 reads as
    /// `Some(0)`. Applying the band, with [`is_plausible`], is the reader's to do before it divides
    /// by the number.
    ///
    /// # Examples
    ///
    /// A zeroed frame reads as unknown, never as a fabricated rate:
    ///
    /// ```
    /// use counter_frequency_contract::CounterFrequencyPage;
    ///
    /// let zeroed = [0u8; counter_frequency_contract::PAGE_CONTENT_BYTES];
    /// // SAFETY: as above.
    /// let page = unsafe { CounterFrequencyPage::new(zeroed.as_ptr() as u64) };
    /// assert_eq!(page.hertz(), None);
    /// ```
    ///
    /// Name: ratified 2026-11-06, renamed from `hz`. The unit is spelled out.
    pub fn hertz(&self) -> Option<u64> {
        let bytes = self.bytes();
        if bytes[MAGIC_OFFSET_BYTES..MAGIC_OFFSET_BYTES + 8] != MAGIC {
            return None;
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&bytes[HERTZ_OFFSET_BYTES..HERTZ_OFFSET_BYTES + 8]);
        Some(u64::from_le_bytes(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `buffer` read as a mapped [`CounterFrequencyPage`] for the length of `f`, and the one place these tests
    /// argue `CounterFrequencyPage::new`'s contract. The page is lent to `f` by reference; a test that copied it
    /// out would break the argument, and none does.
    fn with_page<R>(
        buffer: &[u8; PAGE_CONTENT_BYTES],
        f: impl FnOnce(&CounterFrequencyPage) -> R,
    ) -> R {
        // SAFETY: `buffer` is live, sized and aligned for the page by its type, and borrowed for
        // the whole call, so it outlives every use of the page inside `f`.
        f(&unsafe { CounterFrequencyPage::new(buffer.as_ptr() as u64) })
    }

    /// The round trip: build a page for a real-looking rate, read it back through a raw pointer
    /// the way a mapped frame would be named.
    #[test]
    fn a_built_page_round_trips_its_rate() {
        let bytes = build_page(2_400_000_000);
        with_page(&bytes, |page| {
            assert_eq!(page.hertz(), Some(2_400_000_000));
        });
    }

    /// A zeroed frame (nothing has ever written a page into it, or calibration has not run) reads
    /// as "unknown", not as a fabricated 0 hertz or a stale value.
    #[test]
    fn a_zeroed_frame_reads_as_unknown() {
        let zeroed = [0u8; PAGE_CONTENT_BYTES];
        with_page(&zeroed, |page| {
            assert_eq!(page.hertz(), None);
        });
    }

    /// An unrecognized magic refuses the rate even when the bytes after it look plausible: the
    /// magic check runs first.
    #[test]
    fn an_unrecognized_magic_reads_as_unknown() {
        let mut bytes = build_page(1_000_000_000);
        bytes[MAGIC_OFFSET_BYTES] ^= 0xff;
        with_page(&bytes, |page| {
            assert_eq!(page.hertz(), None);
        });
    }

    /// The band accepts every rate real machines have and refuses the two shapes the old
    /// `> 0` assertions let through: a register firmware never wrote, and a register firmware wrote
    /// nonsense into. The real rates are pinned by value so that a later reader narrowing the band
    /// for tidiness fails here rather than on somebody's board.
    #[test]
    fn the_band_accepts_real_rates_and_refuses_nonsense() {
        for hertz in [
            4_000_000,     // the VisionFive 2's `time` register
            10_000_000,    // QEMU virt, riscv64
            62_500_000,    // QEMU virt, aarch64 under TCG
            24_000_000,    // a common ARM board crystal
            3_000_000_000, // a TSC
        ] {
            assert!(
                is_plausible(hertz),
                "{hertz} hertz is a rate a real machine has"
            );
        }
        for hertz in [0, 1, 1_000, u64::MAX] {
            assert!(
                !is_plausible(hertz),
                "{hertz} hertz is a measurement that failed"
            );
        }
    }

    /// A page built for 0 reads as `Some(0)`: `hertz` returns the number as written, and the band
    /// is the reader's to apply.
    #[test]
    fn a_rate_outside_the_band_reads_back_as_written() {
        for hertz in [0, u64::MAX] {
            let bytes = build_page(hertz);
            with_page(&bytes, |page| {
                assert_eq!(page.hertz(), Some(hertz));
            });
        }
    }

    /// The layout constants do not overlap and the page fits in one frame, pinned so a mutant
    /// swapping an offset is caught here rather than by a torn read on real hardware.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_layout_offsets_do_not_overlap() {
        assert_eq!(HERTZ_OFFSET_BYTES, 8);
        assert_eq!(PAGE_CONTENT_BYTES, 16);
        assert!(PAGE_CONTENT_BYTES < 4096, "the page must fit in one frame");
    }

    /// The frozen identity, spelled out as literals so that no edit to a constant passes unseen:
    /// the magic as the bytes a page carries, the version, the id, and the rate's offset and byte
    /// order in a built page. The `x86_64` address is the arm the host build takes.
    #[test]
    fn the_identity_keeps_its_frozen_values() {
        assert_eq!(MAGIC, *b"TIMEBAS1");
        assert_eq!(VERSION, 1);
        assert_eq!(INTERFACE_ID, 9);
        assert_eq!(PAGE_CONTENT_BYTES, 16);
        let page = build_page(0x0102_0304_0506_0708);
        assert_eq!(&page[..8], b"TIMEBAS1");
        assert_eq!(page[8..], [8, 7, 6, 5, 4, 3, 2, 1]);
        #[cfg(not(target_arch = "riscv64"))]
        assert_eq!(PAGE_VIRTUAL_ADDRESS, 0x0000_7000_0000_0000);
        #[cfg(target_arch = "riscv64")]
        assert_eq!(PAGE_VIRTUAL_ADDRESS, 0x0000_0038_0000_0000);
    }
}

/// Machine-checked proofs, run with `cargo kani`. The reader's one decision is the magic compare,
/// and the writer's one promise is that the reader gets the number back. No proof covers the
/// caller's promise that the address names a mapped page, which Kani cannot see.
#[cfg(kani)]
mod verification {
    use super::*;

    /// Falsification: replayable `crates/counter_frequency_contract/falsifications/verification.hertz_answers_none_exactly_when_the_magic_differs.patch`
    /// **`hertz` is `None` exactly when the first eight bytes differ from the magic**, over every
    /// 16-byte frame, and otherwise it is the little-endian `u64` of the next eight bytes.
    #[kani::proof]
    fn hertz_answers_none_exactly_when_the_magic_differs() {
        let bytes: [u8; PAGE_CONTENT_BYTES] = kani::any();
        // SAFETY: `bytes` is a live, byte-aligned buffer of exactly `PAGE_CONTENT_BYTES`.
        let page = unsafe { CounterFrequencyPage::new(bytes.as_ptr() as u64) };
        let mut rate = [0u8; 8];
        rate.copy_from_slice(&bytes[8..16]);
        if bytes[..8] == MAGIC {
            assert_eq!(page.hertz(), Some(u64::from_le_bytes(rate)));
        } else {
            assert_eq!(page.hertz(), None);
        }
    }

    /// Falsification: replayable `crates/counter_frequency_contract/falsifications/verification.a_built_page_reads_back_its_rate.patch`
    /// **A built page reads back its rate**: for any `u64`, `build_page(x)` read through the reader
    /// is `Some(x)`.
    #[kani::proof]
    fn a_built_page_reads_back_its_rate() {
        let hertz: u64 = kani::any();
        let bytes = build_page(hertz);
        // SAFETY: `bytes` is a live, byte-aligned buffer of exactly `PAGE_CONTENT_BYTES`.
        let page = unsafe { CounterFrequencyPage::new(bytes.as_ptr() as u64) };
        assert_eq!(page.hertz(), Some(hertz));
    }
}
