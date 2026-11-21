# Review package: `line_editor`

*Name: provisional, minted by lane lane/line-editor-review-packages. This is the `line_editor`
package of [crate-reviews.md](../crate-reviews.md). It has its own file because the packages
together outgrow the page. Its twin is [`terminal_contract`](terminal-contract.md), the wire half
the cut took out of this crate.*

The architect reads this to clear the crate for its move to its own repository. It describes the
crate as `main` leaves it at `97d16ae8b`. The findings and their statuses are in
[milestone 1098 (`line_editor`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1098-line-editor-outside-review.md);
this package does not restate them, and checks which have landed.

## In one paragraph

A sans-IO, allocation-free line editor for a serial terminal. The caller feeds it one byte at a
time; it writes echo through a `Sink` the caller implements and returns an `Event` when a line ends
or the user presses `^C`, `^D`, `^Z` or Tab. It edits at a cursor, kills and yanks, keeps eight
lines of history and reads the ANSI cursor-key sequences. A `RawQueue` holds bytes for a reader
in raw mode, and the `handoff` module writes and reads the editor's state as a versioned blob so a
replacement instance carries on with the half-typed line. The crate is `no_std`, forbids `unsafe`,
has no dependency, and two Kani harnesses prove the cursor and range-delete invariants.

## Identity

| item | value |
|---|---|
| crate | `line_editor`, ratified |
| identity values | none for a contract: the crate is not a wire contract and carries no `INTERFACE_ID`. The `handoff` blob carries `MAGIC` `0x5444_454c` ("LEDT" in little-endian bytes) and `LAYOUT` 1, which a reader checks before it restores |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, none in the manifest (the move writes it) |
| license | inherited from the nife workspace; no license file in the crate (the move writes both) |
| `publish` | `false`; journey 5 keeps a moved crate unpublished until its release |
| dependencies | none, of any kind |
| size | `src` 2493 lines in 11 files, 1166 of them tests and 106 test support |
| files | 15 tracked, two of them falsification patches; a README of 96 lines |
| listed for the move | `crates/externalization-list.txt` names it |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components` (the `line_editor` program, `display_terminal` and the shell's prompt), `fuzz` (targets `line_editor_feed` and `line_editor_handoff`); `crates/video_terminal` as a development dependency, whose keymap tests check its arrow sequences against this editor |
| prose and tables | `crates/externalization-list.txt`, `packages/terminal.package.toml`, `packages/base-image.toml`, `helpers/trust_boundary_crates.tsv`, `helpers/interface_stability.py` |

No kernel crate and no contract crate depends on it since the cut ([#3587](https://github.com/nifeos/nife/pull/3587)):
the kernel and every client of the protocol link `terminal_contract`, and the supervisor's routing
lives in `line_editor_component`, which stays in nife.

## What was run

All commands ran in this lane's worktree, at `main` with this lane's edits to documents only.

| check | result |
|---|---|
| `script/crate-review-packet line_editor` | no dependency, no type from another crate in a public signature, no `unsafe` site, two harnesses each paired with one patch; three paths into the nife tree, five section citations and five milestone citations in shipped text, first at `src/lib.rs` line 8 |
| `script/crate-standalone line_editor` | 9 failures, all of them the move's work: 44 names not ratified, inherited `license` and `lints`, no `rust-version`, no license files, and the citations above. The build passes on the host and on the three bare-metal targets on stable, both Kani harnesses verify, and each fails under its falsification patch |
| `cargo test -p line_editor` | 66 unit tests and 4 doctests pass |
| `cargo clippy -p line_editor --all-targets -- -D warnings` | no warning |
| `cargo doc -p line_editor --no-deps` with warnings denied | no warning |
| `script/names line_editor` and `python3 helpers/name_release.py --draft line_editor` | the crate and the program ratified; `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and `RAW_QUEUE_MAXIMUM` ratified before the units rule; four module names and `handoff`, `Event::Suspend` provisional; the standing `tests` and `verification` modules ratified; every other public item has no block |
| the template's gating name checks (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`, `question_shape`, and `service_model --prose`), from `nifeos/crate-workflows` at `56437de` | two fail. `spelled_out` refuses the parameter `dir` (`src/line_disc.rs` line 685). `register_names` refuses five length fields with no unit: `len`, `done_len`, `kill_len` and `prompt_len` (`src/line_disc.rs` lines 144 to 158) and `len` (`src/raw_queue.rs` line 24). The other six pass |
| the template's `abbreviations` report | 54 components in 7 files, 40 of them outside the tests: `buf`, `hist`, `esc`, `params`, `csi`, `cr`, `elen`, `slen`, `scur`, `prev`, `seq`, `src`, `dir`, `bs`, `bel`, `ss`, `eof` and `handoff` |
| `python3 helpers/lint_externalized_crates.py --count` | 0 hits |

Not run, and so not claimed: the stranger test in a scratch copy, `cargo-mutants`, the two fuzz
targets, Miri, `cargo package` and `cargo-semver-checks`. A mutation census belongs to its own
window, and milestone 1098's appendix holds the recorded figures; a first release has no semver
baseline. No
repository exists, so the template's `check`, `proofs` and `weekly` workflows did not run.

## Public API

| kind | items |
|---|---|
| the engine | `LineDisc` with `new`, `start_line`, `resume_line`, `feed`, `line`, `pending`, `insert_text`, `repaint`, `abandon`, `save`, `restore`; `Event::{None, Line, Eof, Interrupt, Suspend, Tab}` |
| limits | `LINE_MAXIMUM` (256), `PROMPT_MAXIMUM` (64), `HIST` (8), `RAW_QUEUE_MAXIMUM` (64) |
| output | the trait `Sink` (one method, `put`), `expand_output(src, out)` |
| raw mode | `RawQueue` with `new`, `is_empty`, `push`, `pop8`, `save`, `restore` |
| `handoff` | `MAGIC`, `LAYOUT`, `Writer` (`new`, `bytes`, `u32`, `field`, `finish`), `Reader` (`new`, `bytes`, `u32`, `field`), `Refusal::{NoBlob, Layout, Malformed}` |

No nife type appears in a signature. What a stranger may find surprising:

- A byte at or above 0x80 is dropped without echo, so UTF-8 typed into the line vanishes. The crate
  root's `BUGS` section and the README both say so.
- The redraw math assumes one row; the editor never asks the terminal for its size, because a
  terminal that never answers would block it.
- `restore` on a refused blob may leave the editor half written, and the documentation tells the
  caller to discard it.
- `pop8` returns a count and up to eight bytes packed little-endian in a `u64`, the shape the
  terminal protocol's register-only replies use. A Rust reader outside nife meets a wire shape in
  an editor's API.

## What landed

Checked against the source on `main`:

| item | state | evidence |
|---|---|---|
| Item 2, the cut | landed ([#3587](https://github.com/nifeos/nife/pull/3587)) | no `proto` or `component` module; `[dependencies]` is empty; `terminal_contract` and `line_editor_component` exist |
| Item 5, bindings and limits | landed | the crate root holds the key-binding and limits tables, and the README repeats them |
| Item 6, `forbid(unsafe_code)`, fuzz, Kani | landed | `#![forbid(unsafe_code)]` at `src/lib.rs` line 86; `fuzz/fuzz_targets/line_editor_feed.rs` and `line_editor_handoff.rs`; two harnesses with patches |
| Item 7, the handoff version policy | landed | the `handoff` module's doc states that any change to order, meaning or width bumps `LAYOUT` and that a reader refuses every other layout |
| Item 8, one `QUIESCED` | landed | `components/src/line_editor.rs` line 85 asserts `terminal_contract::QUIESCED == hot_swap_contract::QUIESCED` at compile time |
| Items 9 to 14 | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | as milestone 1098's finding status records |

What has not landed:

| item | state |
|---|---|
| Item 1, the crate stands alone | open; the move carries it. The README exists, so what remains is both licenses, literal `license`, `lints` and `rust-version`, the manifest's `description`, `repository` and `readme`, the `Name:` strip, and the citations of this tree |
| Item 3, `LineDisc` to `LineEditor` and `HIST` to `HISTORY_LINES` (decision 2) | ruled, not built. `main` still holds `struct LineDisc` and `HIST`, and no open pull request or pushed branch carries the rename |
| Item 3, the three limits to `MAXIMUM_LINE_BYTES`, `MAXIMUM_PROMPT_BYTES` and `MAXIMUM_RAW_QUEUE_BYTES` (decision 3) | ruled, not built, for the same reason |
| Item 3, `pop8` | open; the maintainer rules its new name, and the rename lane builds it |
| Item 4, the engine's fields | open; `buf`, `cur`, `hist`, `esc` and `params` keep their names, and the template's `spelled_out` and `register_names` gates refuse six names today (What was run) |

The rename lane for item 3 has not landed: nothing on `main`, in an open pull request or on a
pushed branch renames any of the five names decisions 2 and 3 cover.

## Names

The architect ratified the crate and program names. `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and
`RAW_QUEUE_MAXIMUM` carry a ratification from before the units rule, and decision 3 of milestone
1098 replaces them provisionally. Everything else is provisional or has no block. This lane
ratifies nothing; the architect ratifies, changes or refuses each row.

| item | status | rule or reason |
|---|---|---|
| `LineDisc` | no block; ruled provisionally to become `LineEditor` (decision 2) | an abbreviation of a word the crate-name ruling refused |
| `HIST` | no block; ruled provisionally to become `HISTORY_LINES` (decision 2) | a count names the things it counts |
| `LINE_MAXIMUM`, `PROMPT_MAXIMUM`, `RAW_QUEUE_MAXIMUM` | ratified before the units rule; ruled provisionally to become `MAXIMUM_LINE_BYTES`, `MAXIMUM_PROMPT_BYTES`, `MAXIMUM_RAW_QUEUE_BYTES` (decision 3) | a limit opens `MAXIMUM_` and ends with its unit |
| `pop8` | no block; open | the maintainer's to rule before the tag (item 3) |
| `Event::Suspend` | provisional | the `^Z` event, beside `Interrupt` |
| `Event::Eof` | no block; kept by decision 2 | the standard library's spelling (`UnexpectedEof`) |
| `handoff` (module) | provisional | the blob a replacement reads |
| `line_disc`, `output`, `raw_queue`, `test_support` (private modules) | provisional | `line_disc` renames with its type under decision 2 |
| `Sink`, `expand_output`, `RawQueue`, `Writer`, `Reader`, `Refusal`, `MAGIC`, `LAYOUT` and the methods | no block | plain words; `is_empty`, `new`, `push` and `u32` are standard library spellings |
| the private fields and locals the gates and the report list | no block | item 4's worklist |

## What still blocks the move

| item | owner |
|---|---|
| The rename lane for item 3 (decisions 2 and 3 and `pop8`) and item 4's fields, so the template's `check` job passes in the new repository from its first run. | rename lane |
| Item 1: both licenses, the manifest rows, the `Name:` strip and the citations of this tree. | move lane |
| Convert `components`, `fuzz` and `crates/video_terminal` to `git` and `rev` with `version = "0.1.0"`, and read every file `script/crate-move --dry-run` lists. The two fuzz targets either move with the crate or stay in nife's `fuzz/` against the pinned revision. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Recommendation

Ready for review. The cut landed, the engine depends on nothing and carries no nife type, two
proofs each have a falsifying patch, and every item milestone 1098 lists as needing no ruling has
landed. What remains is the ruled rename, which has not landed, and the move's own work. The
template's `check` job would fail today on six names, so the move waits for the rename lane even if
the architect clears the review now.

## Open questions

- `pop8`'s new name is the maintainer's to rule as a two-way door. One candidate is `pop_packed`,
  which says what the `u64` holds without naming the count.
- `register_names` refuses a field called `len`. The rename lane either spells the unit
  (`length_bytes`) or keeps `len` in the kept form the template allows for a standard library
  spelling. Both are two-way until the first tag.
- The `handoff` blob is a format two separately built `line_editor` binaries exchange during a
  replacement. Milestone 956 (the linked interfaces carry checkable versions) gives an interface id
  only to a format exchanged between separately released parts; whether a replacement from a newer
  release counts is not settled anywhere. `MAGIC` and `LAYOUT` already make a mismatch a refusal.

## Decisions for the architect

1. Clear the review, which is the go to create `nifeos/line_editor` and move the crate (one-way:
   the template's workflows and nife's pin act on the repository from its first day). Recommended: A.
   - A. Clear it now; the move waits for the item 3 rename lane, as for `designware_ethernet`.
   - B. Hold the clearance until the rename lane lands, and review the names as built.
2. Ratify the Names table once the rename lands, including the three limits as the re-ratification
   of the names #2797 ratified. Two-way before the first tag; one-way after it.
3. Freeze the `handoff` identity, `MAGIC` "LEDT" and `LAYOUT` 1, as the first published layout.
   Recommended: yes, since a replacement from a later release reads it.

Nothing here asks for a tag or a publication.
