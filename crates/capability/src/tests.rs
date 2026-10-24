use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Obj {
    Console,
    PageFrame(u64),
}

/// **The occupied mask is the occupied slots and nothing above them.** A sweep walks this word
/// in place of `0..N`, so a bit set for a free slot, or for a slot past `N`, is a visit to
/// something that is not there; the sweep's own guard would hide it, so the word is checked
/// itself.
#[test]
fn the_occupied_mask_is_exactly_the_occupied_slots() {
    let cap = |n| Cap {
        object: Obj::PageFrame(n),
        rights: Rights::ALL,
    };
    let mut cs: CapabilityTable<Obj, 8> = CapabilityTable::new();
    assert_eq!(cs.occupied_mask(), 0);
    cs.insert_at(2, cap(1)).unwrap();
    cs.insert_at(5, cap(2)).unwrap();
    assert_eq!(cs.occupied_mask(), 0b0010_0100);
    cs.delete(2).unwrap();
    assert_eq!(cs.occupied_mask(), 0b0010_0000);
    let mut full: CapabilityTable<Obj, 64> = CapabilityTable::new();
    assert_eq!(full.occupied_mask(), 0);
    full.insert_at(63, cap(3)).unwrap();
    assert_eq!(full.occupied_mask(), 1 << 63);
}

/// **A new process holds nothing.** The decision, as an assertion.
#[test]
fn a_new_capability_table_can_name_nothing() {
    let cs: CapabilityTable<Obj, 16> = CapabilityTable::new();

    assert!(cs.is_empty());
    for slot in 0..cs.len() as u64 {
        assert_eq!(cs.get(slot).err(), Some(Error::NoSuchSlot));
    }
}

/// `delete_matching` is `get`-then-`delete` over every slot, done in place: everything the
/// predicate matches goes, and **a non-matching neighbor survives untouched**, rights and all.
/// The second half is the property worth the test: a revocation sweep that took a bystander's
/// capability would be a denial of service wearing a security fix's clothes.
#[test]
fn delete_matching_takes_the_matched_and_spares_the_neighbor() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::PageFrame(0x1000),
            rights: Rights::READ,
        },
    )
    .unwrap();
    cs.put(
        1,
        Cap {
            object: Obj::Console,
            rights: Rights::WRITE,
        },
    )
    .unwrap();
    cs.put(
        15,
        Cap {
            object: Obj::PageFrame(0x2000),
            rights: Rights::ALL,
        },
    )
    .unwrap();

    cs.delete_matching(|o| matches!(o, Obj::PageFrame(_)));

    // Both frames are gone, wherever they sat, including the last slot.
    assert_eq!(cs.get(0).err(), Some(Error::NoSuchSlot));
    assert_eq!(cs.get(15).err(), Some(Error::NoSuchSlot));
    // The bystander keeps its object and its rights.
    let survivor = cs.get(1).unwrap();
    assert_eq!(survivor.object, Obj::Console);
    assert_eq!(survivor.rights, Rights::WRITE);
}

/// You cannot fabricate a capability. There is nothing to guess.
#[test]
fn an_unheld_slot_is_not_permission_denied_it_is_nothing() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::Console,
            rights: Rights::WRITE,
        },
    )
    .unwrap();

    assert_eq!(cs.get(0).unwrap().object, Obj::Console);

    // Every other slot, and every slot past the end.
    assert_eq!(cs.get(1).err(), Some(Error::NoSuchSlot));
    assert_eq!(cs.get(15).err(), Some(Error::NoSuchSlot));
    assert_eq!(cs.get(16).err(), Some(Error::NoSuchSlot));
    assert_eq!(cs.get(u64::MAX).err(), Some(Error::NoSuchSlot));
}

/// **Rights only ever narrow.** If delegation could widen, the model is theatre: you would
/// simply derive yourself a better capability from the one you have.
#[test]
fn derive_cannot_widen_rights() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::PageFrame(0x1000),
            rights: Rights::READ,
        },
    )
    .unwrap();

    // Narrowing to nothing: fine.
    assert!(cs.derive(0, 1, Rights::NONE).is_ok());
    // Same rights: fine.
    assert!(cs.derive(0, 2, Rights::READ).is_ok());

    // **Asking for WRITE when we only hold READ.**
    assert_eq!(
        cs.derive(0, 3, Rights::WRITE).err(),
        Some(Error::CannotWiden)
    );
    assert_eq!(cs.derive(0, 3, Rights::ALL).err(), Some(Error::CannotWiden));

    // And the failed derive left nothing behind.
    assert_eq!(cs.get(3).err(), Some(Error::NoSuchSlot));
}

