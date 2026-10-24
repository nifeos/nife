//! **The attacker against a per-directory grant** (milestone 47 (navigation and naming)).
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::{dir, fs, xattr};
use filesystem_fixture as fixture;
use fixture::{dirscape as esc, tree};
use user_mode_runtime::{call, exit, send};

use crate::channel::{
    FILE, REPORT, dir_create, dir_descend, dir_mkdir, dir_open, dir_rename, get_page, put_page,
    put_page_at,
};

/// The attacker against a per-directory grant (milestone 47): tries to reach its parent or a
/// sibling, or use a right it was not given, through one `fs_subtree_caretaker`.
///
/// `run` only keeps the names it creates distinct across runs on one image; it is told nothing
/// else about its grant. It attempts every verb and reports a bitmap of what got through, and the
/// kernel test asserts the exact set per configuration, so three runs of this code are each other's
/// controls. `motd` (the parent) and `other/secret` (a sibling) are real and one entry from the
/// caretaker. It never writes `sub/inner` or outside `sub`, so the post-run host check can pin those
/// bytes. See notes/fs-server/the-test-client.md.
pub(crate) fn dir_attacker(run: u64) -> ! {
    let mut v = 0u64;
    let mut buf = [0u8; 256];

    // 1. The parent. `motd` is in the granted directory's parent and is not in the granted
    //    directory, so this must find nothing. Not a refusal: in this scope there is no such name.
    if dir_open(fs::ROOT, fixture::MOTD_NAME) >= 0 {
        v |= esc::REACHED_PARENT;
    }
    // 2. The sibling, by both routes: descending into it and naming the file inside it.
    if dir_descend(fs::ROOT, tree::OTHER, dir::ALL) >= 0 || dir_open(fs::ROOT, tree::SECRET) >= 0 {
        v |= esc::REACHED_SIBLING;
    }
    // 3. `..`, which is not a component this contract accepts, at any rights.
    if dir_descend(fs::ROOT, "..", dir::ALL) >= 0 || dir_open(fs::ROOT, "..") >= 0 {
        v |= esc::WALKED_UP;
    }

    // 4. The control: the file inside the grant. A capability that reaches nothing is trivially
    //    unescapable, so without this bit every refusal above proves nothing.
    let own = dir_open(fs::ROOT, tree::INNER);
    if own >= 0 {
        v |= esc::OPENED_ITS_OWN;
        let (n, _) = call(
            FILE,
            fs::req(fs::READ, own as u64, tree::INNER_BODY.len() as u64),
            0,
        );
        get_page(n as usize, &mut buf);
        if (n as i64) < 0
            || n as usize != tree::INNER_BODY.len()
            || &buf[..n as usize] != tree::INNER_BODY
        {
            v |= esc::GRANTED_ACCESS_FAILED;
        }

        v |= probe_its_own_attributes(own);
    }

    v |= probe_enumeration();

    // 6. Making a name inside the grant, and 7. making a directory inside it. Both need CREATE;
    //    `mkdir` needs DESCEND too, because a directory you could not have walked into would be a
    //    way to mint a capability out of a right that was withheld.
    let made = run_name(tree::MADE, run);
    let created = dir_create(fs::ROOT, made_str(&made));
    if created >= 0 {
        v |= esc::CREATED;
        v |= write_what_it_made(created);
    }
    let made_dir = run_name(tree::MADE_DIR, run);
    if dir_mkdir(fs::ROOT, made_str(&made_dir), dir::READ) >= 0 {
        v |= esc::MADE_A_DIR;
    }

    // 9. Descending inside the grant, which is allowed exactly when DESCEND was granted. Asking for
    //    DESCEND alone, so this probe is about reaching the child rather than about its rights.
    let child = dir_descend(fs::ROOT, tree::DEEPER, dir::DESCEND);
    if child >= 0 {
        v |= esc::DESCENDED;
    }

    v |= probe_widening(created);

    // 11. **Renaming, which is the only verb on this wire that consults `REMOVE`.** It moves the
    //     name it made itself, never `inner`, so a run that is allowed to do this damages only its
    //     own leavings and the post-run host check can still pin the fixture. The attempt is made
    //     whatever the grant is: the server checks the rights before it resolves anything, so a
    //     capability with no `REMOVE` gets the same refusal whether or not the source is there,
    //     and this program is not told which case it is in.
    let moved = run_name(tree::MOVED, run);
    if dir_rename(fs::ROOT, made_str(&made), fs::ROOT, made_str(&moved)) >= 0 {
        v |= esc::RENAMED;
    }

    // 12. Handle guessing, past anything the caretaker could have minted for it. The caretaker
    //     numbers its client's handles in its own space, so a number it never issued is refused by
    //     one check.
    let mut guess = 9u64;
    while guess < 16 {
        if call(FILE, fs::req(fs::READ, guess, 8), 0).0 as i64 >= 0
            || call(FILE, fs::req(fs::READDIR, guess, 0), 0).0 as i64 >= 0
        {
            v |= esc::FORGED_HANDLE;
        }
        guess += 1;
    }

    // Nothing at all worked: report it, so a capability that reaches nothing cannot pass as a
    // capability that is perfectly confined.
    if v & (esc::OPENED_ITS_OWN | esc::ENUMERATED | esc::DESCENDED | esc::CREATED) == 0 {
        v |= esc::GRANTED_ACCESS_FAILED;
    }

    send(REPORT, fixture::VERDICT, v, 0);
    exit();
}

