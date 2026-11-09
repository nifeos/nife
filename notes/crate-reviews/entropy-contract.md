# Review package: `entropy_contract`

*Name: provisional, minted by lane lane/review-prep-entropy-contract. The `entropy_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together
outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, the first crate of
step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` plus this package's pull request leaves it.

## In one paragraph

The words a client and an entropy service exchange to move random bytes: one request word that
packs an opcode and a byte count, and two reply words, a count of 0 to 8 and the bytes. The crate
also holds the readiness rule a service follows at bring-up: it reports `READY` only once its first
draw holds a nonzero byte. It is `no_std`, with no dependency, no `unsafe` and no system call; the
caller passes the `CALL` as a closure. It has no Kani harness. Two of its items carry ratified
names, and sixteen carry none or a provisional one. Nothing in the crate points back into nife,
but nife's `xtask` copies its `src/lib.rs` into the patched standard library by path, which the
move must change.

## Identity

| item | value |
|---|---|
| crate | `entropy_contract`, ratified ("the service's own word, `entropy`, with the contract suffix") |
| identity values | `INTERFACE_ID` 12, `VERSION` 1, no magic (a message contract, not a page). Both match row 12 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table and its "version 1 when assigned". Both names are provisional, and no ruling has frozen either value |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest (no workspace inheritance) |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished until their own release milestone |
| size | one `src/lib.rs` of 437 lines: about 250 of library and 187 of tests |
| package | `cargo package` packs 8 files, 13.3 KiB compressed |
| tests | 15 unit tests and 4 doctests; one doctest is the README's example |
| mutation | the census row reads 37 caught and 3 missed |

## Who depends on it in nife

Six manifests reach it by path, with `version = "0.1.0"`: `kernel`, `components`,
`crates/user_mode_runtime`, `crates/system_initializer`, `fixtures`, and `system_tests` as a
development dependency. The kernel spawns the entropy service and tests it. `components` holds the
virtio and JH7110 entropy backends, and 15 programs there declare the pair of `INTERFACE_ID` and
`VERSION`. Every edge points from nife to the
crate, which milestone 1017 permits.

One dependent reads the source file, not the crate. `xtask/src/farm/generate.rs` copies
`crates/entropy_contract/src/lib.rs` into the patched standard library as
`sys/pal/nife/entropyproto.rs`, and `xtask/src/farm/stamp.rs` rebuilds the farm when that path
changes. The standard library's `sys/random` calls `req`, `GET`, `delivered`, `take` and
`MAXIMUM_BYTES` through that copy, so every rename below reaches the `std-nife` overlay too.

No reference runs the other way. The manifest has no path or development dependency. No test reads
a file outside the crate, and no `include_str!` reaches past its README. nife's `fuzz/` workspace
has no target for it.

## What was run

