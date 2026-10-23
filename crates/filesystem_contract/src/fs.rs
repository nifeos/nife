/// **The session's bound directory, which is always handle 0.** The server installs it in the
/// handle table before it serves anything, so every name-taking verb resolves against a handle.
/// Two clients on two endpoints both say `0` and mean different directories; file handles start
/// at 1. See notes/dir-capability.md, "Handle 0 is the bound directory".
pub const ROOT: u64 = 0;

/// Open the name in the shared page ([`req_len`] bytes) under the directory handle in
/// [`req_handle`] ([`ROOT`] for the bound directory). Reply `r0` = a handle or an error, and `r1` =
/// the file's size at the open, or 0 for "not given": a hint for sizing a buffer, never an answer
/// to a later size question ([`FSTAT`]).
///
/// Needs [`super::dir::READ`] or [`super::dir::WRITE`], and the handle inherits both bits; without
/// either, `ENOENT`. `EISDIR` for a directory. A relative path `a/b/f` is walked as [`OPENDIR`]
/// hops asking for `DESCEND | w1`, `w1` being the per-step rights: a step that comes up short is
/// [`super::dir::EPERM`], and an empty step, `.` or `..` is `EINVAL`. See
/// notes/fs-server/file-protocol.md.
pub const OPEN: u64 = 1;
/// Read up to [`req_len`] bytes from the handle ([`req_handle`]) at offset `w1` into the shared
/// page. Reply `r0` = bytes read (≥ 0, 0 at EOF) into the shared page, or an error.
pub const READ: u64 = 2;
/// Write [`req_len`] bytes from the shared page to the handle at offset `w1`. Reply `r0` = bytes
/// written (≥ 0), or an error.
pub const WRITE: u64 = 3;
/// Close the handle ([`req_handle`]). Reply `r0` = 0, or an error if the handle was not open.
pub const CLOSE: u64 = 4;
/// The current size in bytes of the handle's file. Reply `r0` = size (≥ 0), or an error.
pub const FSTAT: u64 = 5;
/// Create the name in the shared page ([`req_len`] bytes) under the directory handle in
/// [`req_handle`] and open it: [`OPEN`]'s shape, second word 0. Reply `r0` = a handle, or an error.
///
/// Needs [`super::dir::CREATE`], else [`super::dir::EROFS`]. `EEXIST` if the name exists, and
/// nothing is modified: create is create, not create-or-open. See notes/fs-server/file-protocol.md.
pub const CREATE: u64 = 6;
/// Set the size of the handle's ([`req_handle`]) file to exactly `w1` bytes; [`req_len`] is 0.
/// Reply `r0` = 0, or an error.
///
/// POSIX `ftruncate` in both directions: shrinking discards the bytes past the new size, growing
/// extends with zeroes. Without the shrink a write shorter than the file would leave the old tail
/// (DECISIONS §27 (the filesystem service)). Truncating to the current size is a no-op, which
/// `std::fs::write` relies on, since it truncates unconditionally.
pub const TRUNCATE: u64 = 7;
/// **Descend**: resolve the name in the shared page under the directory handle in [`req_handle`]
/// and hand back a handle to the child directory. `w1` is the requested [`super::dir`] mask.
/// Reply `r0` = the new directory handle, or an error.
///
/// The child's rights are [`super::dir::Rights::attenuate`] of the parent's, so no descendant
/// carries a right its ancestor lacked. A short intersection is [`super::dir::EPERM`]; no
/// [`super::dir::DESCEND`] on the parent is `ENOENT`; a file is `ENOTDIR`; an empty component, `.`
/// or `..` is `EINVAL`. A relative path is walked hop by hop, each step obeying these rules and
/// asking for `DESCEND | requested`. See notes/fs-server/file-protocol.md.
pub const OPENDIR: u64 = 8;
/// **Enumerate a directory handle.** [`req_handle`] is the directory and `w1` a cursor, the index
/// of the first entry to return. Reply `r0` = bytes written into the shared page as
/// [`super::dirent`] records; 0 means the cursor is past the end. [`req_len`] is ignored.
///
/// Needs [`super::dir::ENUMERATE`]; its absence is [`super::dir::EPERM`], not an empty listing
/// (DECISIONS §42 (a filesystem declares what it offers and must be truthful)). Entries are sorted
/// by name, so a directory changed between two calls can repeat or skip a name.
pub const READDIR: u64 = 9;
/// **Make a child directory and hand back a capability to it**: [`OPENDIR`]'s shape and its
/// attenuation, so making a directory mints no more authority than finding one. Needs
/// [`super::dir::CREATE`] and [`super::dir::DESCEND`]; `EEXIST` if the name is there, for
/// [`CREATE`]'s reason.
pub const MKDIR: u64 = 10;
/// **Move a name to another directory, or to another name in the same one.** The source is
/// [`req_handle`]/[`req_len`] of the request word; the destination rides in `w1`, packed by
/// [`rename_dst`]. Both names are in the shared page, source first: `[0, req_len)`, then
/// `[req_len, req_len + dst_len)`. Reply `r0` = 0, or an error.
///
/// Atomic against concurrency and against a crash. Needs [`super::dir::REMOVE`] on the source and
/// [`super::dir::CREATE`] on the destination, else [`super::dir::EROFS`]. A destination of the same
/// kind is replaced; a file over a directory is `EISDIR`, the reverse `ENOTDIR`, a non-empty
/// destination directory `ENOTEMPTY`, and a directory moved between directories `EINVAL`. A rename
/// onto itself succeeds. See notes/dir-capability/rename.md.
pub const RENAME: u64 = 11;
/// **Remove a name from a directory: `unlink`, not revocation.** The name in the shared page
/// ([`req_len`] bytes) leaves the directory in [`req_handle`]; second word 0. Reply `r0` = 0, or an
/// error.
///
/// A handle already open on the file keeps reading; the node goes when its last name and last
/// handle are gone. Needs [`super::dir::REMOVE`], else [`super::dir::EROFS`], checked before the
/// name is resolved so a refusal says nothing about the name. A directory is
/// [`super::dir::EISDIR`]: [`RMDIR`] takes only an empty one, so no single call removes a subtree.
/// See notes/fs-server/file-protocol.md and notes/rm.md.
pub const UNLINK: u64 = 12;
/// **Remove an empty directory**, `rmdir(2)`: [`UNLINK`]'s shape, and it hands nothing back.
/// Reply `r0` = 0, or an error.
///
/// A non-empty directory is [`super::dir::ENOTEMPTY`], refused rather than emptied; `rm -r`
/// recurses in userspace (`components/src/rm.rs`), one step and one set of rights at a time. Needs
/// [`super::dir::REMOVE`] on the parent, else [`super::dir::EROFS`], checked before the name is
/// resolved. A file is `ENOTDIR`. Not revocation: a handle onto the removed directory then fails
/// with `ENOENT`. See notes/rm.md and its BUGS.
pub const RMDIR: u64 = 13;

