//! **The linker script and the address-space map agree**: `link.ld`, which this crate delivers to
//! every program through `build.rs`, links at `address_space_layout::IMAGE_BASE`.
//!
//! Name: provisional. A test-only module, named for the file it checks.

use std::vec::Vec;

/// **The linker script and the map agree**, which is the one thing a reader of either most
/// needs to be true. The script is parsed, not grepped for a spelling, so a reformatted line
/// still counts and a moved base fails here with both numbers.
#[test]
fn the_user_linker_script_links_at_the_image_base() {
    let script = include_str!("../link.ld");
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
        std::vec![address_space_layout::IMAGE_BASE],
        "link.ld does not link at IMAGE_BASE"
    );
}
