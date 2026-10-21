//! **The user address-space map**: which band of a process's low half holds what, the same on all
//! three architectures (§171 (where a program image starts), option D).
//!
//! ```text
//!   band              start         end           size       what goes in it
//!   NULL_GUARD        0x0000_0000   0x0020_0000     2 MiB    nothing, ever
//!   PAIR_PAGES        0x0020_0000   0x1000_0000   254 MiB    pages one builder agrees with the one program it builds
//!   RUNTIME_WINDOWS   0x1000_0000   0x4000_0000   768 MiB    windows a loader or runtime places for a program
//!   HEAP              0x4000_0000   0x5000_0000   256 MiB    the heap, and nothing else
//!   SERVICE_WINDOWS   0x5000_0000   0x6000_0000   256 MiB    large shared windows a service contract names
//!   IMAGE             0x6000_0000   0x7F00_0000   496 MiB    the program's own ELF segments
//!   STACK             0x7F00_0000   0x7FFF_0000   ~16 MiB    the stack, growing down from STACK_TOP
//!   PROCESS_PAGES     0x7FFF_0000   0x8000_0000    64 KiB    per-process pages the kernel maps (current CPU)
//! ```
//!
//! The bands tile `[0, 2 GiB)` with no gap (a host test holds that); above 2 GiB only the timebase
//! page (`counter_frequency_contract::PAGE_VA`) is assigned. A new page goes in the first band that
//! fits: the program's code or data, [`IMAGE`] (a hand-built child's code page at [`IMAGE_BASE`]
//! too); the stack, [`STACK`]; a page the kernel maps into every process unasked,
//! [`PROCESS_PAGES`], sharing the stack's page tables; the heap, [`HEAP`]; a window a loader or
//! runtime places for programs that did not choose it, [`RUNTIME_WINDOWS`], sited with an argument
//! in its protocol crate; a large window a service contract names, [`SERVICE_WINDOWS`]; otherwise
//! a pair page, [`PAIR_PAGES`], which pairs reuse freely and [`pair_page`] checks at compile time.
//!
//! The image band ends at 2 GiB because `x86_64` builds with `code-model: small` and static
//! relocation, so a static image must end below `0x8000_0000` ([`CODE_MODEL_CEILING`]); one layout
//! for all three is parity. It starts past the heap and the block channel, whose addresses std's
//! PAL carries. The stack sits directly above it, so a loader can name the stack's base as the
//! limit, and [`STACK`]'s lowest page is never mapped ([`MAX_STACK_PAGES`]). The process pages
//! share the stack's 2 MiB table: `notes/benchmarks/spawn-el0.md` measured the current-CPU page
//! at 1,245 ticks per spawn with three fresh tables and 741 with one.
//!
//! # BUGS
//!
//! - Pair pages are checked for membership, not for collisions within one process; fixtures with
//!   several windows assert their own disjointness (`components/src/swish.rs`).
//! - `std_runtime_contract` and `counter_frequency_contract` restate their numbers, pinned by tests.
//! - The userspace loader reports a too-large image as "could not spawn", not [`check_image`]'s why.
//!
//! Name: provisional: a map of one address space; `user_layout` and `address_space` were set aside.

#![cfg_attr(not(test), no_std)]

/// The page size every loader maps in, on all three architectures.
pub const PAGE: u64 = 4096;

/// **A half-open range of user virtual addresses**, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// The first address in the band.
    pub start: u64,
    /// The first address past it.
    pub end: u64,
}

impl Band {
    /// The band's width in bytes.
    pub const fn bytes(&self) -> u64 {
        self.end - self.start
    }

    /// Is `va` inside the band?
    pub const fn contains(&self, va: u64) -> bool {
        va >= self.start && va < self.end
    }

    /// Does the whole of `[start, end)` lie inside the band? An empty or inverted range does not.
    pub const fn holds(&self, start: u64, end: u64) -> bool {
        start < end && start >= self.start && end <= self.end
    }
}

/// **The highest address a static `x86_64` image may reach.** See the crate docs: `code-model:
/// small` with a static relocation model puts every symbol under 2 GiB. The image band ends at or
/// below this, and every band on the map lies under it.
pub const CODE_MODEL_CEILING: u64 = 0x8000_0000;

/// **Nothing is ever mapped here**, so a null pointer, and a null pointer plus any offset under 2
/// MiB, faults instead of reading something.
pub const NULL_GUARD: Band = Band {
    start: 0,
    end: 0x20_0000,
};

