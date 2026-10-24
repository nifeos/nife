//! **The extended-attribute witness** (milestone 57 (partitioning and formatting a real drive, and
//! extended attributes)) and the four attribute requests it makes.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::{dir, fs, xattr};
use filesystem_fixture as fixture;
use fixture::attrs;
use user_mode_runtime::call;

use crate::channel::{
    FILE, WINDOW, dir_create, dir_descend, dir_open, dir_rename, dir_unlink, get_page, put_page,
    put_page_at,
};

/// Fill `len` bytes of the shared page at `off` with `b`. The oversize probe writes a value larger
/// than any local this program could afford, so it goes straight to the page.
fn fill_page(off: usize, len: usize, b: u8) {
    for i in 0..len {
        WINDOW.w8((off + i) as u64, b);
    }
}

/// `SETXATTR`: the only verb here with two payloads, so the name and the value go into the page back
/// to back and the value's length and type code ride packed in the second word.
fn xattr_set(handle: u64, name: &[u8], kind: u32, value: &[u8]) -> i64 {
    put_page(name);
    put_page_at(name.len(), value);
    call(
        FILE,
        fs::req(fs::SETXATTR, handle, name.len() as u64),
        xattr::spec(kind, value.len() as u64),
    )
    .0 as i64
}

/// `GETXATTR`: the name goes out on the page and the value comes back on it, so the reply's packed
/// kind and length are all this returns; the bytes are read with [`get_page`].
fn xattr_get(handle: u64, name: &[u8]) -> i64 {
    put_page(name);
    call(FILE, fs::req(fs::GETXATTR, handle, name.len() as u64), 0).0 as i64
}

/// `LISTXATTR`: no name, no cursor. The reply is how many bytes of records landed in the page.
fn xattr_list(handle: u64) -> i64 {
    call(FILE, fs::req(fs::LISTXATTR, handle, 0), 0).0 as i64
}

/// `REMOVEXATTR`: [`xattr_get`]'s shape, and the reply is 0 or an error.
fn xattr_remove(handle: u64, name: &[u8]) -> i64 {
    put_page(name);
    call(FILE, fs::req(fs::REMOVEXATTR, handle, name.len() as u64), 0).0 as i64
}

/// The extended-attribute witness (milestone 57): every claim of the layer, attempted against the
/// real FS server, returned as a bitmap the kernel test asserts exactly.
///
/// A bitmap names which claim broke: dropped type codes, a store keyed on a path rather than a
/// node, and a purge that never ran each fail a different bit. It removes its attribute from
/// `scratch` and unlinks the two files it makes, so the image differs only by the store directory,
/// which the host tool is expected to show (notes/host-recovery.md). See notes/xattr.md.
pub(crate) fn attr_witness(scratch: u64) -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 512];

    // 1. Set an attribute and read it back: its bytes AND its type code. The kind is the half that
    //    catches a layer which stored the value and threw the type away, which is the thing that
    //    would foreclose indexing later without anyone noticing.
    if xattr_set(scratch, attrs::NAME, attrs::KIND, attrs::VALUE) >= 0 {
        let r = xattr_get(scratch, attrs::NAME);
        if r >= 0
            && xattr::reply_kind(r) == attrs::KIND
            && xattr::reply_value_len(r) == attrs::VALUE.len()
        {
            get_page(attrs::VALUE.len(), &mut buf);
            if &buf[..attrs::VALUE.len()] == attrs::VALUE {
                v |= attrs::SET_AND_READ_BACK;
            }
        }
    } else {
        v |= attrs::ATTRS_FAILED;
    }

    // 2. The listing names it, and names nothing else. A store keyed on the wrong node would show up
    //    here as somebody else's attributes rather than as a missing one.
    let n = xattr_list(scratch);
    if n > 0 {
        get_page(n as usize, &mut buf);
        let mut count = 0;
        let mut saw = false;
        for name in xattr::list::iter(&buf[..(n as usize).min(buf.len())]) {
            count += 1;
            if name == attrs::NAME {
                saw = true;
            }
        }
        if saw && count == 1 {
            v |= attrs::LISTED;
        }
    }

    // 3. A value past the ceiling is refused with its own word, not stored short. The probe is wider
    //    than any local here, so it is written straight onto the page.
    let big = b"user.toobig";
    put_page(big);
    fill_page(big.len(), xattr::MAX_VALUE + 1, b'Z');
    let r = call(
        FILE,
        fs::req(fs::SETXATTR, scratch, big.len() as u64),
        xattr::spec(xattr::RAW, (xattr::MAX_VALUE + 1) as u64),
    )
    .0 as i64;
    if filesystem_contract::reply_errno(r) == Some(xattr::E2BIG) {
        v |= attrs::OVERSIZE_REFUSED;
    }

    // 4. Removing it takes it away, and reading it afterwards is ENODATA. Without this the round
    //    trip above is equally true of a store that never forgets anything.
    if xattr_remove(scratch, attrs::NAME) >= 0
        && filesystem_contract::reply_errno(xattr_get(scratch, attrs::NAME)) == Some(xattr::ENODATA)
    {
        v |= attrs::GONE_AFTER_REMOVE;
    }

    v |= an_attribute_follows_its_file_across_a_rename();

    // 6. And the file's death takes its attributes with it: unlink the name, make it again, and the
    //    new file has nothing on it. The node id is very likely reused, which is the case that makes
    //    a leaked blob a correctness bug rather than wasted space.
    if dir_unlink(fs::ROOT, attrs::PROBE_MOVED) >= 0 {
        let fresh = dir_create(fs::ROOT, attrs::PROBE_MOVED);
        if fresh >= 0 && xattr_list(fresh as u64) == 0 {
            v |= attrs::GONE_AFTER_UNLINK;
        }
        let _ = dir_unlink(fs::ROOT, attrs::PROBE_MOVED);
    }

    // 7. The store is not in the namespace: it cannot be opened, created, or descended into.
    if dir_open(fs::ROOT, xattr::STORE_DIR) < 0
        && dir_create(fs::ROOT, xattr::STORE_DIR) < 0
        && dir_descend(fs::ROOT, xattr::STORE_DIR, dir::ALL) < 0
    {
        v |= attrs::STORE_UNNAMEABLE;
    }

    v |= a_full_enumeration_does_not_name_the_store();

    v
}

