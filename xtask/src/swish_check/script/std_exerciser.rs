//! **A `std` program spawned by the progenitor rather than by the kernel's test harness**
//! (milestone 595): the slot-by-slot preview, the run whose every phrase is a slot or a page
//! landing where `std` looks for it, and the unexpanded-pattern refusal.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The `std_exerciser` rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **A `std` program, spawned by the progenitor rather than by the kernel's test harness**
    // (milestone 595 (provisional)). Until this, every `std` program that ran on nife was built by
    // `system_tests/src/user/std_service.rs`, and the progenitor had never produced a child in the layout
    // nife's `std` reads (`crates/std_runtime_contract`): eight fixed slots, three shared pages,
    // thirty-two stack pages. The preview first, because it is where a person learns the slots
    // moved, and slot 0 is not even the same kind of object as a native child's.
    line(
        0,
        "caps std_exerciser",
        &[
            "cap 0  untyped   heap.",
            "cap 1  endpoint  result   stdout and stderr",
            "cap 5  frame     clock",
            "cap 6  endpoint  entropy  WRITE",
            "cap 7  frame     config",
            "cap 8  frame     args",
        ],
    ),
    // **Every phrase is a slot or a page landing where `std` looks for it**; the program panics
    // when one is missing. `vec sum` is the heap (slot 0); any output is stdout (1); the two
    // `honestly unsupported` lines are slots 4 and 2 left empty; `wall clock ok` is slot 5 and
    // `CLOCK_PAGE`; `entropy ok` is 6; `config seeded` is 7 and `CONFIG_PAGE`, read before `main`;
    // the last line is `process::exit` reaching the supervisor as an exit. The stack has no phrase.
    // `args [...]` is DECISIONS §170 (how a foreign program is told what to do): the words reach
    // slot 8, and `args_os()` reads `argv[0]` first and the quoted phrase as one word.
    line(
        1,
        "std_exerciser one 'two words'",
        &[
            "hello from std on nife",
            "args [\"std_exerciser\", \"one\", \"two words\"]",
            "vec sum 149985000",
            "fs honestly unsupported",
            "net honestly unsupported",
            "wall clock ok",
            "entropy ok",
            "config seeded",
            "exiting through process::exit",
        ],
    ),
    // **An unquoted pattern is refused at the prompt, with nothing spawned** (§170): the words
    // carry no authority, so expanding one would hand over names and nothing they name.
    line(0, "std_exerciser *.rs", &["its words are not expanded"]),
    line(0, "echo shell-boot-gate-done", &["shell-boot-gate-done"]),
];
