//! **The reply word's error range**, in one place: see [`MAXIMUM_ERRNO`].
//!
//! Name: provisional, minted 2026-11-14 (UTC) by the lane of milestone 1090 (an outside review of `filesystem_contract`).

/// The largest error code a reply word carries: an error reply is a word in `-MAXIMUM_ERRNO..=-1`
/// and its code is the word negated, `1..=MAXIMUM_ERRNO`. Linux's bound (`MAX_ERRNO` in
/// `include/linux/err.h`), adopted before any implementer outside this project exists, because a
/// bound can be loosened later compatibly and cannot be tightened.
///
/// **The reply word's whole rule**, in one place. The first word of a reply is an [`i64`]:
///
/// - `0..=i64::MAX` is a result (a length, a handle, a packed value) and never an error.
/// - `-MAXIMUM_ERRNO..=-1` is an error, and the code is `1..=MAXIMUM_ERRNO`.
/// - Any other negative word, `i64::MIN..=-MAXIMUM_ERRNO - 1`, is a **protocol violation**. A
///   decoder reads it as [`EPROTO`], so a corrupt or hostile word is never read as success and
///   never as an arbitrary truncated code.
///
/// Name: provisional, minted 2026-11-14 (UTC) by the lane of milestone 1090 (an outside review of `filesystem_contract`).
pub const MAXIMUM_ERRNO: i32 = 4095;

/// `EPROTO` (71): what [`reply_errno`] answers for a negative word outside
/// `-MAXIMUM_ERRNO..=-1`, a reply that breaks the protocol.
///
/// Name: provisional, minted 2026-11-14 (UTC) by the lane of milestone 1090 (an outside review of `filesystem_contract`).
pub const EPROTO: i32 = 71;

/// Turn a redox/POSIX errno into a reply's first word: the negated number, so the client can invert
/// it. This is the ONE place the error convention is defined; the FS server calls it at its boundary.
///
/// # Panics
///
/// If `errno` is not in `1..=`[`MAXIMUM_ERRNO`], **in every build**. Zero would send success, a
/// negative code would send a positive result a client reads as a length, and a code past the bound
/// would be a protocol violation on arrival; each is a bug in the sender, and the sender is the
/// place that can still name it. The check is one comparison on a path that already builds a reply,
/// and a release build that skipped it would turn the same bug into a silent success on the wire.
/// Every caller passes a named errno constant or an engine errno, all far below the bound.
pub const fn reply_err(errno: i32) -> i64 {
    assert!(
        1 <= errno && errno <= MAXIMUM_ERRNO,
        "an error code is 1..=MAXIMUM_ERRNO"
    );
    -(errno as i64)
}

/// The errno behind an error reply, or `None` if the reply is a (non-negative) success. The inverse
/// of [`reply_err`], for a client turning a wire error back into an `ErrorKind`.
///
/// A word in `-MAXIMUM_ERRNO..=-1` is the code `1..=MAXIMUM_ERRNO`. Any other negative word is a
/// protocol violation and reads as [`EPROTO`]; it is never `None`, so it cannot pass for success,
/// and it never truncates (`i64::MIN` has no negation in an [`i64`]).
///
/// # Examples
///
/// One word, no flag bit: a length and an errno come out of the same register.
///
/// ```
/// use filesystem_contract::{EPROTO, reply_err, reply_errno};
///
/// // A READ that moved 4096 bytes, and a READ at end of file. Both are results.
/// assert_eq!(reply_errno(4096), None);
/// assert_eq!(reply_errno(0), None);
///
/// // ENOENT, mapped once at the server's serve loop and nowhere else.
/// let wire = reply_err(2);
/// assert_eq!(wire, -2);
/// assert_eq!(reply_errno(wire), Some(2));
///
/// // A negative word past the bound is a protocol violation, not an errno and not a success.
/// assert_eq!(reply_errno(-4096), Some(EPROTO));
/// assert_eq!(reply_errno(i64::MIN), Some(EPROTO));
/// ```
pub const fn reply_errno(r0: i64) -> Option<i32> {
    if r0 >= 0 {
        None
    } else if r0 >= -(MAXIMUM_ERRNO as i64) {
        Some((-r0) as i32)
    } else {
        Some(EPROTO)
    }
}

/// Machine-checked, because "an error word is never read as success and never truncates" is a claim
/// about every `i64`, and a test can only try the words somebody thought of.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// **Decoding is total and bounded for every word.** A non-negative word is no error, a
    /// negative one always is, and the code is in `1..=MAXIMUM_ERRNO` or is `EPROTO`.
    /// Falsification: replayable `crates/filesystem_contract/falsifications/reply.proofs.every_word_decodes_within_the_bound.patch`
    #[kani::proof]
    fn every_word_decodes_within_the_bound() {
        let word: i64 = kani::any();
        match reply_errno(word) {
            None => assert!(word >= 0),
            Some(code) => {
                assert!(word < 0);
                assert!((1..=MAXIMUM_ERRNO).contains(&code));
                if word >= -(MAXIMUM_ERRNO as i64) {
                    assert_eq!(code as i64, -word);
                } else {
                    assert_eq!(code, EPROTO);
                }
            }
        }
    }

    /// **Every code the sender may send comes back as itself.**
    /// Falsification: replayable `crates/filesystem_contract/falsifications/reply.proofs.a_sent_code_round_trips.patch`
    #[kani::proof]
    fn a_sent_code_round_trips() {
        let code: i32 = kani::any();
        kani::assume((1..=MAXIMUM_ERRNO).contains(&code));
        let word = reply_err(code);
        assert!(word < 0);
        assert_eq!(reply_errno(word), Some(code));
    }
}