/// A narrowed capability names the **same object**. Delegation moves authority, not identity.
#[test]
fn a_derived_capability_names_the_same_object_with_less_authority() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::PageFrame(0x4000),
            rights: Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
        },
    )
    .unwrap();

    cs.derive(0, 5, Rights::READ).unwrap();

    let orig = cs.get(0).unwrap();
    let copy = cs.get(5).unwrap();

    assert_eq!(orig.object, copy.object, "not the same object");
    assert!(orig.rights.allows(Rights::WRITE));
    assert!(!copy.rights.allows(Rights::WRITE), "the copy kept WRITE");
}

/// Deriving from an empty slot fails. You cannot lend what you do not hold.
#[test]
fn you_cannot_delegate_what_you_do_not_hold() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    assert_eq!(cs.derive(3, 4, Rights::READ).err(), Some(Error::NoSuchSlot));
}

#[test]
fn get_with_refuses_rights_you_do_not_have() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::Console,
            rights: Rights::READ,
        },
    )
    .unwrap();

    assert!(cs.get_with(0, Rights::READ).is_ok());
    assert_eq!(
        cs.get_with(0, Rights::WRITE).err(),
        Some(Error::CannotWiden)
    );
    assert_eq!(cs.get_with(9, Rights::READ).err(), Some(Error::NoSuchSlot));
}

#[test]
fn deleting_a_capability_makes_the_object_unnameable() {
    let mut cs: CapabilityTable<Obj, 16> = CapabilityTable::new();
    cs.put(
        0,
        Cap {
            object: Obj::Console,
            rights: Rights::ALL,
        },
    )
    .unwrap();

    cs.delete(0).unwrap();
    assert_eq!(cs.get(0).err(), Some(Error::NoSuchSlot));
    assert_eq!(cs.delete(0).err(), Some(Error::NoSuchSlot));
}

#[test]
fn rights_arithmetic() {
    assert!(Rights::ALL.allows(Rights::WRITE));
    assert!(!Rights::READ.allows(Rights::WRITE));
    assert!(Rights::NONE.is_subset_of(Rights::READ));
    assert!(Rights::READ.is_subset_of(Rights::ALL));
    assert!(!Rights::ALL.is_subset_of(Rights::READ));
    assert_eq!(Rights::ALL.intersect(Rights::READ), Rights::READ);
}

/// The four cases of §32's reap gate, spelled out as the readable companion to the symbolic
/// proofs: my corpse, my live child, another supervisor's child, an unsupervised thread (which
/// is also what a stale tid looks like, because a name that does not resolve carries no
/// rendezvous).
#[test]
fn reap_authorizes_only_a_corpse_this_rendezvous_supervises() {
    assert_eq!(reap_decision(Some(7), 7, true), Reap::Permitted);
    assert_eq!(reap_decision(Some(7), 7, false), Reap::StillAlive);
    assert_eq!(reap_decision(Some(9), 7, true), Reap::NotSupervised);
    assert_eq!(reap_decision(None, 7, true), Reap::NotSupervised);
}

/// The scoping half of §32's gate, and the reason it has a `cargo test` companion at all: a
/// mutation run of 2026-09-19 (milestone 326) replaced `survey_includes` with a constant `true`
/// and again with a constant `false`, and **both survived the whole suite**. Kani proves this
/// predicate for every input, but `script/verify` is a different gate on a different cadence,
/// so a build that answered "everyone is in your domain" would reach a reviewer with
/// `script/test` green. Liveness is deliberately absent: a corpse is still in the domain, which
/// is what lets `ps` show a `Dead` child its supervisor has not collected.
#[test]
fn a_survey_shows_this_rendezvous_children_and_nobody_else() {
    assert!(survey_includes(Some(7), 7));
    assert!(survey_includes(Some(0), 0));
    assert!(!survey_includes(Some(9), 7));
    assert!(!survey_includes(None, 7));
}

/// **Both halves of the packed high-water word, read exactly rather than as a floor.**
///
/// `PEAK` is shared by every table in this binary, so a floor cannot pin either half: `>>` for
/// `<<` in the peak half and `&` for `|` or `^` in the ceiling half survived one (milestone 326
/// (nobody has been assigned to turn a mutation score upward)). Exactness holds because
/// `fetch_max` only raises and no other test exceeds 64 of 64 (the free-slot edge test ties it,
/// writing the same word); a test that outgrows it fails loudly.
#[test]
fn the_high_water_word_reports_the_occupancy_and_the_capacity_that_set_it() {
    let cap = Cap {
        object: Obj::PageFrame(3),
        rights: Rights::READ,
    };
    // Full at sixty-four, the most a table may have (`MAX_SLOTS`).
    let mut cs: CapabilityTable<Obj, 64> = CapabilityTable::new();
    for _ in 0..64 {
        cs.insert(cap).expect("sixty-four slots hold sixty-four");
    }

    assert_eq!(
        highest_seen(),
        (64, 64),
        "no table in this test binary can exceed 64 of 64, so this record is the one standing"
    );
}