/// **Pages one builder agrees with the one program it builds.** See question 7 in the crate docs.
/// The kernel test harnesses, the fixtures, `hello`'s roles and the progenitor's native children
/// all place their shared windows here, and pairs reuse addresses freely. A program's own private
/// windows go here as well.
pub const PAIR_PAGES: Band = Band {
    start: NULL_GUARD.end,
    end: 0x1000_0000,
};

/// **Windows a loader or runtime places on a program's behalf**: std's runtime pages
/// (`std_runtime_contract`), std's per-socket frames, a builder's scratch cursor, the initial
/// ramdisk window the kernel hands the progenitor, the swap image window.
pub const RUNTIME_WINDOWS: Band = Band {
    start: PAIR_PAGES.end,
    end: 0x4000_0000,
};

/// **The heap.** `std_runtime_contract::HEAP_BASE` and `memory_region_heap::DEFAULT_BASE` are
/// its start, and std's `HEAP_MAX` is its width. A program with no heap still leaves it empty.
pub const HEAP: Band = Band {
    start: RUNTIME_WINDOWS.end,
    end: 0x5000_0000,
};

/// **Large windows a service contract names**, which several programs speaking one protocol share:
/// the block channel (`0x5000_0000`, 8 MiB, used by the filesystem server, the installer, the disk
/// partitioner and `mkfs`), the C seam's grant pages, the screen aperture.
pub const SERVICE_WINDOWS: Band = Band {
    start: HEAP.end,
    end: 0x6000_0000,
};

/// **The program's own ELF segments.** `crates/user_mode_runtime/link.ld` links at [`IMAGE_BASE`],
/// and a host test here holds the two to the same number.
pub const IMAGE: Band = Band {
    start: SERVICE_WINDOWS.end,
    end: 0x7F00_0000,
};

/// **The stack**, growing down from [`STACK_TOP`]. Its lowest page is never mapped.
pub const STACK: Band = Band {
    start: IMAGE.end,
    end: 0x7FFF_0000,
};

/// **Pages the kernel maps into every process unasked**, placed beside the stack so they share its
/// page tables. Sixteen pages, of which the current-CPU page uses the last.
pub const PROCESS_PAGES: Band = Band {
    start: STACK.end,
    end: CODE_MODEL_CEILING,
};

/// Every band, in address order, with the name a message or a table prints.
pub const BANDS: [(&str, Band); 8] = [
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
pub const IMAGE_BASE: u64 = IMAGE.start;

/// **The stack pointer a thread starts with.** Stacks grow down, so this is one past the highest
/// stack byte. Was `kernel::user::USER_STACK_TOP` and `supervision_protocol::CHILD_STACK_VA + PAGE`,
/// both `0x50_1000`.
pub const STACK_TOP: u64 = STACK.end;

/// **The highest stack page**: the one page every loader maps, and the page below which a program
/// given a deeper stack gets the rest. Was `kernel::user::USER_STACK_VA` and
/// `supervision_protocol::CHILD_STACK_VA`, both `0x50_0000`.
pub const STACK_TOP_PAGE: u64 = STACK_TOP - PAGE;

/// **The most stack pages a loader may map**, down from [`STACK_TOP_PAGE`]. One fewer than the band
/// holds: the lowest page is a guard, so a stack at its limit faults before it can touch the last
/// page of a maximal image.
pub const MAX_STACK_PAGES: u64 = STACK.bytes() / PAGE - 1;

/// **Where the kernel maps the current-CPU page** (`current_cpu_contract::PAGE_VA`): the last page
/// of [`PROCESS_PAGES`], in the same 2 MiB page-table window as [`STACK_TOP_PAGE`].
pub const CURRENT_CPU_PAGE: u64 = PROCESS_PAGES.end - PAGE;

/// **A pair page's address, checked against the map at compile time.** Use it in a `const`:
/// `const CTL_VA: u64 = pair_page(0x60_0000);`. The value is unchanged, so the address still reads
/// in the source; what the call adds is that an address outside [`PAIR_PAGES`], or not page
/// aligned, stops the build instead of colliding with a runtime page at run time.
pub const fn pair_page(va: u64) -> u64 {
    assert!(va.is_multiple_of(PAGE), "a pair page must be page aligned");
    assert!(
        PAIR_PAGES.contains(va),
        "a pair page must lie in PAIR_PAGES (address_space_map)"
    );
    va
}

/// **A runtime window's address, checked against the map at compile time.** As [`pair_page`], for
/// [`RUNTIME_WINDOWS`].
pub const fn runtime_window(va: u64) -> u64 {
    assert!(
        va.is_multiple_of(PAGE),
        "a runtime window must be page aligned"
    );
    assert!(
        RUNTIME_WINDOWS.contains(va),
        "a runtime window must lie in RUNTIME_WINDOWS (address_space_map)"
    );
    va
}

/// **A service window's address, checked against the map at compile time.** As [`pair_page`], for
/// [`SERVICE_WINDOWS`].
pub const fn service_window(va: u64) -> u64 {
    assert!(
        va.is_multiple_of(PAGE),
        "a service window must be page aligned"
    );
    assert!(
        SERVICE_WINDOWS.contains(va),
        "a service window must lie in SERVICE_WINDOWS (address_space_map)"
    );
    va
}

/// **Why an image does not fit the map.** Both variants name the numbers a reader needs to fix the
/// program, which is the point: the loader used to report the first page it could not map, as
/// `AlreadyMapped`, and that named an overlap rather than a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePlacement {
    /// The image starts in [`IMAGE`] and runs past its end, into the stack band.
    TooLarge {
        /// One past the image's last byte, rounded up to a page.
        image_end: u64,
        /// The lowest address the stack band reaches, which is where the image had to stop.
        stack_base: u64,
    },
    /// The image does not start in [`IMAGE`]: it was linked for another layout, most likely the old
    /// one at `0x40_0000`, and would land among other bands' pages.
    OutsideBand {
        /// The image's first byte, rounded down to a page.
        image_start: u64,
        /// One past its last byte, rounded up to a page.
        image_end: u64,
    },
}

