//! **The filesystem wire protocols**: the block protocol ([`blk`]) and the file protocol ([`fs`]),
//! in one crate so the block server, the filesystem server, the client and the kernel-side tests
//! share one definition. The kernel routes these words as any inter-process communication and never
//! reads an opcode, so a new verb changes this crate and its note, not the syscall surface.
//!
//! Each request is an endpoint `CALL` of two words, answered through the one-shot Reply
//! capability; bytes travel only in the region the two parties share. A reply's first word is an
//! [`i64`], a result or the negated errno ([`reply_err`]): `-MAXIMUM_ERRNO..=-1` is an error, a
//! negative word outside it is a protocol violation read as `EPROTO`, and anything else is a
//! result ([`MAXIMUM_ERRNO`] has the rule whole).
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review), renamed from
//! `fs_proto`; `_contract` per calef's 2026-10-18 ruling.

#![no_std]

// Index exception (2026-11-08): the README's examples compile as doctests from the crate root, where
// the include has to sit; it is not an item a feature could grow from.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// One page, in bytes, and one RedoxFS block (`redoxfs::BLOCK_SIZE`), so a block move is a page
/// move and one frame of the direct map holds exactly one block. Both channels are built from it:
/// a `blk` request moves up to [`blk::TRANSFER_BLOCKS`] of these and a file request up to
/// [`fs::TRANSFER_PAGES`]. A file read or write larger than [`fs::TRANSFER_MAXIMUM`], or a `Disk`
/// call spanning more than [`blk::TRANSFER_MAXIMUM`], is chunked by the caller.
pub const PAGE: usize = 4096;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position
/// the other message contracts of the nife kernel use, so the contracts read alike.
pub const OPERATION_SHIFT: u32 = 56;

/// Build a request's first word from just an opcode (the block protocol's shape: the operand, a
/// block index, rides in the second word).
pub const fn req(operation: u64) -> u64 {
    operation << OPERATION_SHIFT
}

/// The opcode of any request word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

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
pub const MAXIMUM_ERRNO: i32 = 4095;

/// `EPROTO` (71): what [`reply_errno`] answers for a negative word outside
/// `-MAXIMUM_ERRNO..=-1`, a reply that breaks the protocol.
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
/// use filesystem_contract::{reply_err, reply_errno, EPROTO};
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

/// **The block-IPC protocol** (FS server to block server): read blocks, write blocks, ask the
/// size, flush, shaped like the RedoxFS `Disk` trait the FS server implements over it. The unit is
/// one filesystem block ([`PAGE`] bytes), and [`blk::READ`] and [`blk::WRITE`] carry up to
/// [`blk::TRANSFER_BLOCKS`] of them. The block server moves a request as one descriptor over the
/// 512-byte virtio sectors the device transfers.
pub mod blk;

/// **The file-service protocol** (client to FS server). The endpoint a client holds is the
/// directory capability, bound in the server to one directory node, and every name is resolved
/// relative to a handle under it: no global namespace, no absolute path. A handle is a small
/// integer the server mints and validates against this session's table, so forging one is
/// meaningless.
pub mod fs;

/// **What a directory capability carries**: the rights ladder ([`dir::Rights`]) and its one
/// constructor for a non-root set, [`dir::Rights::attenuate`], a bitwise AND with the parent, so no
/// child carries more than its parent. A missing right answers one of three errnos, chosen for
/// what the holder learns: a naming right ([`dir::READ`], [`dir::WRITE`], [`dir::DESCEND`])
/// `ENOENT`, a mutating right [`dir::EROFS`], and [`dir::ENUMERATE`] [`dir::EPERM`].
pub mod dir;

/// **The verb table**: each [`fs`] request word's argument shape and the rights the server will
/// demand, declared once, so the three caretakers dispatch off it instead of a `match` each. A verb
/// in [`fs`] without a row is a compile error ([`verb::TABLE`]'s const assertions). It shares the
/// dispatch and never the attenuation.
///
/// # BUGS
///
/// - A wrong row is wrong in three programs at once. It is pure data in a host-testable crate.
/// - It does not make the caretakers interchangeable: what each attenuates to stays hand-written.
pub mod verb;

/// **How [`fs::READDIR`] packs a directory listing into the shared page.**
///
/// One record per entry: a flags byte, a length byte, then that many bytes of name. Length-prefixed
/// because a name is bytes to this contract and a terminator inside one would truncate it; one
/// length byte because RedoxFS's own limit on an entry is well under 255. The reply's `r0` is how
/// many bytes the server filled. A record is never split across replies: the server stops before one
/// that would not fit and the cursor resumes there.
pub mod dirent;

/// **How [`fs::STATFS`] packs the volume's numbers into the shared page.**
///
/// Three little-endian `u64`s, in order: the allocation unit in bytes, the volume's size in those
/// units, and how many are free. The unit is a field, not [`PAGE`]. The reply's length is the
/// version: a client reads the fields it knows when `r0 >= LEN` ([`statfs::decode`]), so there is
/// no version word.
///
/// # BUGS
///
/// - Free space is a forecast, not a reservation; see [`fs::STATFS`]'s BUGS.
/// - There is no inode count: SMB's volume classes carry none, so nothing above has asked.
pub mod statfs;

