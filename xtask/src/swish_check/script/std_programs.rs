//! **`std` programs at the prompt** (milestones 205 and 595, and milestone 121's `rg`
//! walk): `std_echo`, `std_grep` and unmodified `ripgrep`, each granted only what a word on
//! the line names.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The `std`-program rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **An installed `std` program run by path hears its words** (milestone 205 (how a foreign
    // program is told what to do)). `std_echo`'s note declares `ProgramArgumentDeclaration::Words` in the `std`
    // runtime, so the shell sent the line as its argv, the progenitor sized a `std` region from the
    // argv bit and built the `std` layout for an image, and `std::env::args_os()` read the page.
    // Unvouched, so its grants are §219's two pages at the `std` slots, and the preview says so.
    line(
        0,
        "caps installed/std-echo one",
        &[
            "cap 5  frame     clock",
            "cap 7  frame     config",
            "cap 8  frame     args",
            "its manifest note asks for: output bytes, its words, the clock, the configuration page",
        ],
    ),
    line(
        1,
        "installed/std-echo one 'two words'",
        &["words [\"installed/std-echo\", \"one\", \"two words\"]"],
    ),
    // **An image the size of `rg` runs from the prompt** (milestone 595 (the shell runs a `std`
    // program)). `crate::disk::INSTALLED_STD_ECHO_LARGE` is `std_echo` padded to 768 pages, twelve
    // times the cap this path had, so the shell staged it across two 2 MiB page-table spans, the
    // progenitor copied and hashed all of it in its image pool, and built it in a region sized from
    // its length. Before, the shell refused it as "larger than an image may be".
    line(
        1,
        "installed/std-echo-large big",
        &["words [\"installed/std-echo-large\", \"big\"]"],
    ),
    // **A foreign program's words designate what it may read, and nothing else** (milestone 205's
    // designation half, §170 (how a foreign program is told what to do) clauses 2 to 5). In a
    // directory of its own, `std_grep` (unvouched, so read-only) is granted `docs` because a word
    // named it, and finds the needle there. With no word naming anything it is granted nothing
    // (calef's N1 ruling, 2026-09-27T06:27Z), and its search of `.` fails loudly. Run by absolute
    // path, because the shell stands in `hay`; `/` is this shell's own root.
    line(0, "mkdir hay", &[]),
    line(0, "cd hay", &[]),
    line(0, "mkdir docs", &[]),
    line(0, "cd docs", &[]),
    line(0, "echo find the needle here > n.txt", &[]),
    line(0, "cd ..", &[]),
    line(
        0,
        "caps /installed/std-grep needle docs",
        &["cap 4  endpoint  dir", "docs", "read-only"],
    ),
    line(
        1,
        "/installed/std-grep needle docs",
        &["docs/n.txt:find the needle here"],
    ),
    line(
        1,
        "/installed/std-grep needle",
        &["std_grep: .: no directory was granted to search"],
    ),
    // **Unmodified `ripgrep` searches what its words name, from the prompt** (milestone 595 (the
    // shell runs a `std` program), fatal risk 1). The same lines `std_grep` answered, typed to
    // somebody else's program: crates.io's `rg` 14.1.1 as basalt packaged it, unvouched, carrying only the note `cargo
    // xtask foreign-note` linked in. `docs` is granted read-only because a word named it, and the
    // match is found there. `../hay/secret.txt` names the same directory's other file by a path
    // whose first component is not an entry here, so nothing is granted for it and `rg` cannot
    // open it, though the needle is in it: the caretaker refuses the `..` by name, in the parallel
    // walker's words when milestone 812 (`std::thread::spawn` runs real threads in one address
    // space) gives `rg` more than one core. With no word naming anything `rg` holds no directory and asks for its working
    // directory, which the PAL refuses by design (its `sys/paths/nife.rs`),
    // so `rg` says so with a wrong guess ("did your CWD get deleted?"). Typed only when
    // `cargo xtask basalt-fetch` has run, as CI's rows do.
    line(0, "echo the needle outside the grant > secret.txt", &[]),
    line(
        0,
        "caps /installed/rg needle docs",
        &[
            "cap 4  endpoint  dir",
            "(only docs in it, named on the line)",
            "...read-only",
            "provenance: unvouched",
        ],
    ),
    line(
        1,
        "/installed/rg needle docs ../hay/secret.txt",
        &[
            "docs/n.txt:find the needle here",
            "rg: ../hay/secret.txt: ", // then walkdir's "IO error for operation on", one core only
            "`..` would leave the granted directory, which no capability designates",
        ],
    ),
    line(
        1,
        "/installed/rg needle",
        &["rg: failed to get current working directory: operation not supported on this platform"],
    ),
    line(0, RG_WALK_CD, &[]),
    // **Unmodified `ripgrep` walks a real tree from the prompt** (milestone 121 (`ripgrep`:
    // enumeration as a capability), calef's 2026-10-07 ruling on fatal risk 1). `/search/crates` is
    // a copy of this repository's `crates/` (`crate::disk::RIPGREP_CORPUS`), hundreds of files in nested
    // directories, granted read-only because the word named it. No flag narrows the walk: `rg`
    // lists every directory it meets, opens every file, skips the binary ones itself, and prints
    // every matching line. The answer is wanted whole, so this line lists nothing here and
    // [`rg_walk_verdict`] compares it line for line with what the host's `rg` printed over the same
    // copy. Typed only when `cargo xtask basalt-fetch` has run, like the lines above.
    line(1, RG_WALK_LINE, &[]),
    line(0, "cd /", &[]),
];