/// **The free-slot word at its edge, and first-free after a hole.** Sixty-four slots is the
/// case where every bit of the `u64` is a slot, so "all free" is `u64::MAX` and "full" is zero
/// with no bits above `N` to mistake for room. A hole punched in the middle must be the next
/// slot `insert` takes, lowest first, because the kernel's Reply mint relies on first-free.
#[test]
fn the_free_slot_word_fills_to_zero_and_hands_back_the_lowest_hole() {
    let cap = Cap {
        object: Obj::PageFrame(1),
        rights: Rights::READ,
    };
    let mut cs: CapabilityTable<Obj, 64> = CapabilityTable::new();
    assert_eq!(cs.free_mask(), u64::MAX);
    for want in 0..64u64 {
        assert_eq!(cs.insert(cap), Ok(want));
    }
    assert_eq!(cs.free_mask(), 0, "a full table is a zero word");
    assert_eq!(cs.insert(cap), Err(Error::NoFreeSlot));

    cs.delete(20).unwrap();
    cs.delete(7).unwrap();
    assert_eq!(cs.free_mask(), (1 << 20) | (1 << 7));
    assert_eq!(cs.insert(cap), Ok(7), "lowest hole first");
    assert_eq!(cs.insert(cap), Ok(20));
    assert_eq!(cs.used(), 64);

    // A small table's bits above N stay clear, so full is still zero there too.
    let mut small: CapabilityTable<Obj, 3> = CapabilityTable::new();
    assert_eq!(small.free_mask(), 0b111);
    for _ in 0..3 {
        small.insert(cap).unwrap();
    }
    assert_eq!(small.free_mask(), 0);
    assert_eq!(small.insert(cap), Err(Error::NoFreeSlot));
}

#[test]
fn a_full_table_says_so() {
    let mut cs: CapabilityTable<Obj, 2> = CapabilityTable::new();
    let cap = Cap {
        object: Obj::Console,
        rights: Rights::ALL,
    };
    assert_eq!(cs.insert(cap), Ok(0));
    assert_eq!(cs.insert(cap), Ok(1));
    assert_eq!(cs.insert(cap).err(), Some(Error::NoFreeSlot));
}

/// **`insert_at_or_above` never lands below its floor**, finds the first free slot at or above
/// it, and is `insert` at floor 0 (milestone 812).
#[test]
fn insert_at_or_above_respects_its_floor() {
    let mut t: CapabilityTable<u8, 64> = CapabilityTable::new();
    let capability = |o| Cap {
        object: o,
        rights: Rights::ALL,
    };
    assert_eq!(t.insert_at_or_above(12, capability(1)), Ok(12));
    assert_eq!(t.insert_at_or_above(12, capability(2)), Ok(13));
    assert_eq!(
        t.insert_at_or_above(0, capability(3)),
        Ok(0),
        "floor 0 is first free"
    );
    assert_eq!(
        t.insert_at_or_above(64, capability(4)),
        Err(Error::NoFreeSlot)
    );
    for s in 14..64 {
        t.insert_at(s, capability(5)).unwrap();
    }
    assert_eq!(
        t.insert_at_or_above(12, capability(6)),
        Err(Error::NoFreeSlot),
        "slots below the floor do not count"
    );
}

/// The rights bits are the syscall wire format: delegation names them in a register, and
/// `from_bits` turns the register back into authority, masking off undefined bits. The exact values
/// are pinned because milestone 85 (mutation testing over the host crates)'s mutants (`1 << 1` as
/// `1 >> 1`, `from_bits` using `|` instead of a mask) survived when nothing pinned them. The `ALL`
/// union is spelled out by hand, since a right missing from `ALL` is dropped at every delegation
/// and a check that read `ALL` to check `ALL` would pass whatever it was given.
#[test]
fn rights_bits_are_the_wire_format() {
    assert_eq!(
        [
            Rights::READ.bits(),
            Rights::WRITE.bits(),
            Rights::GRANT.bits(),
            Rights::ENUMERATE.bits(),
            Rights::BIND.bits()
        ],
        [1, 2, 4, 8, 16]
    );
    // Every defined bit is in ALL, and ALL defines nothing that is not a named right.
    assert_eq!(
        Rights::ALL.bits(),
        Rights::READ.bits()
            | Rights::WRITE.bits()
            | Rights::GRANT.bits()
            | Rights::ENUMERATE.bits()
            | Rights::BIND.bits()
    );
    // Undefined bits are dropped, never reinterpreted.
    assert_eq!(Rights::from_bits(!Rights::ALL.bits()), Rights::NONE);
    // Defined bits come back as exactly the rights they name.
    assert_eq!(Rights::from_bits(0b101), Rights::READ.union(Rights::GRANT));
    // Union is a union, not a toggle: adding a right twice cannot remove it.
    assert_eq!(Rights::READ.union(Rights::READ), Rights::READ);
}

