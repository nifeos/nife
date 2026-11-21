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
verdict has one grammar in code: `Verdict` prints it and `ParsedVerdict` reads it. Two more modules
hold the agreement between the kernel, the first user program and the self-test program: the page
the kernel fills (`kernel_half`) and the slots, images and message words of the self-test program
(`self_test_program`). The crate is `no_std`, has no dependency and no `unsafe`, and two Kani
harnesses cover the verdict reader. Nothing in its text points into nife except one wire literal
and two example lines in the README (see What still blocks the move).

## Identity

| item | value |
|---|---|
| crate | `boot_ladder`, ratified (2026-09-14 UTC) |
| identity values | none for the console lines: the version policy in the README makes the Cargo version the contract's version, and no identity constant exists. `kernel_half::MAGIC` is the page's first word, `NIFESTK1` as little-endian bytes, marked as a wire literal (the wire-literal exemption, ruled 2026-11-14 UTC) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root; nothing adapted |
| `publish` | `false`, with the comment that it flips only in the release that publishes |
| size | six files under `src/`, 1721 lines: `verdict.rs` 781, `heads.rs` 293, `kernel_half.rs` 252, `self_test_program.rs` 239, `rungs.rs` 104, `lib.rs` 52 |
| dependencies | none |
| `unsafe` | no site |
| Kani harnesses | 2, in `src/verdict.rs`: `a_count_is_read_without_overflow` and `a_segment_is_read_without_panicking`, each with a falsification patch under `falsifications/` |
| mutation baseline | none recorded; not run |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `serial_console`, `system_initializer`, `kernel`, `xtask` |
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
| `cargo test -p boot_ladder` | 22 unit tests pass; 0 documentation tests |
| `cargo clippy -p boot_ladder --all-targets -- -D warnings` | no warning |
| `cargo doc -p boot_ladder --no-deps` with warnings denied | no warning |
| the template's naming checks (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`), run from a `nifeos/crate-workflows` checkout at `d760537` over `src/*.rs` | 330 names judged, 0 refused, each |
| the template's `question_shape` | 1 refused: the private `const fn str_eq` in `src/verdict.rs` returns `bool` and is not named like a question; the template's ceiling is 0 |
| the template's `abbreviations`, report only | 331 names judged, 0 components that are not words |
| `script/names boot_ladder` and `python3 helpers/name_release.py --draft boot_ladder` | the crate name ratified; the rest listed in Names below |
| `script/lint` | the full run passes, all 151 sections |

Not run, and so not claimed:

- The stranger test. No `docs/stranger-test.md` exists yet.
- The README's two examples. The crate does not include its README as documentation, so `cargo test`
  does not compile them; the stranger test is where they run.
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
| `kernel_half` | `MAGIC`, `KernelHalf` (`has`, `word`, `to_words`, `from_words`), `word::*` (twelve constants), `flag::*` (fifteen constants) |
| `self_test_program` | `slot::*`, `image::*`, `header::*`, `entropy_source::*`, `message::*`, the virtual-address and size constants, `image_virtual_address`, `images_fit` |

No nife type appears in a signature. What a stranger may find surprising:

- `check(name)` panics, at compile time when used in a constant, for a name not in
  `SELF_TEST_CHECKS`; that is its purpose, so an unlisted check does not build.
- `kernel_half` and `self_test_program` describe the agreement of one kernel, one first user program
  and one self-test program. A second system that builds its own self-test program may use
  `Heads`, `verdict` and the check list without them.
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
  UTC, 1078 option A). The names in `verdict`, `kernel_half` and `self_test_program` were outside
  that ruling and go to this release name review.
- The wire-literal exemption (2026-11-14 UTC) covers `NIFESTK1`; it does not cover console text.

## Names

The architect ratified the crate name on 2026-09-14 UTC, the nine head shapes and the fifteen
check names on 2026-11-14 UTC, and the eleven names in the first group below on 2026-11-19 UTC.
Everything else is provisional and is the architect's to ratify; this lane ratifies none of it and
changes none of it. The `Name:` blocks of the eleven ratified names in `src/heads.rs` and
`src/rungs.rs` still read `provisional`; the lane that records the 2026-11-19 ruling brings them
to match; the move strips the blocks.

