//! `std::fs` for nife (milestone 27 phase two): `File` bound to the FS-service contract
//! (DECISIONS §27, `crates/filesystem_contract`). See notes/std/fs.md and notes/std/fs-descent.md.
//!
//! There is no global namespace. A std program holds a **directory capability** (the FS-service
//! endpoint, slot 4 of the std slot convention in `pal/nife/rt.rs`), and `File::open("foo")` means
//! *"foo, under the directory I was granted"*. A `..` anywhere is refused here, before the wire,
//! with [`io::ErrorKind::InvalidFilename`]; a leading `/` is the granted directory itself
//! (`walk::count_names`). A nested path is a chain of `OPENDIR` descents (milestone 122). A program
//! granted no directory capability gets `Unsupported` from everything, as `std::net` does when the
//! `Stack` endpoint is absent.

//! # What binds
//!
//! [`File::open`] (`OPEN`), `read` (`READ`), `write` (`WRITE`), `seek`/`tell` (an offset this side
//! keeps, since the contract's read and write are positional), `file_attr`/`size` (`FSTAT`), close
//! on `Drop` (`CLOSE`), [`stat`]/[`exists`] (open, fstat, close), `create` and `create_new`
//! (`CREATE`), `truncate` and `File::set_len` (`TRUNCATE`), `read_dir`, `create_dir`,
//! `remove_file`, `remove_dir` and `rename` (`OPENDIR`, `READDIR`, `MKDIR`, `UNLINK`, `RMDIR`,
//! `RENAME`), [`copy`], [`Dir`] (`#![feature(dirfd)]`), `remove_dir_all`, `Metadata::modified`
//! (`GETMTIME`, by name only: see [`Mtime`]) and `set_times`/`set_times_nofollow` (`SETMTIME_AT`, §112).

//! # What stays Unsupported
//!
//! Each because no verb in the contract backs it: symlinks and hard links, `canonicalize`,
//! permissions, access and creation times, setting a time through an open `File`, locks, and
//! duplicating a handle (`File::try_clone`, and `Dir::try_clone` for anything but the granted
//! directory: a handle is a token the server minted, and there is no dup verb). Before adding a
//! refusal here, check the contract: several outlived the verb that ended them.
//! notes/std/fs.md has the list with reasons.

use crate::ffi::OsString;
use crate::fmt;
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::path::{Component, Path, PathBuf};
use crate::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use crate::sys::pal::nife::fsproto::{self, fs as proto};
use crate::sys::pal::nife::rt;
use crate::sys::time::{SystemTime, UNIX_EPOCH};
use crate::sys::{unsupported, unsupported_err};

// Nothing in the contract links or canonicalizes, so those keep upstream's `unsupported`
// implementations rather than a nife-shaped copy of the same refusal.
#[expect(dead_code)]
#[path = "unsupported.rs"]
mod unsupported_fs;
pub use unsupported_fs::{canonicalize, link, readlink, symlink};

/// **`std::fs::rename_noreplace` stays Unsupported** (added upstream in the 2026-10-13 nightly).
///
/// `RENAME` replaces an existing destination and has no flag to refuse one. DECISIONS §129 (whether
/// `RENAME` grows a `NOREPLACE` flag) holds that flag until a customer needs it, while recording
/// that `redoxfs_server` could check it atomically for one branch and a spare bit in `rename_dst`'s
/// word. Building it here instead, as a `stat` of the destination followed by `rename`, would be
/// two requests with another client free to create the name between them: the racy emulation §42
/// (a filesystem declares what it offers and must be truthful) refuses. So this is upstream's own
/// `unsupported` version, and wiring it is one line here once the verb grows the flag.
pub use unsupported_fs::rename_noreplace;

/// `remove_dir_all` is std's generic implementation, which needs no nife code (milestone 122).
///
/// It is written in terms of `read_dir`, `remove_file` and `remove_dir` on paths it composes with
/// `DirEntry::path`, and [`walk`] resolves those holding one directory capability per level. The
/// rights it needs at every level are `dir::REMOVE_TREE`'s (`ENUMERATE` to see, `DESCEND` to walk
/// in, `REMOVE` to take the name out), asked for one verb at a time by the walk. See
/// notes/std/fs-descent.md.
pub use crate::sys::fs::common::remove_dir_all;

/// No nife-specific fields behind `std::fs::{HomeDirs, MediaDirs}` (rust-lang/rust#158936): the
/// empty structs every non-Unix platform takes. Nothing in this system names a home or a media
/// directory, so there is nothing for an extension trait to carry; `home_dir` is `None` for the
/// same reason (`sys/paths/nife.rs`).
pub use crate::sys::fs::common::{ExtraHomeDirs, ExtraMediaDirs};

/// The FS-service endpoint: this process's entire authority over files. Naming a file over it is a
/// request the server resolves under the one directory the endpoint is bound to.
const FS: u64 = rt::FS_DIR_SLOT;

/// The page shared with the FS server, mapped by the loader alongside the grant.
const PAGE_VA: u64 = rt::FS_PAGE;

/// The contract's transfer unit: one page, so one request never moves more than this.
const PAGE: usize = fsproto::PAGE;

// One child per seam (§282 (when a file is split, the parent becomes an index)). Each takes
// `use super::*` and is re-exported whole, the shape `kernel/src/sched.rs` uses, because it is part
// of this module split across files rather than a client of it: `sys/fs/mod.rs` names `imp::File`,
// `imp::stat` and the rest. The three constants above are the handful every child reads.
mod dir;
mod errors;
mod file;
mod metadata;
mod open_options;
mod page;
mod path_operations;
mod path_request;
mod reachable;
mod read_dir;
mod walk;

pub use dir::*;
pub use errors::*;
pub use file::*;
pub use metadata::*;
pub use open_options::*;
pub use page::*;
pub use path_operations::*;
pub use path_request::*;
pub use reachable::*;
pub use read_dir::*;
pub use walk::*;
