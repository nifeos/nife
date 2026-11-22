---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 223, 563, 697, 857, 1017
decision_dependencies: 151, 197
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1107. `sealed_pair`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/sealed_pair` as a stranger to this tree would meet it. It
changes nothing; it lists what the next lanes change. The number is the recording lane's.

The review had no settled baseline to start from. `notes/crate-reviews.md` carries no package for
this crate. The only record is `notes/sealed-pair.md` plus the `Name:` block at the head of
`src/lib.rs`. That block marks the crate name provisional and records two refusals (`card_check`,
`seal`). Nothing below reopens those refusals. Every name in the crate is unratified, so every
naming item here is a proposal for the architect and not a rename a lane may perform.

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-review-packet`,
`script/names`, `helpers/name_checks.py`, the mutation census series. No new tool.

## Goal

A person who copies a kernel, an archive and a boot environment onto a card runs `script/card-check`
and gets the same answer the board will give. The crate says what the kernel and the progenitor
will each do, in their own words, and nothing it says is true only of one build shape. When it
leaves this tree, it leaves after the two crates it builds on, with a README that a stranger can
build and test from.

## Finding status


- P0-1, it cannot leave before `nifefs` and `measured_boot`: Waiting on repositories for `nifefs`
  and `measured_boot`. Decision A ruled the order on 2026-11-16 UTC, and the crate needs nothing
  more.
- P1-2, the loader's digest words and the three-file card set: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-3, a bad table line reported as an unreadable table: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-4, the crate does not stand alone: At the move. The scaffold files, a measured
  `rust-version`, inlined lints, rewritten citations, a README and the `Name:` strip remain, and
  Decision A orders the move.
