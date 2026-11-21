//! **The user address-space map**: which band of a process's low half holds what, the same on all
//! three architectures.
//!
//! The bands tile `[0, 2 GiB)` with no gap (a `const` assert holds that, and Kani proves it per
//! address). Above 2 GiB only the timebase page is assigned.
//!
//! | band | start | end | size | what goes in it |
//! |---|---|---|---|---|
//! | [`NULL_GUARD`] | `0x0000_0000` | `0x0020_0000` | 2 MiB | nothing, ever |
//! | [`PAIR_PAGES`] | `0x0020_0000` | `0x1000_0000` | 254 MiB | pages one builder agrees with the one program it builds, and a program's private windows |
//! | [`RUNTIME_WINDOWS`] | `0x1000_0000` | `0x4000_0000` | 768 MiB | windows a loader or runtime places for a program |
//! | [`HEAP`] | `0x4000_0000` | `0x5000_0000` | 256 MiB | the heap and nothing else |
//! | [`SERVICE_WINDOWS`] | `0x5000_0000` | `0x6000_0000` | 256 MiB | large windows a service contract names |
//! | [`IMAGE`] | `0x6000_0000` | `0x7F00_0000` | 496 MiB | the program's code and data |
//! | [`STACK`] | `0x7F00_0000` | `0x7FFF_0000` | 16 MiB less 64 KiB | the stack, growing down; its lowest page is never mapped |
//! | [`PROCESS_PAGES`] | `0x7FFF_0000` | `0x8000_0000` | 64 KiB | pages the kernel maps into every process unasked |

//! # Where a new page goes
//!
//! Ask in order, and stop at the first yes.
//!
//! 1. Is it the program's code or data? [`IMAGE`].
//! 2. Is it the stack? [`STACK`], down from [`STACK_TOP_PAGE_VIRTUAL_ADDRESS`].
//! 3. Does the kernel map it into every process without being asked? [`PROCESS_PAGES`].
//! 4. Is it the heap? [`HEAP`].
//! 5. Does a loader or a runtime place it, for programs that did not choose the address?
//!    [`RUNTIME_WINDOWS`], with a written argument for the address in the crate that owns it.
//! 6. Is it a large window a service contract names for several programs? [`SERVICE_WINDOWS`].
//! 7. Otherwise it is a pair page: [`PAIR_PAGES`]. Two pairs never share an address space, so pairs
//!    reuse addresses freely.
//!
//! Then write it with the band's function, so the compiler checks it: [`pair_page`],
//! [`runtime_window`] and [`service_window`] return their argument unchanged and stop the build if
//! it is outside the band or not page aligned. A window of many pages goes through [`window`],
//! which checks the whole of it against the band.

//! # Why these numbers
//!
//! - **The image band ends at 2 GiB** ([`CODE_MODEL_CEILING_VIRTUAL_ADDRESS`]) because `x86_64` builds with
//!   `code-model: small` and a static relocation model, which promise that every symbol fits a
//!   sign-extended 32-bit immediate. The other two architectures would accept a higher image; one
//!   layout for all three is the rule.
//! - **The stack sits directly above the image**, so the one limit on an image is the stack's base,
//!   and a refusal can say exactly that ([`check_image`]). The lowest page of [`STACK`] is never
//!   mapped, so a stack at its limit faults before it touches the last page of a full image
//!   ([`MAXIMUM_STACK_PAGES`]).
//! - **The process pages sit beside the stack**, so they share its page tables: the stack's top
//!   page and the current-CPU page lie in one 2 MiB window.

//! # BUGS
//!
//! - Pair pages are checked for membership, not for collisions within one process; a program with
//!   several windows asserts their disjointness itself.
//! - [`pair_page`], [`runtime_window`] and [`service_window`] check one page. A window of many
//!   pages is written with [`window`]; one written with a band function alone can run into the next
//!   band unseen.
//! - [`window`] checks a window against its band, not against another window in the same band;
//!   disjoint windows are asserted by the crate that places them.
//! - Other crates restate some of these numbers, pinned by their own tests.
//! - A loader that reports a too-large image as "could not spawn" hides [`check_image`]'s reason.
//! - **The layout is fixed.** Every band and every fixed window sits at one address in every
//!   process on every architecture, so there is no address-space layout randomization. The design
//!   buys a pair page checked at compile time and one linker script; it gives up the
//!   unpredictability that a randomizing system offers an attacker's guess.
//!
//! Name: ratified 2026-11-14 (UTC) (block 1076 decision N1): the layout of one address space; `user_layout` and `address_space` were set aside.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// The layout's version: the bands, the pair-page arithmetic and the fixed windows a loader and a
/// kernel compile in from this crate, all one map. One constant for the whole layout; nothing
/// checks it at run time, and a program's declaration of the interfaces it links names it.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 2, naming the address-space layout constants. The number
/// is the crate's to keep across a rename and no other interface ever reuses it; the ids of
/// every interface crate must differ. A program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const INTERFACE_ID: u64 = 2;

