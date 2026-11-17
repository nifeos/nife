# The ledger

An appendix to [the milestone](../1088-elf-outside-review.md).

| item | state |
|---|---|
| before the move: citations, fuzz seeds, manifest and files | done: shipped text cites nothing of this tree, the seeds live in `tests/fixtures/`, and the manifest states every key as a literal with `rust-version` 1.88, built and tested at 1.88. The license files are in the crate. Left for the move: the scaffold's other files and the fuzz target's move |
| `page_range` and a page size as input | done: `PageBytes` holds only a power of two, `page_range` takes one, and `Elf::parse_with` gives the overlap check the loader's page size. The kernel passes its frame size |
| four `as usize` casts | done: `phoff`, `p_offset`, `filesz` and `memsz` convert with `usize::try_from` (`errors_print_and_a_64_bit_field_is_never_truncated`); the three bounds harnesses keep their signature |
| `Display`, `core::error::Error` and derives | done: both errors print, box and hash, and `Elf` derives `Debug`, `Clone` and `Copy` |
| the public vocabulary | done: the names D3 ruled, across the crate and its dependents, each with the specification's spelling as its alias |
| a README and a shared forge | done: the README and `examples/parse_static_executable.rs`. Left: the shared forge. The unit tests, the doc examples and the dependents each forge differently, so one forge is a test refactor with no finding behind it |
| the proofs' hold | done: nine harnesses, with the program-header offset proved in bounds, and the module doc says what they cover |
| an entry in the zero-filled tail | done: refused as `EntryNotExecutable`, a maintainer ruling, provisional (2026-11-16 UTC). Nothing is published, and a refusal added now can be dropped later without breaking a file that passed |
| what the parser ignores | done: the README's "What it does not check" |
| `segments()` discards an error | done: `segments` states the invariant in a comment; `exactly_the_maximum_header_count_is_accepted` already counts |
| four timeouts | left: the first weekly run |
| the note padding rule | done: `padding` says which values read as 4-byte padding and where LLVM differs, from the review's recollection of LLVM, not a re-read of its source |
