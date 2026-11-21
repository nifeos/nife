# Review package: `boot_ladder`

*Name: provisional, minted by lane milestone/1078-boot-ladder-review-package. The `boot_ladder`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together
outgrow the page.*

The architect reads this to clear the crate for its move to its own repository. It is the package
that [milestone 1078 (`boot_ladder`: what an outside maintainer would fix before it
leaves)](../../design/roadmap/1078-boot-ladder-outside-review.md) waits on. It describes the crate
as `main` leaves it. It gathers facts and proposes; it ratifies nothing.

## In one paragraph

The fixed heads of the lines a kernel and its shell print while booting, and the list of self-test
check names, as code that both the printer and the reader use. A system builds `Heads` from its own
name; the crate fixes each head's shape (`<system> on `, `<system> self-test: ` and five more), and
a `Head` prints through `Display` and finds itself in a line without allocating. The self-test
verdict has one grammar in code: `Verdict` prints it and `ParsedVerdict` reads it. The crate is
`no_std`, has no dependency and no `unsafe`, and two Kani harnesses cover the verdict reader.
Nothing in its text points into nife. The page the kernel fills for the self-test program, and
that program's slots, images and message words, are not in this crate. They are nife's
`self_test_contract`, which depends on this one.

## Identity

| item | value |
|---|---|
| crate | `boot_ladder`, ratified (2026-09-14 UTC) |
| identity values | none: the version policy in the README makes the Cargo version the contract's version, and no identity constant exists |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root; nothing adapted |
| `publish` | `false`, with the comment that it flips only in the release that publishes |
| size | four files under `src/`, 1233 lines: `verdict.rs` 780, `heads.rs` 293, `rungs.rs` 106, `lib.rs` 54 |
| dependencies | none |
| `unsafe` | no site |
| Kani harnesses | 2, in `src/verdict.rs`: `a_count_is_read_without_overflow` and `a_segment_is_read_without_panicking`, each with a falsification patch under `falsifications/` |
| mutation baseline | none recorded; not run |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `serial_console`, `kernel`, `xtask`, `self_test_contract` |
| the printer | the kernel prints the banner, the machine line and the per-check lines through `Heads`; the self-test program prints the verdict through `Verdict`; the shell prints its banner through `Heads::shell_banner` |
| the readers | `serial_console` and `xtask` read heads through `Heads` and the verdict through `ParsedVerdict`; `xtask` builds `Heads::new("nife")` |
| prose and tables | `helpers/trust_boundary_crates.tsv`, `helpers/architect-label-rules.py`, `helpers/error_paths.py`, `script/boot-check`, `script/netboot-rehearsal`, `script/stranger-test`, `script/verify` |
| a fuzz target | none |

Console text that spells a head by hand remains in tests and test fixtures (for example the
transcript fixtures of `xtask` and the screen tests of `serial_console`). Those literals are the
bytes the crate must keep printing, so a test that pins them is the intended check.

## What was run

Run from this lane's worktree on the head named in the pull request.

| check | result |
|---|---|
| `script/crate-review-packet boot_ladder` | the table in Public API below comes from it; 0 `unsafe` sites, 0 dependencies, no foreign type in a public signature, 2 harnesses with 2 patches, no reference back into nife and no parent-path literal |
| `script/crate-standalone boot_ladder` | one failure, `names`: 116 names not ratified, the table in Names below. The manifest, the files, the citations and the build on 1.88 for the host and the three bare-metal targets pass; Kani proves 2 of 2 harnesses; both falsification patches turn their harness red |
| `cargo test -p boot_ladder` | 19 unit tests and the README's 2 examples pass |
| `cargo clippy -p boot_ladder --all-targets -- -D warnings` | no warning |
| `cargo doc -p boot_ladder --no-deps` with warnings denied | no warning |
| the template's naming checks (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`), run from a `nifeos/crate-workflows` checkout at `d760537` over `src/*.rs` | 330 names judged, 0 refused, each |
| the template's `question_shape` | 0 refused; the private comparison is `strings_are_equal` |
| the template's `abbreviations`, report only | 331 names judged, 0 components that are not words |
| `script/names boot_ladder` and `python3 helpers/name_release.py --draft boot_ladder` | the crate name ratified; the rest listed in Names below |
| `script/lint` | the full run passes, all 151 sections |

Not run, and so not claimed:

- The stranger test. No `docs/stranger-test.md` exists yet.
- A mutation census. It falls outside its window, and the crate has no baseline.
- `cargo-semver-checks`, because a first release has no baseline.
- The template's workflows, because no repository exists.
- Any QEMU boot that exercises the printers.

## Public API

| kind | items |
|---|---|
| heads | `Heads::new(system)` and the seven accessors `banner`, `machine`, `self_test`, `self_test_subject`, `core_runs`, `shell_banner`, `screen_held`; each yields a `Head` with `find_in`, `is_in`, `strip_prefix_of`, `parts` and `Display` |
| the verdict's words | `SELF_TEST_FAILED` (`FAILED: `), `SELF_TEST_ABSENT` (`absent: `), `SELF_TEST_CHECKS` (fifteen names), `EARLIER_SELF_TEST_CHECK_COUNTS` (`[5, 13]`) |
| `verdict` | `CHECKS`, `PACKED_FIELD_BITS`, `check(name)`, `Outcome::{Passed, Failed, Absent}`, `Tally` (`new`, `record`, `is_recorded`, `fail_unrecorded`, `to_words`, `from_words`, `to_packed`, `from_packed`, `verdict`), `Verdict`, `CheckLine`, `SubjectLine`, `subject_thread`, `ParsedVerdict` (`in_line`, `parse`, `absent_count`, `failed_count`, `is_red`, `absent_names`, `failed_names`) |