/// The page size every loader maps in, on all three architectures, in bytes.
///
/// Name: ratified 2026-11-21 (UTC).
pub const PAGE_BYTES: u64 = 4096;

/// **A half-open range of user virtual addresses**, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// The first address in the band.
    pub start: u64,
    /// The first address past it.
    pub end: u64,
}

impl Band {
    /// **A band from `start` to `end`, checked where it is built.** It stops the build when a
    /// `const` band is empty or inverted, or does not start and end on a page. The fields stay
    /// public, so a band written as a literal still compiles; this is the checked way to write one.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    ///
    /// # Examples
    ///
    /// ```
    /// use address_space_layout::Band;
    ///
    /// const SCRATCH: Band = Band::new(0x10_0000, 0x20_0000);
    /// assert_eq!(SCRATCH.bytes(), 0x10_0000);
    /// ```
    pub const fn new(start: u64, end: u64) -> Band {
        assert!(start < end, "a band must start below its end");
        assert!(
            start.is_multiple_of(PAGE_BYTES),
            "a band must start on a page"
        );
        assert!(end.is_multiple_of(PAGE_BYTES), "a band must end on a page");
        Band { start, end }
    }

    /// The band's width in bytes.
    pub const fn bytes(&self) -> u64 {
        self.end - self.start
    }

    /// Is `virtual_address` inside the band?
    pub const fn contains(&self, virtual_address: u64) -> bool {
        virtual_address >= self.start && virtual_address < self.end
    }

    /// Does the whole of `[start, end)` lie inside the band? An empty or inverted range does not.
    pub const fn holds(&self, start: u64, end: u64) -> bool {
        start < end && start >= self.start && end <= self.end
    }
}

/// **The highest address a static `x86_64` image may reach.** See the crate docs: `code-model:
/// small` with a static relocation model puts every symbol under 2 GiB. The image band ends at or
/// below this, and every band on the map lies under it.
///
/// Name: ratified 2026-11-21 (UTC).
pub const CODE_MODEL_CEILING_VIRTUAL_ADDRESS: u64 = 0x8000_0000;

/// **Nothing is ever mapped here**, so a null pointer, and a null pointer plus any offset under 2
/// MiB, faults instead of reading something.
pub const NULL_GUARD: Band = Band {
    start: 0,
    end: 0x20_0000,
};

/// **Pages one builder agrees with the one program it builds.** See question 7 in the crate docs.
/// Test harnesses, fixtures and a builder's native children place their shared windows here, and
/// pairs reuse addresses freely. A program's own private windows go here as well.
pub const PAIR_PAGES: Band = Band {
    start: NULL_GUARD.end,
    end: 0x1000_0000,
};

/// **Windows a loader or runtime places on a program's behalf**: a standard library's runtime
/// pages and per-socket frames, a builder's scratch cursor, an initial ramdisk window, a swap image
/// window.
pub const RUNTIME_WINDOWS: Band = Band {
    start: PAIR_PAGES.end,
    end: 0x4000_0000,
};

/// **The heap.** `std_runtime_contract::HEAP_BASE` and `memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS` are
/// its start, and std's `HEAP_MAXIMUM` is its width. A program with no heap still leaves it empty.
pub const HEAP: Band = Band {
    start: RUNTIME_WINDOWS.end,
    end: 0x5000_0000,
};

/// **Large windows a service contract names**, which several programs speaking one protocol share:
/// a block channel (`0x5000_0000`, 8 MiB), a C seam's grant pages, a screen aperture.
pub const SERVICE_WINDOWS: Band = Band {
    start: HEAP.end,
    end: 0x6000_0000,
};

