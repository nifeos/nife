//! **Paths: a chain of names under the granted directory**.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// Check every component of `path` and count the names that survive, or refuse the path.
///
/// This is where "no global namespace" is enforced on the client side, before a byte reaches the
/// server (which enforces it again), so a would-be escape is a legible `io::Error` rather than an
/// `ENOENT` that reads like a missing file. Every refusal is [`io::ErrorKind::InvalidFilename`],
/// never `PermissionDenied`: no permission was consulted, and no capability designates what was
/// asked. Refused, each with its own message: `..` at any position, a Windows-shaped prefix, and a
/// name that is not UTF-8 or is longer than the page. A nested path is a chain of names [`walk`]
/// descends (milestone 122), and a leading `/` is the root of *this process's* namespace
/// (milestone 47), so `/motd` and `motd` name one file. See notes/std/fs.md.
pub(super) fn count_names(path: &Path) -> io::Result<usize> {
    let mut n = 0;
    for c in path.components() {
        match c {
            // "./motd" is "motd", and "a/./b" is "a/b": the current directory IS the one the walk
            // has reached.
            Component::CurDir => {}
            // `/motd` is `motd`: this process has one root, the directory it was granted, so the
            // slash selects nothing and a path built from `current_dir()` resolves. The slash is
            // relative to the base the walk was given, a deliberate divergence from `openat`:
            // `Dir::open_file("/x")` resolves under that `Dir`, because a handle is its own
            // namespace (notes/std/fs.md). That widens nothing, since a process that holds a `Dir`
            // holds the root it descended from.
            Component::RootDir => {}
            // A Windows-shaped prefix (`C:`, `\\?\`) is still nothing here, and unlike a leading
            // slash there is no root it could name.
            Component::Prefix(_) => {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidFilename,
                    "a path prefix names nothing here: this process holds a directory \
                     capability, not a drive or a share"
                ));
            }
            // Refused at every position, not just the first. `sub/../other` would be a descent
            // followed by an ascent, and there is no verb for the ascent and no capability that
            // would designate what it reached: a handle names a directory, and nothing on the wire
            // names its parent. This is the property that makes the walk safe by construction
            // rather than by checking, and it is why `cap-primitives` has to work so much harder on
            // Unix (see notes/std/fs-descent.md).
            Component::ParentDir => {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidFilename,
                    "`..` would leave the granted directory, which no capability designates"
                ));
            }
            Component::Normal(part) => {
                let name = part.to_str().ok_or_else(|| {
                    io::const_error!(
                        io::ErrorKind::InvalidFilename,
                        "a file name must be UTF-8 to cross this contract"
                    )
                })?;
                if name.len() > PAGE {
                    return Err(io::const_error!(
                        io::ErrorKind::InvalidFilename,
                        "a name is longer than the page shared with the FS server"
                    ));
                }
                n += 1;
            }
        }
    }
    Ok(n)
}

/// The names in `path`, in order. Only ever called after [`count_names`] has agreed there is
/// nothing else in there, so the `to_str` that cannot fail is dropped rather than unwrapped.
pub(super) fn names(path: &Path) -> impl Iterator<Item = &str> {
    path.components().filter_map(|c| match c {
        Component::Normal(part) => part.to_str(),
        _ => None,
    })
}

/// A directory handle a request is issued against: [`proto::ROOT`] for the granted directory, or
/// one a descent minted.
///
/// It closes what it minted and never closes `ROOT`, which is not this process's to close: `ROOT`
/// is the directory the endpoint is bound to rather than a handle anybody handed out. The server's
/// table has the bound directory in slot 0 and mints from 1 up, so `handle != ROOT` is exactly
/// "we made this".
pub(super) struct At(pub(super) u64);

impl Drop for At {
    fn drop(&mut self) {
        if self.0 != proto::ROOT {
            // A failure has nowhere to go: we are done with the handle either way, and a leaked
            // server-side slot is the cost of a reply we could not have acted on.
            let _ = request(proto::req(proto::CLOSE, self.0, 0), 0);
        }
    }
}