- P1-5, `BOOT_PROGRAMS` restates `PROGENITOR`: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-6, the notice constant is a sentence: Done, [#3610](https://github.com/nifeos/nife/pull/3610).
- P1-7, `EntrySeal` can say refused with no digest: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-8, the refusal text asserts a cause: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-9, a British spelling in `test_support::pair`: Done,
  [#3610](https://github.com/nifeos/nife/pull/3610).
- P1-10, `Unreadable` is not a `std::error::Error`: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-11, duplicate and non-UTF-8 entries unreported: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580), by the item's own alternative: the crate's
  `BUGS` states the three facts.
- P2-12, the stick-path haystack contains the archive: Done, [#3810](https://github.com/nifeos/nife/pull/3810). `embedded_archive` returns the
  archive's span, `card_check` zeroes that span before it judges the rest of the boot file, and a
  test carries the `progenitor` digest in an archive entry. The `BUGS` line is gone.
- P2-13, the cost of the `contains` window scan: Done,
  [#3610](https://github.com/nifeos/nife/pull/3610) (about 1.6 ms per MiB).
- P2-14, no proof: Done, [#3610](https://github.com/nifeos/nife/pull/3610) and [#3810](https://github.com/nifeos/nife/pull/3810). The
  `contains` harness and the table-comparison harness each carry a falsification patch and share one `script/verify` row.
- P2-15, `hex_string` and `measured_entries` public with one reader: At the move. Both stay
  public until then; a two-way door the maintainer rules at the move.
- P2-16, the stale list in `notes/boot-stick.md`: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- Decision A, when it leaves: Done, [#3810](https://github.com/nifeos/nife/pull/3810) (ruled 2026-11-16 UTC). Recorded in the `sealed_pair` row of
  `notes/releasable-crates-2026-10-07/inventory.md` and in milestone 1017's order.
- Decision B, two public names: Done, [#3610](https://github.com/nifeos/nife/pull/3610)
  (maintainer, provisional, 2026-11-14 UTC).

## Open decisions

None.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1107-sealed-pair-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1107-sealed-pair-outside-review/prior-art.md). The tree already recorded one weakness of a check that reads bytes: milestone 563 (A seal check that reads bytes cannot see a check that was dropped).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside this repository acts on it after a release (a public name, a published behavior,
a repository); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The crate cannot leave before the two crates it builds on. LEFT: no change here; Decision A rules the order.
   Finding: both dependencies are path dependencies on sibling crates that have no repository of
   their own (`nifefs`, `measured_boot`). The ruling of milestone 1017 (the contract crates leave in this order) is that a crate leaves
   bottom-up and nothing that has left depends on a path or a pin into this tree. `measured_boot`
   is in the kernel's graph and `nifefs` is the kernel's archive format; neither is on journey 5's
   table yet. A repository for `sealed_pair` today would pin two unreleased crates by git revision
   into this tree, which the ruling forbids.
   Evidence: `crates/sealed_pair/Cargo.toml` lines 13 to 19; `notes/releasable-crates-2026-10-07/inventory.md`
   row `sealed_pair` (class C, graph `measured_boot, nifefs`); the second direction in
   `design/roadmap/1017-the-contract-crates-leave-in-this-order.md`.
   Change: none in this crate. Decision A rules the order: `nifefs` and `measured_boot` first,
   then this crate with or after `uefi_loader`. It gets no repository now.
   Door: one-way (a repository). Effort: none here.

### P1: before the move, and one before the next bench evening

The P1 items are in [the appendix](1107-sealed-pair-outside-review/before-the-move.md).

### P2: later

The P2 items are in [the appendix](1107-sealed-pair-outside-review/later-items.md).

## Decisions

The architect ruled A. The maintainer decided B's two names provisionally.

### A. When `sealed_pair` leaves, and in whose company

Background. The crate is class C in the inventory: bound to this tree's formats. Its two
dependencies are the kernel's archive format and the kernel's measured-boot primitive, neither of
which has a repository. Its two readers are `xtask` (the card checker) and `uefi_loader`'s build
script. Its vocabulary is this tree's: the progenitor, the console, `MEASURED BOOT REFUSED`. The
standing rule is that a crate gets a repository of its own when it is publishable or used by more
than one repository, and otherwise stays with the package that uses it. Whether more
than one repository uses it depends on how many kernels there are.

Ruled 2026-11-16 (UTC). `sealed_pair` gets no repository now. It leaves after `nifefs`
and `measured_boot`, with or after `uefi_loader`. With more than one kernel, the expectation is that
it becomes a crate of its own, used by several repositories. If only one repository ends up using
it, it moves into that repository as a module and never publishes. Nothing in the crate needs a
repository to get fixed; items 2 to 10 land here.

Prior art. systemd keeps `systemd-measure` inside the systemd repository beside `ukify`, the tool
that builds what it measures; Android keeps `avbtool` in the `avb` repository with the library it
verifies against. Neither publishes its checker on its own. A concrete example: a card checker
published to crates.io with `nifefs = "0.1"` and `measured_boot = "0.1"` in its manifest would
force both of those crates to publish first, and `measured_boot` is in the kernel's graph.

### B. Two public names, if and when it leaves

Background. Names are the architect's. Two of this crate's public names fail a reading a stranger
would give them, and a release makes them permanent. One is `THE_BOARD_IS_THE_AUTHORITY` (item 6: a
sentence, a `THE_` prefix, "board" for "kernel"). The other is the parameter of `test_support::pair` (item 9: a
British spelling the template refuses). Every other name reads as what it is: `inspect`, `Seal`,
`EntrySeal`, `TableSeal`, `TableDisagreement`, `Vouch`, `Unreadable`, `embedded_archive`,
`is_sealed`, `kernel_refuses`, `progenitor_refuses`, `hex_string`, `test_support`.

Recommendation. `AUTHORITY_NOTICE` for the constant, and `build` for the parameter (`pair(build:
&str)`, since it names one build against another).

Prior art. The standard library names a constant for what it holds (`EPSILON`, `MAX`), never for
its opening words; this tree's naming page refuses `THE_` on an assertion constant for the same
reason. A concrete example: `card_check` prints the constant after every refusal, so a reader meets the name at one call site and the text at every bench.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, since these are names
in an unreleased crate that is not leaving, which the architect may revisit.
`THE_BOARD_IS_THE_AUTHORITY` (`src/lib.rs` line 241, one caller at `xtask/src/card_check.rs` line
79) becomes `AUTHORITY_NOTICE`, because a constant's name says what it holds and the naming page
refuses `THE_`. The parameter of `test_support::pair` (line 406) becomes `build`, which also drops
the British spelling. No old spelling stays as an alias. The renames do not wait for decision A.
Items 6 and 9 are the work. Ratifying the names stays the architect's.

## Done when

The checks are in [the appendix](1107-sealed-pair-outside-review/done-when.md).

## BUGS

- Item 2 came from reading the kernel and `script/board-image`, not by booting a card with
  stale words. The change is cheap enough to land without a bench run, and a bench run would
  confirm the halt text.
- The review did not run `script/crate-standalone` or the template's `check` workflow; the packet
  and `standalone.py` list what they would flag, and item 1 says the move cannot happen yet.
- No review package exists for this crate, so the baseline this review respected is the `Name:`
  block's two refusals and nothing else. A package would normally come first; the recording lane
  may want to say whether it owes one.

## Follow-on

What is left of this review is under "Finding status" above.

- **Milestone 1017.** The contract crates leave in its order; items 1, 4 and 15 follow the order
  Decision A rules.
- **Recorded.** Item 11 is in `crates/sealed_pair/src/lib.rs`'s `BUGS`.

## Index row

An outside review of `sealed_pair` before any move: one blocker (its two dependencies have no
repository), nine items before the move, six for later. The nine cover these: the loader's two
digest words, which the kernel now also requires and the crate never reads. A bad table line
misreported as an empty table. A duplicated `progenitor` literal. The standalone checker's ten
findings. A notice constant named by its opening words. A refusal type that can carry no digest. A
refusal text that asserts a cause. One British spelling. An error type without `std::error::Error`.