/// **The per-file grant**: holding one file rather than a directory, and how the narrowing travels.
///
/// The file caretaker holds the directory capability, opens the
/// granted name once at startup, and serves the same [`fs`] protocol on its own endpoint:
/// [`fs::OPEN`] answers only the granted name (any other is `ENOENT`), [`fs::CREATE`] is
/// `ENOTDIR`, and [`fs::WRITE`] and [`fs::TRUNCATE`] are [`grant::EROFS`] without
/// [`grant::WRITE`]. The confined program holds nothing that names the FS server, so it cannot
/// route around the narrowing.
pub mod grant;

/// **A directory capability attenuated to a set of names**, what `rm *.txt` designates, served by
/// the name-set caretaker.
///
/// A set does not fit in `START`'s argument words, so it rides in a frame the wiring maps read-only
/// into the caretaker, and [`nameset::MAXIMUM_NAMES`] and [`nameset::NAME_BYTES`] are the
/// capability's ceilings. A record is a length byte, a flags byte ([`nameset::IS_DIR`]), then the
/// name; a zero length ends the set, so a name may not be empty. The caretaker answers `READDIR`
/// from the set, so the type is what the directory said when the grant was planned.
pub mod nameset;

/// **Extended attributes: the limits, the encodings, and the store the FS server keeps them in**,
/// a layer above RedoxFS rather than a change to its format. Every path to the disk goes through
/// [`fs`], so the layer is as authoritative as the filesystem. There is no attribute right:
/// [`fs::GETXATTR`] and [`fs::LISTXATTR`] need [`dir::READ`], [`fs::SETXATTR`] and
/// [`fs::REMOVEXATTR`] need [`dir::WRITE`]. Each attribute carries a `u32` kind the layer stores
/// and never interprets ([`xattr::RAW`] for a POSIX client).
///
/// **BUGS.** The kind is 31 bits, because it rides in the sign-protected half of a reply word
/// ([`xattr::reply`]); [`fs::SETXATTR`] answers `EINVAL` for a kind with its top bit set.
pub mod xattr;

/// The FS server's readiness sentinel: sent once, after it has opened the RedoxFS image over blk
/// IPC and before it serves clients. A test waiting for it tells a hang in `open` (the blk path)
/// from a hang in the serve path, and catches a booted-but-empty run. It is a word shipping
/// programs send each other, so it lives here rather than in a test fixture, as every other
/// protocol keeps its readiness word in its own crate.
pub const READY: u64 = 0xF5_0BEEF5;

/// **A caretaker's descent was refused**, sent on the same endpoint [`READY`] would have gone
/// out on, with the errno in the second word. The caretaker then exits without serving.
///
/// Under the system initializer the waiter is init, the spawn service for the whole prompt, with no
/// other thread: a caretaker that panicked would park it in `RECEIVE` forever. A grant naming a
/// directory that is not there is an ordinary thing to type, so it is answered with a message.
///
/// Name: provisional (2026-08-17, milestone 31 (a capability shell) phase 3): it names the one
/// step that can fail before a caretaker serves anything, which is the single `OPENDIR` its
/// whole attenuation lives in. It moved here from the fixture with [`READY`] (milestone 886).
pub const DESCENT_REFUSED: u64 = 0xF5_0DEAD5;

/// What the `rm` program reports, and how a diagnostic is told from the verdict.
pub mod rm;

#[cfg(test)]
mod tests;

/// Machine-checked, because "an error word is never read as success and never truncates" is a claim
/// about every `i64`, and a test can only try the words somebody thought of.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// **Decoding is total and bounded for every word.** A non-negative word is no error, a
    /// negative one always is, and the code is in `1..=MAXIMUM_ERRNO` or is `EPROTO`.
    /// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.every_word_decodes_within_the_bound.patch`
    #[kani::proof]
    fn every_word_decodes_within_the_bound() {
        let word: i64 = kani::any();
        match reply_errno(word) {
            None => assert!(word >= 0),
            Some(code) => {
                assert!(word < 0);
                assert!(1 <= code && code <= MAXIMUM_ERRNO);
                if word >= -(MAXIMUM_ERRNO as i64) {
                    assert_eq!(code as i64, -word);
                } else {
                    assert_eq!(code, EPROTO);
                }
            }
        }
    }

    /// **Every code the sender may send comes back as itself.**
    /// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.a_sent_code_round_trips.patch`
    #[kani::proof]
    fn a_sent_code_round_trips() {
        let code: i32 = kani::any();
        kani::assume(1 <= code && code <= MAXIMUM_ERRNO);
        let word = reply_err(code);
        assert!(word < 0);
        assert_eq!(reply_errno(word), Some(code));
    }
}