This lane ran each check in its own worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone entropy_contract` | two failures. `names`: 16 names not ratified. `proofs`: no Kani harness. The build passes on 1.88 for the host and the three bare-metal targets |
| `script/crate-move --dry-run --into <scratch>/entropy_contract-move entropy_contract` | exit 1 with three findings, all in the template's scaffold (below). Copy, rewrite, build, prose caps, file length, orphan modules, links and falsification records pass. No patch exists to apply |
| `cargo test -p entropy_contract` | 15 unit tests and 4 doctests pass |
| `cargo clippy --all-targets` and `cargo doc`, warnings denied | no warning |
| `script/names entropy_contract` | the crate name ratified |
| `python3 helpers/name_release.py --draft entropy_contract` | 19 names: 3 ratified, 2 provisional, 14 with no block |
| the template's checks at `vendor/crate-workflows` (`--manifest Cargo.toml` over `src/lib.rs`) | `spelled_out`, `unit_symbols`, `question_shape`, `british_spelling`, `register_names` and `service_model`: 75 names judged, 0 refused each. `abbreviations` (report only): 7 components not words, the baseline's 7. `prose_acronyms`: 0 after this branch |

The dry run's three findings are the scaffold's own lines: the pull request template names the
architect, `CONTRIBUTING.md` cites a nife path, and the commit hook, `rustfmt.toml`, `deny.toml`
and `docs/stranger-test.md` mention nife. The runbook's BUGS already holds that template finding.
The dry run also flagged two lines of the crate's own text that named nife; this branch rewords
both.

Not run, and so not claimed: Kani (no harness exists), the stranger test, `cargo-semver-checks` (a
first release has no baseline) and the template's workflows, which run in the repository.

## What this branch changes in the crate

No name, value or signature changes.

- The README's opening and `OPERATION_SHIFT`'s doc no longer name nife.
- `VERSION`'s doc claimed a check at bind, a client asking for the served versions. The service
  answers `GET` alone, and the bind-time check is
  [milestone 1000 (message contracts carry bind-time version checks)](../../design/roadmap/1000-message-contracts-carry-bind-time-version-checks.md),
  not started. The doc now says a program declares the version, and the crate's `BUGS` says that
  no peer checks it.
- The prose spells out PAL and DMA, which the template's acronym gate found unexpanded.
- `fill_with`'s `Name:` block quoted the decider; it now states the ruling and its pull request.

## Public API

| kind | items |
|---|---|
| request | `OPERATION_SHIFT` (56), `GET` (1), `req(operation, n)`, `operation(w0)`, `want(w0)` |
| reply | `MAXIMUM_BYTES` (8), `NO_ENTROPY` (0), `delivered(r0) -> Option<usize>`, `take(n, word, out) -> usize`, `fill_with(out, call) -> Option<()>` |
| readiness | `READY` (ASCII `RNGUP`), `DEAD` (`0xDEAD_0000_0000_0000`), `STEP_NO_FIRST_BYTES` (0x10), `STEP_FIRST_ALL_ZERO` (0x11), `bringup_failure(step)`, `readiness(first)` |
| identity | `VERSION`, `INTERFACE_ID` |

No type of another crate appears in a signature, and the crate defines no type of its own.

What a stranger may find surprising:

- The reply's first word tells a count from a kernel error by size alone: every error is a small
  negative, so `delivered` answers `None` for any word above 8. That rests on the kernel's errors
  staying in -1 to -8, a fact this crate does not own.
- `want` clamps a request for more than 8 bytes to 8 instead of refusing it, and `take` clamps
  again to the slice it fills.
- `fill_with` refuses a short reply, while the standard library's caller accepts partial counts.
  The doc says both.
- `readiness` treats an all-zero first draw as a dead device, wrong with probability 2^-64 or less;
  the README's BUGS says so.

## Names

| name | kind | status | rule check |
|---|---|---|---|
| `entropy_contract` | crate | ratified | passes |
| `MAXIMUM_BYTES` | constant | ratified | passes: a limit that opens `MAXIMUM_` and ends with its unit |
| `fill_with` | function | ratified | passes: std's `_with` for a function that takes a closure |
| `VERSION`, `INTERFACE_ID` | constants | provisional | pass; `block_roster_contract` ratified the same pair, and `id` is an accepted word |
| `OPERATION_SHIFT` | constant | no block | passes; six other crates share the name and value |
| `GET` | constant | no block | passes; the protocol's one operation |
| `NO_ENTROPY`, `READY`, `DEAD` | constants | no block | pass |
| `STEP_NO_FIRST_BYTES`, `STEP_FIRST_ALL_ZERO` | constants | no block | pass |
| `operation`, `readiness` | functions | no block | pass; parameters `w0` and `first` are finding 5 |
| `req` | function | no block | finding 1 |
| `want` | function | no block | finding 2 |
| `bringup_failure` | function | no block | finding 3 |
| `delivered` | function | no block | finding 4 |
| `take` | function | no block | finding 4 |

No name holds an address, a mask or a register offset, so those rules judge nothing here. No item
carries a `doc(alias)`, and no source outside this tree spells any of them, so no alias is owed.

### Proposed names

Each is provisional; this lane renames nothing. Every one is a two-way door while only nife consumes
the crate, and every rename reaches the `std-nife` overlay through the copied file.

| # | now | proposed | why |
|---|---|---|---|
| 1 | `req(operation, n)` | `request(operation, byte_count)` | `req` abbreviates `request` (the abbreviations gate says so). Five contract crates spell their builder `req` (`byte_sink_contract`, `clock_contract`, three in `filesystem_contract`), and `domain_name_resolution_contract` spells it `request`. A ruling here sets the family |
| 2 | `want(w0)` | `requested_bytes(request)` | a verb for a function that reads a quantity; a quantity ends with its unit |
| 3 | `bringup_failure(step)` | `bring_up_failure(step)` | `bringup` is not a word; the tree writes `bring_up` 106 times in Rust and `bringup` 17 |
| 4 | `delivered(r0)`, `take(n, word, out)` | `delivered_bytes(first_reply_word)`, `copy_delivered_bytes(byte_count, reply_word, out)` | `delivered` returns a byte count, so it takes the unit. std's `take` moves a value out and leaves a default (`Option::take`, `mem::take`); this one copies bytes into a slice |
| 5 | parameters `n`, `w0`, `r0`, `word`, `first` | `byte_count`, `request`, `first_reply_word`, `reply_word`, `first_bytes` | the naming rules reach parameters; single letters and register-style abbreviations say nothing |

Private names, at the move or before it: the test locals `err`, `buf`, `w` and `i`, and
`bufferful` in two test names (the abbreviations baseline counts these with `req` and `bringup`).

## What the move still needs

| item | owner |
|---|---|
| Rule the identity values and the names above. | architect |
| Point `xtask/src/farm/generate.rs` and `stamp.rs` at the pinned checkout's `src/lib.rs` (from `cargo metadata`), not `crates/entropy_contract`. Thirteen other crates are copied the same way (`abi`, `counter_frequency_contract` and most contract crates among them), so one change serves them all. | move lane |
| Convert the six path dependencies to `git` and `rev` with `version = "0.1.0"`. | move lane |
| Repoint `helpers/interface_declarations_check.py`, `helpers/interface_stability.py`, `helpers/lint_externalized_crates.py`, `helpers/trust_boundary_crates.tsv`, `packages/contracts.package.toml`, the mutation baseline row and `helpers/template_report_baseline.txt`. | move lane |
| Shipped text that assumes this tree without naming it: "the same shape every driver here uses" and "three programs" (`READY`, `DEAD`), "the three backends" (`readiness`), "the kernel-side tests" (the crate header), and test docs citing `abi::Error`, `NoSuchSlot`, `random16` and a dated bench defect. | move lane |
| `VERSION`'s doc asks for "a dated ledger entry" per break; the history belongs in the crate's changelog. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Open questions

1. Whether the crate needs a Kani harness before it leaves. `crate-standalone` fails `proofs`
   without one, as it does for `counter_frequency_contract`. Two harnesses would cost little:
   `take` never writes past `out` and copies `min(n, 8, out.len())` bytes, and `fill_with` either
   fills `out` exactly or answers `None`. Each needs a falsification patch. The maintainer can
   decide it.
2. Whether `delivered`'s rule, any word above 8 is an error, should name the bound it relies on.
   It holds while the kernel's errors stay in -1 to -8. The maintainer can decide it.

## Recommendation

Not ready for review until the architect rules the identity values and the names. The crate
itself is a leaf and builds alone; the move's real cost is the `xtask` copy, which one change for
all fourteen copied crates pays.