| group | items | status |
|---|---|---|
| ratified, as built | crate `boot_ladder`; accessors `banner`, `machine`, `self_test`, `self_test_subject`, `core_runs`, `shell_banner`, `screen_held`; constants `SELF_TEST_FAILED`, `SELF_TEST_ABSENT`, `SELF_TEST_CHECKS`, `EARLIER_SELF_TEST_CHECK_COUNTS`; the fifteen check strings; `TRUE_RANDOM_NUMBER_GENERATOR` (slot) and `TRUE_RANDOM_NUMBER_GENERATOR_ANSWERED` (flag) | ratified |
| modules | `heads`, `rungs`, `kernel_half`, `self_test_program`, `verdict` | provisional; `heads` and `rungs` are private, the other three public |
| head types | `Head`, `Heads`, `Heads::new`, `Head::find_in`, `is_in`, `strip_prefix_of`, `parts` | provisional, to ratify |
| `verdict` | `Verdict`, `ParsedVerdict`, `Tally`, `Outcome`, `CheckLine`, `SubjectLine`, `CHECKS`, `PACKED_FIELD_BITS`, `check`, `subject_thread`, and the methods `new`, `record`, `is_recorded`, `fail_unrecorded`, `to_words`, `from_words`, `to_packed`, `from_packed`, `verdict`, `in_line`, `parse`, `absent_count`, `failed_count`, `is_red`, `absent_names`, `failed_names` | provisional, to ratify |
| `kernel_half` | `KernelHalf`, `MAGIC`, `word::*`, `flag::*` | provisional, to ratify |
| `self_test_program` | `slot`, `image`, `header`, `entropy_source`, `message` and their items, and the address and size constants | provisional, to ratify |
| a change proposed and not applied | `DIRECT_MEMORY_ACCESS_PEEK_VIRTUAL_ADDRESS` to `DIRECT_MEMORY_ACCESS_OBSERVER_VIRTUAL_ADDRESS` | the milestone proposes it; a two-way door, awaiting a ruling |
| a change proposed here | the private `str_eq` in `verdict.rs` to a question-shaped name, for example `strings_are_equal` | provisional, to ratify; it is private, so no reader of the crate sees it |

Two items to judge as names. The `self_test_program` and `kernel_half` modules name the agreement of
one kernel with its first user program; a second system may not want them in a crate named for
console lines. That is the first of the open questions below.

## What still blocks the move

| item | owner |
|---|---|
| Add the `repository` line to the manifest; the move tool's `add_repository` writes it. | move lane |
| Add `CHANGELOG.md`, `_typos.toml` and `docs/stranger-test.md` from the scaffold, and run the stranger test. The README and both licenses exist. | move lane |
| Rename the private `str_eq` so the template's `question_shape` check passes at its ceiling of 0. | maintainer |
| README lines 63 and 64 spell `nife self-test:` in the verdict's example. The README's own first example uses `example`; the two lines should too, since an externalized crate's prose never names nife. `kernel_half::MAGIC` keeps `NIFESTK1` under the wire-literal exemption. | maintainer |
| The `script/lint` section that refuses a head literal outside `crates/boot_ladder`, tests and fixtures (milestone 1078, P1-7). It is nife's gate, and as a new lint section it is a reviewed interface. | maintainer; not a block on the crate, a block on the milestone's Done when |
| Decide where the soak markers and job-mix field names that `notes/bugs-without-milestones/drivers-boot-and-install-1.md` proposes hoisting into this crate live, and land [milestone 972 (the boot stick's shared names are code)](../../design/roadmap/0972-the-boot-sticks-shared-names-are-code.md) or record that it waits (milestone 1078, P2-12). Milestone 972 is `NOT-STARTED`. | maintainer |
| Convert the five path dependents to `git` and `rev` with `version = "0.1.0"`, and read every file the move dry run lists. | move lane |
| Run `script/crate-move --dry-run` for this crate. It was not run here. | move lane |

None of these changes a printed byte.

## Proposed items

1. Rename `str_eq` (private) to a question-shaped name.
2. Replace the two `nife self-test:` lines in the README's verdict example with the example system's
   name.
3. Update the eleven `Name:` blocks of the ratified names to read ratified, in the lane that
   records the 2026-11-19 ruling.
4. Make `cargo test` compile the README's two examples (for instance `#![doc = include_str!(...)]`
   under `cfg(doctest)`), so a stranger's first code runs.
5. Write the lint section of P1-7 in nife.
6. Rule provisionally on the `self_test_program` and `kernel_half` names (about eighty items),
   including the one change milestone 1078 proposes.

## Open questions

1. Should `kernel_half` and `self_test_program` stay in this crate? The kernel, the
   first user program and the self-test program of one system use them, and no reader of console lines does. The
   crate's one-sentence purpose does not name them. Keeping them avoids a second repository and
   keeps one pin; splitting them leaves the console-line crate small and reusable by a second
   system. The maintainer found no second consumer today.
2. Does the milestone 972 growth (the boot stick's names) land before the move, or after, as a
   release here and a bump in nife?

## Recommendation

Clear the move, with no tag and no publication. The crate stands alone: no dependency, no `unsafe`,
no reference back into nife apart from the items above, a README with the matching rule, the
verdict grammar as shared code, and two proofs with falsification patches. The code carries the architect's rulings on names, and the open names sit above for ratification. The items under
What still blocks the move are small, none changes a printed byte, and the move lane or the
maintainer can land them before or with the move.

## Decisions for the architect

1. Clear the review, which is the go to create `nifeos/boot_ladder` and move the crate (one-way:
   the template's workflows and nife's pin act on the repository from its first day). Recommended: A.
   - A. Clear it now, with the questions above settled provisionally by the maintainer.
   - B. Hold until milestone 972's names land, so the move carries them.
2. Ratify the provisional names in the Names table, in one pass or group by group. Two-way before
   the first tag; one-way after it.