/// Step 5 of [`attr_witness`]. Returns the bits it earned.
fn an_attribute_follows_its_file_across_a_rename() -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 512];
    // 5. **The headline: an attribute follows its file across a rename.** The two names are unlinked
    //    first, because `NIFE_KEEP_REDOXFS` makes a second boot meet the first boot's leftovers
    //    and an `EEXIST` here would read as a broken layer.
    let _ = dir_unlink(fs::ROOT, attrs::PROBE);
    let _ = dir_unlink(fs::ROOT, attrs::PROBE_MOVED);
    let probe = dir_create(fs::ROOT, attrs::PROBE);
    if probe >= 0
        && xattr_set(probe as u64, attrs::NAME, attrs::KIND, attrs::VALUE) >= 0
        && dir_rename(fs::ROOT, attrs::PROBE, fs::ROOT, attrs::PROBE_MOVED) >= 0
    {
        // Opened at its NEW name, through a handle minted after the rename, so nothing carried over
        // from the handle that set it.
        let moved = dir_open(fs::ROOT, attrs::PROBE_MOVED);
        let r = if moved >= 0 {
            xattr_get(moved as u64, attrs::NAME)
        } else {
            -1
        };
        if r >= 0 && xattr::reply_value_len(r) == attrs::VALUE.len() {
            get_page(attrs::VALUE.len(), &mut buf);
            if &buf[..attrs::VALUE.len()] == attrs::VALUE {
                v |= attrs::SURVIVED_RENAME;
            }
        }
    } else {
        v |= attrs::ATTRS_FAILED;
    }
    v
}

/// Step 8 of [`attr_witness`]. Returns the bits it earned.
fn a_full_enumeration_does_not_name_the_store() -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 512];
    // 8. And a full enumeration does not name it. Driven to the end with a cursor, because a listing
    //    that stopped early would be a clean report about the part it happened to read.
    let (mut cursor, mut entries, mut clean, mut finished) = (0u64, 0u64, true, false);
    for _ in 0..64 {
        let n = call(FILE, fs::req(fs::READDIR, fs::ROOT, 0), cursor).0 as i64;
        if n <= 0 {
            finished = n == 0;
            break;
        }
        get_page(n as usize, &mut buf);
        let mut seen = 0u64;
        for (name, _) in filesystem_contract::dirent::iter(&buf[..(n as usize).min(buf.len())]) {
            if name == xattr::STORE_DIR.as_bytes() {
                clean = false;
            }
            seen += 1;
        }
        if seen == 0 {
            break; // a cursor that cannot advance is a failure, not a clean listing
        }
        cursor += seen;
        entries += seen;
    }
    if clean && finished && entries > 0 {
        v |= attrs::STORE_UNLISTED;
    }
    v
}
