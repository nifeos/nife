---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1110. `subtree_scope`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/subtree_scope` as a stranger to this tree would meet it,
run on a fresh shallow clone of `main` at `7fc52ac8`. It changes nothing; it lists what the move
lane and the lanes after it change.

There is no review package for this crate in `notes/crate-reviews/`, so there is no settled
baseline to take rulings from. The baseline this review took instead has four parts. The name `subtree_scope` is
provisional (its `Name:` block and `script/names subtree_scope` both say so). `Binding::SyncOnly`
and `Bindings::bind_sync_only` are ratified names. The unknown-badge rule of milestone 726 (an
unknown badge fails closed in subtree_scope) is a ruling. The crate leaves at step 9 of
milestone 1017 (the contract crates leave in this order), after the contract crates it links. None
of those is reopened here. One ratified-name check: no later rule (units, standard-library
spellings) contradicts `SyncOnly` or `bind_sync_only`.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-move`,
`script/crate-review-packet`, Kani, cargo-mutants. No new tool.

## Goal

A stranger who finds `subtree_scope` in its own repository can tell in one screen what it decides:
whether a path stays beneath one directory, and which handles one client may use. The same stranger
can build, test and prove it from the README alone, call `walk` from a doctest that compiles, and
read every refusal as a POSIX errno without a nife glossary. The repository's gates are green for
the crate's own reasons.

## Finding status


- P0-1, the crate does not stand alone: At the move. The README, both license files, the inlined
  `license`, lints and `rust-version`, and a `publish` comment a stranger can act on remain.
- P0-2, the shipped text speaks to this tree: At the move. The module doc still opens with a
  milestone, a dated ruling and `fs_subtree_caretaker`; the rewrite in the crate's own words takes
  the hard-cases prose of `notes/walk-cost.md`.