/// **The program's own ELF segments.** A program's linker script links at [`IMAGE_BASE_VIRTUAL_ADDRESS`], and the
/// host test that holds the two to the same number lives with the script's owner, not here.
pub const IMAGE: Band = Band {
    start: SERVICE_WINDOWS.end,
    end: 0x7F00_0000,
};

/// **The stack**, growing down from [`STACK_TOP_VIRTUAL_ADDRESS`]. Its lowest page is never mapped.
pub const STACK: Band = Band {
    start: IMAGE.end,
    end: 0x7FFF_0000,
};

/// **Pages the kernel maps into every process unasked**, placed beside the stack so they share its
/// page tables. Sixteen pages, of which the current-CPU page uses the last.
pub const PROCESS_PAGES: Band = Band {
    start: STACK.end,
    end: CODE_MODEL_CEILING_VIRTUAL_ADDRESS,
};

/// Every band, in address order, with the name a message or a table prints.
pub const BANDS: &[(&str, Band)] = &[
    ("NULL_GUARD", NULL_GUARD),
    ("PAIR_PAGES", PAIR_PAGES),
    ("RUNTIME_WINDOWS", RUNTIME_WINDOWS),
    ("HEAP", HEAP),
    ("SERVICE_WINDOWS", SERVICE_WINDOWS),
    ("IMAGE", IMAGE),
    ("STACK", STACK),
    ("PROCESS_PAGES", PROCESS_PAGES),
];

/// **Where every program is linked.** The linker script's `. =` line, as a number.
///
/// Name: ratified 2026-11-21 (UTC).
pub const IMAGE_BASE_VIRTUAL_ADDRESS: u64 = IMAGE.start;

/// **The stack pointer a thread starts with.** Stacks grow down, so this is one past the highest
/// stack byte.
///
/// Name: ratified 2026-11-21 (UTC).
pub const STACK_TOP_VIRTUAL_ADDRESS: u64 = STACK.end;

/// **The highest stack page**: the one page every loader maps, and the page below which a program
/// given a deeper stack gets the rest.
///
/// Name: ratified 2026-11-21 (UTC).
pub const STACK_TOP_PAGE_VIRTUAL_ADDRESS: u64 = STACK_TOP_VIRTUAL_ADDRESS - PAGE_BYTES;

/// **The most stack pages a loader may map**, down from [`STACK_TOP_PAGE_VIRTUAL_ADDRESS`]. One fewer than the band
/// holds: the lowest page is a guard, so a stack at its limit faults before it can touch the last
/// page of a maximal image.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_STACK_PAGES: u64 = STACK.bytes() / PAGE_BYTES - 1;

/// **Where the kernel maps the current-CPU page**: the last page
/// of [`PROCESS_PAGES`], in the same 2 MiB page-table window as [`STACK_TOP_PAGE_VIRTUAL_ADDRESS`].
///
/// Name: ratified 2026-11-21 (UTC).
pub const CURRENT_CPU_PAGE_VIRTUAL_ADDRESS: u64 = PROCESS_PAGES.end - PAGE_BYTES;

/// **A window's address, checked against its band at compile time, for the whole window.** Use it
/// in a `const`: `const BLOCK_CHANNEL: u64 = window(SERVICE_WINDOWS, 0x5000_0000, 8 << 20);`. It
/// returns `start_virtual_address` unchanged, and it stops the build unless the start and the
/// length are page aligned, the length is not zero, and all of `[start_virtual_address,
/// start_virtual_address + bytes)` lies inside `band`. [`pair_page`], [`runtime_window`] and
/// [`service_window`] are this for one page of their bands.
///
/// Name: provisional, ruled provisionally by the maintainer on 2026-11-14 (UTC) (block 1076
/// decision 3); the architect may revisit it before the first tag.
///
/// # Examples
///
/// ```
/// use address_space_layout::{window, SERVICE_WINDOWS};
///
/// const BLOCK_CHANNEL: u64 = window(SERVICE_WINDOWS, 0x5000_0000, 8 << 20);
/// assert_eq!(BLOCK_CHANNEL, 0x5000_0000);
/// ```
pub const fn window(band: Band, start_virtual_address: u64, bytes: u64) -> u64 {
    assert!(
        start_virtual_address.is_multiple_of(PAGE_BYTES),
        "a window must start on a page"
    );
    assert!(
        bytes.is_multiple_of(PAGE_BYTES),
        "a window must be a whole number of pages"
    );
    let Some(end_virtual_address) = start_virtual_address.checked_add(bytes) else {
        panic!("a window must not wrap past the top of the address space");
    };
    assert!(
        band.holds(start_virtual_address, end_virtual_address),
        "a window must lie wholly inside its band"
    );
    start_virtual_address
}