/// **Read one extended attribute of the node a handle names**, file or directory. The name is
/// [`req_len`] bytes at the start of the shared page, and `w1` is 0. Reply `r0` =
/// [`super::xattr::reply`], packing the value's type code and length, with the value in the shared
/// page; `ENODATA` if it is not set.
///
/// Needs [`super::dir::READ`] on the handle, else `EBADF`, as [`READ`] does. See [`super::xattr`]
/// for why there is no attribute right.
pub const GETXATTR: u64 = 14;
/// **Set one extended attribute on the node a handle names.** The name is [`req_len`] bytes at the
/// start of the shared page and the value follows it; `w1` packs the value's length and type code
/// ([`super::xattr::spec`]). Reply `r0` = 0.
///
/// Create and replace are one operation, and `XATTR_CREATE`/`XATTR_REPLACE` are not offered
/// (DECISIONS §42). Needs [`super::dir::WRITE`], else [`super::dir::EROFS`]. `ERANGE` for a name the
/// store cannot hold, [`super::xattr::E2BIG`] for an over-long value, [`super::xattr::ENOSPC`] at
/// [`super::xattr::MAX_COUNT`] attributes. See notes/fs-server/file-protocol.md.
pub const SETXATTR: u64 = 15;
/// **List the attribute names on the node a handle names** (milestone 57).
///
/// [`req_len`] and the second word are both 0. Reply `r0` = how many bytes of the shared page
/// were filled with [`super::xattr::list`] records; 0 means the node has no attributes.
///
/// There is **no cursor**, unlike [`READDIR`], and that is a property rather than an omission:
/// [`super::xattr::MAX_COUNT`] names of [`super::xattr::MAX_NAME`] bytes fit one page exactly,
/// so a listing is always complete in one reply and cannot be observed half-changed.
///
/// Needs [`super::dir::READ`], refused with `EBADF`, for [`GETXATTR`]'s reason.
pub const LISTXATTR: u64 = 16;
/// **Remove one extended attribute from the node a handle names** (milestone 57).
///
/// [`GETXATTR`]'s shape: the name is [`req_len`] bytes at the start of the shared page, the
/// second word is 0. Reply `r0` = 0, or `ENODATA` if the attribute was not set. Removing an
/// attribute that is not there is an **error, not a no-op**, because the caller asked about a
/// specific thing and "it was not there" is the answer.
///
/// Needs [`super::dir::WRITE`], refused with [`super::dir::EROFS`].
pub const REMOVEXATTR: u64 = 17;

