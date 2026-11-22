# Review package: `subtree_scope`

*Name: provisional, minted by lane 1110-renames-package. The `subtree_scope` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect cleared this package for the crate's move to its own repository. The move is step 9 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
where it leaves as the repository `nifeos/subtree_scope`. It describes the crate as
`main` plus this branch leaves it. The outside review behind it is
[milestone 1110 (`subtree_scope`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1110-subtree-scope-outside-review.md).

## In one paragraph

What a subtree grant can reach, decided in one place a file server links and a prover checks. `walk`
resolves a path one name at a time and refuses `.`, `..`, a link, a mount point and any step that
would widen the grant. `admit` decides whether a request may touch a handle, and `Bindings` keeps
which badge may use which root, failing closed for a badge it never bound. The crate is `no_std`,
has no `unsafe`, and has one dependency, `file_system_contract`, for the rights arithmetic. Six Kani
harnesses prove that a step is one name, that a walk asks for every component before the last `/`
in order, and that it never widens, follows a link or crosses a mount. They also prove that a
bound badge reaches only its own root and the handles it minted. Its one consumer is
`redoxfs_server`. The crate refers to nife by milestone number in its documentation; the move
lane rewrites that text (P0-2 of the milestone).

## Identity

| item | value |
|---|---|
| crate | `subtree_scope`, provisional (the `Name:` block of the crate documentation) |
| identity values | none: the crate has no wire page and no magic; its `Refusal::errno` numbers are the POSIX values the file contract states |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; the manifest sets none, the move lane writes 1.88 (P0-1) |
| license | `license.workspace = true` today; the move lane inlines `MIT OR Apache-2.0` and copies both texts (P0-1) |
| `publish` | `false`; milestone 1017 keeps moved crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| dependencies | `file_system_contract` only |
| size | one `src/lib.rs` of about 860 lines: the library, 12 host tests, 2 doctests and 6 Kani harnesses |

## Who in nife depends on it

One crate by a path dependency with `version = "0.1.0"`: `redoxfs_server`, whose `server.rs`,
`memo.rs`, `names.rs`, `lib.rs`, `dispatch.rs`, its binary, its directory-capability tests and the
fuzz target `fuzz/fuzz_targets/redoxfs_server_session.rs` call `walk`, `admit`, `check_landing` and
`Bindings`.

Beyond that, the tooling and the lists:

- `packages/filesystem.package.toml` and `packages/redoxfs.package.toml` name it, and
  `crates/externalization-list.txt` lists it for step 9.
- `.cargo/mutants-baseline.txt` holds the census row (caught 57, missed 1, timeout 0, unviable 4).
  `helpers/template_report_baseline.txt` holds two rows (`abbreviations` 13, `prose_acronyms` 2).
  `helpers/kani-peaks.tsv` holds its peak, and `notes/project-metrics/falsification-times.tsv` its
  five falsification times.
- `script/verify` carries its per-crate cost row, which balances the Kani shards.
- Documents cite it: `notes/walk-cost.md` and milestones 633, 634, 726 and 1110 by number.
- `crates/capability_manifest_note` and `crates/file_system_contract` mention the crate in text only.
- The fuzz workspace's falsification patch
  `fuzz/falsifications/redoxfs_server_session.unknown_badge_fails_open.patch` edits the crate's
  `Bindings::binding`.

What references back into nife from the crate: its documentation cites milestone numbers, a ruling
date and the program `fs_subtree_caretaker`, and its doc comment points at `notes/walk-cost.md`.
Its manifest reads `license.workspace` and `lints.workspace`. It has no path dependency into nife
beyond `file_system_contract`, no `include_str!` and no test that reads a nife file.

## What was run

All on the branch's tip, on a loaded shared host.

| check | result |
|---|---|
| `cargo test -p subtree_scope` | 12 host tests and 2 doctests pass |
| `cargo clippy -p subtree_scope --all-targets -- -D warnings`, `cargo doc` with warnings denied | no warning |
| the crate and `file_system_contract` copied outside the tree with the lints and license inlined, `cargo +1.88.0 test` | 12 tests and 2 doctests pass on the declared minimum version |
| Miri over the host tests | passes |
| `cargo kani`, the five harnesses | verify; the peak stays near the recorded 5.9 GiB |
| `script/falsifications --check` | every patch of the crate applies |
| `script/falsifications --sweep subtree_scope` | 5 swept, 0 survivors: every harness fails under its patch (the slowest replay, `a_step_is_one_name`, took 833 s on a loaded host) |
| `cargo test` in `redoxfs_server` | 95 tests pass with the renames |
| `script/crate-standalone subtree_scope` | 15 failures, every one move work: the crate name provisional and 21 names not ratified, the `file_system_contract` path dependency, the inherited `license` and `lints`, no `rust-version`, no README or license files, and the citations of milestones, the architect's name, a nife path and sections in the crate documentation. The build and proofs steps are skipped because the copy does not build without the dependency |
| `script/crate-move --dry-run subtree_scope` | stages at template `v8`; the five falsification patches apply with no offset and no fuzz; passive voice, history phrases, spelled-out words, unit symbols and file length pass. 8 findings for the move lane: no README, the build (the inherited `license`), the README's prose caps, and the citations in the crate documentation, all P0-1 and P0-2 |
| `script/lint` | all 149 sections pass |

The measurement behind P1-9 is below under Proofs.

Not run, and so not claimed: the stranger test, and the template's workflows, which run in the
repository. No mutation census ran on the branch: the baseline row stands from 2026-10-19, and the
renames change no logic. Nor did `cargo-semver-checks`, because a first release has no baseline, or
`cargo package`, because the manifest has no `repository` key until the move.

## Public API

| kind | items |
|---|---|
| refusal | `Refusal { Malformed, NotFound, NotADirectory, Narrowed, Symlink, MountCrossing, IsADirectory, NotYours, Refused }`, `Refusal::errno(self) -> i32` |
| a name | `is_name(component: &[u8]) -> bool`, `check_path(path) -> Result<(), Refusal>` |
| a walk | `walk(start, rights, path, wanted, lookup) -> Result<(node, rights, last), E>` for any `E: From<Refusal>` |
| the node a step reaches | `NodeKind { Directory, File, Symlink, MountPoint }`, `check_landing(kind, want_directory) -> Result<(), Refusal>` |
| a request | `Handle(pub u64)`, `admit(...) -> Result<_, Refusal>` |
| the badge table | `Bindings<const B: usize>`, `new`, `binding(badge) -> Binding`, `bind(caller, badge, root)`, `bind_sync_only(caller, badge)`, `unbind(caller, badge) -> Result<Option<u64>, Refusal>` |
| what a badge holds | `Binding { Unbound, Bound { .. }, SyncOnly, Revoked }` |
| proof bounds | `PROOF_PATH_BYTES` (5), `PROOF_BADGES` (3), `PROOF_OPERATIONS` (4) |

No nife type, capability or handoff appears in a public signature; `walk` takes the caller's
`lookup` as a closure and the caller's node type as a generic.

What a stranger may find surprising:

- A badge is both a window and a grant (§230 (badged endpoint capabilities)), so `Revoked` is a fact about a badge and not about
  a handle. Badge 0 is the unbadged value and cannot be bound.
- `Binding::SyncOnly` holds a badge that may only flush, with no directory to hand back, so
  `unbind` answers `None` for it. `Some(0)` means a badge bound to the root handle, which is a
  directory to close.
- An unknown badge fails closed (milestone 726 (an unknown badge fails closed in subtree_scope)): `binding` answers `Revoked` for one past the table
  and for one that does not fit a machine word.
- `walk` asks `lookup` for every component but the last and returns the last unresolved, so the
  verb decides what to do with it. A file verb on a directory lands on `IsADirectory`.

## Proofs

Six Kani harnesses, each with a falsification patch under
`crates/subtree_scope/falsifications/`:
`a_step_is_one_name`, `a_walk_never_widens_follows_or_crosses`,
`a_walk_asks_for_every_component_in_order`, `a_badge_with_no_window_is_never_open`,
`a_badge_once_bound_is_never_open_again` and `a_bound_badge_reaches_only_its_own`. The
falsification times on file are 212.2 s and 110.9 s for the first two, none yet for the third, and
under 8 s for the rest.
The proof is bounded: a path of `PROOF_PATH_BYTES` bytes at unwind 7, three badges and four
operations. The crate documentation says that no loop behaves differently past the bounds is an
argument, not a proof.

### What the walk proofs leave out (P1-9)

Neither walk harness states that `walk` asks `lookup` for every component before the last slash, in
order; a `walk` that skipped a component passes both, and the unit tests catch it. The milestone
asked for one harness, `a_walk_asks_for_every_component_in_order`, whose `lookup` records the steps
and compares them with the components of the path.

Three shapes were measured. The third is in the tree.

| version | what it did | result |
|---|---|---|
| 1 | copied each component's bytes into the record | killed after 18.8 minutes of wall time and 17.8 minutes of CPU at about 5.2 GiB, with no verdict |
| 2 | recorded each step's offset and length against the path's own start, with the same bounds | verified in 11 minutes 10 seconds of wall time (546 s user, 112 s system) at a peak of 4.6 GiB |
| 3 | kept one running offset: each step begins where the last ended and ends at a `/`, and a walk that succeeds has reached the last `/` | verified in 7 minutes 42 seconds of wall time (360 s user, 97 s system) at a peak of 3.0 GiB, the solver itself 10 s |

The third needs no record of the steps. Each step must begin at the offset where the one before it
ended, one byte past a `/`, and must end at a `/`; after a successful walk the offset stands one
byte past the path's last `/`. With `a_step_is_one_name`, which holds every step to a name, the
steps are the components before the last `/`, in order. Its falsification makes `walk` skip the
first component. The rights are the widest and every node a directory, so no refusal cuts the
walk short; the other walk harness covers those.

The host was loaded for all three, so the wall figures run high; symbolic execution, not the
solver, takes most of the time. The first CI `prove` run of the crate's row gives the figure the
verify table carries.

Open: whether `walk` should answer `NotFound` and `Narrowed` where the harnesses do not state it.
The README the move lane writes says the unit tests cover those two refusals and the proofs do not.

## Names

The architect ratified the names below on 2026-11-20 (UTC). The `Name:` block of each public item
says so. No alias carries an old spelling.

| item | kind | spelling and status |
|---|---|---|
| `subtree_scope` | crate | provisional since 2026-09-27; its `Name:` block and `script/names` say so |
| `PROOF_PATH_BYTES` | constant | ratified 2026-11-20 |
| `PROOF_BADGES`, `PROOF_OPERATIONS` | constants | stand: counts of things; no block, in the baseline |
| `walk`'s `directories` | binding | ratified 2026-11-20 |
| `check_landing`'s `want_directory` | parameter | ratified 2026-11-20 |
| `is_name`'s `component` | parameter | ratified 2026-11-20 |
| `NodeKind` | enum | ratified 2026-11-20 |
| `Binding::Unbound` | variant | ratified 2026-11-20 |
| `check_landing` | function | ratified 2026-11-20 |
| `Bindings::binding` | method | ratified 2026-11-20 |
| `Refusal::IsADirectory` | variant | ratified 2026-11-20 |
| `Binding::SyncOnly`, `Bindings::bind_sync_only` | variant, method | ratified 2026-10-06 |
| `walk`'s `requested_rights` | parameter | ratified 2026-11-20; a `u64` of rights bits added to `DESCEND` at every step |
| the other items | | `Refusal` and its other variants, `is_name`, `check_path`, `walk`, `Handle`, `admit`, `Bindings`, `new`, `bind`, `unbind`, `Binding`, `Bound`, `Revoked` pass the rules as written; 13 have no `Name:` block and sit in `design/module-block-baseline.txt` |

## License

`MIT OR Apache-2.0`, written in the manifest by the move lane. The crate adapts no source. The
walk's refusals follow `openat2` with `RESOLVE_BENEATH` in behavior (see
[the milestone's prior art](../../design/roadmap/1110-subtree-scope-outside-review/prior-art.md)),
and it copies no Linux or `cap-std` text.

## What still blocks the move

| item | owner |
|---|---|
| P0-3: replace the `file_system_contract` path dependency with `git` and `rev` and `version = "0.1.0"`; it waits on `file_system_contract`'s repository at step 2 of milestone 1017. `deny.toml` already admits git sources on the organization. | move lane, after step 2 |
| P0-1: the README (`## EXAMPLES`, `## What is proven`, `## BUGS`, `## Minimum Rust version`, `## License`), both license files, the inlined `license`, `rust-version = "1.88"` and lints, and a `publish` comment a stranger can act on | move lane |
| P0-2: rewrite the crate documentation in the crate's own words (no milestone, no dated ruling, no `fs_subtree_caretaker`), with the hard cases of `notes/walk-cost.md` | move lane |
| P1-10: put the passive constructions the template counts (26 in 4,081 words, against 8) into the active voice, with P0-2 | move lane |
| P2-15: the README's section on where the crate is stricter than `openat2` and `cap-std` | move lane |
| `redoxfs_server`'s manifest switches from the path to the repository's `git` and `rev`, and `packages/filesystem.package.toml`, `packages/redoxfs.package.toml`, `crates/externalization-list.txt`, `.cargo/mutants-baseline.txt` and `helpers/template_report_baseline.txt` follow | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository | move lane |

## Open questions

1. Clearing the move: answered. The architect cleared this package on 2026-11-20 (UTC) and approved
   the repository `nifeos/subtree_scope` with `publish = false`, no tag and no publish. The move
   waits on `file_system_contract`'s repository for the `git` pin of P0-3.
2. Whether to spend a prove shard's room on the completeness harness of P1-9: answered. The
   running-offset shape costs less than the measured record, and it is in the tree.

## Recommendation

Cleared to move (2026-11-20 UTC, no tag, no publish).
