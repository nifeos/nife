---
status: PARTIAL
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

## Where this stands

Finished: the review fixes that need no ruling (item 5's private parts and most prose, items 8, 12's assertion, 14, 16's wording and 17) landed in #3566, and #3589 records the maintainer's provisional rulings on D1 and D2.

- Item 4 (verdict names): ruled as D1, not built; a lane.
- Item 5 (prose spellings): partly done; five British spellings and a doc-example `pa` remain; a lane.
- Items 1, 5, 12, 13 (review package, public names): not built; a lane writes the package, then decision for calef on each name.
- Item 2 (order of leaving, D3): open; decision for calef.
- Item 10 (scope): ruled as D2; the README sentence waits on the move.
- Items 3, 6, 7, 9 (stand-alone text, design record, manifest, mutation run): the move, which waits on D3, on `intrusive_queue` leaving, and on review clearance from calef.
- Items 11, 12 iterator, 15, 16 diagnostic structs (maintainer options): two-way; a lane, mostly after the move.

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

Each is one decision, with the background first and the recommendation before the alternatives.
D1 and D2 are decided, provisionally.

### D1. The names of the verdict enums

Background: the crate hands back what an operation decided as an enum per operation: `Send`,
`Receive`, `Signal`, `Wait`, `Arm`. The prose in the same files calls these "verdicts". `Send` at
the crate root is also the name of the standard library's marker trait, in the same namespace, so
a user who imports it loses the trait in that module (item 4). Once published, a rename is a
breaking change for every user; before, it costs one commit in the kernel.

Recommendation: `SendVerdict`, `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict`, `ArmVerdict`.
One suffix across the four modules, the word the documentation already uses, and no collision
with anything in `core`.

Alternatives: `Sent`/`Received` (shorter, but a past participle reads as a fact, and `Blocked` names
nothing that anyone sent). Keep `Send` and tell users to import by path (the footgun stays for a
glob import). `SendOutcome` (fine, but the tree says "verdict" everywhere it explains these).

Prior art: Rust's `std::cmp::Ordering`, `std::ops::ControlFlow` and `std::task::Poll` are the
standard library's verdict enums, each a noun that names the kind of answer; none shares a name
with a trait. seL4's equivalents are not types at all (a C function's side effects).

Concrete example: today, `use inter_process_communication::{Rendezvous, Send}; fn park<T: Send>()`
fails to compile with "expected trait, found enum `Send`". With the recommendation it compiles,
and `match ep.send(me) { SendVerdict::Blocked => ..., }` reads as the documentation already does.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The five take the `Verdict` suffix: `SendVerdict`,
`ReceiveVerdict`, `SignalVerdict`, `WaitVerdict` and `ArmVerdict`, with no alias. Item 4 is the
work.

### D2. Whether the timer and the futex table stay in this crate

Background: the crate has its ratified name. Its contents grew: the rendezvous and the notification
are inter-process communication; the timer and the futex wait queues are not, and arrived later
because they share the token discipline and the proof shape. While nife is the only user, moving
a module out is a two-way door. The first publish makes the crate's contents a claim the name
makes to strangers (item 10).

Recommendation: keep the four together through the move, publish nothing (publishing stays on hold in
any case), and settle the scope at the publish review with the measured demand in hand. State the
shared reason in the README's first line so a stranger is not surprised. If the scope is then
split, `timer` and `futex` leave as their own crates before the first tag, which nobody outside
nife will have pinned.

Alternatives: split now (two more repositories to review and move before this one can; the
timer is 150 lines); rename the crate to say what it holds (reopens a ratified name for a
question the publish review can answer later).

Prior art: seL4 keeps endpoints and notifications in one kernel module family and timers in
another; Linux keeps `futex` and `hrtimer` apart; Zircon keeps channels, futexes and timers as
separate dispatcher types. No kernel bundles the four under a communication name; all four bundle
them under one lock, which is what this crate mirrors.

Concrete example: a crates.io search for "futex wait queue" finds `parking_lot_core` and
`futex`, never a crate named for inter-process communication. A search for "rendezvous endpoint
seL4" would find this one, and `timer::NEVER` in the sidebar would surprise the reader.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. The four modules stay together, and the scope is settled at the publish review if this
crate ever goes to one. Nothing is published and nothing outside nife links the crate.

### D3. The order of leaving: `intrusive_queue` first

Background: every public signature here exposes `Unqueued<T>` and `Node` from `intrusive_queue`,
which has not left and appears in no plan (item 2). A crate that has left may not reference back
into nife. Pinning a path dependency is not an option, and a published crate cannot depend on
an unpublished one.

Recommendation: `intrusive_queue` gets its own package and repository first, this crate pins it
by git revision on `main`, and milestone 1017's list (or a sibling) records the pair in that order.
`intrusive_queue` is small, its names stand ratified, it has one harness with a falsification, and
no dependency, so its review is the shorter one.

Alternatives: move both in one lane as two repositories (saves a round trip; two packages to
rule on at once, which the one-decision-at-a-time rule discourages). Inline the queue into this
crate (duplicates the kernel's run-queue type, which is the thing the crate exists not to do).

Prior art: `generational_table` left before the kernel pinned it; `extensible_host_controller_interface`
left and the kernel pins its revision. The same step, one level down.

Concrete example: after the move, the manifest line reads
`intrusive_queue = { version = "0.1.0", git = "https://github.com/nifeos/intrusive_queue", rev = "<full sha>" }`,
the form the kernel already uses for the two crates that left.

## Follow-on

- **Outstanding.** The order of leaving (D3), which the architect rules, and the verdict names and scope (D1, D2), ruled provisionally and not yet built. Checked: the ledger's LEFT rows for items 2, 4, 10, 5, 12, 13 and 16 are these.
- **Outstanding.** The move's own work: the review package, the stand-alone text, the manifest rows, `docs/design.md` with a README, and one full mutation run (items 1, 3, 6, 7, 9). Checked: each needs the new repository.
- **Outstanding.** Options for the maintainer after the move: the one-queue layout, one shared test node and an iterator on `intrusive_queue` (items 11, 12, 15). Checked: each changes proofs or another crate.

[The ledger](1095-inter-process-communication-outside-review/ledger.md) lists each item.

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
later. Three decisions go to the architect, on the verdict names, the crate's scope and the order
in which it and `intrusive_queue` leave.