/// **How much room the store behind this capability has**: POSIX `statfs`, cut to three numbers.
/// [`req_len`] and `w1` are 0; [`req_handle`] is any handle this server minted, [`ROOT`] included,
/// and holding one is the whole qualification: it needs no right. Reply `r0` = the bytes of the
/// shared page filled ([`super::statfs::LEN`]) with the [`super::statfs`] record, or an error;
/// `EBADF` for a handle the server did not mint. See notes/fs-server/statfs.md.

/// # BUGS
///
/// - It answers about the whole image, never a subtree; there are no quotas, so no smaller number
///   would be true.
/// - Two subtrees on one image can signal through the free count; isolation needs two images
///   (notes/fs-server/file-protocol.md).
/// - The numbers are a forecast: the store's `ENOSPC` at write time decides.
pub const STATFS: u64 = 18;

/// **Make everything this server has acknowledged durable** (milestone 55 (Time Machine)).
/// [`req_len`] and `w1` are 0; [`req_handle`] is any handle this server minted. Reply `r0` = how
/// many device flushes the block server has completed since it started, this one included, so a
/// success is `>= 1`; or an error, `EBADF` for a handle the server did not mint.
///
/// The FS server commits each write before replying, so after a successful `SYNC` every
/// acknowledged write is on the medium the device calls durable. Needs [`super::dir::WRITE`], else
/// [`super::dir::EROFS`]: a flush stalls the queue for every client, and a holder that may not write
/// has nothing to make durable. The block server's `EOPNOTSUPP` passes through unchanged. See
/// notes/fs-server/file-protocol.md.

/// # BUGS
///
/// - It syncs the whole device, never a subtree or a file.
/// - It does not fence: a write and a sync issued at once on two handles are unordered.
/// - The count is a fact about the device, so two clients of one image can watch each other sync.
pub const SYNC: u64 = 19;

/// **The modification time of `name`, in Unix seconds** (DECISIONS §112 (`touch`'s two behaviors
/// need two rights)). [`UNLINK`]'s shape: the name is [`req_len`] bytes at the start of the shared
/// page, resolved directly under [`req_handle`], `w1` 0. Reply `r0` = the mtime, or an error.
///
/// Name-taking, as the setters are, because `touch` never opens what it acts on. Needs
/// [`super::dir::READ`], a naming right, so its absence is `ENOENT`, checked before the name is
/// resolved. A directory answers as a file does. See notes/touch.md.
///
/// Name: provisional (milestone 47 (navigation and naming)'s mtime lane, 2026-08-24); an architect's
/// to ratify.
pub const GETMTIME: u64 = 20;

/// **Set `name`'s modification time to now** (DECISIONS §112). [`GETMTIME`]'s shape, `w1` 0.
/// Reply `r0` = 0, or an error.
///
/// "Now" is this server's own advancing logical clock, the one that stamps every [`WRITE`], not
/// the wall clock, so two `touch`es in order see increasing mtimes. Needs [`super::dir::WRITE`],
/// else [`super::dir::EROFS`], checked before the name is resolved: the value is one the server
/// observed, so recording it needs no more than writing the file. See notes/touch.md, whose BUGS
/// has the missing wall clock.
///
/// Name: provisional (milestone 47's mtime lane, 2026-08-24); an architect's to ratify.
pub const SETMTIME: u64 = 21;

/// **Set `name`'s modification time to a value the caller supplies** (`touch -t`). [`GETMTIME`]'s
/// shape, with the Unix-seconds value in `w1`. Reply `r0` = 0, or an error.
///
/// Needs [`super::dir::WRITE`] and [`super::dir::SETTIME`], else [`super::dir::EROFS`], checked
/// before the name is resolved: the caller asserts this value, and DECISIONS §112 keeps that
/// authority out of `WRITE`. Any `u64` is accepted. See notes/touch.md.
///
/// Name: provisional (milestone 47's mtime lane, 2026-08-24); an architect's to ratify.
pub const SETMTIME_AT: u64 = 22;