/// Step 4b: read, set and remove an attribute on the file inside the grant, `own`. Returns the
/// verdict bits it earned.
fn probe_its_own_attributes(own: i64) -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 256];
    // 4b. **Its attributes** (milestone 61). Reading them needs what reading the file needs, so
    //     every configuration that got here should manage it; writing one needs `dir::WRITE`,
    //     and the caretaker checks nothing, so the refusal comes from the FS server acting on
    //     the rights the caretaker's one `OPENDIR` left on this handle.
    //
    //     The attribute is set on `inner` and then removed, so the post-run host check still
    //     sees the file it pins, byte for byte, with nothing attached.
    let (l, _) = call(FILE, fs::req(fs::LISTXATTR, own as u64, 0), 0);
    if (l as i64) < 0 {
        v |= esc::GRANTED_ACCESS_FAILED;
    } else {
        v |= esc::READ_ATTRS;
    }

    put_page(fixture::attrs::NAME);
    put_page_at(fixture::attrs::NAME.len(), fixture::attrs::VALUE);
    let (s, _) = call(
        FILE,
        fs::req(fs::SETXATTR, own as u64, fixture::attrs::NAME.len() as u64),
        xattr::spec(fixture::attrs::KIND, fixture::attrs::VALUE.len() as u64),
    );
    if (s as i64) >= 0 {
        put_page(fixture::attrs::NAME);
        let (g, _) = call(
            FILE,
            fs::req(fs::GETXATTR, own as u64, fixture::attrs::NAME.len() as u64),
            0,
        );
        get_page(xattr::reply_value_len(g as i64), &mut buf);
        if (g as i64) >= 0
            && xattr::reply_kind(g as i64) == fixture::attrs::KIND
            && xattr::reply_value_len(g as i64) == fixture::attrs::VALUE.len()
            && buf[..fixture::attrs::VALUE.len()] == *fixture::attrs::VALUE
        {
            v |= esc::SET_AN_ATTR;
        } else {
            v |= esc::GRANTED_ACCESS_FAILED;
        }
        put_page(fixture::attrs::NAME);
        let (rm, _) = call(
            FILE,
            fs::req(
                fs::REMOVEXATTR,
                own as u64,
                fixture::attrs::NAME.len() as u64,
            ),
            0,
        );
        if (rm as i64) < 0 {
            v |= esc::GRANTED_ACCESS_FAILED;
        }
    }
    v
}