- P0-3, the dependency on `filesystem_contract` has nowhere to point: Waiting on
  `filesystem_contract`'s repository (milestone 1017 step 2). The template half
  landed in [nifeos/crate-workflows#66](https://github.com/nifeos/crate-workflows/pull/66), so
  `deny.toml` admits git sources on the organization; the `git` and `rev` pin needs the repository,
  and the contract crates leave in the order milestone 1017 gives.
- P1-4, `landing` answers `EINVAL` for `EISDIR`: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-5, `unbind` uses handle 0 for nothing to close: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-6, the badge-to-slot cast truncates on 32 bits: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-7, names the rules refuse: Done, [#3813](https://github.com/nifeos/nife/pull/3813). The eight
  renames (`PROOF_PATH_BYTES`, `directories`, `want_directory`, `component`, `NodeKind`,
  `Binding::Unbound`, `check_landing`, `Bindings::binding`) stand in the tree and in
  `redoxfs_server`. The architect ratified them with the review package on 2026-11-20 UTC.
- P1-8, no example anywhere: At the move. The doctests on `walk` and `Bindings` landed in
  [#3580](https://github.com/nifeos/nife/pull/3580); the README's `## EXAMPLES` remains.
- P1-9, what the walk proofs leave out and cost: Open. The completeness harness
  `a_walk_asks_for_every_component_in_order` verifies at unwind 7, but costs 11 minutes of wall
  time on a loaded host (658 s of CPU) at a peak of 4.6 GiB, against the crate's whole row of 234
  seconds. That overruns a prove shard's room, so it stays out of the tree. The package records the
  measurement and the harness's shape. A lane that makes room in the shards, or shrinks the
  harness, adds it.
- P1-10, passive voice over the template's ceiling: At the move. The template counts 26 passives in
  4,081 words against an allowance of 8; the rewrite goes with P0-2.
- P2-11, `Refusal::errno` restates the contract's numbers: Done,
  [#3610](https://github.com/nifeos/nife/pull/3610).
- P2-12, `admit` takes three `u64` words in a row: Done,
  [#3610](https://github.com/nifeos/nife/pull/3610).
- P2-13, `cfg_attr(not(test), no_std)`: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-14, how the requested rights mask: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-15, where the crate is stricter than Linux: At the move. The README's section on `openat2` and
  `cap-std` goes with P0-1.
- Decision, the crate's name: Done, [#3589](https://github.com/nifeos/nife/pull/3589) (maintainer,
  provisional, 2026-11-14 UTC; nothing to build).

## Decisions

### Clearing the move of `subtree_scope`

Ruled 2026-11-20 (UTC): the architect cleared the
[review package](../../notes/crate-reviews/subtree-scope.md) and approved the repository
`nifeos/subtree_scope` with `publish = false`, no tag and no publish. The move waits on
`filesystem_contract`'s repository for P0-3's `git` pin. The package holds the ratified names.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1110-subtree-scope-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1110-subtree-scope-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside this tree acts on it after the move or the release (a repository name, a published
name or behavior, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The crate does not stand alone. LEFT: move work.
   Finding: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`; `license.workspace = true`,
   `[lints] workspace = true`, no `rust-version`; `publish = false` with a comment that points at
   `crates/publish-allowlist.txt`, a file the crate repository will not have.
   Evidence: `crates/subtree_scope/Cargo.toml` lines 5 to 9; the dry run's `standalone` step, 10
   lines; the review packet's manifest and files tables.
   Change: inline `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the version every
   review package states), and `[lints.rust] unsafe_op_in_unsafe_fn = "warn"`,
   `missing_docs = "warn"`; copy both license files; replace the `publish` comment with one
   sentence a stranger can act on. Write the README in the scaffold's shape (`## EXAMPLES`,
   `## What is proven`, `## BUGS`, `## Minimum Rust version`, `## License`), with the Kani time
   and peak memory the template's README asks for (the recorded figures: about 4.4 minutes and
   5.9 GiB at one job).
   Door: two-way. Effort: S.

2. The shipped text speaks to this tree, not to a stranger. LEFT: move work (the module doc rewrite).
   Finding: the module doc opens with a milestone number, the architect's name and a dated ruling,
   and goes on in nife's vocabulary: an eligible server, the progenitor, a window, a caretaker,
   `RECEIVE_CAPABILITY`, ruling D, a note under `notes/`. After the move strips the `Name:`
   blocks, 4 milestone citations, 3 lines naming the architect, 3 section citations and 1 path into
   this tree remain. The one `consumer-name` hit of the externalization lint is
   `fs_subtree_caretaker`. The first `BUGS` entry describes the window-reuse hazard in terms of the
   progenitor's reap, which a stranger cannot act on.
   Evidence: `crates/subtree_scope/src/lib.rs` lines 1 to 20, 38 to 39, 163, 200 to 201, 435 to
   436; the dry run's `citations` steps; `helpers/lint_externalized_crates.py --count`.
   Change: rewrite the module doc as the crate's own words. A subtree grant is a directory and
   everything a path can reach beneath it. Three things never count as beneath (`..`, a symbolic
   link, a mount point). A bound client reaches its root and the handles it minted. The doc ends
   with the table of refusals and their errnos. Carry the hard-cases prose of `notes/walk-cost.md` ("The hard cases,
   as one crate decides them") into the module doc, since the note stays behind. Rewrite the first
   `BUGS` entry as the caller's obligation: the caller must not hand a revoked badge to a new client
   while its old holder can still send. Rewrite the second as "the proofs are bounded, and here are the
   bounds". `fs_subtree_caretaker` goes.
   Door: two-way. Effort: M.

3. The dependency on `filesystem_contract` has nowhere to point yet. PARTIAL: the template's rule landed (nifeos/crate-workflows#66). LEFT: `filesystem_contract`'s repository, then the pin.
   Finding: the only dependency is `filesystem_contract` by path. The runbook's bottom-up rule
   (`notes/crate-move.md`, "A crate leaves only when nothing it depends on is still in this
   tree") holds the move until `filesystem_contract` has its own repository, which step 2 of
   milestone 1017 schedules. The scaffold's `deny.toml` admits git sources on the organization
   (nifeos/crate-workflows#66), and the maintainers hold publishing to crates.io, so the
   dependency takes a `git` and `rev` pin. This is the first moved crate to depend on another
   nifeos crate.
   Evidence: `crates/subtree_scope/Cargo.toml` line 13; `crate-workflows/scaffold/deny.toml`
   lines 30 to 33; `design/roadmap/1017-the-contract-crates-leave-in-this-order.md`, the step
   table.
   Change: in the template, an `allow-git` entry for `https://github.com/nifeos/` so a crate may
   depend on a sibling by `git` and `rev` during the soak, with the rule written beside it; this
   block names the dependency on the blocked side. In the crate, once `filesystem_contract` has
   its tag: `filesystem_contract = { version = "0.1.0", git =
   "https://github.com/nifeos/filesystem_contract", rev = "<commit of the tag>" }`. The
   alternative cuts the dependency by making `walk` generic over a rights trait. This review does
   not recommend it. The walk's one job is the contract's rights arithmetic, and a second spelling
   of `DESCEND` would be a second place for it to be wrong.
   Door: two-way for the pin; the template rule is the maintainer's. Effort: S, in the template.

### P1: before the first release

The P1 items are in [the appendix](1110-subtree-scope-outside-review/before-the-release.md).

### P2: later

The P2 items are in [the appendix](1110-subtree-scope-outside-review/later-items.md).

## Decisions for the architect

One decision, ruled; the move is under "Open decisions" above.

### The crate's name

The crate keeps `subtree_scope` (maintainer, provisional, 2026-11-14 UTC). It is a noun phrase,
spelled out, and it says what the crate is: the scope of a subtree grant. `use
subtree_scope::{walk, admit}` reads as what the two calls do. The door is two-way. The crate's one
consumer is `redoxfs_server`, and step 9 of milestone 1017 places it in its own repository,
`nifeos/subtree_scope`.
The publishing hold keeps it off crates.io. If the crate is ever published on
its merits, its crates.io name comes to the architect then, and ratifying it stays the architect's.

The alternatives lost. `beneath` is the field's term (`RESOLVE_BENEATH`, `O_RESOLVE_BENEATH`), but
it is a preposition, and a crate takes a noun. `subtree_grant` names the thing a program gets,
where the crate decides what a grant reaches and never makes one. `directory_scope` loses the word
that says the whole tree beneath counts. Linux and FreeBSD name the behavior by the preposition;
`cap-std` names the type (`Dir`) and leaves the property to prose; none has a crate whose only job
is this decision, so none offers a name to copy.

## Done when

The checks are in [the appendix](1110-subtree-scope-outside-review/done-when.md).

## BUGS

- Item 6 came from reading, not from a build for a 32-bit target. The fix is cheap enough to land
  without one.
- Item 9's cost figures are this tree's recorded medians on its own runners. Nobody has verified
  the hosted runner's memory against the 5.9 GiB peak. The first `proofs` run in the repository
  will say.
- The review did not run `cargo kani`; the recorded falsification times and the reach record stand
  in for it. The reach record predates `Binding::SyncOnly`.
- No review package exists for this crate, so the baseline above is this review's reading of the
  `Name:` blocks and the roadmap, not an architect's clearance.

## Follow-on

- **Milestone 1017.** The crate moves at its step 9; the move lane takes the items marked At the
  move, and P0-3's pin waits on `filesystem_contract`'s repository at step 2.
- **Outstanding.** Item 9, which needs a Kani run of about 6 GiB.

## Index row

An outside review of `subtree_scope` before it leaves. Three findings block the move (the
manifest and files, the in-tree vocabulary of its text, and a dependency on `filesystem_contract`
that has nowhere to point yet). Seven land before the first tag, and five wait for later. The seven
cover an `EISDIR` arm left to the caller, a handle-0 sentinel in `unbind`, a 32-bit badge
truncation, eight names, the missing examples, a walk-completeness proof and the passive-voice
ceiling.