/// **Bind a badge to a directory: a subtree grant the server enforces itself** (milestone 606 (a
/// directory walk costs what it does on Linux), ruling D). [`req_handle`] is a directory handle the
/// caller holds, `w1` a badge (a client window's index, under §230 (badged endpoint
/// capabilities)). A request carrying that badge then sees the directory as its [`ROOT`] and
/// reaches only handles it minted (`subtree_scope::Bindings::bind`). Reply `r0` = 0, or `EPERM`
/// when the caller's own badge is bound, the badge is 0, or it is bound already.
///
/// Outside [`super::verb::TABLE`]'s range, so no caretaker forwards it. A server that does not
/// resolve through `subtree_scope` answers `EINVAL`. See notes/fs-server/file-protocol.md. Name and
/// number provisional; the number sits above the directory verbs on purpose.
pub const BIND: u64 = 64;
/// **Take a badge's grant back** ([`BIND`]'s inverse). The second word is the badge; the handle
/// field is ignored. The server closes the grant's root and every handle the badge minted, and
/// the badge reaches nothing from then on: it never falls back to the endpoint's whole
/// authority. Same caller rule as [`BIND`]. Reply `r0` = 0 or an error.
///
/// This is revocation for a server-enforced grant. The kernel has no way to take a capability
/// back out of a client's table, so the client keeps an endpoint that answers `EBADF`, where a
/// caretaker grant's revocation ended the caretaker instead.
pub const UNBIND: u64 = 65;

/// **Make a badge sync-only** (DECISIONS §251 (restarting the machine is a kernel object the
/// progenitor hands out)). `w1` is the badge; the handle field is ignored. A request carrying it may
/// then be [`SYNC`] and nothing else, answered as any writer's; every other verb is `EPERM` before
/// a handle is read. [`BIND`]'s caller rule and [`UNBIND`]'s revocation apply. The one holder is
/// `reboot`, which syncs before it restarts the machine. Outside [`super::verb::TABLE`]'s range,
/// for [`BIND`]'s reason.
///
/// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
/// request and its capability, and flush for the device cache only."). The number is provisional.
pub const BIND_SYNC: u64 = 66;

/// **How many contiguous pages the file channel spans**, so one [`READ`] or [`WRITE`] carries up
/// to [`TRANSFER_MAX`] bytes. Sixteen, so 64 KiB: a tuning number, and this is where it changes.
///
/// The FS server maps all of them. A client maps as many as it intends to use and asks for no
/// more, so one that moves 4096 bytes maps one page and needs no change. Nothing checks that a
/// client asked for no more than it mapped: one that sends `len = TRANSFER_MAX` with one page
/// mapped faults on its own second page, after the server has done the work. See
/// notes/fs-server/file-protocol.md.
pub const TRANSFER_PAGES: usize = 16;

/// **The largest payload one [`READ`] or [`WRITE`] may carry**, in bytes: the whole file
/// channel ([`TRANSFER_PAGES`] pages). The FS server clamps every request's length to this,
/// which is the one bound that keeps a request inside the region it shares.
///
/// Setting [`TRANSFER_PAGES`] to 1 makes this [`super::PAGE`], the one-page contract, which is
/// what `bench/transfer-size-sweep.sh` uses to measure one against the other on one harness.
///
/// # Examples
///
/// A bulk request is the same word with a bigger length, and a one-page client is untouched.
///
/// ```
/// use filesystem_contract::{PAGE, fs};
///
/// // The largest read this contract permits, in one request.
/// let w0 = fs::req(fs::READ, 3, fs::TRANSFER_MAX as u64);
/// assert_eq!(fs::req_len(w0), fs::TRANSFER_MAX);
/// assert_eq!(fs::TRANSFER_MAX, fs::TRANSFER_PAGES * PAGE);
///
/// // A client that shares only the first page keeps working untouched: it asks for one page,
/// // and one page is what the region behind its request has always been.
/// let w0 = fs::req(fs::READ, 3, PAGE as u64);
/// assert_eq!(fs::req_len(w0), PAGE);
/// ```
pub const TRANSFER_MAX: usize = TRANSFER_PAGES * super::PAGE;

