//! Tests of the program table: its wire ids, its names, the `const` checks that guard it, and
//! properties every row's manifest must keep.

use super::*;
use crate::*;

/// **The wire ids already shipped, pinned** (milestone 150, written before the bookkeeping was
/// generated so that the generation had something to be wrong against).
///
/// [`Prog::id`] is a thing two programs agree on: the shell sends it and the progenitor decodes
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
        match Prog::from_name(name.as_bytes()) {
            Some(p) => assert_eq!(p.id(), id, "`{name}` was renumbered from {id}"),
            None => {
                if let Some(other) = Prog::from_id(id) {
                    panic!(
                        "wire id {id} belonged to `{name}` and is now reused by `{}`",
                        other.name()
                    );
                }
            }
        }
    }
}

/// **A `std` program declares only what the progenitor can place in the std layout**
/// (milestone 595 (provisional)). `std_runtime_contract` fixes eight slots, and the progenitor's
/// std spawn fills the heap, the output, the clock, the configuration page, entropy and a
/// directory. Everything else a manifest can ask for has no slot there or is not wired yet:
///
/// - a file or an input stream: the contract has no slot for either (std's stdin is unbuilt);
/// - `--mem`: slot 0 is already the heap, and the progenitor's, not the shell's;
/// - a process domain: native `DOMAIN_SLOT` is 7, which in the std layout is the config page;
/// - a second output stream: native `DIAGNOSTICS_SLOT` is 8, past the fixed eight.
///
/// The network is held since milestone 809 (the package client becomes a program): the
/// progenitor places the stack at slot 2 and the job's own region at slot 3, which is the
/// budget the net PAL retypes each socket's frame from.
///
/// The shell would plan any of these and the progenitor would silently not deliver it, so
/// this is the gate: a manifest cannot declare one without failing here first.
#[test]
fn a_std_program_declares_only_what_the_std_layout_can_hold() {
    let mut std_programs = 0;
    for &p in Prog::ALL {
        let m = p.manifest();
        if m.startup != StartupContract::Std {
            continue;
        }
        std_programs += 1;
        let name = p.name();
        assert_eq!(
            m.file,
            FileSpec::Forbidden,
            "{name}: no std slot carries a file"
        );
        assert_eq!(
            m.input,
            InputSpec::Forbidden,
            "{name}: no std slot carries an input"
        );
        assert_eq!(m.mem, MemSpec::Forbidden, "{name}: slot 0 is the heap");
        assert!(
            !m.domain,
            "{name}: DOMAIN_SLOT is the config page's in the std layout"
        );
        assert!(!m.interruptible, "{name}: no std slot carries a job frame");
        assert_eq!(
            m.output,
            OutputSpec::Bytes,
            "{name}: std's output is one byte stream"
        );
    }
    assert!(std_programs > 0, "a sweep over nothing proves nothing");
}

/// **No manifest declares both a file and an input** (milestone 150). Both take a bare name on
/// the command line, so a program declaring both would leave the parser two indistinguishable
/// positions and nothing but order to tell them apart; `plan_against_with`'s input operand
/// relies on the combination never occurring. That was a sentence in a comment until this
/// test; an argument together with an input is a different case and is allowed (see
/// design/roadmap/proposals/a-program-that-takes-an-argument-and-an-input.md).
#[test]
fn no_program_declares_both_a_file_and_an_input() {
    for &p in Prog::ALL {
        let m = p.manifest();
        assert!(
            !(matches!(m.file, FileSpec::Required { .. })
                && matches!(m.input, InputSpec::Required { .. })),
            "{} declares a file and an input, and the command line cannot tell them apart",
            p.name()
        );
    }
}

/// **Every program round trips through its id and its name, and every other id is a hole.**
///
/// Since milestone 150 the id, the name and [`PROG_COUNT`] are generated from one declaration
/// (`programs!`), so the failures this test used to exist for (an arm missing from
/// `from_id` or `from_name`, and [`PROG_COUNT`] not widened, which hid both) cannot be written
/// any more. What is left is worth keeping as a statement of the contract the progenitor relies
/// on: it sizes its table by [`PROG_COUNT`] and indexes it by id, so every declared id must fit,
/// and an id nothing declares must decode to `None` rather than to a neighbour.
///
/// The history is in git and in notes/adding-a-program.md: this test's own doc comment once
/// claimed it caught a forgotten [`PROG_COUNT`], and measurement on 2026-08-18 found it could
/// not, because the sweep counted up to the constant it was meant to check.
#[test]
fn prog_id_round_trips() {
    assert!(!Prog::ALL.is_empty(), "a sweep over nothing proves nothing");
    for &p in Prog::ALL {
        assert!(
            (p.id() as usize) < PROG_COUNT,
            "{p:?}'s id {} does not fit the progenitor's table of {PROG_COUNT}",
            p.id()
        );
        assert_eq!(Prog::from_id(p.id()), Some(p));
        assert_eq!(Prog::from_name(p.name().as_bytes()), Some(p));
    }
    for id in 0..PROG_COUNT as u64 + 8 {
        if let Some(p) = Prog::from_id(id) {
            assert!(Prog::ALL.contains(&p));
            assert_eq!(
                p.id(),
                id,
                "{p:?} does not answer to the id it decoded from"
            );
        }
    }
    assert_eq!(Prog::from_id(PROG_COUNT as u64), None);
    assert_eq!(Prog::from_name(b"no_such_program"), None);
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
    for &p in Prog::ALL {
        let expected = if p == Prog::Jig { Some("") } else { None };
        assert_eq!(p.resolver_zone(), expected, "{}", p.name());
    }
}
