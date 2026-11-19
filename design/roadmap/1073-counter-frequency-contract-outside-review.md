---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1073. `counter_frequency_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/counter_frequency_contract` as a stranger to this tree
would meet it. The review read the crate's five tracked files, every caller in the tree, the review
package, the naming rules and the crate template, and ran a local dry run of the move. It changes
nothing; it lists what the next lanes change.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/counter-frequency-contract.md) and reopens none of
the decisions it settles. Those are the crate name and the thirteen ratified names (`MAGIC`,
`VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`,
`MINIMUM_PLAUSIBLE_HERTZ`, `MAXIMUM_PLAUSIBLE_HERTZ`, `is_plausible`, `build_page`,
`CounterFrequencyPage`, `new`, `hertz` and the `hertz` and `virtual_address` parameters). They are
also the kept wire value `TIMEBAS1`, the dual license, and the three edits the package assigns to
the move lane: the README's "written for the nife kernel" sentence, the three sibling-crate
citations, and the machine and tree names in the doc comments. The package's open question on `PAGE_VIRTUAL_ADDRESS` existing on aarch64 stays
with the maintainer and is not reopened. Where a later rule contradicts the baseline, the item
says so; one does (item 7, the `#[doc(alias)]` sentence).

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-move --dry-run`,
`script/crate-standalone`, Kani, cargo-llvm-cov, Miri and the sibling contract crates' tests as
patterns. No new tool.

## Goal

A stranger who finds `counter_frequency_contract` in its own repository can tell in one screen
what the page is, build and test it from the README alone, and trust every claim the documentation
makes about what is checked and by whom. The repository's gates are green on the day the crate
arrives, for the crate's own reasons, and nife's own build keeps working the day the crate leaves.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1073-counter-frequency-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the prior art appendix](1073-counter-frequency-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move (a published name, a wire value, a
layout, a fixed address, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move or the first green run

1. nife's `std` build reads this crate's source by path, and loses it the day the crate leaves.
   Finding: `xtask/src/farm/generate.rs` copies `crates/counter_frequency_contract/src/lib.rs`
   verbatim into the `std` platform layer as `sys/pal/nife/counterfreqproto.rs`, and
   `xtask/src/farm/stamp.rs` hashes the same path to decide when the farm rebuilds. Both are
   `root.join("crates/...")`. After the move the path does not exist, `cargo xtask std-src` fails,
   and no `std` program builds. The crate's review package does not list this; the packages for
   `clock_contract` and `entropy_contract` do, and `notes/crate-move.md` records it as a known
   gap for thirteen crates of milestone 1017 (the contract crates leave in this order).
   Evidence: `xtask/src/farm/generate.rs` line 142; `xtask/src/farm/stamp.rs` line 70;
   `notes/crate-move.md` lines 295 to 301; `patches/std-nife/overlay/std/src/sys/pal/nife/mod.rs`
   lines 65 to 72.
   Change: both files resolve each copied crate's source directory through `cargo metadata`
   (the `manifest_path` of the package the workspace depends on), so a crate pinned by `rev`
   comes from Cargo's checkout and a crate still in `crates/` comes from there. One change serves
   every copied crate. It lands before the first of them leaves, and it is not this crate's alone.
   The generated copy also keeps the `#[cfg(doctest)] include_str!("../README.md")` item; it is
   harmless inside `std` because `doctest` is never set there, and a sentence in the generator's
   comment should say so.
   Door: two-way. Effort: M. Owner: a nife lane, shared with the other copied contract crates.

2. The template's clippy job is red on arrival.
   Finding: the module documentation writes `x86_64` without backticks, `clippy::doc_markdown`
   is `warn` in the manifest's `[lints.clippy]`, and the template runs clippy with `-D warnings`
   on stable. Stable 1.99.1 and 1.88 both refuse the crate. The nightly this tree pins accepts it,
   which is why nife's own gate never saw it (nobody verified why the two differ).
   Evidence: `crates/counter_frequency_contract/src/lib.rs` line 9; `Cargo.toml` lines 25 to 31.
   Change: backticks around `x86_64` on that line, and a `cargo +stable clippy -- -D warnings`
   pass over the standalone copy before the move, which `script/crate-move --dry-run` should run
   (BUGS).
   Door: two-way. Effort: XS.

### P1: before the first tag

The P1 items are in [the appendix](1073-counter-frequency-contract-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1073-counter-frequency-contract-outside-review/later-items.md).

## Decisions for the architect