/// **How many client channels one file service keeps side by side** (milestone 599 (a frame per
/// filesystem client channel)). Each is its own [`TRANSFER_MAX`]-byte window backed by its own
/// frame, and the server picks window `badge` for each request (`abi::rendezvous::BADGE`); window 0
/// is the unbadged default. It bounds how many file clients a boot runs at once, and costs the
/// server `CLIENT_WINDOWS * TRANSFER_MAX` = 512 KiB of mapped frames. Eight is provisional, a
/// capacity decision not yet made. See notes/fs-server/file-protocol.md.
pub const CLIENT_WINDOWS: usize = 8;

/// The largest length or offset that fits the packing below (40 bits). Far above
/// [`TRANSFER_MAX`], so the bit-packing has never been what bounds a transfer; the bound only
/// guards the packing itself, and [`TRANSFER_MAX`] is what bounds a payload.
pub const MAX_LEN: u64 = (1 << 40) - 1;

/// **The channel's arithmetic, checked by the build rather than by a test**, because a test does
/// not run in the EL0 build and a build is the only artifact that can be wrong about this.
///
/// Both halves have a failure mode worth naming. A channel that is not a whole number of pages
/// cannot be wired at all, since a mapping is per page. And a length the packing truncates would
/// arrive at the server as a **smaller successful** read rather than as an error, which is
/// DECISIONS §27's half-working write wearing a different hat.
const _: () = {
    assert!(TRANSFER_PAGES >= 1);
    assert!(TRANSFER_MAX == TRANSFER_PAGES * super::PAGE);
    assert!(TRANSFER_MAX as u64 <= MAX_LEN);
};
/// The largest handle the packing can carry (16 bits). The server's table is far smaller.
pub const MAX_HANDLE: u64 = 0xffff;

/// Pack a file request's first word: opcode (bits 63:56), handle (bits 55:40), length (bits
/// 39:0). [`OPEN`] and [`CREATE`] pass handle 0 and length = the name length;
/// [`READ`]/[`WRITE`] pass the handle and the byte count; [`CLOSE`]/[`FSTAT`]/[`TRUNCATE`] pass
/// the handle and length 0 ([`TRUNCATE`]'s new size rides in the second word).
///
/// # Examples
///
/// Control needs no shared page; the page is for the bytes.
///
/// ```
/// use filesystem_contract::{fs, operation};
///
/// // `read(handle 3, 1024)`.
/// let w0 = fs::req(fs::READ, 3, 1024);
/// assert_eq!(operation(w0), fs::READ); // the opcode field is the same one in both protocols
/// assert_eq!(fs::req_handle(w0), 3);
/// assert_eq!(fs::req_len(w0), 1024);
///
/// // An OPEN sends handle ROOT and the *name's* length; the name itself is in the page.
/// let w0 = fs::req(fs::OPEN, fs::ROOT, b"report.txt".len() as u64);
/// assert_eq!(fs::req_handle(w0), fs::ROOT);
/// assert_eq!(fs::req_len(w0), 10);
/// ```
pub const fn req(operation: u64, handle: u64, len: u64) -> u64 {
    (operation << super::OPERATION_SHIFT) | ((handle & MAX_HANDLE) << 40) | (len & MAX_LEN)
}
/// The handle of a file request word.
pub const fn req_handle(w0: u64) -> u64 {
    (w0 >> 40) & MAX_HANDLE
}
/// The length/count of a file request word.
pub const fn req_len(w0: u64) -> usize {
    (w0 & MAX_LEN) as usize
}

/// Pack [`RENAME`]'s **destination** half into the second word: the same handle and length
/// fields [`req`] uses, minus the opcode, because a rename names two directories and one word
/// cannot carry both. Read back with [`dst_handle`] and [`dst_len`].
pub const fn rename_dst(handle: u64, len: u64) -> u64 {
    ((handle & MAX_HANDLE) << 40) | (len & MAX_LEN)
}
/// The destination directory handle of a [`RENAME`]'s second word.
pub const fn dst_handle(w1: u64) -> u64 {
    (w1 >> 40) & MAX_HANDLE
}
/// The destination name's length, in bytes, of a [`RENAME`]'s second word.
pub const fn dst_len(w1: u64) -> usize {
    (w1 & MAX_LEN) as usize
}

#[cfg(test)]
mod tests;