/// **A pair page's address, checked against the map at compile time.** Use it in a `const`:
/// `const CTL_VA: u64 = pair_page(0x60_0000);`. The value is unchanged, so the address still reads
/// in the source; what the call adds is that an address outside [`PAIR_PAGES`], or not page
/// aligned, stops the build instead of colliding with a runtime page at run time.
pub const fn pair_page(virtual_address: u64) -> u64 {
    window(PAIR_PAGES, virtual_address, PAGE_BYTES)
}

/// **A runtime window's address, checked against the map at compile time.** As [`pair_page`], for
/// [`RUNTIME_WINDOWS`].
pub const fn runtime_window(virtual_address: u64) -> u64 {
    window(RUNTIME_WINDOWS, virtual_address, PAGE_BYTES)
}

/// **Where the initial ramdisk archive is mapped**, read-only, before a program's `_start` runs: a
/// window of [`RUNTIME_WINDOWS`], checked against it at compile time. The kernel maps the archive
/// here and `user_mode_runtime::initial_ramdisk` reads it here, both through this one constant.
///
/// Name: ratified 2026-10-28 (UTC) (calef, relayed by the maintainer), renamed from
/// `INITIAL_RAMDISK_VA`: `va` is ruled to expand. Moved here from the kernel's and the runtime's
/// two copies of it.
pub const INITIAL_RAMDISK_VIRTUAL_ADDRESS: u64 = runtime_window(0x2000_0000);

/// **A service window's address, checked against the map at compile time.** As [`pair_page`], for
/// [`SERVICE_WINDOWS`].
pub const fn service_window(virtual_address: u64) -> u64 {
    window(SERVICE_WINDOWS, virtual_address, PAGE_BYTES)
}

/// **Why an image does not fit the map.** Both variants name the numbers a reader needs to fix the
/// program, which is the point: the loader used to report the first page it could not map, as
/// `AlreadyMapped`, and that named an overlap rather than a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePlacement {
    /// The image starts in [`IMAGE`] and runs past its end, into the stack band.
    TooLarge {
        /// One past the image's last byte, rounded up to a page.
        ///
        /// Name: ratified 2026-11-21 (UTC).
        image_end_virtual_address: u64,
        /// The lowest address the stack band reaches, which is where the image had to stop.
        ///
        /// Name: ratified 2026-11-21 (UTC).
        stack_base_virtual_address: u64,
    },
    /// The image does not start in [`IMAGE`]: it was linked for another layout, most likely the old
    /// one at `0x40_0000`, and would land among other bands' pages.
    OutsideBand {
        /// The image's first byte, rounded down to a page.
        ///
        /// Name: ratified 2026-11-21 (UTC).
        image_start_virtual_address: u64,
        /// One past its last byte, rounded up to a page.
        ///
        /// Name: ratified 2026-11-21 (UTC).
        image_end_virtual_address: u64,
    },
}

impl core::fmt::Display for ImagePlacement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            ImagePlacement::TooLarge {
                image_end_virtual_address,
                stack_base_virtual_address,
            } => write!(
                f,
                "the program image is too large: it ends at {image_end_virtual_address:#x}, past the stack's base at \
                 {stack_base_virtual_address:#x}; an image may be at most {} MiB",
                IMAGE.bytes() >> 20
            ),
            ImagePlacement::OutsideBand {
                image_start_virtual_address,
                image_end_virtual_address,
            } => write!(
                f,
                "the program image at {image_start_virtual_address:#x}..{image_end_virtual_address:#x} is outside the image band \
                 {:#x}..{:#x}; relink it at {IMAGE_BASE_VIRTUAL_ADDRESS:#x}",
                IMAGE.start, IMAGE.end
            ),
        }
    }
}

