//! `std::env`'s *path* half on nife (milestone 64): the four functions that name places, and the
//! two that take a list of them apart.
//!
//! `chdir` and `current_exe` refuse with [`io::ErrorKind::Unsupported`], and `home_dir` is `None`:
//! each can say no in its own signature, and each needs something this process was not given.
//! `getcwd` answers `/`, the root of this process's namespace (the directory it was granted), or
//! `Unsupported` without a directory capability (the `is_reachable()` probe `std::fs` uses).
//! `temp_dir` and `split_paths` answer, because their signatures leave no way to refuse and
//! upstream's `unsupported` backend panics there. The rule: **fix the ones that abort, leave the
//! ones that refuse.** See notes/std/caveats.md.

//! # `temp_dir`
//!
//! `TMPDIR` first, exactly as `sys/paths/unix.rs` does, so a program given the variable is steered
//! with no change here; then `/`. `/` is where `getcwd` says you are: a process holds one directory
//! capability, so the only place a temporary file can go is where every other file goes. `/tmp`
//! loses because it names a subdirectory nothing creates (every temporary file would fail with
//! `NotFound`) and puts a Unix fiction in a string a program may print.

//! # BUGS
//!
//! - **A temporary file lands in the program's granted directory, beside its real files.** A
//!   caller that needs isolation should be given a directory of its own and use `tempfile_in`.
//! - **`tempfile` still does not work**: its platform ladder has no nife arm, so it selects
//!   `other.rs`, which returns "operation not supported".
//! - **`chdir` refuses**: the one directory capability is fixed at spawn, and moving would mean a
//!   descent handle as mutable process state, buildable and not built. Use the `_in`/`Dir` forms.
//! - **`split_paths` splits on `:` with no escaping**; `join_paths` refuses a path with a colon.

use crate::ffi::{OsStr, OsString};
use crate::path::{self, Path, PathBuf};
use crate::sys::pal::unsupported;
use crate::{fmt, io};

/// The byte that separates entries in a path list. `:`, because nife's path separator is `/`
/// (`sys/path/mod.rs` routes everything that is not Windows-shaped to its `unix` module) and a
/// list separator has to be a character a path cannot contain.
const PATH_SEPARATOR: u8 = b':';

/// `/`: the root of this process's own namespace, which is the directory it was granted.
///
/// Not an ambient namespace and not a lie: `sys/fs/nife.rs` resolves a leading `/` against exactly
/// that directory, so a program that joins onto this path opens a file it holds a capability for,
/// and one that joins its way upwards gets `InvalidFilename` rather than somebody else's file.
///
/// A process that holds no directory capability is `Unsupported`, the same answer all of
/// `std::fs` gives it. There is a difference between "you are at your root" and "you have no
/// root", and a `PathBuf` cannot carry it.
pub fn getcwd() -> io::Result<PathBuf> {
    if crate::sys::fs::nife::is_reachable() {
        Ok(PathBuf::from("/"))
    } else {
        unsupported()
    }
}

/// Refused: a process's directory capability is fixed at spawn, so `/` is the only place it can
/// be. See the `BUGS` note on what moving would take.
pub fn chdir(_p: &path::Path) -> io::Result<()> {
    unsupported()
}

/// Refused: nothing tells a nife process the path it was loaded from. The loader maps an ELF out
/// of the initial ramdisk and the program never learns a name for itself.
pub fn current_exe() -> io::Result<PathBuf> {
    unsupported()
}

/// `None`: nobody gave this program a home. Not a failure, an absence, and the same answer a Unix
/// box gives when `HOME` is unset.
pub fn home_dir() -> Option<PathBuf> {
    None
}

/// Where a temporary file goes. `TMPDIR` if the program set one, otherwise the directory it holds,
/// spelled the way [`getcwd`] spells it. See the module docs for why the answer is `/` and not
/// `/tmp`.
pub fn temp_dir() -> PathBuf {
    crate::env::var_os("TMPDIR")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The iterator over a `PATH`-shaped list, borrowing each entry from the list itself. A byte slice
/// and a cursor: nothing about splitting a list on a separator is platform-specific.
///
/// `env::split_paths_ref` (rust-lang/rust#158936) returns `None` on platforms whose list syntax
/// needs unescaping, which forces an allocation. Ours has no escaping, so it answers `Some` the way
/// Unix does, and [`SplitPaths`] is this iterator plus a copy, so there is one splitting loop.
pub struct SplitPathsRef<'a> {
    /// The bytes not yet yielded, or `None` once the last entry has been. `Some(b"")` and `None`
    /// are different states on purpose: `"a:"` has a trailing empty entry and `""` has one entry,
    /// which is what every Unix does and what `join_paths` round-trips against.
    rest: Option<&'a [u8]>,
}

pub fn split_paths_ref(unparsed: &OsStr) -> Option<SplitPathsRef<'_>> {
    Some(SplitPathsRef { rest: Some(unparsed.as_encoded_bytes()) })
}

impl<'a> Iterator for SplitPathsRef<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<&'a Path> {
        let rest = self.rest?;
        let (head, tail) = match rest.iter().position(|&b| b == PATH_SEPARATOR) {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        self.rest = tail;
        // SAFETY: `OsStr::from_encoded_bytes_unchecked` requires the bytes to have come from
        // `as_encoded_bytes` and to be split on a boundary its encoding permits. Both hold: the
        // slice is a subslice of one such buffer, and the only place it is cut is at an ASCII
        // `:`, which is a character boundary in every encoding `OsStr` uses.
        Some(Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(head) }))
    }
}

/// [`SplitPathsRef`] with each entry copied out, for the stable `env::split_paths`.
pub struct SplitPaths<'a>(SplitPathsRef<'a>);

pub fn split_paths(unparsed: &OsStr) -> SplitPaths<'_> {
    SplitPaths(SplitPathsRef { rest: Some(unparsed.as_encoded_bytes()) })
}

impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<PathBuf> {
        self.0.next().map(Path::to_path_buf)
    }
}

/// An entry contained the separator, so joining it would produce a list that reads back as two
/// entries. The same failure Unix reports, for the same reason.
#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut joined = Vec::new();
    for (i, path) in paths.enumerate() {
        let path = path.as_ref().as_encoded_bytes();
        if i > 0 {
            joined.push(PATH_SEPARATOR);
        }
        if path.contains(&PATH_SEPARATOR) {
            return Err(JoinPathsError);
        }
        joined.extend_from_slice(path);
    }
    // SAFETY: every byte came from `as_encoded_bytes`, and the only byte inserted between them is
    // an ASCII `:`, which no encoding `OsStr` uses treats as part of a multi-byte sequence.
    Ok(unsafe { OsString::from_encoded_bytes_unchecked(joined) })
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "path segment contains separator `{}`", char::from(PATH_SEPARATOR))
    }
}

impl crate::error::Error for JoinPathsError {}