impl core::fmt::Display for ImagePlacement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            ImagePlacement::TooLarge {
                image_end,
                stack_base,
            } => write!(
                f,
                "the program image is too large: it ends at {image_end:#x}, past the stack's base at \
                 {stack_base:#x}; an image may be at most {} MiB",
                IMAGE.bytes() >> 20
            ),
            ImagePlacement::OutsideBand {
                image_start,
                image_end,
            } => write!(
                f,
                "the program image at {image_start:#x}..{image_end:#x} is outside the image band \
                 {:#x}..{:#x}; relink it at {IMAGE_BASE:#x}",
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
/// use address_space_map::{check_image, pair_page, ImagePlacement, IMAGE, STACK};
///
/// // A fixture's shared page is a pair page, checked at compile time.
/// const CTL_VA: u64 = pair_page(0x60_0000);
/// assert_eq!(CTL_VA, 0x60_0000);
///
/// // An image that fits is placed; one that runs into the stack is told so, in those words.
/// assert!(check_image(IMAGE.start, IMAGE.start + 0x29_c000).is_ok());
/// let e = check_image(IMAGE.start, IMAGE.end + 0x1000).unwrap_err();
/// assert_eq!(e, ImagePlacement::TooLarge { image_end: IMAGE.end + 0x1000, stack_base: STACK.start });
/// ```
pub const fn check_image(start: u64, end: u64) -> Result<(), ImagePlacement> {
    if IMAGE.holds(start, end) {
        Ok(())
    } else if IMAGE.contains(start) && end > IMAGE.end {
        Err(ImagePlacement::TooLarge {
            image_end: end,
            stack_base: STACK.start,
        })
    } else {
        Err(ImagePlacement::OutsideBand {
            image_start: start,
            image_end: end,
        })
    }
}

// The derived constants, checked where every consumer compiles them.
const _: () = assert!(IMAGE_BASE.is_multiple_of(2 * 1024 * 1024));
const _: () = assert!(STACK_TOP_PAGE >> 21 == CURRENT_CPU_PAGE >> 21);
const _: () = assert!(IMAGE.end <= CODE_MODEL_CEILING);

#[cfg(test)]
mod tests {
    use super::*;

    /// Sv39's low half is the smallest of the three: every user address sits under
    /// `0x40_0000_0000` there (`crates/paging/src/sv39.rs`).
    const SV39_LOW_HALF_END: u64 = 0x40_0000_0000;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_bands_tile_the_first_two_gigabytes_in_order_with_no_gap() {
        let mut at = 0;
        for (name, band) in BANDS {
            assert_eq!(
                band.start, at,
                "{name} does not start where the band below it ends"
            );
            assert!(band.end > band.start, "{name} is empty");
            assert_eq!(band.start % PAGE, 0, "{name} is not page aligned");
            assert_eq!(band.end % PAGE, 0, "{name} does not end on a page");
            at = band.end;
        }
        assert_eq!(at, CODE_MODEL_CEILING);
        assert!(CODE_MODEL_CEILING <= SV39_LOW_HALF_END);
    }

    /// **The linker script and the map agree**, which is the one thing a reader of either most
    /// needs to be true. The script is parsed, not grepped for a spelling, so a reformatted line
    /// still counts and a moved base fails here with both numbers.
    #[test]
    fn the_user_linker_script_links_at_the_image_base() {
        let script = include_str!("../../user_mode_runtime/link.ld");
        let bases: Vec<u64> = script
            .lines()
            .map(str::trim)
            // The base is the one `. =` that sets a number; `. = ALIGN(..)` lines only round up.
            .filter(|l| l.starts_with(". =") && !l.contains("ALIGN"))
            .map(|l| {
                let hex = l
                    .trim_start_matches(". =")
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .trim_start_matches("0x")
                    .replace('_', "");
                u64::from_str_radix(&hex, 16).expect("the linker script's `. =` is not hex")
            })
            .collect();
        assert_eq!(
            bases,
            vec![IMAGE_BASE],
            "crates/user_mode_runtime/link.ld does not link at IMAGE_BASE"
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn an_image_the_size_of_ripgrep_fits_and_the_old_ceiling_is_gone() {
        // Milestone 121 measured ripgrep's image at 0x40_0000..0x69c_000.
        let ripgrep = 0x69c_000 - 0x40_0000;
        assert!(check_image(IMAGE_BASE, IMAGE_BASE + ripgrep).is_ok());
        assert!(
            IMAGE.bytes() / ripgrep >= 100,
            "less than 100x headroom over ripgrep"
        );
        assert!(IMAGE.bytes() > 896 * 1024);
    }

    #[test]
    fn an_image_past_the_band_is_too_large_and_names_the_stack_base() {
        let end = IMAGE.end + PAGE;
        assert_eq!(
            check_image(IMAGE_BASE, end),
            Err(ImagePlacement::TooLarge {
                image_end: end,
                stack_base: STACK.start
            })
        );
        let text = format!("{}", check_image(IMAGE_BASE, end).unwrap_err());
        assert!(text.contains("too large"), "{text}");
        assert!(text.contains(&format!("{end:#x}")), "{text}");
        assert!(text.contains(&format!("{:#x}", STACK.start)), "{text}");
        // Exactly full is not too large.
        assert!(check_image(IMAGE_BASE, IMAGE.end).is_ok());
    }

    #[test]
    fn an_image_linked_for_the_old_layout_is_outside_the_band() {
        assert_eq!(
            check_image(0x40_0000, 0x4e_0000),
            Err(ImagePlacement::OutsideBand {
                image_start: 0x40_0000,
                image_end: 0x4e_0000
            })
        );
        // And so is one that starts below the band and reaches into it.
        assert!(matches!(
            check_image(IMAGE_BASE - PAGE, IMAGE_BASE + PAGE),
            Err(ImagePlacement::OutsideBand { .. })
        ));
        // An empty image is refused rather than trivially placed, and as misplaced, not as large.
        assert_eq!(
            check_image(IMAGE_BASE, IMAGE_BASE),
            Err(ImagePlacement::OutsideBand {
                image_start: IMAGE_BASE,
                image_end: IMAGE_BASE
            })
        );
        // An image that starts below the band and ends past it is outside it; only one that starts
        // inside the band and overruns the far edge is too large.
        assert_eq!(
            check_image(IMAGE_BASE - PAGE, IMAGE.end + PAGE),
            Err(ImagePlacement::OutsideBand {
                image_start: IMAGE_BASE - PAGE,
                image_end: IMAGE.end + PAGE
            })
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_stack_keeps_a_guard_page_between_it_and_a_full_image() {
        let lowest = STACK_TOP_PAGE - (MAX_STACK_PAGES - 1) * PAGE;
        assert_eq!(lowest, STACK.start + PAGE);
        assert!(IMAGE.end < lowest);
        // Linux's default main-thread stack, 8 MiB, fits.
        assert!(MAX_STACK_PAGES * PAGE >= 8 << 20);
    }

    #[test]
    fn the_band_checks_accept_their_own_band_and_nothing_else() {
        for (name, band) in BANDS {
            let va = band.start;
            let std_panics = |f: fn(u64) -> u64| std::panic::catch_unwind(|| f(va)).is_err();
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
