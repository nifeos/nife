# Review package: `intrusive_queue`

*Name: provisional, minted by lane 1097-rulings-and-package. The `intrusive_queue` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, step 10 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` plus this branch leaves it. The outside review is
[milestone 1097 (`intrusive_queue`: what an outside maintainer would fix before its first release)](../../design/roadmap/1097-intrusive-queue-outside-review.md).

## In one paragraph

An intrusive first in, first out queue. The link lives inside the node, so a push and a pop cannot
allocate or fail, and a pop hands back the node itself. A node is minted into an `Unqueued` token
with one `unsafe` call, and a push consumes the token, so a node cannot sit on two queues without
`unsafe`. The caller keeps three rules: a node is on at most one queue, a node outlives its time on
the queue, and the caller serializes access and mints each token from the node's owning pointer. The
crate is `no_std`, has no dependency, and has `unsafe` in the token's mint and the queue's own pointer
walks. Two Kani harnesses prove first in, first out order and that a popped node leaves with a clear
link. `Queue<T>` is `Send` only when `T` is. Nothing in it needs the kernel.

## Identity

| item | value |
|---|---|
| crate | `intrusive_queue`, ratified 2026-10-27 (#2797) |
| version | `0.1.0`; no tag |
| edition | 2024 |
| license | the manifest inherits `license` and `lints` from the nife workspace; the move writes `MIT OR Apache-2.0` and the template's lints |
| `publish` | `false`; the publishing hold governs the first release |
| dependencies | none |
| size | one `src/lib.rs` of 659 lines: the library, seven host tests and two Kani harnesses; two falsification patches |

## Names

| item | status |
|---|---|
| `intrusive_queue`, `Queue` | ratified 2026-10-27 (#2797) |
| `Node` with `next` and `set_next` | ratified 2026-11-19 (UTC), option A |
| `Queue::push_back`, `pop_front`, `is_empty`, `len` | ratified 2026-11-19 (UTC); `VecDeque`'s spellings |
| `Unqueued` with `new`, `as_ptr`, `as_non_null` | ratified 2026-11-19 (UTC); `as_ptr` is `NonNull`'s spelling |

The `Name:` blocks in `src/lib.rs` read ratified; the move strips them. No name carries an alias.

## Who in nife depends on it

- `kernel`: the run queues and migration inboxes (`kernel/src/cpu.rs`, `sched.rs`, `thread.rs`);
  `Thread` implements `Node` and is `Send`, so the new bound holds.
- `crates/inter_process_communication`: the futex and rendezvous queues, and the notification
  queue.
- `components` and the other programs do not name the crate; they link it through the kernel's
  consumers only on the host build.

Files that name the crate and that the move lane reads (from `script/crate-move --dry-run`):
`.github/workflows/kani-reach.yml`, `helpers/kani-peaks.tsv`, `helpers/name_checks.py`,
`helpers/rust_source.py`, `helpers/template_report_baseline.txt`, `helpers/trust_boundary_crates.tsv`,
`packages/kernel.package.toml`, `script/lint`, `script/verify` and the workspace `Cargo.toml`.

What references back into nife from the crate: three notes pointers in the docs, one decision
citation, and the `Name:` blocks (see What the move lane does).

## What was run

| check | result |
|---|---|
| `cargo test -p intrusive_queue` | 7 host tests and 2 doctests pass; 2 compile-fail doctests fail to compile as they should (the double push and the `Rc` node on a thread, `E0277`) |
| `cargo clippy -p intrusive_queue -p inter_process_communication --all-targets -D warnings` | no warning |
| `cargo kani -p intrusive_queue` | 2 of 2 harnesses verified, 0 failures, 27 seconds |
| `script/crate-standalone intrusive_queue` | `build`, `proofs` and both `falsification` rows ok. Eight failures, all the move lane's: manifest inherits `license` and `lints`, no `rust-version`, no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, three notes path lines, one section citation line. The `names` check passes |
| `script/crate-move --dry-run` | stages at template `v8` (a69ab4829b02); 27 scaffold files; builds for the host and three targets once the manifest is its own. Failures: no README, manifest inheritance, the same standalone and citation lines, a 56-line comment block at `src/lib.rs:130` over the 40-line cap, and README prose caps (the scaffold's placeholder) |
| `python3 helpers/name_release.py --draft intrusive_queue` | all 11 names carry a ratified block, none missing |
| `cargo build -p kernel -p components --target` for aarch64, riscv64 and x86_64 | each builds with the bound |
| `cargo test -p inter_process_communication` | passes |

Not run, and so not claimed: Miri (the pinned nightly has no `miri` component installed on this
host, so the crate's tests were not run under it; the template's weekly job runs it), mutation (the
list is 26 mutants; the template gives the first figures after the move), the stranger test (no
README yet), `cargo-semver-checks` (a first release has no baseline), and the template's workflows.

## Public API

| kind | items |
|---|---|
| trait | `unsafe trait Node: Sized` with `next(&self)` and `set_next(&mut self, ..)` |
| token | `Unqueued<T>`: `unsafe const fn new(NonNull<T>)`, `as_non_null`, `as_ptr`; `Debug`, `Eq`, `PartialEq` with `NonNull<T>` |
| queue | `Queue<T: Node>`: `new`, `Default`, `push_back(Unqueued<T>)`, `pop_front() -> Option<Unqueued<T>>`, `is_empty`, `len`; `Send` when `T: Send` |

No nife type appears in a public signature.

## Proofs

Two Kani harnesses, each with a falsification patch under `falsifications/` that `script/crate-standalone`
applied and saw its harness fail: any push and pop interleaving is first in, first out and lossless,
and a popped node leaves with a clear link. The harnesses cover the queue's list logic. They do not
cover the aliasing rules of the caller's contract, which Miri covers (item 5 of the milestone,
Done in #3523).

## What the move lane does

- Write the manifest the template's way: `license`, `rust-version`, `description`, `repository`
  and the `[lints]` block (item 1).
- Write the README, the two license files, `CHANGELOG.md`, `_typos.toml`, `docs/stranger-test.md`
  and `accepted-words`, and run the stranger test (items 2 and 3).
- Remove the notes pointers, the DECISIONS citation and the `Name:` blocks from `src/lib.rs`, and
  split the 56-line comment block at line 130.
- Share one test node (item 13), and decide item 9 (`remove`) before the first release.
- Pin `nifeos/intrusive_queue` by `rev` in `kernel` and `inter_process_communication`, drop the
  workspace member, and update the files listed above in the nife-side pull request.

## Open questions

None. Item 9's surface question is two-way and the maintainer's.

## Recommendation

Ready for review. Decisions 1 to 3 of milestone 1097 are ruled. The go to create `nifeos/intrusive_queue`
(no tag, no publish) is the architect's.