The architect froze the identity: magic `TIMEBAS1`, version 1, interface 9 (ruled 2026-11-11 UTC,
#3516).
### Clearing the review package

Ruled 2026-11-19 (UTC): the review of `counter_frequency_contract` is cleared. Clearing the review
approves creating `nifeos/counter_frequency_contract` and moving the crate there. The review
package is [the review package](../../notes/crate-reviews/counter-frequency-contract.md). It
states the crate, its thirteen ratified names, its frozen identity (`TIMEBAS1`, version 1,
interface 9) and what the move changes.

### The pre-release tag

Ruled 2026-11-19 (UTC): the tag `v0.1.0-pre.1` is approved, once the move lane finds that the new
repository matches this tree's copy and the release dry run is green. These are the two conditions
of the approval of 2026-11-11 (UTC) for seven cleared crates. After the move, nife pins the crate
by `rev` at the commit of the tag, with `Cargo.toml` carrying the same version. Nothing publishes:
a contract crate never publishes to crates.io, and the tag is its release.

### How the page crates spell their offset constants (item 10)

Ruled 2026-11-14 (UTC) for the whole family: the byte position of a page field or region-layout
field is named `<FIELD>_OFFSET_BYTES`, with the field name spelled out under `design/naming.md`.
This crate's constants are `MAGIC_OFFSET_BYTES` and `HERTZ_OFFSET_BYTES`. A constant that counts
words says `_OFFSET_WORDS`. No old spelling is kept as an alias.

## Done when

The checks are in [the done-when appendix](1073-counter-frequency-contract-outside-review/done-when.md).

## BUGS

- `script/crate-move --dry-run` runs the pinned template's checks, not the template head's, and
  its notes say so. The dry run also runs no clippy pass, so item 2 was invisible to it, and its
  citation check did not report the manifest comment of item 6. The same gap hid two more refusals
  that the template head and stable clippy made over the staged copy: a README with no badge row,
  and a bare `VisionFive` that nife's own `clippy.toml` accepts and the standalone copy does not.
  Both are fixed in the crate. All of these are findings against `helpers/crate_move.py`, for a
  lane of its own.
- The dry run's three remaining findings (`.github/pull_request_template.md` line 2,
  `CONTRIBUTING.md` line 23, and five "mentions nife" lines in `.githooks/commit-msg`, `deny.toml`,
  `docs/stranger-test.md` and `rustfmt.toml`) are in the pinned `v7` scaffold, not in this crate.
  The template head's scaffold names nife nowhere, so a move at the next template release clears
  them.
- Item 5 came from reading, not from a build for a 32-bit target; the fix is cheap enough to land
  without one, and the three targets the crate names are all 64-bit.
- The kernel's writer caches the page's physical frame on first use. If the rate has not resolved
  at that moment, the frame stays zeroed for the machine's lifetime, and every process reads the
  rate as unknown (`timebase_page_phys` in `kernel/src/user/loader.rs`). The crate's contract does
  not cover when the writer runs.

## Finding status

- Item 1, the `std` farm reads the source by path: Done,
  [#3514](https://github.com/nifeos/nife/pull/3514).
- Item 2, clippy red on stable: Done, [#3513](https://github.com/nifeos/nife/pull/3513).
- Item 3, BUGS in the contract's own terms: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 4, the band is the reader's to apply: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 5, pointer-width assertion: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 6, manifest `publish` comment: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 7, documentation a stranger reads: Done, [#3556](https://github.com/nifeos/nife/pull/3556),
  with the review package's three move edits.
- Item 8, the template's prose gates: Done, [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 9, two Kani harnesses with falsification patches: Done,
  [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 10, private offset constants: Done, [#3571](https://github.com/nifeos/nife/pull/3571).
- Item 11, README says where the end-to-end test lives: Done,
  [#3556](https://github.com/nifeos/nife/pull/3556).
- Item 12, `Debug` prints the address: Declined. No change until the next change to the crate's
  public interface ([the appendix](1073-counter-frequency-contract-outside-review/later-items.md)).
- The frozen identity: Done, [#3516](https://github.com/nifeos/nife/pull/3516).
- The mutation census after the harnesses: Done,
  [run 43876176886](https://github.com/nifeos/nife/actions/runs/43876176886): 23 mutants tested,
  0 missed.
- The kernel's zeroed frame: Done, [#3283](https://github.com/nifeos/nife/pull/3283): the kernel
  states the outcome at `timebase_page_phys` (BUGS below).
- The move dry run's gaps: Open. `helpers/crate_move.py` runs no clippy pass and does not report a
  manifest comment that cites a nife path (BUGS below); a lane of its own fixes both.
- The review package: Done, [#3615](https://github.com/nifeos/nife/pull/3615),
  [#3626](https://github.com/nifeos/nife/pull/3626) and
  [#3635](https://github.com/nifeos/nife/pull/3635); it states that it is ready.
- Clearance of the review package: Done. Ruled 2026-11-19 (UTC), under "Clearing the review
  package" above.
- The move to its own repository: Open. A lane creates `nifeos/counter_frequency_contract` and
  moves the crate there.
- A pre-release tag: Open until the move. Ruled 2026-11-19 (UTC): `v0.1.0-pre.1` once the move
  check and the release dry run pass, under "The pre-release tag" above.
- Publishing: Open. A contract crate never publishes to crates.io; the tag is its release.

## Open decisions

None. The publishing hold stands.

## Follow-on

- **Recorded.** Item 10, the spelling of the page crates' offset constants: ruled, see the decisions above.
- **Recorded.** The review is cleared (ruled 2026-11-19 UTC), which approves creating the
  repository and moving the crate. The tag `v0.1.0-pre.1` is approved on the same date, after the
  move check and the release dry run pass.
- **Outstanding.** The move itself, which a lane carries out, and the move dry run's two gaps in
  `helpers/crate_move.py`. The finding status above says what each needs.
- **Recorded.** Item 12 (`Debug` prints the address) is `design/roadmap/1073-counter-frequency-contract-outside-review/later-items.md`
  item 12: no change until the next API change.

## Index row

An outside review of `counter_frequency_contract` before it leaves this tree. Items 1 to 9 and 11
have landed. They cover the `std` farm's source lookup, the stable clippy failure and the BUGS
entries in the contract's own terms. They also cover the band the reader applies, the pointer-width
assertion, the manifest comment and the documentation fixes. The rest are the template's prose
gates, two Kani harnesses, and a README sentence on where the end-to-end test lives. Item 10, the private offset constants, follows the family rule. The identity does not change, and
the review is cleared and the tag `v0.1.0-pre.1` is approved (ruled 2026-11-19 UTC), and the crate moves to its own repository next.
