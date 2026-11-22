---
status: BUILT
built: 2026-11-22
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: 101, 154
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1095. `inter_process_communication`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/inter_process_communication`, read as a stranger
who meets the crate for the first time would read it. It changes nothing; it lists what the lanes
that move the crate change, in the order they should land.

The review looked for a settled baseline first and found none. `notes/crate-reviews/` holds no
package for this crate and `notes/crate-reviews.md` has no row for it, so the only ratified name
is the crate's own (§154 (the acronym test is whether the phrase is spoken)). Every other public
name carries a `provisional` block. Nothing below reopens the crate name. Everything else is open,
and the first item says so.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its name gates
(`helpers/spelled_out.py`, `question_shape.py`, `register_names.py`, `british_spelling.py`,
`standalone.py`, `abbreviations.py`), `typos`, Kani, cargo-mutants, cargo-llvm-cov, and the
crate's own harness helpers (`seed`, `mint`, `any_timers`). No new tool.

Effort: XS is under an hour for a lane, S is half a day, M is a day or two, L is more. A door is
one-way when somebody outside this repository acts on it after the move or the release. Examples are a published name, a dependency another repository pins, a wire format and a layout somebody else reads. Everything else is two-way and the maintainer decides it. Until the crate first publishes,
every public name is two-way in fact and one-way in intent, because the first publish freezes it;
the items say which.

## Finding status

Every finding not Done blocks the first crates.io publish, which waits until every outside-review
milestone reaches delivery.

- P0-1, no review package: Done, [#3824](https://github.com/nifeos/nife/pull/3824).
  [`notes/crate-reviews/inter-process-communication.md`](../../notes/crate-reviews/inter-process-communication.md)
  is the package, with a row in the index.
- P0-2, the one dependency has not left and appears in no plan: Done, [#3892](https://github.com/nifeos/nife/pull/3892). `intrusive_queue`
  left first (D3), and the crate takes `nifeos/intrusive_queue` by `rev`.
- P0-3, the crate does not stand alone: At the move. The move strips every `Name:` block and the
  citations, and gives the manifest its own `license`, `lints` and `rust-version` in place of
  `license.workspace` and `[lints]`, which `Cargo.toml` still inherits.
- P1-4, a public type named `Send`: Done, [#3824](https://github.com/nifeos/nife/pull/3824). The five operation enums are `SendVerdict`,
  `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict` and `ArmVerdict` (D1).
- P1-5a, gate names in the private parts: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-5b, prose spellings: Done, [#3824](https://github.com/nifeos/nife/pull/3824).
- P1-5c, gate names in the public parts: Done, [#3824](https://github.com/nifeos/nife/pull/3824). `Rendezvous::one_queue_invariant_holds`
  and `Key::virtual_address` (D4).
- P1-6, the design record does not travel: At the move. The move writes `docs/design.md` and the
  README in place of the links to `notes/inter-process-communication.md`.
- P1-7, manifest metadata: At the move. The move writes the description, repository, keywords and
  categories rows.
- P1-8, a harness recorded as unfalsifiable: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-9, the mutation record is stale: At the move. A full run in the new repository, with the three
  survivors triaged.
- P2-10, a timer and a futex table in a crate named for inter-process communication: At the move.
  The README's first line, which states the shared reason (D2), comes with the move.
- P2-11, the one-queue invariant could hold by construction: Done,
  [#3963](https://github.com/nifeos/nife/pull/3963) (maintainer, provisional, 2026-11-22 UTC).
  `Rendezvous` holds one queue under a `Waiting` tag, five words on a 64-bit target in place of
  seven. `one_queue_invariant_holds` checks the half the layout leaves open, that a tag names a
  queue only while a thread is on it, and every rendezvous harness has a falsification patch cut
  against the new code.
- P2-12a, `Futexes<T, 0>` panics: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-12b, `Futexes` as a stranger reads it: Done, [#3963](https://github.com/nifeos/nife/pull/3963)
  and nifeos/intrusive_queue#2, which adds the read-only `Queue::iter` that lets
  `AddressWaitTable::waiter_count` take `&self`. [#3824](https://github.com/nifeos/nife/pull/3824) built the names (D5),
  [#3874](https://github.com/nifeos/nife/pull/3874) spelled the type out as `AddressWaitTable`,
  and [#3883](https://github.com/nifeos/nife/pull/3883) ratified them.
- P2-13, `Timer::deadline` answers with a sentinel: Done, [#3842](https://github.com/nifeos/nife/pull/3842). `Timer::deadline` and
  `earliest` answer `Option<u64>` (D7).
- P2-14, the pending count's saturation has no host test: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-15, five copies of the test node: Done, [#3842](https://github.com/nifeos/nife/pull/3842). `src/test_node.rs` holds the one node; the
  futex tests keep their own, which carries the key it waits on.
- P2-16a, documentation details: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-16b, tuples from `debug_counts`: Done, [#3824](https://github.com/nifeos/nife/pull/3824). `RendezvousCounts` and `NotificationCounts`.
- P2-17, two Kani toolchain warnings: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- D1, the names of the verdict enums: Done, [#3824](https://github.com/nifeos/nife/pull/3824), ratified in [#3883](https://github.com/nifeos/nife/pull/3883).
- D2, whether the timer and the futex table stay in this crate: At the move, with P2-10.
- D3, which crate leaves first: Done, [#3892](https://github.com/nifeos/nife/pull/3892).
- D4, two public names the gates refuse: Done, [#3824](https://github.com/nifeos/nife/pull/3824), ratified in [#3883](https://github.com/nifeos/nife/pull/3883).
- D5, the futex table's names: Done, [#3824](https://github.com/nifeos/nife/pull/3824), ratified in [#3883](https://github.com/nifeos/nife/pull/3883).
- D6, review clearance and the repository: At the move. [#3883](https://github.com/nifeos/nife/pull/3883) cleared the package and ratified
  the names; `nifeos/inter_process_communication` does not exist yet.
- D7, the timer's deadline type: Done, [#3842](https://github.com/nifeos/nife/pull/3842).

## Open decisions

None. Each decision carries the maintainer's provisional ruling and is in the rules in force
below.

## Goal

A stranger who finds `nifeos/inter_process_communication` can tell in one screen what the crate
decides and what it leaves to a kernel. The stranger can build, test and prove it from the README
alone, and meets no name, path or citation that only makes sense inside `nifeos/nife`. The crate's gates in
its own repository are green for the crate's own reasons. Its one dependency has left before it
or with it.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1095-inter-process-communication-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1095-inter-process-communication-outside-review/prior-art.md).

## The items

### P0: blocks the move

1. **No review package exists.**
   Finding: the move process every other crate followed starts with a package in
   `notes/crate-reviews/` and a row in the index. This crate has neither. Every public name but
   the crate's is `provisional`: `Rendezvous`, `Send`, `Receive`, `Notification`, `Signal`, `Wait`,
   `Futexes`, `Key`, `Timer`, `Arm`, `NEVER`, and every method. Three modules say in their own
   header that their name is provisional.
   Evidence: `notes/crate-reviews/` listing; `src/lib.rs` line 8; `src/rendezvous.rs` line 4;
   `src/notification.rs` lines 10 to 11; `src/futex.rs` line 38; `src/timer.rs` line 6.
   Change: write the package in the shape of `notes/crate-reviews/generational-table.md`: the
   public items with signatures, the names the gates refuse (item 5), the proposed renames
   (items 4, 12, 13), the open questions below, and the dependency position (item 2). The
   architect rules names from it, one at a time.
   Door: the package records one-way names. Effort: M.

2. **The one dependency has not left and appears in no plan.**
   Finding: `intrusive_queue` is a path dependency inside nife, not on the publish allowlist, not
   on crates.io, and absent from milestone 1017 (the contract crates leave in this order) and from
   every extraction milestone. A crate that has left must not reference back into nife, so this
   crate cannot leave before `intrusive_queue` does, or the two leave together. `intrusive_queue`
   has one harness, ratified names and no dependency of its own, so it is the smaller job; its
   `Unqueued<T>` is the type every signature here exposes.
   Evidence: `Cargo.toml` line 9; `crates/intrusive_queue/Cargo.toml`;
   `crates/publish-allowlist.txt`; the releasable-crates note in `notes/` lists it as "moves" with
   no milestone.
   Change: a review package and a move for `intrusive_queue` first, then this crate pins it by
   git revision on that repository's `main`, the way the kernel pins `generational_table`. Record
   the order in milestone 1017's list or a sibling.
   Door: one-way (a dependency another repository pins). Effort: S here, L for the dependency's
   own review and move.

3. **The crate does not stand alone.**
   Finding: `standalone.py` reports 15 section-citation lines, 14 milestone-citation lines, 13
   nife-path lines, 10 provenance-block lines and 7 lines naming the architect, across the sources
   and the 14 falsification patch headers. The manifest inherits `license` and `lints` from a
   workspace that will not be there, carries no `rust-version`, and carries `publish = false`
   with a comment that points at `crates/publish-allowlist.txt`. Every type's documentation sends
   the reader to `notes/inter-process-communication.md`, which stays in nife.
   Evidence: `standalone.py --manifest-dir crates/inter_process_communication`; `Cargo.toml`
   lines 5 to 12; `src/rendezvous.rs` lines 5 and 9; `src/notification.rs` line 8;
   `src/futex.rs` line 8; `src/timer.rs` line 4; `falsifications/*.patch` headers.
   Change: the move commit strips every `Name:` block (git holds the history). It rewrites each
   citation as the fact it cites (the §101 (notification objects: async multiplexing without wait-any) rule order becomes "a waiter first, then the bound
   receiver, then the word"). It replaces the note paths with the crate's own `docs/design.md`
   (item 6), writes the license and lints into the manifest, and sets `rust-version` (item 7).
   The patch headers lose their nife paths and keep the defect description.
   Door: two-way. Effort: M.

### P1: before the first tag

The P1 items are in [the appendix](1095-inter-process-communication-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1095-inter-process-communication-outside-review/later-items.md).

## Decisions for the architect

The rules in force, with their background and prior art, are in
[the appendix](1095-inter-process-communication-outside-review/ruled-decisions.md):

- D1, the verdict enums: the maintainer's provisional ruling of 2026-11-14 (UTC). `SendVerdict`,
  `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict` and `ArmVerdict`, with no alias.
- D2, the timer and the futex table: the maintainer's provisional ruling of 2026-11-14 (UTC). The
  four modules stay together, and a publish review settles the scope if the crate goes to one.
- D3, the order of leaving: the maintainer's provisional ruling of 2026-11-19 (UTC). `intrusive_queue`
  leaves first, then this crate pins it by git revision on that repository's `main`.
- D4, two names the gates refuse: the maintainer's provisional ruling of 2026-11-19 (UTC).
  `one_queue_invariant_holds` and `Key::virtual_address`.
- D5, the futex table's names: the maintainer's provisional ruling of 2026-11-19 (UTC), with the
  type spelled out by the ruling of 2026-11-21 (UTC) that spells futex out as address wait.
  `AddressWaitTable<T, const BUCKETS: usize>`, `len()` and `Key::address_space`.
- D6, review clearance: the maintainer's provisional ruling of 2026-11-19 (UTC). The package is
  written now; no tag and no publication.
- D7, the timer's deadline type: the maintainer's provisional ruling of 2026-11-20 (UTC).
  `Timer::deadline` and `earliest` answer `Option<u64>`; `NEVER` stays as the kernel's cache word.


## Follow-on

- **Recorded.** Every finding marked At the move under "Finding status" lands with the move, each
  checked against the crate's sources on main and `nifeos/intrusive_queue`.
- **Recorded.** `crates/externalization-list.txt` does not name this crate; milestone 1017's list
  records the pair, `intrusive_queue` first.

## Done when

The checks are in [the appendix](1095-inter-process-communication-outside-review/done-when.md).

## BUGS

- The review found no package to take as its baseline, so it may list as open a name the
  architect ruled in a conversation nobody recorded. Item 1 is the fix either way.
- The size figures in item 11 come from counting field types, not measuring; a lane that takes the
  option measures first.
- Item 2's claim that `intrusive_queue` appears in no extraction milestone rests on a text search
  of `design/roadmap/` and `notes/`; a plan under another name would escape it.
- The crates.io figures for `parking_lot_core` and `cordyceps` were not fetched; their rows in the
  prior-art box say so.
- The review did not replay the fourteen falsification patches under Kani, only applied them with
  `git apply --check`; the template's `falsify` job is the replay.
- One observation about the kernel's use of `bind_to_interrupt` went to the maintainer
  by reference and is not in this file.

## Index row

An outside review of `inter_process_communication` before it leaves. Three items block the
move (no review package, a dependency that has not left, text that does not stand alone). Six
land before the first tag (a type named `Send`, five gate findings, the design record, manifest
metadata, a harness whose unfalsifiable claim is false, a stale mutation record). Eight wait for
later. The verdict names, the crate's scope, the order in which it and `intrusive_queue` leave, three
public names, two gate-refused names and review clearance are all ruled provisionally; no decision
is open.
