//! **Machine-checked proofs (Kani) of the user address-space map**: every address below the
//! code-model ceiling lies in exactly one band, whole pages and in address order; each band
//! function accepts exactly the pages its doc allows; and [`check_image`] places exactly the images
//! the image band holds. Behind `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them.
//! See notes/address-space-map.md and notes/verification.md.
//!
//! The constant facts (the bands tile `[0, CODE_MODEL_CEILING)`, each page aligned, the ceiling a
//! low-half address on every architecture) are also `const` asserts in `lib.rs`, where every
//! consumer's build checks them. These harnesses state the same map per address, through
//! [`Band::contains`], which is what every band function and loader asks. So a falsification of a
//! band constant has to delete the matching `const` assert too, or the build stops before Kani runs.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by address-space-map-proven, after the module every
//! other crate's harnesses live in. calef has not named it.

use super::*;

/// **Marks the point a `#[kani::should_panic]` harness must never reach.** Kani counts every
/// assertion as a panic, `kani::assert` included, so a should-panic harness that ended in one would
/// pass even when the code under test returned. An unwinding failure is not a panic, so under
/// `#[kani::unwind(1)]` reaching this loop fails the harness: the call above it returned for an
/// input that had to be refused.
#[allow(clippy::empty_loop)]
fn refused_inputs_never_reach_here() -> ! {
    loop {}
}

/// How many bands contain `va`.
fn bands_containing(va: u64) -> usize {
    let mut n = 0;
    for (_, band) in BANDS {
        if band.contains(va) {
            n += 1;
        }
    }
    n
}

/// The index in [`BANDS`] of the band containing `va`, or [`BANDS`]'s length when none does.
fn band_index(va: u64) -> usize {
    let mut i = 0;
    while i < BANDS.len() && !BANDS[i].1.contains(va) {
        i += 1;
    }
    i
}

/// **Every address below the ceiling lies in exactly one band, and no address above it in any.**
/// The page around it lies wholly in the same band, which is what page aligned at both ends means
/// for a band at least a page wide, and its top bits are clear on all three architectures:
/// aarch64 splits the halves at bit 48, `x86_64` at bit 47, `riscv64` (Sv39) at bit 38, as
/// `LOW_HALF_SPLIT_SHIFTS` restates from `crates/paging`.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.every_address_below_the_ceiling_lies_in_exactly_one_band.patch`
#[kani::proof]
#[kani::unwind(9)]
fn every_address_below_the_ceiling_lies_in_exactly_one_band() {
    let va: u64 = kani::any();
    if va >= CODE_MODEL_CEILING {
        assert_eq!(bands_containing(va), 0);
        return;
    }
    assert_eq!(bands_containing(va), 1);
    let band = BANDS[band_index(va)].1;
    let page = va - va % PAGE;
    assert!(band.contains(page) && band.contains(page + PAGE - 1));
    for (_, shift) in LOW_HALF_SPLIT_SHIFTS {
        assert_eq!(va >> shift, 0);
    }
}

/// **[`BANDS`] is in address order**: of two addresses in the map, the lower one is never in a
/// later band. The names a message prints come from this order, and a reader takes the table as a
/// picture of the address space.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.the_bands_are_listed_in_address_order.patch`
#[kani::proof]
#[kani::unwind(9)]
fn the_bands_are_listed_in_address_order() {
    let low: u64 = kani::any();
    let high: u64 = kani::any();
    kani::assume(low < high && high < CODE_MODEL_CEILING);
    assert!(band_index(low) <= band_index(high));
}

/// The band function's contract, stated without the function: page aligned and inside `band`.
fn allowed(band: Band, va: u64) -> bool {
    va.is_multiple_of(PAGE) && va >= band.start && va < band.end
}

/// What every accepted address must satisfy: returned unchanged, its whole page inside its own
/// band, and in no other band.
fn assert_placed(band: Band, va: u64, returned: u64) {
    assert_eq!(returned, va);
    assert!(band.start <= returned && returned + PAGE <= band.end);
    assert_eq!(bands_containing(returned), 1);
}