/// One hop: `OPENDIR name` under `parent`, asking for `want` on the child.
///
/// **`want` is the whole of the authority question** and the trap in this half of the PAL. The
/// server intersects the request with the parent's rights and answers `EPERM` when the result comes
/// up *short of the request* (§47's monotonicity is the intersection; the refusal is the server
/// refusing to hand back less than was asked for without saying so). A client cannot read its own
/// capability, so asking for more than the operation needs breaks under every narrowed grant while
/// passing every test that grants the root's full rights, which is the mistake `readdir` records
/// having nearly shipped.
pub(super) fn descend(p: &mut Page, parent: &At, name: &str, want: u64) -> io::Result<At> {
    p.put(name.as_bytes());
    let handle = request(proto::req(proto::OPENDIR, parent.0, name.len() as u64), want)?;
    Ok(At(handle))
}

/// **Resolve `path` to the directory its last name lives in, plus that name** (milestone 122).
///
/// `File::open("a/b/c")` is `OPENDIR a`, `OPENDIR b`, then `OPEN c` against `b`'s handle: the
/// contract's `openat` semantics with a path-shaped std on top (milestone 122's option A; [`Dir`]
/// is the other interface). Every hop asks for `DESCEND | needs`, where `needs` is what the *final*
/// verb requires on the directory it lands in (`filesystem_contract::verb::TABLE`; [`open_rights`]
/// resolves `OPEN`'s "READ or WRITE"). That is not a widening: a child's rights are its parent's
/// intersected with the request, so the walk asks for exactly the most the grant could give at that
/// depth. **At most two handles are open at once**, because the assignment in the loop drops, and
/// so closes, the previous hop. See notes/std/fs-descent.md.
pub(super) fn walk<'a>(p: &mut Page, from: u64, path: &'a Path, needs: u64) -> io::Result<(At, &'a str)> {
    let total = count_names(path)?;
    if total == 0 {
        return Err(io::const_error!(
            io::ErrorKind::InvalidFilename,
            "the granted directory itself is not a file this contract can open"
        ));
    }
    let mut at = At(from);
    let mut seen = 0;
    for name in names(path) {
        seen += 1;
        if seen == total {
            return Ok((at, name));
        }
        at = descend(p, &at, name, fsproto::dir::DESCEND | needs)?;
    }
    // `count_names` counted the same components this loop walks, so the return above always fires.
    Err(io::const_error!(
        io::ErrorKind::InvalidFilename,
        "the granted directory itself is not a file this contract can open"
    ))
}

/// Resolve `path` to a **directory** handle, asking for `want` on it.
///
/// The empty path and `"."` name the granted directory itself, which is [`proto::ROOT`] and costs
/// no message: it is the directory the endpoint is bound to rather than a name inside it. Anything
/// else is [`walk`] followed by one more descent, since the last component of a path a caller wants
/// listed is itself the directory rather than a name inside one.
pub(super) fn dir_at(p: &mut Page, path: &Path, want: u64) -> io::Result<At> {
    let names_in_path = count_names(path)?;
    if names_in_path == 0 {
        return Ok(At(proto::ROOT));
    }
    // One `OPENDIR` for the whole path when the server takes paths (milestone 606, ruling A); the
    // server descends each step asking for `DESCEND | want`, which is what the walk below sends.
    if names_in_path > 1 && paths_offered() {
        let joined = joined(path);
        match descend(p, &At(proto::ROOT), &joined, want) {
            Err(e) if refused_as_a_path(&e) => {}
            done => return done.inspect(|_| paths_seen()),
        }
        let (parent, name) = walk(p, proto::ROOT, path, want)?;
        let at = descend(p, &parent, name, want)?;
        paths_refused();
        return Ok(at);
    }
    let (parent, name) = walk(p, proto::ROOT, path, want)?;
    descend(p, &parent, name, want)
}
