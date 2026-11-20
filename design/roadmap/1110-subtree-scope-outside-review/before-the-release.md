# P1: before the first release

An appendix to [the milestone](../1110-subtree-scope-outside-review.md).

4. `landing` answers `EINVAL` where the contract says `EISDIR`, and leaves the arm to the caller. DONE (`a_file_verb_on_a_directory_is_is_a_directory`; `Refusal::IsADirectory` is provisional; the server's special case is gone).
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

5. `unbind` uses handle 0 to mean "nothing to close". DONE (`unbinding_a_badge_bound_to_handle_zero_is_not_unbinding_a_sync_only_one`; `unbind` returns `Option`).
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

6. The badge-to-slot cast truncates on a 32-bit target. DONE (`a_badge_past_a_machine_word_binds_nothing_and_is_revoked`; `index` returns `Option`).
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

7. Names the naming rules refuse, or that read wrong to a stranger. DONE (the spellings below and `walk`'s `requested_rights`, ratified, in the tree and in `redoxfs_server`; `is_name`'s parameter is `component`).
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
   `component`; `NodeKind`; `Binding::Unbound`; `check_landing`; `Bindings::binding`. `walk`'s
   parameter is `requested_rights`, a `u64` of rights bits added to `DESCEND` at every step. One pull request, after the maintainer rules the two-way
   names provisional.
   Door: two-way before the tag; every public one is one-way after it. Effort: S.

8. There is no example anywhere. DONE (doctests on `walk` and `Bindings`; the README example is move work).
   Finding: 0 doctests, no `examples/`, no README. A stranger has the test module's `tree`
   function as the only picture of how to write `lookup`.
   Evidence: `cargo test` output ("Doc-tests subtree_scope: running 0 tests");
   `crates/subtree_scope/src/lib.rs` lines 463 to 472.
   Change: a doctest on `walk` over a four-node toy tree (the test's `tree`, lifted into the doc).
   It shows a success, a `Symlink` refusal and a `Narrowed` refusal. A second doctest on
   `Bindings` shows bind, admit through `ROOT`, unbind, and the `NotYours` after. The README's `## EXAMPLES`
   carries the first.
   Door: two-way. Effort: S.

9. What the walk proofs leave out, and what they cost. MEASURED, NOT ADDED: the completeness harness costs more than a prove shard has room for (see Finding status).
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

10. Passive voice over the template's ceiling. LEFT: move work.
    Finding: `passive_voice.py` at the template's head counts 20 passive constructions in 3,366
    words against a ceiling of 6. The pinned release (v7) does not run it; the first pin bump past
    v7 will make `check` red for this reason alone.
    Evidence: `passive_voice.py` over the staged `src/lib.rs`, five of the lines: 257, 321, 339,
    427, 587 (staged numbering).
    Change: put 14 sentences into the active voice while item 2 rewrites the doc; one pass.
    Door: two-way. Effort: S.
