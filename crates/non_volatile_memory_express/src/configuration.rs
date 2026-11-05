//! **The Controller Configuration value** that enables the controller for this driver.
//!
//! Name: provisional. The module holding [`enabled_controller_configuration`].

/// The `CC` value that enables the controller for this driver's configuration: EN=1, CSS=0 (the
/// NVM command set), MPS=0 (4 KiB memory pages, matching the caller's frames), AMS=0
/// (round-robin), IOSQES=6 (64-byte submission entries), IOCQES=4 (16-byte completion entries).
/// The entry sizes are the specification's only defined ones and the controller checks them before
/// honoring a queue creation, which is why they are set here rather than defaulted.
///
/// Name: ratified 2026-10-25 (#2720). Refused `cc_enabled` (it returns the Controller Configuration
/// value that enables the controller, which `cc_enabled` did not say).
pub fn enabled_controller_configuration() -> u32 {
    1 | (6 << 16) | (4 << 20)
}

/// The configuration word's three fields share no bit (the enable at bit 0, IOSQES at 16:18,
/// IOCQES at 20:21), which the compiler checks here; the mutation exclusion for
/// [`enabled_controller_configuration`] names this constant. The constant-assertion lint is off
/// on purpose: the assertion is constant by construction, and the compiler checking it is its job.
///
/// Name: provisional. Renamed from `THE_CONTROLLER_CONFIGURATION_FIELDS_SHARE_NO_BIT`: no `THE_`
/// prefix (ruled 2026-11-05 UTC), and the `_DO_NOT_OVERLAP` ending the naming rule gives an
/// assertion constant.
#[allow(clippy::assertions_on_constants, clippy::eq_op, dead_code)]
const CONTROLLER_CONFIGURATION_FIELDS_DO_NOT_OVERLAP: () =
    assert!(1 + (6 << 16) + (4 << 20) == 0x0046_0001);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_controller_configuration_sets_en_and_the_queue_entry_sizes() {
        let configuration = enabled_controller_configuration();
        assert_eq!(configuration & 1, 1, "EN");
        assert_eq!(
            (configuration >> 16) & 0xf,
            6,
            "IOSQES: 64-byte submission entries, 2^6"
        );
        assert_eq!(
            (configuration >> 20) & 0xf,
            4,
            "IOCQES: 16-byte completion entries, 2^4"
        );
    }

    #[test]
    fn the_three_fields_share_no_bit_so_or_and_xor_agree() {
        // Both `|` operators in `enabled_controller_configuration` survive being mutated to `^`,
        // and this is why: EN (bit 0), IOSQES (`6 << 16`, bits 17-18) and IOCQES (`4 << 20`, bit
        // 22) never share a bit, so `|` and `^` compute the exact same `u32` for these three
        // literals. The function takes no argument, so there is no wider case to check.
        assert_eq!(1u32 & (6u32 << 16), 0);
        assert_eq!(1u32 & (4u32 << 20), 0);
        assert_eq!((6u32 << 16) & (4u32 << 20), 0);
    }
}
