use crate::*;

// The crate is `no_std`, so its own tests have to say where the allocating types come from. They
// are here only to build fixtures; nothing under test allocates.
extern crate std;

/// **A refusal explains itself as a fact, never as a policy.** The sentences are part of §47's
/// design (the errno decides what the holder learns), so they are pinned here rather than left
/// to drift in whichever program prints them. The negative half is the load-bearing one: not one
/// of these lines may read like Unix's "permission denied", because there is no policy in this
/// contract that could have said yes.
#[test]
fn every_refusal_explains_itself_without_implying_a_policy() {
    for errno in [
        2,
        dir::EPERM,
        dir::EROFS,
        dir::EISDIR,
        dir::ENOTDIR,
        17,
        dir::ENOTEMPTY,
        9,
        24,
        dir::EINVAL,
        xattr::ENODATA,
        xattr::ERANGE,
        xattr::E2BIG,
        xattr::ENOSPC,
        xattr::ENOTSUP,
        // An errno this contract does not name still has to render as something a human can
        // read, rather than as an empty line or a panic.
        5,
    ] {
        let s = dir::explain(errno);
        assert!(!s.is_empty(), "errno {errno} renders as nothing");
        assert!(
            !s.contains("denied") && !s.contains("permission"),
            "errno {errno} explains itself as a policy: {s:?}",
        );
    }
    // The two rungs whose whole design is that they answer different words must not collapse
    // into one sentence, or the distinction is invisible where a human meets it.
    assert_ne!(dir::explain(dir::EROFS), dir::explain(dir::EPERM));
    assert_ne!(dir::explain(2), dir::explain(dir::EPERM));
}

/// **Every errno this contract names has its own sentence.** The test above proves the
/// sentences are facts rather than policies; this one proves the map is not degenerate. The
/// errno is a *decision* about what the holder learns, so two errnos sharing a sentence, or one
/// falling through to the fallback, would print a true-sounding wrong explanation at the one
/// place a human meets the refusal. Pairwise distinctness, with the fallback in the set, makes
/// either failure visible.
#[test]
fn every_explained_errno_has_its_own_sentence() {
    let fallback = dir::explain(-1);
    assert_eq!(
        fallback,
        dir::explain(0),
        "the fallback must not depend on the number"
    );
    let mut seen = std::collections::BTreeSet::new();
    seen.insert(fallback);
    for errno in [
        2,
        dir::EPERM,
        dir::EROFS,
        dir::EISDIR,
        dir::ENOTDIR,
        17,
        dir::ENOTEMPTY,
        9,
        24,
        dir::EINVAL,
        xattr::ENODATA,
        xattr::ERANGE,
        xattr::E2BIG,
        xattr::ENOSPC,
        xattr::ENOTSUP,
    ] {
        assert!(
            seen.insert(dir::explain(errno)),
            "errno {errno} shares its sentence with another, or fell through to the fallback"
        );
    }
}

// --- The directory capability (milestone 47 (navigation and naming)) ---

/// **A child never carries a right its parent lacked**, over every single right and every
/// request, at one level and at two. The Kani harness proves this for every mask; this is the
/// millisecond version that runs in `cargo test`, and it exists because the proofs are behind a
/// cfg an ordinary build never sets.
#[test]
fn attenuation_is_monotonic_at_every_depth() {
    use dir::*;
    let every = [ENUMERATE, READ, WRITE, CREATE, REMOVE, DESCEND];
    for &held in &every {
        let parent = Rights::root(held);
        // Asking for everything gets exactly what the parent had, never more.
        assert_eq!(parent.attenuate(ALL), parent);
        for &wanted in &every {
            let child = parent.attenuate(wanted);
            for &probe in &every {
                assert!(
                    !child.allows(probe) || parent.allows(probe),
                    "a child carried {probe:#x} that its parent ({held:#x}) did not",
                );
            }
            // And a grandchild that asks for everything is still bounded by the root.
            let grand = child.attenuate(ALL);
            for &probe in &every {
                assert!(!grand.allows(probe) || parent.allows(probe));
            }
        }
    }
}

/// `allows` is "all of", not "any of". An operation needing two rights that a capability with
/// one of them could perform is a hole, and the two spellings differ only in an `==`.
#[test]
fn allows_means_all_of_them_not_any_of_them() {
    use dir::*;
    let r = Rights::root(CREATE);
    assert!(r.allows(CREATE));
    assert!(!r.allows(CREATE | REMOVE), "a rename needs both");
    assert!(Rights::root(CREATE | REMOVE).allows(CREATE | REMOVE));
    // The empty request is vacuously allowed, which is what makes a verb needing no right work.
    assert!(Rights::root(0).allows(0));
    assert!(Rights::root(0).denies_all(ALL));
    assert!(!Rights::root(READ).denies_all(READ | WRITE));
}

/// A root cannot be built with bits this contract has not defined. Otherwise a caller could set
/// bit 60 today and have some later version of `dir` give it a meaning it was never granted.
#[test]
fn undefined_rights_bits_cannot_be_smuggled_into_a_root() {
    assert_eq!(dir::Rights::root(u64::MAX).bits(), dir::ALL);
    assert_eq!(
        dir::ALL.count_ones(),
        7,
        "seven rungs on the ladder since DECISIONS §112"
    );
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// The directory rights and the two named bundles, as numbers. `ALL` and `REMOVE_TREE` are
/// built with `|`, and a mutant that made one an `&` collapsed the bundle to zero: a mount
/// whose root carries nothing, with every test still green.
#[test]
fn the_rights_are_the_bits_they_say() {
    assert_eq!(
        [
            dir::ENUMERATE,
            dir::READ,
            dir::WRITE,
            dir::CREATE,
            dir::REMOVE,
            dir::DESCEND,
            dir::SETTIME,
        ],
        [1, 2, 4, 8, 16, 32, 64]
    );
    assert_eq!(dir::ALL, 127);
    assert_eq!(dir::REMOVE_TREE, 1 | 16 | 32);
}

/// **The parent-rights sentinel is no rights mask, and resolving it never widens** (§98). Every
/// subset of `dir::ALL` is a mask, so the test walks all of them: none equals the sentinel, none
/// of the sentinel's bits is in `ALL`, and `resolve` leaves every real mask exactly as it was. The
/// sentinel resolves to the parent's own bits whatever they are, so the child is `parent & parent`.
#[test]
fn the_parent_rights_sentinel_is_not_a_mask_and_resolves_to_the_parents_own() {
    assert_eq!(dir::PARENT_RIGHTS & dir::ALL, 0);
    assert_eq!(dir::PARENT_RIGHTS, 1 << 63);
    for mask in 0..=dir::ALL {
        assert_ne!(mask, dir::PARENT_RIGHTS, "{mask:#x} is a real mask");
        for parent in 0..=dir::ALL {
            let parent = dir::Rights::root(parent);
            assert_eq!(parent.resolve(mask), mask, "a real mask passes through");
        }
    }
    for parent in 0..=dir::ALL {
        let parent = dir::Rights::root(parent);
        let child = parent.attenuate(parent.resolve(dir::PARENT_RIGHTS));
        assert_eq!(
            child, parent,
            "the sentinel asks for exactly what the parent holds"
        );
    }
    // Only the exact word is the sentinel: a mask that merely has bit 63 set among others keeps
    // the meaning it always had.
    assert_eq!(
        dir::Rights::root(dir::READ).resolve(dir::PARENT_RIGHTS | dir::READ),
        dir::PARENT_RIGHTS | dir::READ
    );
}