No nife type appears in a signature. What a stranger may find surprising:

- `check(name)` panics, at compile time when used in a constant, for a name not in
  `SELF_TEST_CHECKS`; that is its purpose, so an unlisted check does not build.
- The verdict's example in `SELF_TEST_ABSENT`'s documentation counts `9 of 13`, and the README's
  counts `12 of 15`. Both are well formed; the second matches the current list.
- The README states that the kernel pads a check's name to ten characters and the longest names no
  longer line up. That is the kernel's column, not the crate's.

## Rulings

- The heads take the caller's system name and fix the shape after it (the architect, 2026-11-14 UTC, 1078
  decision W, option A). The architect ratified the nine shapes as printed; nife passes `nife`, so the
  printed bytes stay the same. The shell banner keeps its text and takes no period.
- The printed check names (the architect, 2026-11-14 UTC, 1078 decision C): `blockdev` became
  `block_device`, `trng` became `true_random_number_generator`, and the other thirteen are
  ratified, `device_interrupt` included. The slot and the flag that name the generator follow
  (`TRUE_RANDOM_NUMBER_GENERATOR`, `TRUE_RANDOM_NUMBER_GENERATOR_ANSWERED`). The crate keeps no list
  of earlier spellings.
- The seven `Heads` accessors and the constants `SELF_TEST_FAILED`, `SELF_TEST_ABSENT`,
  `SELF_TEST_CHECKS` and `EARLIER_SELF_TEST_CHECK_COUNTS` stand as built (the architect, 2026-11-19
  UTC, 1078 option A). The names in `verdict` were outside that ruling and go to this release name
  review.
- The review is cleared (the architect, 2026-11-21 UTC, option A). The crate moves to
  nifeos/boot_ladder with `publish = false`, no tag and no publication, carrying only the console
  lines. `kernel_half` and `self_test_program` stay in nife as `self_test_contract` (a provisional
  name), with `kernel_half::MAGIC`.

## Names

The architect ratified the crate name on 2026-09-14 UTC, the nine head shapes and the fifteen
check names on 2026-11-14 UTC, and the eleven names in the first group below on 2026-11-19 UTC.
Everything else is provisional and is the architect's to ratify. The `Name:` blocks of the eleven
ratified names read ratified; the move strips the blocks.

| group | items | status |
|---|---|---|
| ratified, as built | crate `boot_ladder`; accessors `banner`, `machine`, `self_test`, `self_test_subject`, `core_runs`, `shell_banner`, `screen_held`; constants `SELF_TEST_FAILED`, `SELF_TEST_ABSENT`, `SELF_TEST_CHECKS`, `EARLIER_SELF_TEST_CHECK_COUNTS`; the fifteen check strings; `TRUE_RANDOM_NUMBER_GENERATOR` (slot) and `TRUE_RANDOM_NUMBER_GENERATOR_ANSWERED` (flag) | ratified |
| modules | `heads`, `rungs`, `verdict` | provisional; `heads` and `rungs` are private, `verdict` public |
| head types | `Head`, `Heads`, `Heads::new`, `Head::find_in`, `is_in`, `strip_prefix_of`, `parts` | provisional, to ratify |
| `verdict` | `Verdict`, `ParsedVerdict`, `Tally`, `Outcome`, `CheckLine`, `SubjectLine`, `CHECKS`, `PACKED_FIELD_BITS`, `check`, `subject_thread`, and the methods `new`, `record`, `is_recorded`, `fail_unrecorded`, `to_words`, `from_words`, `to_packed`, `from_packed`, `verdict`, `in_line`, `parse`, `absent_count`, `failed_count`, `is_red`, `absent_names`, `failed_names` | provisional, to ratify |
| private | `strings_are_equal` in `verdict.rs` | provisional; no reader of the crate sees it |

The names of `kernel_half` and `self_test_program` are nife's, in `self_test_contract`, and are not
part of this review.

## What still blocks the move

| item | owner |
|---|---|
| Add the `repository` line to the manifest; the move tool's `add_repository` writes it. | move lane |
| Add `CHANGELOG.md`, `_typos.toml` and `docs/stranger-test.md` from the scaffold, and run the stranger test. The README and both licenses exist. | move lane |
| The `script/lint` section that refuses a head literal outside `boot_ladder`, tests and fixtures (milestone 1078, P1-7). It is nife's gate, and as a new lint section it is a reviewed interface. | maintainer; not a block on the crate, a block on the milestone's Done when |
| Decide where the soak markers and job-mix field names that `notes/bugs-without-milestones/drivers-boot-and-install-1.md` proposes hoisting into this crate live, and land [milestone 972 (the boot stick's shared names are code)](../../design/roadmap/0972-the-boot-sticks-shared-names-are-code.md) or record that it waits (milestone 1078, P2-12). Milestone 972 is `NOT-STARTED`. | maintainer |
| Convert the five path dependents to `git` and `rev` with `version = "0.1.0"`, and read every file the move dry run lists. | move lane |

None of these changes a printed byte.

## Proposed items

1. Write the lint section of P1-7 in nife.

## Open questions

1. Does the milestone 972 growth (the boot stick's names) land before the move, or after, as a
   release here and a bump in nife?
