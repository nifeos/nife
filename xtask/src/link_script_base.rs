//! **The user linker script and the address-space map agree on the image base.** The script is
//! parsed, not grepped for a spelling, so a reformatted line still counts and a moved base fails
//! with both numbers.
//!
//! The check lives here because `user_mode_runtime`, which owns the script, has no host test
//! target, and `address_space_map` must name no file outside its own repository.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane maintainer/outside-review-fixes-1075-1088.

#[cfg(test)]
mod tests {
    use address_space_map::IMAGE_BASE;

    #[test]
    fn the_user_linker_script_links_at_the_image_base() {
        let script = include_str!("../../crates/user_mode_runtime/link.ld");
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
}
