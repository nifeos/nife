//! **Fill a buffer from the entropy service**, the client half of `entropy_contract` (milestone 56
//! (secrets, credentials, and the entropy to make them safe), §44 (entropy is a capability),
//! notes/entropy.md).
//!
//! The loop, where the refusals live, is `entropy_contract::fill_with`, host-tested there; this
//! module supplies the `CALL` as a closure, so the contract does not depend on this runtime. Scoped
//! to a module because only a program granted an entropy endpoint has a use for it. See
//! notes/user-mode-runtime.md.

//! # BUGS
//!
//! - **A short answer is a refusal, not a retry.** The service answers fewer bytes than asked only
//!   when its device is dry, and these callers would rather write nothing than wait on it. The std
//!   PAL accepts a partial count and loops; the two policies differ on purpose and are recorded at
//!   both ends.

/// Fill `out` with random bytes from the entropy service whose endpoint is in `slot`, or `None` if
/// that cannot be done in full.
///
/// `None` covers both failures, which every caller so far treats alike: this process holds no
/// entropy endpoint in `slot`, or the service answered with fewer bytes than asked. On `None` the
/// contents of `out` are unspecified and must not be used. See `entropy_contract::fill_with` for
/// the loop and its tests.
///
/// Name: ratified 2026-10-07 (calef, #1795). His word: "Yes". getrandom's verb; the module already
/// says entropy.
#[must_use]
pub fn fill(slot: u64, out: &mut [u8]) -> Option<()> {
    entropy_contract::fill_with(out, |w0| super::call(slot, w0, 0))
}