/// **Does an image spanning `[start, end)` fit the map?** `start` and `end` are the lowest and
/// highest page bounds over the image's loadable segments. Both loaders call this before mapping
/// anything, so an image that does not fit costs nothing and says why.
///
/// # Examples
///
/// ```
/// use address_space_layout::{check_image, pair_page, ImagePlacement, IMAGE, STACK};
///
/// // A fixture's shared page is a pair page, checked at compile time.
/// const CTL_VA: u64 = pair_page(0x60_0000);
/// assert_eq!(CTL_VA, 0x60_0000);
///
/// // An image that fits is placed; one that runs into the stack is told so, in those words.
/// assert!(check_image(IMAGE.start, IMAGE.start + 0x29_c000).is_ok());
/// let e = check_image(IMAGE.start, IMAGE.end + 0x1000).unwrap_err();
/// assert_eq!(e, ImagePlacement::TooLarge { image_end_virtual_address: IMAGE.end + 0x1000, stack_base_virtual_address: STACK.start });
/// ```
pub const fn check_image(start: u64, end: u64) -> Result<(), ImagePlacement> {
    if IMAGE.holds(start, end) {
        Ok(())
    } else if IMAGE.contains(start) && end > IMAGE.end {
        Err(ImagePlacement::TooLarge {
            image_end_virtual_address: end,
            stack_base_virtual_address: STACK.start,
        })
    } else {
        Err(ImagePlacement::OutsideBand {
            image_start_virtual_address: start,
            image_end_virtual_address: end,
        })
    }
}

/// **The bit at which each architecture's user half ends**, so every user address is below
/// `1 << shift`: aarch64 48, `x86_64` 47, `riscv64` under Sv39 38. The page-table code that splits
/// the halves restates these and tests its own numbers against this table, so the two cannot drift
/// apart; this crate depends on nothing.
///
/// Name: ratified 2026-11-21 (UTC).
pub const LOW_HALF_SPLIT_SHIFTS: [(&str, u32); 3] =
    [("aarch64", 48), ("x86_64", 47), ("riscv64", 38)];

// The map's shape, checked where every consumer compiles it: the bands tile
// `[0, CODE_MODEL_CEILING_VIRTUAL_ADDRESS)` in `BANDS` order, each non-empty and page aligned at both ends, which is
// what makes them disjoint and ordered; and the ceiling is a user address on all three
// architectures. `verification.rs` proves the same map per address with Kani.
const _: () = {
    let mut at = 0;
    let mut i = 0;
    while i < BANDS.len() {
        let band = BANDS[i].1;
        assert!(
            band.start == at,
            "a band does not start where the band below it ends"
        );
        assert!(band.start < band.end, "a band is empty");
        assert!(
            band.start.is_multiple_of(PAGE_BYTES),
            "a band does not start on a page"
        );
        assert!(
            band.end.is_multiple_of(PAGE_BYTES),
            "a band does not end on a page"
        );
        at = band.end;
        i += 1;
    }
    assert!(
        at == CODE_MODEL_CEILING_VIRTUAL_ADDRESS,
        "the bands do not end at the code-model ceiling"
    );
    let [(_, aarch64), (_, x86_64), (_, riscv64)] = LOW_HALF_SPLIT_SHIFTS;
    assert!(
        (CODE_MODEL_CEILING_VIRTUAL_ADDRESS - 1) >> aarch64 == 0,
        "the map reaches past aarch64's user half"
    );
    assert!(
        (CODE_MODEL_CEILING_VIRTUAL_ADDRESS - 1) >> x86_64 == 0,
        "the map reaches past x86_64's user half"
    );
    assert!(
        (CODE_MODEL_CEILING_VIRTUAL_ADDRESS - 1) >> riscv64 == 0,
        "the map reaches past riscv64's user half"
    );
};