/// Step 5: list the granted directory, and flag any name from outside the grant in the listing.
fn probe_enumeration() -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 256];
    // 5. Enumeration, and what it is allowed to contain. A listing is a rendering of authority, so
    //    a name from outside the grant appearing in one is an escape even though nothing was opened.
    let (n, _) = call(FILE, fs::req(fs::READDIR, fs::ROOT, 0), 0);
    if (n as i64) >= 0 {
        v |= esc::ENUMERATED;
        // Clamped to the local buffer: the page is sixteen times larger, and indexing past the
        // buffer with a length the server chose would be a panic this program cannot report.
        let n = (n as usize).min(buf.len());
        get_page(n, &mut buf);
        for (name, _) in filesystem_contract::dirent::iter(&buf[..n]) {
            for stranger in [
                fixture::MOTD_NAME,
                fixture::SCRATCH_NAME,
                tree::OTHER,
                tree::SECRET,
            ] {
                if name == stranger.as_bytes() {
                    v |= esc::ENUMERATED_A_STRANGER;
                }
            }
        }
    }
    v
}

/// Step 8: write to the file it just made, `created`, and read the write straight back.
fn write_what_it_made(created: i64) -> u64 {
    let mut v = 0u64;
    let mut buf = [0u8; 256];
    // 8. A write, to what it just made, read straight back: "the server accepted my write" and
    //    "my write landed" are different claims. Never to `inner`, which the host check pins.
    put_page(tree::MADE_BODY);
    let (w, _) = call(
        FILE,
        fs::req(fs::WRITE, created as u64, tree::MADE_BODY.len() as u64),
        0,
    );
    if (w as i64) >= 0 {
        v |= esc::WROTE;
        let (rb, _) = call(
            FILE,
            fs::req(fs::READ, created as u64, tree::MADE_BODY.len() as u64),
            0,
        );
        get_page(tree::MADE_BODY.len(), &mut buf);
        if rb as usize != tree::MADE_BODY.len() || &buf[..tree::MADE_BODY.len()] != tree::MADE_BODY
        {
            v |= esc::GRANTED_ACCESS_FAILED;
        }
    }
    v
}

/// Step 10: can a descent carry more than it was given? `created` is step 6's answer, which says
/// whether the grant itself holds CREATE.
fn probe_widening(created: i64) -> u64 {
    let mut v = 0u64;
    // 10. **Widening, the question this milestone exists to answer**, probed two ways.
    //
    //   (a) A child asked for NOTHING must be able to do nothing. If a zero-rights directory
    //       capability opens, lists or creates, rights are not being carried on the handle at all.
    let nothing = dir_descend(fs::ROOT, tree::DEEPER, 0);
    if nothing >= 0 {
        let listed = call(FILE, fs::req(fs::READDIR, nothing as u64, 0), 0).0 as i64;
        if dir_open(nothing as u64, tree::LEAF) >= 0
            || dir_create(nothing as u64, "smuggled") >= 0
            || listed >= 0
        {
            v |= esc::WIDENED;
        }
    }
    //   (b) A child cannot carry what the parent did not. If creating in the grant was refused, the
    //       grant has no CREATE, so a descent that asks for CREATE must be refused too.
    if created < 0 && dir_descend(fs::ROOT, tree::DEEPER, dir::ALL) >= 0 {
        v |= esc::WIDENED;
    }
    v
}

/// A name with the run index appended, so three attacker runs sharing one image do not collide on
/// `EEXIST` and read it as a refusal. Fixed-size because this program has no allocator.
fn run_name(base: &str, run: u64) -> ([u8; 16], usize) {
    let mut out = [0u8; 16];
    let n = base.len().min(15);
    out[..n].copy_from_slice(&base.as_bytes()[..n]);
    out[n] = b'0' + (run % 10) as u8;
    (out, n + 1)
}

/// The `&str` inside a [`run_name`]. Its bytes are ASCII by construction, so the conversion cannot
/// fail; `unwrap_or` keeps the panic-free discipline anyway.
fn made_str(name: &([u8; 16], usize)) -> &str {
    core::str::from_utf8(&name.0[..name.1]).unwrap_or("made")
}
