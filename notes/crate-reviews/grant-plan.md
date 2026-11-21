# Review package: `grant_plan`

*The `grant_plan` package
of [crate-reviews.md](../crate-reviews.md), written with `script/crate-review-packet grant_plan`.
It is the first of two for the split in milestone 1094 (`grant_plan`: what an outside maintainer would fix before it leaves);
[grant-contract.md](grant-contract.md) is the second.*

## In one paragraph

`grant_plan` turns one line a person typed at a shell into a plan: which program runs, which
holdings each stream reaches, and which of the person's authorities the program receives. It
tokenizes the line, parses pipelines and redirections, expands wildcards a directory listing at a
time, resolves paths against the person's current directory and bindings, and refuses with a typed
`Refusal` that carries its own sentence. It is `no_std` with no allocator and no `unsafe`. It
depends on the leaf `grant_contract` (the manifest types and the slot constants a program links),
on `byte_glob` and on `program_argument_contract`. The catalog of nife's programs stays in
`program_table`, which depends on it only in tests.

## Identity

| item | value |
|---|---|
| crate | `grant_plan`; the crate name is ratified; `script/crate-standalone` reports the names in it that are not |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; none |
| license | inherited from the nife workspace; no `LICENSE-MIT` or `LICENSE-APACHE` in the crate |
| `publish` | `false` |
| size | 21 files, 8,950 lines under `src/`, about 3,000 of tests |
| consumers in nife | `components`, `program_table`, `swish`, `timetable`, `walk_pricing` (dev), `fixtures`, `kernel`, `system_tests` (dev) |

## Public API

The public modules are `expansion`, `line`, `navigation` and `word`; `command`, `escalation`,
`lexing`, `parsing`, `planning` and `refusal` are private, and the crate root re-exports what a
caller needs. The groups:

| group | items |
|---|---|
| lexing | `lexing::MAXIMUM_FILE_NAME_BYTES` and the tokenizer over a byte line |
| parsing | the pipeline and redirection grammar; `kill_operands` comes back as `Result<_, ()>` (item 7 below) |
| expansion | `expansion::Expander`, `Name`, `NameSet`, `Batch`, `Resume`, `Designation`, `Expansion`; `MAXIMUM_NAME_SET_BYTES` |
| navigation | `navigation::DirectoryPath`, `Bindings`, `BindEntry` (`position`), `TwoRoots`, `Path`, `GrantSide`, `Step`, `PathRefusal`, `BindRefusal`; `MAXIMUM_NAME_BYTES`, `MAXIMUM_DIRECTORY_NAME_BYTES`, `MAXIMUM_RENDERED_PATH_BYTES` |
| planning | the holdings, expansion and stream types the planner reads, and the plan it returns |
| refusal | `Refusal`, with its sentence built from the constants that bound it |
| command | `Command`, whose builtin variants keep the typed word (`Fg`, `Bg`, `Cd`, `Pwd`, `Ls`), and `CommandLine` |
| escalation | `Escalation` and `COOPERATIVE_GRACE_POLLS` |

The full list, with signatures, is in the packet (`script/crate-review-packet grant_plan`, kept
under `target/`). The list leaves out the items of an `impl`.

## Proofs and falsification patches

None. The crate has no Kani harness and no falsification patch. 31 test files and modules cover the
tokenizer, the parser, expansion, navigation, planning and the refusal sentences; the review
(milestone 1094) counted the cases. Item 1 below proposes harnesses for the claims a test cannot
cover: no input indexes past a line, and no path walk leaves the person's root.

## Names

`python3 helpers/name_release.py --draft grant_plan` lists 202 names: 82 ratified (2026-11-20 UTC),
11 provisional (the modules and a few older items) and 109 with no block, mostly ordinary nouns and
verbs on methods and fields that the architect brings by family. The ratified names include the
module names `navigation` and `expansion`, the unit-spelled `MAXIMUM_*_BYTES` family,
`COOPERATIVE_GRACE_POLLS`, `BUILTINS`, `line_verb`, `Designation`, `Scope`, `PathRefusal`,
`BindRefusal`, `DirectoryPath`, `GrantSide` and the expansion methods `next_resume`, `try_push`,
`try_add_name`, `designate_here` and `Expansion::new`.

The crate carries no `doc(alias)` of an earlier nife spelling.

## Naming rules

Checked with `helpers/name_checks.py` (`script/lint`, "names meet the current naming rules"): no
finding.

## License

The manifest inherits `license` from the nife workspace. The move writes `MIT OR Apache-2.0` into
the crate's own manifest and adds both license files. The crate adapts no source.

## What the release still needs

| item | owner |
|---|---|
| A README, with an example a doctest runs. | the move lane |
| `rust-version`, `repository`, and the license files. | the move lane |
| A review of 85 milestone citations, 65 section citations and 39 nife paths in shipped text, which the move strips or rewrites. | the move lane |
| The tag `v0.1.0`. | calef |

## Proposed items

These findings of the outside review remain unbuilt. Each is Open in milestone 1094.

1. P1-5: a Kani harness per claim, with a falsification patch for each.
2. P0-3: the manifest and the repository root belong to the workspace; the move writes them.
3. P1-9: documentation written for a stranger, and P1-9b: `missing_docs` denied once it holds.
4. P1-11: take `&ShellHoldings`, `&Expansion` and `&Streams` instead of two-kilobyte values by value.
   The milestone records no ruling for it, so it is not built.
5. P1-13: an undeclared widening option in `Manifest::is_well_formed`.
6. P1-14: a mutation run on the split crates.
7. P2-16: the `Result<_, ()>` return of `kill_operands`.
8. P2-19: a runnable example under `examples/`.

## Open questions

None for this package. The names with no block are the architect's to rule by family, and the clearance covers
`grant_plan` and `grant_contract` together.

## Result of `script/crate-standalone`

The run fails. The first is the names not yet ratified, which the architect clears by
ruling on them (`python3 helpers/name_release.py --draft grant_plan` prints the table). The rest are
the manifest, the missing README and license files and the citations listed above, which are the
move's work and not defects of the source. The run skips the build and proofs because the copy
needs the sibling crates. The six path dependencies resolve to sibling crates in
nife, three of them dev-dependencies that the move rewrites.

## Recommendation

Ready for review.