/// `insert_at` is how a supervisor reserves the fault slot, so what matters is that the
/// capability lands in THAT slot, the slot comes back as the receipt, and an occupied slot
/// refuses rather than replaces (two things must never share the slot the kernel reads at
/// START).
#[test]
fn insert_at_fills_exactly_the_named_slot() {
    let mut cs: CapabilityTable<Obj, 4> = CapabilityTable::new();
    let cap = Cap {
        object: Obj::PageFrame(7),
        rights: Rights::READ,
    };
    assert_eq!(cs.insert_at(2, cap), Ok(2));
    assert_eq!(cs.get(2).unwrap().object, Obj::PageFrame(7));
    assert!(!cs.is_empty());
    // The other slots are still empty, so it did not spray.
    assert_eq!(cs.get(0).err(), Some(Error::NoSuchSlot));
    // Occupied refuses; out of range is a different answer, as everywhere.
    assert_eq!(cs.insert_at(2, cap).err(), Some(Error::NoFreeSlot));
    assert_eq!(cs.insert_at(9, cap).err(), Some(Error::NoSuchSlot));
    // len is the table's size in slots, not its population.
    assert_eq!(cs.len(), 4);
}

/// **The high-water mark is a mark, not a level**, which is the whole reason it exists: a boot
/// that filled the table and then emptied it reached the wall, and a count sampled afterwards
/// says it did not.
///
/// Every mutator that can occupy an empty slot is exercised here (`insert`, `insert_at`, and
/// `put` over an empty slot), because each has its own call to the private growth path and a
/// missed one would be invisible: the number would simply be a little too low, forever.
#[test]
fn the_peak_records_what_was_reached_and_never_what_is_left() {
    let cap = Cap {
        object: Obj::PageFrame(1),
        rights: Rights::READ,
    };
    let mut cs: CapabilityTable<Obj, 4> = CapabilityTable::new();
    assert_eq!((cs.used(), cs.peak()), (0, 0));

    assert_eq!(cs.insert(cap), Ok(0));
    assert_eq!(cs.insert_at(2, cap), Ok(2));
    cs.put(3, cap).expect("slot 3 is in range");
    assert_eq!((cs.used(), cs.peak()), (3, 3));

    // Replacing an occupant is not growth.
    cs.put(3, cap).expect("slot 3 is in range");
    assert_eq!((cs.used(), cs.peak()), (3, 3));

    // And emptying it does not lower the mark.
    cs.delete(0).expect("slot 0 is occupied");
    cs.delete_matching(|o| matches!(o, Obj::PageFrame(_)));
    assert_eq!((cs.used(), cs.peak()), (0, 3));

    // Refusals leave the count alone. Both of `insert_at`'s, and `delete`'s.
    assert!(cs.insert_at(9, cap).is_err());
    assert!(cs.delete(1).is_err());
    assert_eq!((cs.used(), cs.peak()), (0, 3));

    // A full table refuses, and the mark is the capacity rather than one past it.
    let mut full: CapabilityTable<Obj, 2> = CapabilityTable::new();
    assert!(full.insert(cap).is_ok());
    assert!(full.insert(cap).is_ok());
    assert_eq!(full.insert(cap).err(), Some(Error::NoFreeSlot));
    assert_eq!((full.used(), full.peak()), (2, 2));
}

/// The global is a convenience over the per-table mark and must never read *below* it, which is
/// the only property the kernel relies on: it prints the global and would understate the boot
/// if this were ever false. It is a floor rather than an equality because the static is shared
/// by every instantiation in the binary, including the other tests in this module.
#[test]
fn the_global_high_water_mark_is_never_below_a_table_that_reached_it() {
    let cap = Cap {
        object: Obj::PageFrame(2),
        rights: Rights::WRITE,
    };
    let mut cs: CapabilityTable<Obj, 8> = CapabilityTable::new();
    for _ in 0..5 {
        cs.insert(cap).expect("eight slots hold five");
    }
    let (peak, ceiling) = highest_seen();
    assert!(
        peak >= cs.peak(),
        "the global said {peak} while a table had reached {}",
        cs.peak()
    );
    assert!(
        ceiling > 0,
        "a recorded peak carries the table it came from"
    );
}
