//! **The filesystem wire protocols**: the block protocol ([`block`]) and the file system protocol ([`file_system`]),
//! in one crate so the block server, the filesystem server, the client and the kernel-side tests
//! share one definition. The kernel routes these words as any inter-process communication and never
//! reads an opcode, so a new verb changes this crate and its note, not the system call surface.
//!
//! Each request is an endpoint `CALL` of two words, answered through the one-shot Reply
//! capability; bytes travel only in the region the two parties share. A reply's first word is an
//! [`i64`], a result or the negated errno ([`error::word`]): `-MAXIMUM_ERRNO..=-1` is an error, a
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

/// One page, in bytes, and one RedoxFS block (`redoxfs::BLOCK_BYTES`), so a block move is a page
/// move and one frame of the direct map holds exactly one block. Both channels are built from it:
/// a `blk` request moves up to [`block::TRANSFER_BLOCKS`] of these and a file request up to
/// [`file_system::TRANSFER_PAGES`]. A file read or write larger than [`file_system::MAXIMUM_TRANSFER_BYTES`], or a `Disk`
/// call spanning more than [`block::MAXIMUM_TRANSFER_BYTES`], is chunked by the caller.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const PAGE_BYTES: usize = 4096;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position
/// the other message contracts of the same system use, so the contracts read alike.
pub const OPERATION_SHIFT: u32 = 56;

/// Build a request's first word from just an opcode (the block protocol's shape: the operand, a
/// block index, rides in the second word).
///
/// Name: provisional, minted 2026-11-21 (UTC) by the root-request lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn request(operation: u64) -> u64 {
    operation << OPERATION_SHIFT
}

/// The opcode of any request word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

/// **The reply word's error range**: [`MAXIMUM_ERRNO`], [`EPROTO`], [`error::word`] and
/// [`error::code`], the one place the error convention is defined.
///
/// Name: ratified 2026-11-22.
pub mod error;
pub use error::{EPROTO, MAXIMUM_ERRNO};

/// **The block-IPC protocol** (filesystem server to block server): read blocks, write blocks, ask the
/// size, flush, shaped like the RedoxFS `Disk` trait the filesystem server implements over it. The unit is
/// one filesystem block ([`PAGE_BYTES`] bytes), and [`block::READ`] and [`block::WRITE`] carry up to
/// [`block::TRANSFER_BLOCKS`] of them. The block server moves a request as one descriptor over the
/// 512-byte virtio sectors the device transfers.
pub mod block;

/// **The file system protocol** (client to file system server). The endpoint a client holds is the
/// directory capability, bound in the server to one directory node, and every name is resolved
/// relative to a handle under it: no global namespace, no absolute path. A handle is a small
/// integer the server mints and validates against this session's table, so forging one is
/// meaningless.
///
/// The module is the standard library's `std::fs`, spelled out; `fs` finds it as an alias.
///
/// Name: ratified 2026-11-22.
#[doc(alias = "fs")]
pub mod file_system;

/// **What a directory capability carries**: the rights ladder ([`directory::Rights`]) and its one
/// constructor for a non-root set, [`directory::Rights::attenuate`], a bitwise AND with the parent, so no
/// child carries more than its parent. A missing right answers one of three errnos, chosen for
/// what the holder learns: a naming right ([`directory::rights::READ`], [`directory::rights::WRITE`], [`directory::rights::DESCEND`])
/// `ENOENT`, a mutating right [`directory::EROFS`], and [`directory::rights::ENUMERATE`] [`directory::EPERM`].
pub mod directory;

/// **The verb table**: each [`file_system`] request word's argument shape and the rights the server will
/// demand, declared once, so the three caretakers dispatch off it instead of a `match` each. A verb
/// in [`file_system`] without a row is a compile error ([`verb::TABLE`]'s const assertions). It shares the
/// dispatch and never the attenuation.
///
/// # BUGS
///
/// - A wrong row is wrong in three programs at once. It is pure data in a host-testable crate.
/// - It does not make the caretakers interchangeable: what each attenuates to stays hand-written.
pub mod verb;