// The derived constants, checked where every consumer compiles them.
const _: () = {
    assert!(IMAGE_BASE_VIRTUAL_ADDRESS.is_multiple_of(2 * 1024 * 1024));
    assert!(STACK_TOP_PAGE_VIRTUAL_ADDRESS >> 21 == CURRENT_CPU_PAGE_VIRTUAL_ADDRESS >> 21);
    assert!(
        STACK.contains(STACK_TOP_PAGE_VIRTUAL_ADDRESS)
            && STACK_TOP_PAGE_VIRTUAL_ADDRESS.is_multiple_of(PAGE_BYTES)
    );
    assert!(
        STACK_TOP_PAGE_VIRTUAL_ADDRESS - (MAXIMUM_STACK_PAGES - 1) * PAGE_BYTES
            == STACK.start + PAGE_BYTES
    );
    assert!(
        PROCESS_PAGES.bytes() == 16 * PAGE_BYTES
            && PROCESS_PAGES.contains(CURRENT_CPU_PAGE_VIRTUAL_ADDRESS)
    );
};

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn an_image_the_size_of_a_large_program_fits_and_the_old_ceiling_is_gone() {
        // A measured ripgrep image spanned 0x40_0000..0x69c_000.
        let large_program = 0x69c_000 - 0x40_0000;
        assert!(
            check_image(
                IMAGE_BASE_VIRTUAL_ADDRESS,
                IMAGE_BASE_VIRTUAL_ADDRESS + large_program
            )
            .is_ok()
        );
        assert!(
            IMAGE.bytes() / large_program >= 100,
            "less than 100x headroom over ripgrep"
        );
        assert!(IMAGE.bytes() > 896 * 1024);
    }

    #[test]
    fn an_image_past_the_band_is_too_large_and_names_the_stack_base() {
        let end = IMAGE.end + PAGE_BYTES;
        assert_eq!(
            check_image(IMAGE_BASE_VIRTUAL_ADDRESS, end),
            Err(ImagePlacement::TooLarge {
                image_end_virtual_address: end,
                stack_base_virtual_address: STACK.start
            })
        );
        let text = format!(
            "{}",
            check_image(IMAGE_BASE_VIRTUAL_ADDRESS, end).unwrap_err()
        );
        assert!(text.contains("too large"), "{text}");
        assert!(text.contains(&format!("{end:#x}")), "{text}");
        assert!(text.contains(&format!("{:#x}", STACK.start)), "{text}");
        // Exactly full is not too large.
        assert!(check_image(IMAGE_BASE_VIRTUAL_ADDRESS, IMAGE.end).is_ok());
    }

    #[test]
    fn an_image_linked_for_the_old_layout_is_outside_the_band() {
        assert_eq!(
            check_image(0x40_0000, 0x4e_0000),
            Err(ImagePlacement::OutsideBand {
                image_start_virtual_address: 0x40_0000,
                image_end_virtual_address: 0x4e_0000
            })
        );
        // And so is one that starts below the band and reaches into it.
        assert!(matches!(
            check_image(
                IMAGE_BASE_VIRTUAL_ADDRESS - PAGE_BYTES,
                IMAGE_BASE_VIRTUAL_ADDRESS + PAGE_BYTES
            ),
            Err(ImagePlacement::OutsideBand { .. })
        ));
        // An empty image is refused rather than trivially placed, and as misplaced, not as large.
        assert_eq!(
            check_image(IMAGE_BASE_VIRTUAL_ADDRESS, IMAGE_BASE_VIRTUAL_ADDRESS),
            Err(ImagePlacement::OutsideBand {
                image_start_virtual_address: IMAGE_BASE_VIRTUAL_ADDRESS,
                image_end_virtual_address: IMAGE_BASE_VIRTUAL_ADDRESS
            })
        );
        // An image that starts below the band and ends past it is outside it; only one that starts
        // inside the band and overruns the far edge is too large.
        assert_eq!(
            check_image(
                IMAGE_BASE_VIRTUAL_ADDRESS - PAGE_BYTES,
                IMAGE.end + PAGE_BYTES
            ),
            Err(ImagePlacement::OutsideBand {
                image_start_virtual_address: IMAGE_BASE_VIRTUAL_ADDRESS - PAGE_BYTES,
                image_end_virtual_address: IMAGE.end + PAGE_BYTES
            })
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_stack_keeps_a_guard_page_between_it_and_a_full_image() {
        let lowest = STACK_TOP_PAGE_VIRTUAL_ADDRESS - (MAXIMUM_STACK_PAGES - 1) * PAGE_BYTES;
        assert_eq!(lowest, STACK.start + PAGE_BYTES);
        assert!(IMAGE.end < lowest);
        // Linux's default main-thread stack, 8 MiB, fits.
        assert!(MAXIMUM_STACK_PAGES * PAGE_BYTES >= 8 << 20);
    }

    #[test]
    fn a_band_contains_its_start_and_not_its_end() {
        assert!(IMAGE.contains(IMAGE.start) && IMAGE.contains(IMAGE.end - 1));
        assert!(!IMAGE.contains(IMAGE.start - 1) && !IMAGE.contains(IMAGE.end));
    }

    #[test]
    fn a_band_built_with_new_is_the_band_its_fields_name() {
        const BUILT: Band = Band::new(IMAGE.start, IMAGE.end);
        assert_eq!(BUILT, IMAGE);
        // One page is the smallest band.
        assert_eq!(Band::new(0, PAGE_BYTES).bytes(), PAGE_BYTES);
        for &(name, band) in BANDS {
            assert_eq!(Band::new(band.start, band.end), band, "{name}");
        }
    }

    #[test]
    fn a_band_that_is_empty_inverted_or_off_a_page_is_refused() {
        let refused =
            |start: u64, end: u64| std::panic::catch_unwind(|| Band::new(start, end)).is_err();
        assert!(!refused(PAGE_BYTES, 2 * PAGE_BYTES));
        // Empty, inverted, unaligned at the start, unaligned at the end.
        assert!(refused(PAGE_BYTES, PAGE_BYTES));
        assert!(refused(2 * PAGE_BYTES, PAGE_BYTES));
        assert!(refused(1, 2 * PAGE_BYTES));
        assert!(refused(PAGE_BYTES, 2 * PAGE_BYTES - 1));
    }

    #[test]
    fn a_window_is_checked_whole_and_returns_its_start() {
        const BLOCK_CHANNEL: u64 = window(SERVICE_WINDOWS, 0x5000_0000, 8 << 20);
        assert_eq!(BLOCK_CHANNEL, 0x5000_0000);
        // The whole band, and the last page of it, are windows.
        assert_eq!(
            window(
                SERVICE_WINDOWS,
                SERVICE_WINDOWS.start,
                SERVICE_WINDOWS.bytes()
            ),
            SERVICE_WINDOWS.start
        );
        assert_eq!(
            window(
                SERVICE_WINDOWS,
                SERVICE_WINDOWS.end - PAGE_BYTES,
                PAGE_BYTES
            ),
            SERVICE_WINDOWS.end - PAGE_BYTES
        );
    }

    #[test]
    fn a_window_that_runs_past_its_band_or_is_malformed_is_refused() {
        let refused = |band: Band, start: u64, bytes: u64| {
            std::panic::catch_unwind(|| window(band, start, bytes)).is_err()
        };
        // A first page inside the band is not enough: this window ends in IMAGE.
        let last = SERVICE_WINDOWS.end - PAGE_BYTES;
        assert!(!refused(SERVICE_WINDOWS, last, PAGE_BYTES));
        assert!(refused(SERVICE_WINDOWS, last, 2 * PAGE_BYTES));
        assert!(refused(
            SERVICE_WINDOWS,
            SERVICE_WINDOWS.start,
            SERVICE_WINDOWS.bytes() + PAGE_BYTES
        ));
        // Starts below the band, unaligned start, unaligned length, empty, and wrapping.
        assert!(refused(
            SERVICE_WINDOWS,
            SERVICE_WINDOWS.start - PAGE_BYTES,
            2 * PAGE_BYTES
        ));
        assert!(refused(
            SERVICE_WINDOWS,
            SERVICE_WINDOWS.start + 1,
            PAGE_BYTES
        ));
        assert!(refused(
            SERVICE_WINDOWS,
            SERVICE_WINDOWS.start,
            PAGE_BYTES + 1
        ));
        assert!(refused(SERVICE_WINDOWS, SERVICE_WINDOWS.start, 0));
        assert!(refused(
            SERVICE_WINDOWS,
            SERVICE_WINDOWS.start,
            u64::MAX - PAGE_BYTES + 1
        ));
    }

    #[test]
    fn the_band_checks_accept_their_own_band_and_nothing_else() {
        for &(name, band) in BANDS {
            let virtual_address = band.start;
            let std_panics =
                |f: fn(u64) -> u64| std::panic::catch_unwind(|| f(virtual_address)).is_err();
            assert_eq!(
                !std_panics(pair_page),
                band == PAIR_PAGES,
                "pair_page at {name}"
            );
            assert_eq!(
                !std_panics(runtime_window),
                band == RUNTIME_WINDOWS,
                "runtime_window at {name}"
            );
            assert_eq!(
                !std_panics(service_window),
                band == SERVICE_WINDOWS,
                "service_window at {name}"
            );
        }
        assert!(std::panic::catch_unwind(|| pair_page(0x60_0001)).is_err());
    }
}
