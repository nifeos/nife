# Review package: `address_space_layout`

*Name: provisional. The `address_space_layout` package of [crate-reviews.md](../crate-reviews.md),
in its own file because the packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 3
of [milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
The review that raised the findings is [milestone 1076 (`address_space_layout`: what an outside
maintainer would fix before it leaves)](../../design/roadmap/1076-address-space-map-outside-review.md).
This package describes the crate as `main` leaves it. The architect does not need to read the
milestone first; it holds the full item text.

## In one paragraph

The one table every program of a process agrees on: where its image, stack, heap and shared windows
sit in the low half of its address space. Eight bands tile the first 2 GiB, the same on aarch64,
riscv64 and x86_64. `const fn` checks (`pair_page`, `runtime_window`, `service_window` and the
whole-window `window`) stop the build when a page or a window is placed outside its band, and
`check_image` tells a loader why an image does not fit. The crate is `no_std`, has no dependency
and no `unsafe` (`#![forbid(unsafe_code)]`), and eleven Kani harnesses prove the layout. The kernel,
the program builder, the runtime, the linker script test and every fixture that shares a page read
it. Nothing in the crate points back into nife except a few lines of text listed under what still
blocks the move.

## Identity

| item | value |
|---|---|
| crate | `address_space_layout`, ratified (decision N1, ruled 2026-11-14 UTC) |
| identity values | `INTERFACE_ID` 2 and `VERSION` 1, from row 2 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word: the crate has no wire form. `VERSION` and `INTERFACE_ID` still carry provisional `Name:` blocks (see the names table) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0` in the manifest; the two license texts are not yet in the crate directory |
| `publish` | `false`; a separate ruling holds every first publish until the last outside-review milestone closes |
| size | `src/lib.rs` 569 lines and `src/verification.rs` 234; 15 tracked files in all |
| dependencies | none |
| mutation baseline | in nife's triage file: one `check_image` mutant killed and one `>` to `>=` mutant triaged equivalent. Not re-run for this package |

## Who in nife depends on it

Twenty-seven manifests name the crate (twenty-five in the review packet, plus the root
`Cargo.toml` and the crate's own):

| kind | where |
|---|---|
| path dependency, normal | `components`, `kernel`, `fixtures`, `jig`, `xtask`, `redoxfs_server`, `c_seam`, `capability_witness_contract`, `confined_system_call_fuzzer_contract`, `current_cpu_contract`, `driver_process_contract`, `free`, `hot_swap_contract`, `initial_ramdisk_writer`, `machine_statistics_contract`, `memory_region_heap`, `process_builder`, `slabtop`, `soak_page`, `startup_page_contract`, `supervision_contract`, `system_initializer`, `user_mode_runtime`, `video_terminal` |
| path dependency, development | `std_runtime_contract`, `system_tests` |
| prose and tables | `packages/contracts.package.toml`, `helpers/trust_boundary_crates.tsv`, `crates/externalization-list.txt`, `helpers/interface_declarations_check.py` |

The linker script test now lives in `user_mode_runtime` (`src/linker_script.rs`), which parses its
own `link.ld` against `address_space_layout::IMAGE_BASE`. No test in the crate reads a file outside
it, and no `include_str!` remains. The crate has no fuzz target.

## What was run

All of these ran on this lane's tree, at the commit the pull request carries.

| check | result |
|---|---|
| `script/crate-standalone address_space_layout` | five failures, all known and all the move's work: `names` (27 names not ratified), `files` (no `LICENSE-MIT`, no `LICENSE-APACHE`), `citations` (one milestone citation at `src/lib.rs:425`, one path into the nife tree at `src/verification.rs:4`). Passing: the build on 1.88 for the host and the three bare-metal targets (`aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none`), Kani 11 of 11 harnesses verified with 0 failures, and all eleven falsification patches turn exactly their own harness red |
| `cargo test -p address_space_layout` | 7 unit tests and 2 doctests pass |
| `cargo clippy -p address_space_layout --all-targets -- -D warnings` | no warning |
| `cargo doc -p address_space_layout --no-deps` with `RUSTDOCFLAGS="-D warnings"` | no warning |
| the template's `spelled_out`, `unit_symbols`, `question_shape` and `british_spelling` over both source files | 109 names judged, 0 refused each |
| the template's `register_names` over both source files | refuses five Kani harness names that contain the word `address` (item 13, below). Nothing else |
| `script/names address_space_layout` and `python3 helpers/name_release.py --draft address_space_layout` | the crate name ratified; `INITIAL_RAMDISK_VIRTUAL_ADDRESS`, `MAXIMUM_STACK_PAGES` and the `verification` module ratified; `VERSION`, `INTERFACE_ID` and `window` provisional; every other public item has no block |
| `script/crate-review-packet --no-rustdoc address_space_layout` | the packet the tables above draw on; its public API list is the grep fallback, so signatures were read from the source |

Not run, and so not claimed: the stranger test (`script/stranger-test` on the README), the
template's workflows (`check`, `proofs`, `weekly`, `msrv`, mutation, semver), a fresh
`cargo mutants` run, `cargo package`, `cargo-semver-checks` (a first release has no baseline),
`script/crate-move --dry-run`, and `cargo xtask std-src` against a git dependency. The 1.88 floor
is the lowest toolchain installed here; the template's `msrv` job confirms the exact floor.
`script/lint` is run by the lane before it pushes, and CI runs it again.

## Public API

| kind | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| page size | `PAGE` (4096) |
| type | `Band { start, end }` with `bytes`, `contains`, `holds` |
| bands | `NULL_GUARD`, `PAIR_PAGES`, `RUNTIME_WINDOWS`, `HEAP`, `SERVICE_WINDOWS`, `IMAGE`, `STACK`, `PROCESS_PAGES`, and `BANDS: &[(&str, Band)]` in address order |
| addresses | `CODE_MODEL_CEILING`, `IMAGE_BASE`, `STACK_TOP`, `STACK_TOP_PAGE`, `CURRENT_CPU_PAGE`, `INITIAL_RAMDISK_VIRTUAL_ADDRESS` |
| count | `MAXIMUM_STACK_PAGES` |
| checked placement | `window(band, start_virtual_address, bytes)`, `pair_page`, `runtime_window`, `service_window` |
| image check | `check_image(start, end)`, `ImagePlacement::{TooLarge { image_end, stack_base }, OutsideBand { image_start, image_end }}` |

No nife type appears in a signature. What a stranger may find surprising:

- Every band and fixed window sits at one address in every process, so there is no address-space
  layout randomization. The crate root and the README say so, and what the design buys.
- `Band`'s fields are public, so a caller can build an inverted band and `bytes` then underflows
  (item 15, below).
- `window` checks a window against its band, not against another window in the same band; the
  crate that places two windows asserts that they are disjoint.
- `LOW_HALF_SPLIT_SHIFTS` restates the paging code's split shifts and is private, so no test in
  `page_tables` can tie the two (item 17, below).

## Proofs

Eleven Kani harnesses, each with a falsification patch under `falsifications/`: every address
below 2 GiB lies in exactly one band and the map lies in the user half of all three architectures;
`BANDS` is in address order; each of `pair_page`, `runtime_window` and `service_window` accepts
every page of its band and refuses every address its documentation forbids; `window` accepts
every page-aligned, non-empty window its band holds and refuses a malformed or overrunning one;
and `check_image` places, refuses or calls outside exactly as documented. The harnesses name no
unproved assumption. Two further facts the crate checks at compile time rather than by proof: the
bands tile `[0, 2 GiB)` with no gap, and the stack and process pages share one 2 MiB page-table
window.

## Rulings in force

- The crate is `address_space_layout`, in code and as the package name, with no alias of the
  earlier spelling (decision N1, option A, ruled 2026-11-14 UTC).
- The band numbers and `MAXIMUM_STACK_PAGES` are ratified by §171 (where a program image starts),
  option D.
- The whole-window check `window` is the maintainer's provisional ruling of 2026-11-14 UTC, a
  two-way door before the first tag. The three one-page functions are wrappers over it.
- The maintainer rules two-way names provisionally; item 7, item 15 and item 17 are in that class.

## Names

Everything below is provisional or unblocked by a ruling. None of it is ratified by this package;
the architect ratifies, renames or sends it back.

| item | name as built | state | reason | the architect's to ratify |
|---|---|---|---|---|
| 7 | `PAGE` | provisional, not built | a length in bytes; the crate rule for a page contract spells it `PAGE_BYTES` | yes |
| 7 | `IMAGE_BASE`, `STACK_TOP`, `STACK_TOP_PAGE`, `CURRENT_CPU_PAGE`, `CODE_MODEL_CEILING` | provisional, not built | each is an address; the rule says `_VIRTUAL_ADDRESS`, so the recommendation is `IMAGE_BASE_VIRTUAL_ADDRESS` and the like | yes |
| 7 | `ImagePlacement` fields `image_start`, `image_end`, `stack_base` | provisional, not built | addresses; recommendation `_virtual_address` on each | yes |
| 7 | 90 call sites in nife (`IMAGE_BASE` 42, `PAGE` 23, `STACK_TOP_PAGE` 20, `CURRENT_CPU_PAGE` 2, `CODE_MODEL_CEILING` 2, `STACK_TOP` 1) | | the rename lands in the move pull request, where every dependent's manifest changes anyway, with `script/names` listing the sites first | |
| 9 | `window` | provisional, built | one public function that checks a whole window against its band; the alternative is three sibling spans, which triples the surface. The `Name:` block says the architect may revisit it before the first tag | yes |
| 15 | `Band::new` | provisional, not built | a `const` constructor asserting `start < end` and page alignment, so a band built through it cannot be inverted. The fields stay public. Adding it removes nothing | yes |
| 17 | public `LOW_HALF_SPLIT_SHIFTS` | provisional, not built | `page_tables` needs the constant to test its three `SPLIT_SHIFT`s against it; making it `pub` adds one public name | yes |
| - | `VERSION`, `INTERFACE_ID` | provisional | the pair `block_roster_contract` and the other contract crates ratified; this crate's blocks were minted by the lane that added them | yes, as built |
| - | `INITIAL_RAMDISK_VIRTUAL_ADDRESS`, `MAXIMUM_STACK_PAGES` | ratified | | no |
| - | `Band`, the eight band constants, `BANDS`, `bytes`, `contains`, `holds`, `pair_page`, `runtime_window`, `service_window`, `check_image`, `ImagePlacement` and its variants | no block | none was minted with a `Name:` block; they pass the template's naming checks (109 names, 0 refused) | review as part of the clearance |

## What still blocks the move

| item | owner |
|---|---|
| Item 2: the falsification records read `crates/address_space_layout/falsifications/<harness>.patch` and every patch header names `a/crates/address_space_layout/src/lib.rs`. `helpers/crate_move.py` rewrites the records and regenerates every patch in crate-relative form at the move; this package did not run the move, so that is unconfirmed. | move lane |
| Item 5: the license texts, `CHANGELOG.md`, `_typos.toml`, caller workflows, `.github` files and the recorded stranger run | move lane |
| The two lines of nife text: `src/lib.rs:425` names a milestone and `src/verification.rs:4` names `script/verify`; the 27 `Name:` blocks go at the move | move lane |
| Item 11: one `.github/mutants-exempts` line for the `check_image` `>` to `>=` mutant, with the equivalence reason | move lane |
| Item 13: the template's `register_names.py` reads `address` in a harness function name as an address. Until nifeos/crate-workflows fixes it, the crate's `check / standalone` is red on its first run | maintainer, in another repository |
| The pins: all 27 dependents move to `address_space_layout = { version = "0.1.0", git = "https://github.com/nifeos/address_space_layout", rev = "<landed commit>" }` in one pull request (one revision per contract per image), preferably through one root `[workspace.dependencies]` entry | move lane |
| `cargo xtask std-src` generates its core into `std`; whether it resolves a git dependency where it resolved a path is unverified and needs one dry run first | move lane |
| The three provisional names of items 7, 15 and 17 and the name `window`, once ruled | the architect, then the lane that builds them |
| Item 18 and the milestone 1017 table: its blocker row for this crate names a test that has since moved; the lane that edits 1017 drops it | the 1017 lane |

Items 1, 3, 4, 6, 8, 9, 10, 12, 14 and 16 of the milestone are done in this tree. The nife-side
checks that walk `crates/` (item 17) also need `helpers/interface_declarations_check.py` to read
the pinned checkout through `cargo metadata`, in the move's pull request.

## Proposed items

- Item 7: rename the length constant to `PAGE_BYTES` and the five address constants and three
  `ImagePlacement` fields to the `_VIRTUAL_ADDRESS` stem, in the move pull request. Effort: M
  across nife, S in the crate. Two-way until the first tag.
- Item 15: `Band::new`, asserting `start < end` and page alignment at both ends, with the fields
  kept public. Effort: S. Two-way.
- Item 17: make `LOW_HALF_SPLIT_SHIFTS` public and add one `page_tables` test asserting its three
  `SPLIT_SHIFT`s against it. Effort: S. Two-way.
- Item 11: the `.github/mutants-exempts` line. Effort: S.
- A repair the move lane may find: the roadmap text for item 3 asks for `rust-version = "1.87"`
  and the manifest says 1.88. The lowest installed toolchain that builds the crate is 1.88, and
  the template's `msrv` job settles it.

## Open questions

- Does the architect want the three one-page functions kept as wrappers after the first tag, or
  removed in favor of `window` with `PAGE_BYTES`? Keeping them costs nothing and 222 call sites
  stay unchanged.
- Does the architect want `BANDS`' band names (`"NULL_GUARD"` and the rest) to stay as strings in
  a public table? They are the names a message prints.

## Recommendation

Clear the crate to move once the architect has ruled the names above. The crate builds on all
three bare-metal targets, 11 of 11 harnesses verify and every falsification patch turns exactly its
own harness red. The remaining failures are the work of the move itself (licenses, citations,
patch paths, the pins). The one thing that is not mechanical is the set of four provisional
names, which become permanent after the first tag.

## Decisions for the architect

1. Ratify, rename or send back the names of items 7, 9, 15 and 17 (the names table above). The
   recommendation is to ratify `window`, `Band::new` and a public `LOW_HALF_SPLIT_SHIFTS` as
   named, and to ratify the rename of the length and address constants as proposed.
2. Clear the review, which approves creating `nifeos/address_space_layout` and moving the crate,
   with `publish = false`, no tag and no publish. The recommendation is to clear it (option A of
   the milestone's open decision); option B clears it with named changes to the provisional names,
   which land in the move pull request, and option C sends the package back.

## Reproducer

```
script/crate-review-packet address_space_layout
script/crate-standalone address_space_layout
cargo test -p address_space_layout
cargo clippy -p address_space_layout --all-targets -- -D warnings
script/names address_space_layout
python3 helpers/name_release.py --draft address_space_layout
```