/// **How [`file_system::READDIR`] packs a directory listing into the shared page.**
///
/// One record per entry: a flags byte, a length byte, then that many bytes of name. Length-prefixed
/// because a name is bytes to this contract and a terminator inside one would truncate it; one
/// length byte because RedoxFS's own limit on an entry is well under 255. The reply's `r0` is how
/// many bytes the server filled. A record is never split across replies: the server stops before one
/// that would not fit and the cursor resumes there.
pub mod directory_entry;

/// **How [`file_system::GET_VOLUME_STATISTICS`] packs the volume's numbers into the shared page.**
///
/// Three little-endian `u64`s, in order: the allocation unit in bytes, the volume's size in those
/// units, and how many are free. The unit is a field, not [`PAGE_BYTES`]. The reply's length is the
/// version: a client reads the fields it knows when `r0 >= LEN` ([`volume_statistics::decode`]), so there is
/// no version word.
///
/// # BUGS
///
/// - Free space is a forecast, not a reservation; see [`file_system::GET_VOLUME_STATISTICS`]'s BUGS.
/// - There is no inode count: Server Message Block (SMB) volume classes carry none, so nothing above has asked.
///
/// The module is the volume's statistics, as the Portable Operating System Interface (POSIX) call `statfs(2)` reports them; `statfs` finds it as an alias.
///
/// Name: ratified 2026-11-22.
#[doc(alias = "statfs")]
pub mod volume_statistics;

/// **The per-file grant**: holding one file rather than a directory, and how the narrowing travels.
///
/// The file caretaker holds the directory capability, opens the
/// granted name once at startup, and serves the same [`file_system`] protocol on its own endpoint:
/// [`file_system::OPEN`] answers only the granted name (any other is `ENOENT`), [`file_system::CREATE`] is
/// `ENOTDIR`, and [`file_system::WRITE`] and [`file_system::TRUNCATE`] are [`grant::EROFS`] without
/// [`grant::rights::WRITE`]. The confined program holds nothing that names the filesystem server, so it cannot
/// route around the narrowing.
pub mod grant;

/// **A directory capability attenuated to a set of names**, what `rm *.txt` designates, served by
/// the name-set caretaker.
///
/// A set does not fit in `START`'s argument words, so it rides in a frame the wiring maps read-only
/// into the caretaker, and [`nameset::MAXIMUM_NAMES`] and [`nameset::MAXIMUM_NAME_SET_BYTES`] are the
/// capability's ceilings. A record is a length byte, a flags byte ([`nameset::IS_DIRECTORY`]), then the
/// name; a zero length ends the set, so a name may not be empty. The caretaker answers `READDIR`
/// from the set, so the type is what the directory said when the grant was planned.
pub mod nameset;

/// **Extended attributes: the limits, the encodings, and the store the filesystem server keeps them in**,
/// a layer above RedoxFS rather than a change to its format. Every path to the disk goes through
/// [`file_system`], so the layer is as authoritative as the filesystem. There is no attribute right:
/// [`file_system::GETXATTR`] and [`file_system::LISTXATTR`] need [`directory::rights::READ`], [`file_system::SETXATTR`] and
/// [`file_system::REMOVEXATTR`] need [`directory::rights::WRITE`]. Each attribute carries a `u32` kind the layer stores
/// and never interprets ([`extended_attribute::RAW`] for a POSIX client).
///
/// **BUGS.** The kind is 31 bits, because it rides in the sign-protected half of a reply word
/// ([`extended_attribute::reply`]); [`file_system::SETXATTR`] answers `EINVAL` for a kind with its top bit set.
#[doc(alias = "xattr")]
pub mod extended_attribute;

/// The filesystem server's readiness sentinel: sent once, after it has opened the RedoxFS image over blk
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
/// Name: ratified 2026-11-22, as built; the value `0xF5_0DEAD5` is unchanged. Refused: `FAILED`, because it does not say
/// what failed or that the caretaker exits; `SUBTREE_OPEN_FAILED`, because the refusal covers any descent, a missing
/// directory or a refused right, not only opening the subtree.
pub const DESCENT_REFUSED: u64 = 0xF5_0DEAD5;

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests;
