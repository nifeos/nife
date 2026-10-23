//! **The filesystem wire protocols**: the block protocol ([`blk`]) and the file protocol ([`fs`]),
//! in one crate so the block server, the FS server, the client and the kernel-side tests share one
//! definition. The kernel routes these words as any IPC and never reads an opcode, so a new verb
//! changes this crate and its note, not the syscall surface.
//!
//! Each request is an endpoint `CALL` of two words, answered through the one-shot Reply
//! capability; bytes travel only in the region the two parties share. A reply's first word is an
//! [`i64`], a result or the negated errno ([`reply_err`]). See notes/fs-server.md for the three
//! processes and the error boundary, and notes/fs-server/file-protocol.md for each verb's shape.
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review), renamed from
//! `fs_proto`; `_contract` per calef's 2026-10-18 ruling.

#![no_std]

/// One page, in bytes, and one RedoxFS block (`redoxfs::BLOCK_SIZE`), so a block move is a page
/// move and one frame of the direct map holds exactly one block. Both channels are built from it:
/// a `blk` request moves up to [`blk::TRANSFER_BLOCKS`] of these and a file request up to
/// [`fs::TRANSFER_PAGES`]. A file read or write larger than [`fs::TRANSFER_MAX`], or a `Disk` call
/// spanning more than [`blk::TRANSFER_MAX`], is chunked by the caller.
pub const PAGE: usize = 4096;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position
/// `line_editor::proto` uses, so the two contracts read alike.
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

/// Turn a redox/POSIX errno into a reply's first word: the negated number, so the client can invert
/// it. This is the ONE place the error convention is defined; the FS server calls it at its boundary.
pub const fn reply_err(errno: i32) -> i64 {
    -(errno as i64)
}

/// The errno behind an error reply, or `None` if the reply is a (non-negative) success. The inverse
/// of [`reply_err`], for a client turning a wire error back into an `ErrorKind`.
///
/// # Examples
///
/// One word, no flag bit: a length and an errno come out of the same register.
///
/// ```
/// use filesystem_contract::{reply_err, reply_errno};
///
/// // A READ that moved 4096 bytes, and a READ at end of file. Both are results.
/// assert_eq!(reply_errno(4096), None);
/// assert_eq!(reply_errno(0), None);
///
/// // ENOENT, mapped once at the server's serve loop and nowhere else.
/// let wire = reply_err(2);
/// assert_eq!(wire, -2);
/// assert_eq!(reply_errno(wire), Some(2));
/// ```
pub const fn reply_errno(r0: i64) -> Option<i32> {
    if r0 < 0 { Some((-r0) as i32) } else { None }
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
/// meaningless. See notes/fs-server.md, "The contract is capability-shaped from birth".
pub mod fs;

/// **What a directory capability carries**: the rights ladder ([`dir::Rights`]) and its one
/// constructor for a non-root set, [`dir::Rights::attenuate`], a bitwise AND with the parent, so no
/// child carries more than its parent. A missing right answers one of three errnos, chosen for
/// what the holder learns: a naming right ([`dir::READ`], [`dir::WRITE`], [`dir::DESCEND`])
/// `ENOENT`, a mutating right [`dir::EROFS`], and [`dir::ENUMERATE`] [`dir::EPERM`]. See
/// notes/dir-capability.md.
pub mod dir;

/// **The verb table**: each [`fs`] request word's argument shape and the rights the server will
/// demand, declared once, so the three caretakers dispatch off it instead of a `match` each. A verb
/// in [`fs`] without a row is a compile error ([`verb::TABLE`]'s const assertions). It shares the
/// dispatch and never the attenuation. See notes/fs-server/verb-table.md.
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
/// no version word. See notes/fs-server/statfs.md.

/// # BUGS
///
/// - Free space is a forecast, not a reservation; see [`fs::STATFS`]'s BUGS.
/// - There is no inode count: SMB's volume classes carry none, so nothing above has asked.
pub mod statfs;

/// **The per-file grant**: holding one file rather than a directory, and how the narrowing travels.
///
/// The caretaker `components/src/fs_file_caretaker.rs` holds the directory capability, opens the
/// granted name once at startup, and serves the same [`fs`] protocol on its own endpoint:
/// [`fs::OPEN`] answers only the granted name (any other is `ENOENT`), [`fs::CREATE`] is
/// `ENOTDIR`, and [`fs::WRITE`] and [`fs::TRUNCATE`] are [`grant::EROFS`] without
/// [`grant::WRITE`]. The confined program holds nothing that names the FS server, so it cannot
/// route around the narrowing. See notes/grant-expression/per-file-grants.md.
pub mod grant;

/// **A directory capability attenuated to a set of names**, what `rm *.txt` designates, served by
/// `components/src/fs_nameset_caretaker.rs`.
///
/// A set does not fit in `START`'s argument words, so it rides in a frame the wiring maps read-only
/// into the caretaker, and [`nameset::MAX_NAMES`] is the capability's ceiling. A record is one
/// header byte, bit 7 [`nameset::IS_DIR`] and the low seven the length, then the name; a zero header
/// ends the set, so a name may not be empty. The caretaker answers `READDIR` from the set, so the
/// type is what the directory said when the grant was planned. See notes/glob-grant.md.
pub mod nameset;

/// **Extended attributes: the limits, the encodings, and the store the FS server keeps them in**,
/// a layer above RedoxFS rather than a change to its format. Every path to the disk goes through
/// [`fs`], so the layer is as authoritative as the filesystem. There is no attribute right:
/// [`fs::GETXATTR`] and [`fs::LISTXATTR`] need [`dir::READ`], [`fs::SETXATTR`] and
/// [`fs::REMOVEXATTR`] need [`dir::WRITE`]. Each attribute carries a `u32` kind the layer stores
/// and never interprets ([`xattr::RAW`] for a POSIX client). See notes/xattr.md.
///
/// **BUGS.** The kind is 31 bits, because it rides in the sign-protected half of a reply word
/// ([`xattr::reply`]); [`fs::SETXATTR`] answers `EINVAL` for a kind with its top bit set.
pub mod xattr;

/// The FS server's readiness sentinel: sent once, after it has opened the RedoxFS image over blk
/// IPC and before it serves clients. A test waiting for it tells a hang in `open` (the blk path)
/// from a hang in the serve path, and catches a booted-but-empty run. It is a word shipping
/// programs send each other, so it lives here rather than in `filesystem_fixture`, as every other
/// protocol keeps its readiness word in its own crate (`entropy_contract`, `login_contract`).
pub const READY: u64 = 0xF5_0BEEF5;

/// **A caretaker's descent was refused**, sent on the same endpoint [`READY`] would have gone
/// out on, with the errno in the second word. The caretaker then exits without serving.
///
/// Under `system_initializer` the waiter is init, the spawn service for the whole prompt, with no
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
