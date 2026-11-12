---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
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

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1110-subtree-scope-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1110-subtree-scope-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside this tree acts on it after the move or the release (a repository name, a published
name or behavior, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The crate does not stand alone.
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

2. The shipped text speaks to this tree, not to a stranger.
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

3. The dependency on `filesystem_contract` has nowhere to point yet.
   Finding: the only dependency is `filesystem_contract` by path. The runbook's bottom-up rule
   (`notes/crate-move.md`, "A crate leaves only when nothing it depends on is still in this
   tree") holds the move until `filesystem_contract` has its own repository, which step 2 of
   milestone 1017 schedules and which has not happened. When it has, the scaffold's `deny.toml`
   denies every git source (`unknown-git = "deny"`, no `allow-git`), and the maintainers hold
   publishing to crates.io, so neither way of writing the dependency passes `check / supply-chain` today. No moved
   repository depends on another nifeos crate yet: `block_roster_contract`'s dependency table is
   empty (verified through the GitHub API); the others showed no dependency table (unverified
   beyond that).
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

4. `landing` answers `EINVAL` where the contract says `EISDIR`, and leaves the arm to the caller.
   Finding: `landing(Kind::Directory, false)` returns `Refusal::Malformed` (errno 22), with a
   comment that the server maps this one arm itself. The one caller does: `open_file_path` tests
   `kind == Kind::Directory` and answers `EISDIR` before it calls `landing`, so the arm is dead in
   this tree and a wrong answer for any other caller. A refusal enum with a hole in it is a trap a
   stranger falls into on the first file server that is not this one.
   Evidence: `crates/subtree_scope/src/lib.rs` lines 145 to 147; `redoxfs_server/src/server.rs`
   lines 102 to 106; `filesystem_contract/src/dir.rs` line 88 (`EISDIR` is 21).
   Change: add `Refusal::IsADirectory` with errno 21, return it from that arm, delete the comment
   and the server's special case (the server's own copy of the rule goes, the behavior stays).
   Adding a variant to a public enum is one-way after the first release, so it lands before the
   tag.
   Door: two-way now, one-way after the tag. Effort: XS.

5. `unbind` uses handle 0 to mean "nothing to close".
   Finding: for a sync-only badge, `unbind` returns `Ok(ROOT)` as "no directory to hand back".
   `ROOT` is 0, and 0 is also a handle a binder may bind a badge to: `bind(caller, badge, 0)` succeeds, and in the server that is the image root with every right. The server tells the two
   apart with `root != ROOT`, which happens to be right for both (handle 0 is never closed), but
   the API says one value for two facts. The same function computes `Self::index(badge)` twice
   under two names (`f`, then `i`), and the equivalent-mutant survivor of the census is the
   `i != 0` guard this shape needs.
   Evidence: `crates/subtree_scope/src/lib.rs` lines 283 to 302; `redoxfs_server/src/server.rs`
   lines 760 to 774; `notes/project-metrics/mutation-triage.csv` row 305.
   Change: `unbind` returns `Result<Option<u64>, Refusal>`: `Some(root)` for a bound badge,
   `None` for a sync-only one. One `index` call, one `match` over the slot. The server closes on
   `Some`. The census then has no survivor to explain.
   Door: two-way (one-way after the tag, as a public signature). Effort: XS.

6. The badge-to-slot cast truncates on a 32-bit target.
   Finding: `index` compares `(badge as usize) < B`. On a target whose `usize` is 32 bits, badge
   2^32 + 2 casts to 2 and folds onto slot 2, so `bind(caller, 2^32 + 2, root)` binds badge 2
   while `of(2^32 + 2)` answers `Revoked`. The `u64` compare on line 236 guards `of`;
   `bind`, `bind_sync_only` and `unbind` have no such guard. Only an open caller can bind, and the three
   bare-metal targets the move stages are 64-bit, so this is a hole in the claim, not a defect a
   current user can reach. The harness `a_badge_with_no_window_is_never_open` runs on a 64-bit
   host and cannot see it.
   Evidence: `crates/subtree_scope/src/lib.rs` lines 225 to 231 (`index`), 254, 272, 289, 294.
   Change: make `index` return `Option<usize>` through `usize::try_from(badge).ok().filter(|&i|
   i < B)`, have `of` use it, and have the three mutators refuse `None` as they refuse slot 0. One
   sentence in `Bindings`' doc. No behavior changes on any 64-bit target.
   Door: two-way. Effort: XS.

7. Names the naming rules refuse, or that read wrong to a stranger.
   Finding, each with the rule: `PROOF_PATH` is a length in bytes and carries no unit
   (`design/naming.md`, Units: a length carries its unit; `PROOF_BADGES` and `PROOF_OPERATIONS`
   are counts of things and stand). The binding `dirs` (line 115) and the parameter `want_dir`
   (line 139) abbreviate `directory`, which the template's `spelled_out.py` refuses at a ceiling
   of 0. `is_name(c: &[u8])` has a one-letter parameter (the rules apply to parameters). `Kind`
   is a generic word the rules name as a failure mode. `Binding::Open` collides with the file verb
   in a filesystem crate. `landing` is a noun on a function that judges, beside `check_path`.
   `Bindings::of` has no standard-library precedent (`get` would promise an `Option`).
   Evidence: `crates/subtree_scope/src/lib.rs` lines 83, 115, 139, 70, 156, 139, 235, 306.
   Change, recommended spellings: `PROOF_PATH_BYTES`; `directories`; `want_directory`;
   `component`; `NodeKind`; `Binding::Unbound`; `check_landing`; `Bindings::binding`. The `hop`
   parameter of `walk` could take the server's word, `requested`, since the doc already explains
   it as the rights each step asks for. One pull request, after the maintainer rules the two-way
   names provisional.
   Door: two-way before the tag; every public one is one-way after it. Effort: S.

8. There is no example anywhere.
   Finding: 0 doctests, no `examples/`, no README. A stranger has the test module's `tree`
   function as the only picture of how to write `lookup`.
   Evidence: `cargo test` output ("Doc-tests subtree_scope: running 0 tests");
   `crates/subtree_scope/src/lib.rs` lines 463 to 472.
   Change: a doctest on `walk` over a four-node toy tree (the test's `tree`, lifted into the doc).
   It shows a success, a `Symlink` refusal and a `Narrowed` refusal. A second doctest on
   `Bindings` shows bind, admit through `ROOT`, unbind, and the `NotYours` after. The README's `## EXAMPLES`
   carries the first.
   Door: two-way. Effort: S.

9. What the walk proofs leave out, and what they cost.
   Finding: the two walk harnesses state that every step is a name and that a successful walk went
   through directories only and never widened. Neither states that `walk` asks `lookup` for every
   component but the last, in order; a `walk` that skipped a component, or stopped early and
   returned the start, passes both (the unit tests catch it, the proofs do not). Neither says
   when `walk` answers `NotFound` and `Narrowed`. The reach record agrees: the walk harness killed 3
   of the 19 mutants in its direct reach and the one-name harness 2 of 8 (the record predates
   `SyncOnly`; the proportions are what is on file). The two harnesses also cost the most:
   falsification replays of 242 s and 104 s, and the crate's 5.9 GiB peak, for a 5-byte path at
   unwind 7. The template's `proofs` job runs under a 20-minute default budget and `falsify` under
   30. Nobody has verified whether a hosted runner holds the peak.
   Evidence: `crates/subtree_scope/src/lib.rs` lines 329 to 379;
   `notes/kani-reach-2026-10-04.csv` rows 179 to 180;
   `notes/project-metrics/falsification-times.tsv` rows 342 to 343; `helpers/kani-peaks.tsv`.
   Change: one harness, `a_walk_asks_for_every_component_in_order`, whose `lookup` records the
   steps it saw and asserts afterwards that they equal the components before the last `/`, in
   order. Its falsification patch makes `walk` skip the first component. Extend
   `a_walk_never_widens_follows_or_crosses` with the two refusal conditions, or state in the README
   that tests cover them and proofs do not. Measure the peak after; if it stays near 6 GiB, state the
   figure in the README beside the install line.
   Door: two-way. Effort: S.

10. Passive voice over the template's ceiling.
    Finding: `passive_voice.py` at the template's head counts 20 passive constructions in 3,366
    words against a ceiling of 6. The pinned release (v7) does not run it; the first pin bump past
    v7 will make `check` red for this reason alone.
    Evidence: `passive_voice.py` over the staged `src/lib.rs`, five of the lines: 257, 321, 339,
    427, 587 (staged numbering).
    Change: put 14 sentences into the active voice while item 2 rewrites the doc; one pass.
    Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1110-subtree-scope-outside-review/later-items.md).

## Decisions for the architect

One one-way door is open. Everything else above is the maintainer's.

### The crate's name becomes a repository name at the move

Background. The crate decides two things for a file server. The first is whether a path stays
beneath one directory (every component is a name, no symbolic link, no mount point, rights only
narrow). The second is which handles one client may use (its grant's root, and the handles it
minted). Its name has been
provisional since its minting. The move makes it the repository's name and, if the crate is
ever published, the crates.io name. Both are one-way.

Recommendation: keep `subtree_scope`. It is a noun phrase, spelled out, with no abbreviation, and
it says what the crate is: the scope of a subtree grant. `use subtree_scope::{walk, admit}` reads as
what the two calls do.

Alternatives considered. `beneath` is the field's term (`RESOLVE_BENEATH`, `O_RESOLVE_BENEATH`),
one word, and a reader from Linux or FreeBSD knows it; but it is a preposition, and the rules say a
crate takes a noun. `subtree_grant` names the thing a program gets, where the crate decides what a
grant reaches and never makes one. `directory_scope` loses the word that says the whole tree beneath
counts.

Prior art: Linux names the behavior by the preposition (`RESOLVE_BENEATH`); FreeBSD the same
(`O_RESOLVE_BENEATH`); `cap-std` names the type (`Dir`) and leaves the property to prose; Fuchsia
speaks of rights, not scope. None of them has a crate or a module whose only job is this decision,
so none offers a name to copy.

Concrete example: a server's manifest line `subtree_scope = { version = "0.1.0", git = ..., rev =
... }` beside `filesystem_contract`, and the README's first sentence, "What a grant rooted at one
directory can reach, decided in one place a server links."

## Done when

The checks are in [the appendix](1110-subtree-scope-outside-review/done-when.md).

## BUGS

- Item 3 is the template's to decide. Until `deny.toml` admits a sibling git source, or a sibling is
  on crates.io, no crate that depends on another nifeos crate can pass `check / supply-chain`, and
  this is the first such crate in the order.
- Item 6 came from reading, not from a build for a 32-bit target. The fix is cheap enough to land
  without one.
- Item 9's cost figures are this tree's recorded medians on its own runners. Nobody has verified
  the hosted runner's memory against the 5.9 GiB peak. The first `proofs` run in the repository
  will say.
- The review did not run `cargo kani`; the recorded falsification times and the reach record stand
  in for it. The reach record predates `Binding::SyncOnly`.
- No review package exists for this crate, so the baseline above is this review's reading of the
  `Name:` blocks and the roadmap, not an architect's clearance.

## Index row

An outside review of `subtree_scope` before it leaves. Three findings block the move (the
manifest and files, the in-tree vocabulary of its text, and a dependency on `filesystem_contract`
that has nowhere to point yet). Seven land before the first tag, and five wait for later. The seven
cover an `EISDIR` arm left to the caller, a handle-0 sentinel in `unbind`, a 32-bit badge
truncation, eight names, the missing examples, a walk-completeness proof and the passive-voice
ceiling.