/// **[`pair_page`] accepts every page of [`PAIR_PAGES`]**, returns it unchanged, and the page lies in
/// no other band. Neither of its asserts is reachable from an address its doc allows.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.pair_page_accepts_every_page_of_its_band.patch`
#[kani::proof]
#[kani::unwind(9)]
fn pair_page_accepts_every_page_of_its_band() {
    let va: u64 = kani::any();
    kani::assume(allowed(PAIR_PAGES, va));
    assert_placed(PAIR_PAGES, va, pair_page(va));
}

/// **[`pair_page`] refuses every address its doc forbids**: unaligned, or outside [`PAIR_PAGES`].
/// The build stops (a panic in a `const`) for each one, and for at least one.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.pair_page_refuses_every_address_its_doc_forbids.patch`
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(1)]
fn pair_page_refuses_every_address_its_doc_forbids() {
    let va: u64 = kani::any();
    kani::assume(!allowed(PAIR_PAGES, va));
    pair_page(va);
    refused_inputs_never_reach_here();
}

/// **[`runtime_window`] accepts every page of [`RUNTIME_WINDOWS`]**, returns it unchanged, and the
/// page lies in no other band.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.runtime_window_accepts_every_page_of_its_band.patch`
#[kani::proof]
#[kani::unwind(9)]
fn runtime_window_accepts_every_page_of_its_band() {
    let va: u64 = kani::any();
    kani::assume(allowed(RUNTIME_WINDOWS, va));
    assert_placed(RUNTIME_WINDOWS, va, runtime_window(va));
}

/// **[`runtime_window`] refuses every address its doc forbids**, and at least one.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.runtime_window_refuses_every_address_its_doc_forbids.patch`
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(1)]
fn runtime_window_refuses_every_address_its_doc_forbids() {
    let va: u64 = kani::any();
    kani::assume(!allowed(RUNTIME_WINDOWS, va));
    runtime_window(va);
    refused_inputs_never_reach_here();
}

/// **[`service_window`] accepts every page of [`SERVICE_WINDOWS`]**, returns it unchanged, and the
/// page lies in no other band.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.service_window_accepts_every_page_of_its_band.patch`
#[kani::proof]
#[kani::unwind(9)]
fn service_window_accepts_every_page_of_its_band() {
    let va: u64 = kani::any();
    kani::assume(allowed(SERVICE_WINDOWS, va));
    assert_placed(SERVICE_WINDOWS, va, service_window(va));
}

/// **[`service_window`] refuses every address its doc forbids**, and at least one.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.service_window_refuses_every_address_its_doc_forbids.patch`
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(1)]
fn service_window_refuses_every_address_its_doc_forbids() {
    let va: u64 = kani::any();
    kani::assume(!allowed(SERVICE_WINDOWS, va));
    service_window(va);
    refused_inputs_never_reach_here();
}

/// **[`check_image`] places exactly the images the image band holds, for every pair of bounds.** A
/// non-empty `[start, end)` inside [`IMAGE`] is placed. One that starts inside and runs past the end
/// is too large, naming its own end and the stack's base, which is the image band's end. Anything
/// else is outside the band and names its own bounds. It never panics.
/// Falsification: replayable `crates/address_space_map/falsifications/verification.check_image_places_exactly_what_the_image_band_holds.patch`
#[kani::proof]
fn check_image_places_exactly_what_the_image_band_holds() {
    let start: u64 = kani::any();
    let end: u64 = kani::any();
    let starts_inside = (IMAGE.start..IMAGE.end).contains(&start);
    let expected = if starts_inside && start < end && end <= IMAGE.end {
        Ok(())
    } else if starts_inside && end > IMAGE.end {
        Err(ImagePlacement::TooLarge {
            image_end: end,
            stack_base: IMAGE.end,
        })
    } else {
        Err(ImagePlacement::OutsideBand {
            image_start: start,
            image_end: end,
        })
    };
    assert_eq!(check_image(start, end), expected);
}
