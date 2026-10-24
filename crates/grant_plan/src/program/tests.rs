//! Tests of the program table: its wire ids, its names, the `const` checks that guard it, and
//! properties every row's manifest must keep.

use super::*;
use crate::*;

/// **The wire ids already shipped, pinned** (milestone 150, written before the bookkeeping was
/// generated so that the generation had something to be wrong against).
///
/// [`Program::id`] is a thing two programs agree on: the shell sends it and the progenitor decodes
/// it, so a renumbering is a flag day rather than a refactor. Two rules, both checked per row:
/// **a name keeps its id** while it exists, and **an id is never reused** by another name once
/// its program is gone. Removing a program therefore leaves this table alone and its id a hole;
/// a program added later needs no row here, because its id is a literal in its own declaration
/// and a change to that literal is visible in review. Rows are only ever appended, and only if
/// someone wants a later id pinned the same way.
#[test]
fn the_wire_ids_already_shipped_never_move() {
    const SHIPPED: [(&str, u64); 13] = [
        ("least_authority_demo", 0),
        ("memory_grant_depleter", 1),
        ("interrupt_heeder", 2),
        ("interrupt_ignorer", 3),
        ("date", 4),
        ("rm", 5),
        ("wc", 6),
        ("mdr", 7),
        ("ps", 8),
        ("pgrep", 9),
        ("uptime", 10),
        ("printenv", 11),
        ("uuid", 12),
    ];
    for (name, id) in SHIPPED {
        match Program::from_name(name.as_bytes()) {
            Some(p) => assert_eq!(p.id(), id, "`{name}` was renumbered from {id}"),
            None => {
                if let Some(other) = Program::from_id(id) {
                    panic!(
                        "wire id {id} belonged to `{name}` and is now reused by `{}`",
                        other.name()
                    );
                }
            }
        }
    }
}

/// **A `std` program declares only what the progenitor can place in the std layout** (milestone
/// 595 (provisional)), whose slots `std_runtime_contract` fixes: no file (no std slot carries
/// one), no `--mem` (slot 0 is the heap, and the progenitor's), no process domain (native
/// `DOMAIN_SLOT`, 7, is the std configuration page), no job frame, and one output byte stream
/// (native `DIAGNOSTICS_SLOT`, 8, is the std argv page). The network and an input stream it may
/// hold, at the std layout's own slots. The shell would plan any of the refused ones and the
/// progenitor would silently not deliver it, so a manifest declaring one fails here first.
#[test]
fn a_std_program_declares_only_what_the_std_layout_can_hold() {
    let mut std_programs = 0;
    for &p in Program::ALL {
        let m = p.manifest();
        if m.startup != StartupContract::Std {
            continue;
        }
        std_programs += 1;
        let name = p.name();
        assert_eq!(
            m.file,
            FileGrantDeclaration::Forbidden,
            "{name}: no std slot carries a file"
        );
        assert_eq!(
            m.memory_grant,
            MemoryGrantDeclaration::Forbidden,
            "{name}: slot 0 is the heap"
        );
        assert!(
            !m.domain,
            "{name}: DOMAIN_SLOT is the config page's in the std layout"
        );
        assert!(!m.interruptible, "{name}: no std slot carries a job frame");
        assert_eq!(
            m.output,
            OutputDeclaration::Bytes,
            "{name}: std's output is one byte stream"
        );
    }
    assert!(std_programs > 0, "a sweep over nothing proves nothing");
}

/// **No manifest declares both a file and an input** (milestone 150). Both take a bare name on
/// the command line, so a program declaring both would leave the parser two indistinguishable
/// positions and nothing but order to tell them apart; `plan_against_with`'s input operand
/// relies on the combination never occurring. An argument together with an input is a different
/// case and is allowed (milestone 498 (whether a program may take an argument and an input
/// stream)).
#[test]
fn no_program_declares_both_a_file_and_an_input() {
    for &p in Program::ALL {
        let m = p.manifest();
        assert!(
            !(matches!(m.file, FileGrantDeclaration::Required { .. })
                && matches!(m.input, InputDeclaration::Required { .. })),
            "{} declares a file and an input, and the command line cannot tell them apart",
            p.name()
        );
    }
}

/// **Every program round trips through its id and its name, and every other id is a hole.**
///
/// The id, the name and [`PROGRAM_COUNT`] are generated from one declaration (`programs!`), so
/// this states the contract the progenitor relies on: it sizes its table by [`PROGRAM_COUNT`] and
/// indexes it by id, so every declared id must fit, and an id nothing declares must decode to
/// `None` rather than to a neighbour.
#[test]
fn prog_id_round_trips() {
    assert!(
        !Program::ALL.is_empty(),
        "a sweep over nothing proves nothing"
    );
    for &p in Program::ALL {
        assert!(
            (p.id() as usize) < PROGRAM_COUNT,
            "{p:?}'s id {} does not fit the progenitor's table of {PROGRAM_COUNT}",
            p.id()
        );
        assert_eq!(Program::from_id(p.id()), Some(p));
        assert_eq!(Program::from_name(p.name().as_bytes()), Some(p));
    }
    for id in 0..PROGRAM_COUNT as u64 + 8 {
        if let Some(p) = Program::from_id(id) {
            assert!(Program::ALL.contains(&p));
            assert_eq!(
                p.id(),
                id,
                "{p:?} does not answer to the id it decoded from"
            );
        }
    }
    assert_eq!(Program::from_id(PROGRAM_COUNT as u64), None);
    assert_eq!(Program::from_name(b"no_such_program"), None);
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// The const-evaluated checks the program table leans on, run on the cases they exist to refuse.
#[test]
fn duplicate_ids_and_names_are_found_wherever_they_sit() {
    assert!(all_distinct_ids(&[]) && all_distinct_ids(&[7]) && all_distinct_ids(&[1, 2, 3]));
    assert!(!all_distinct_ids(&[1, 1]));
    assert!(!all_distinct_ids(&[1, 2, 1]));
    assert!(!all_distinct_ids(&[1, 2, 3, 3]));
    assert!(all_distinct_names(&["a", "b", "ab"]));
    assert!(!all_distinct_names(&["a", "a"]));
    assert!(!all_distinct_names(&["a", "b", "a"]));
    assert!(!all_distinct_names(&["a", "b", "c", "c"]));
}

#[test]
fn same_bytes_is_equality_of_length_and_every_byte() {
    assert!(same_bytes(b"", b""));
    assert!(same_bytes(b"ab", b"ab"));
    assert!(!same_bytes(b"ab", b"ac"));
    assert!(!same_bytes(b"ab", b"abc"));
    assert!(!same_bytes(b"abc", b"ab"));
    assert!(!same_bytes(b"ba", b"aa"));
}

/// **`jig` resolves names in the root zone and no other program resolves any** (milestone 892 (a
/// std program at the prompt holds the network and a resolver), calef's ruling Q5 on #1884). The
/// table is the whole of who may resolve, since an installed manifest cannot carry a zone.
#[test]
fn only_jig_has_a_resolver_zone_and_it_is_the_root() {
    for &p in Program::ALL {
        let expected = if p == Program::Jig { Some("") } else { None };
        assert_eq!(p.resolver_zone(), expected, "{}", p.name());
    }
}
