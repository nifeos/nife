---
status: NOT-STARTED
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
one-way when somebody outside this repository acts on it after the move or the release (a
published name, a dependency another repository pins, a wire format, a layout somebody else
reads); everything else is two-way and the maintainer decides it. Until the crate is published,
every public name is two-way in fact and one-way in intent, because the first publish freezes it;
the items say which.

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
   citation as the fact it cites (the §101 rule order becomes "a waiter first, then the bound
   receiver, then the word"). It replaces the note paths with the crate's own `docs/design.md`
   (item 6), writes the license and lints into the manifest, and sets `rust-version` (item 7).
   The patch headers lose their nife paths and keep the defect description.
   Door: two-way. Effort: M.

### P1: before the first tag

4. **A public type named `Send` shadows `core::marker::Send`.**
   Finding: `pub use rendezvous::{Receive, Rendezvous, Send}` puts a type named `Send` at the
   crate root. Types and traits share one namespace, so `use inter_process_communication::*` or
   `use inter_process_communication::Send` in a user's module hides the prelude's `Send` trait
   there; `T: Send` then fails to parse as a bound. The kernel never imports it, which is why
   nothing has noticed. A stranger's first `use` will.
   Evidence: `src/lib.rs` line 37; `src/rendezvous.rs` line 66.
   Change: the architect's decision (D1 below). The crate's own vocabulary is "verdict"
   (`src/rendezvous.rs` lines 64, 81; `src/notification.rs` line 79), so `SendVerdict` and
   `ReceiveVerdict` are the names the documentation already uses in prose; `Signal`, `Wait` and
   `Arm` would follow for one shape across the four modules.
   Door: one-way at publish (a public name). Effort: S.

5. **Names the template's ceiling-zero gates refuse.**
   Finding, one line each:
   - `Rendezvous::one_queue_invariant` returns `bool` and is not shaped as a question
     (`question_shape`); its twin `Notification::invariant_holds` already follows the rule, and the
     item's own block says the rule postdates it.
   - `futex::Key::address` and the test helper parameter `address`: an address is
     `_virtual_address` or `_physical_address` in every location (`register_names`). The field is a
     virtual address, and the kernel's `futex_wait` passes a `va`.
   - `a_cancelled_or_rearmed_timer_never_fires_stale`: British spelling (`british_spelling`);
     `canceled`.
   - Test bindings `pa` and `pb` in `notification.rs` tests: `spelled_out` reads `pa` as the
     abbreviation of physical address. They are pointers to `a` and `b`; `pointer_a` says so and
     passes.
   - Eight prose spellings `typos` (en-us) refuses: `signalled` (twice), `signalling`,
     `queueing` (twice), `modelled`, `neighbouring`, and `behaviour` in one patch header.
   Evidence: `src/rendezvous.rs` line 180; `src/futex.rs` lines 51 and 172; `src/timer.rs`
   line 250; `src/notification.rs` lines 456 and 518; `src/verification.rs` line 76;
   `src/timer.rs` line 13; `src/futex.rs` line 5; `src/rendezvous.rs` line 38;
   `src/notification.rs` lines 58, 246 and 325;
   `falsifications/verification.send_rendezvous_iff_a_receiver_waited.patch` line 5.
   Change: `one_queue_invariant_holds` (the name the block itself recommends), `Key::virtual_address`,
   the test and prose spellings. All but the first two are mechanical.
   Door: the two public names are one-way at publish; the rest two-way. Effort: S.

6. **The design record does not travel with the crate.**
   Finding: `notes/inter-process-communication.md` is the only place that states the token discipline. It also states the two obligations the harnesses do not check (every node outlives its object; one mint per node), why there are two `remove_*` methods, and why the timer proves a cache invariant instead of keeping a heap. Every doc comment points at it. The note stays in
   nife and moves to the record site, so a crates.io reader has nothing. The scaffold's
   `docs/stranger-test.md` reads `Last run: none` and the release gate refuses on that line.
   Evidence: `src/rendezvous.rs` lines 5, 9, 212; `src/notification.rs` line 8; `src/futex.rs`
   line 8; `src/timer.rs` line 4; `crate-workflows/scaffold/docs/stranger-test.md`.
   Change: `docs/design.md` in the crate's repository, the note's content without nife
   vocabulary, and a README from `scaffold/README-crate.md` whose "Running the proofs" section states what this review measured. `cargo kani` takes about half a minute and about 220 MB. One harness runs as `cargo kani --harness verification::send_preserves_the_invariant --exact`. A passing run prints two `trailing semicolon in macro` warnings from Kani's toolchain (item 17
   removes them). Then a stranger run on record.
   Door: two-way. Effort: M.

7. **Manifest metadata for a crate that readers will open on crates.io.**
   Finding: no `description`, `repository`, `readme`, `keywords`, `categories` or
   `rust-version`. The edition is 2024, which needs Rust 1.85; `Option::is_none_or` in the
   proofs needs 1.82 and the inline `const` block in `Futexes::new` needs 1.79, so 1.85 is the
   floor, and the template's weekly `msrv` job builds at whatever the manifest says.
   Evidence: `Cargo.toml`; `src/verification.rs` line 201; `src/futex.rs` line 69.
   Change: the fields, `rust-version = "1.85"`, and a one-line description that says what the
   crate decides ("the wait-queue decisions of a kernel's rendezvous, notification, timer and
   futex, as safe state machines over intrusive queues").
   Door: two-way (the description is prose). Effort: XS.

8. **A harness recorded as unfalsifiable has a minimal falsification.**
   Finding: `signal_preserves_the_invariant` carries a paragraph arguing that no minimal defect
   in `signal` can turn it red, under §134 (a harness carries a machine-replayable falsification
   record, or it is not evidence). Replacing `self.pending.saturating_add(1)` by
   `self.pending + 1` turns it red: the seed makes `pending` symbolic, so Kani's overflow check
   fails at `u32::MAX`. The harness therefore proves something the paragraph does not claim: a
   signal never panics, whatever the count. That is worth stating and recording.
   Evidence: `src/verification.rs` lines 71 to 90; `src/rendezvous.rs` line 318.
   Change: either add `falsifications/verification.signal_preserves_the_invariant.patch` with
   that one-line defect and rewrite the paragraph to say what the harness guards (no enqueue path,
   and no overflow), or keep the paragraph and say why an overflow failure does not count. The
   first is the §134 shape.
   Door: two-way. Effort: XS.

9. **The mutation record is stale and no one has triaged the three survivors.**
   Finding: the mutation index row says 24 mutants; `cargo mutants --list` today says 178,
   because the futex and timer modules arrived after that count. The latest census row records 3
   missed, all classed "algebraic", 3 unclassified, 1 copied. No note says which three.
   Evidence: `notes/mutation-testing/README.md` row for the crate;
   `notes/mutation-testing/survivor-classification.md` line 53; `cargo mutants --list` output.
   Change: one full `mutation.yml` run in the new repository inside the window, the three
   survivors named and either killed by a test or exempted in `.github/mutants-exempts` with a
   reason. Item 14 is one likely kill.
   Door: two-way. Effort: S, plus the run.

### P2: later

The P2 items are in [the appendix](1095-inter-process-communication-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. Each is one decision, with the background first and the recommendation
before the alternatives.

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
