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
draw holds a nonzero byte. It is `no_std`, with no dependency and no system call, and
`#![forbid(unsafe_code)]` holds it free of `unsafe`; the caller passes the `CALL` as a closure.
Four Kani harnesses prove the request word, the copy out of a reply, the fill loop and the
readiness rule, each with a falsification patch. Every name follows
[design/naming.md](../../design/naming.md), and the architect ratified every name. Nothing in the crate points back into nife, but nife's
`xtask` copies its `src/lib.rs` into the patched standard library by path, which the move must
change.

## Identity

| item | value |
|---|---|
| crate | `entropy_contract`, ratified ("the service's own word, `entropy`, with the contract suffix") |
| identity values | `INTERFACE_ID` 12, `VERSION` 1, no magic (a message contract, not a page). Both match row 12 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table and its "version 1 when assigned". The two names are ratified for every contract crate (#3059). Frozen 2026-11-11 (UTC) on #3480 |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest (no workspace inheritance) |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished until their own release milestone |
| size | one `src/lib.rs` of 593 lines: about 293 of library, 192 of tests and 108 of Kani harnesses |
| package | `cargo package` packs 12 files, 15.6 KiB compressed, the four patches included |
| tests | 15 unit tests and 4 doctests; one doctest is the README's example |

It qualifies to leave under the ruling of 2026-11-04 (UTC) that a crate gets its own repository
only if it is publishable on its merits or more than one repository consumes it. Every program
that draws random bytes links it, and so does the standard library's overlay and the kernel, so
each package repository and the kernel's will consume it.

## Who depends on it in nife

Six manifests reach it by path, with `version = "0.1.0"`: `kernel`, `components`,
`crates/user_mode_runtime`, `crates/system_initializer`, `fixtures`, and `system_tests` as a
development dependency. The kernel spawns the entropy service and tests it. `components` holds the
virtio and JH7110 entropy backends, and 15 programs there declare the pair of `INTERFACE_ID` and
`VERSION`. Every edge points from nife to the crate, which milestone 1017 permits.

One dependent reads the source file, not the crate. `xtask/src/farm/generate.rs` copies
`crates/entropy_contract/src/lib.rs` into the patched standard library as
`sys/pal/nife/entropyproto.rs`, and `xtask/src/farm/stamp.rs` rebuilds the farm when that path
changes. The standard library's `sys/random` calls `request`, `GET`, `delivered_bytes`,
`copy_delivered_bytes` and `MAXIMUM_BYTES` through that copy. The copy drops the crate-level
attributes and everything from the test module on, so the harnesses never reach it.

No reference runs the other way. The manifest has no path or development dependency. No test reads
a file outside the crate, and no `include_str!` reaches past its README. nife's `fuzz/` workspace
has no target for it.

## What was run

This lane ran each check in its own worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone entropy_contract` | clean: names, the build on 1.88 for the host and the three bare-metal targets, Kani proves 4 of 4, and each patch turns its harness red |
| `cargo kani` (Kani 0.68.0) | 4 of 4 harnesses verify, in about three minutes |
| `script/falsifications --sweep entropy_contract` | 4 swept, 0 survivors, 0 stale or broken |
| `cargo test -p entropy_contract` | 15 unit tests and 4 doctests pass |
| `cargo clippy --all-targets` and `cargo doc`, warnings denied | no warning |
| `python3 helpers/name_release.py --draft entropy_contract` | 19 names, every one ratified |
| `helpers/lint_externalized_crates.py` | 0 hits; this branch removes the crate's waiver |
| `script/lint`, `script/test` | in the pull request's checks |

Not run, and so not claimed: the stranger test, mutation, `cargo-semver-checks` (a first release
has no baseline) and the template's workflows, which run in the repository.

## Public API

| kind | items |
|---|---|
| request | `OPERATION_SHIFT` (56), `GET` (1), `request(operation, byte_count)`, `operation(request_word)`, `requested_bytes(request_word)` |
| reply | `MAXIMUM_BYTES` (8), `NO_ENTROPY` (0), `delivered_bytes(first_reply_word) -> Option<usize>`, `copy_delivered_bytes(byte_count, reply_word, out) -> usize`, `fill_with(out, call) -> Option<()>` |
| readiness | `READY` (ASCII `RNGUP`), `BRING_UP_FAILED` (`0xDEAD_0000_0000_0000`), `STEP_NO_FIRST_BYTES` (0x10), `STEP_FIRST_ALL_ZERO` (0x11), `bring_up_failure(step)`, `readiness(first_bytes)` |
| identity | `VERSION`, `INTERFACE_ID` |

No type of another crate appears in a signature, and the crate defines no type of its own.

What a stranger may find surprising:

- The reply's first word tells a count from a failure by size alone: `delivered_bytes` answers
  `None` for any word above 8. The doc states that as the crate's rule and the kernel's side as the
  caller's condition: a kernel whose `CALL` failures are small negative numbers meets it.
- `requested_bytes` clamps a request for more than 8 bytes to 8 instead of refusing it, and
  `copy_delivered_bytes` clamps again to the slice it fills.
- `fill_with` refuses a short reply, while the standard library's caller accepts partial counts.
  The doc says both.
- `readiness` treats an all-zero first draw as a dead device, wrong with probability 2^-64 or less;
  the README's BUGS says so.

## Proofs and falsification patches

Four harnesses, each with one patch under `falsifications/` that turns it red.

| harness | what it proves | the patch's defect |
|---|---|---|
| `a_request_word_carries_its_opcode_and_clamped_count` | for any opcode of one byte and any count, `operation` reads the opcode back and `requested_bytes` reads the count's low byte, clamped to 8 | the byte mask forgotten |
| `copying_writes_exactly_the_delivered_bytes` | for any count, word and slice of up to 12 bytes, the copy returns the least of the count, 8 and the slice length, writes that many low bytes little-endian, and leaves every other byte alone | the word read big-endian |
| `a_fill_succeeds_exactly_when_every_reply_is_full` | for any slice of up to 17 bytes and any replies, each request asks for the remainder clamped to 8, the fill answers `Some` exactly when every reply was full, and then holds the bytes in order after one request per eight bytes | a short reply accepted |
| `readiness_is_reported_exactly_when_a_first_byte_is_nonzero` | for any draw of up to four bytes, `READY` exactly when one is nonzero, else the step that says empty or all zero | only the first byte checked |

Not proved: a slice longer than 17 bytes for the fill, and a draw longer than four bytes for
readiness. The host tests cover the 8-, 32- and 256-byte draws the backends make.

## Names

Every public name meets the rules of [design/naming.md](../../design/naming.md) in force today: std
spellings where std has the operation, units spelled out, no `doc(alias)` for an old spelling of
this tree. No name holds an address, a mask or a register offset, and no source outside this tree
spells any of them, so the crate owes no alias.

| name | kind | status |
|---|---|---|
| `entropy_contract` | crate | ratified |
| `MAXIMUM_BYTES` | constant | ratified: a limit that opens `MAXIMUM_` and ends with its unit |
| `fill_with` | function | ratified: std's `_with` for a function that takes a closure |
| `VERSION`, `INTERFACE_ID` | constants | ratified for every contract crate (#3059) |
| `OPERATION_SHIFT`, `GET`, `NO_ENTROPY`, `READY`, `STEP_NO_FIRST_BYTES`, `STEP_FIRST_ALL_ZERO` | constants | ratified as written (#3480) |
| `BRING_UP_FAILED` | constant | ratified (#3480); `DEAD` refused |
| `operation`, `readiness` | functions | ratified, parameters renamed (below) |
| `request`, `requested_bytes`, `bring_up_failure`, `delivered_bytes`, `copy_delivered_bytes` | functions | renamed (below), ratified |

The renames this branch makes, ratified 2026-11-11 (UTC) on #3480.
They are two-way doors: the crate is unpublished, only this tree links it, and reversing one costs a
pull request. No old spelling keeps a `doc(alias)`, by the rule for a crate before its first
release. The release review rechecks every name before any publication.

| was | is | why |
|---|---|---|
| `req(operation, n)` | `request(operation, byte_count)` | `req` abbreviates `request`. Six other crates spell a builder `req` (`byte_sink_contract`, `clock_contract`, `credential_contract`, `filesystem_contract`, `graphics_contract`, `line_editor`) and `domain_name_resolution_contract` spells it `request`; each of the six renames at its own review |
| `want(w0)` | `requested_bytes(request_word)` | a quantity takes its unit, and a verb does not name a reading |
| `bringup_failure(step)` | `bring_up_failure(step)` | `bringup` is not a word; the tree writes `bring_up` 106 times in Rust |
| `delivered(r0)` | `delivered_bytes(first_reply_word)` | it returns a byte count, so it takes the unit |
| `take(n, word, out)` | `copy_delivered_bytes(byte_count, reply_word, out)` | std's `take` moves a value out and leaves a default (`Option::take`, `mem::take`); this copies bytes into a slice |
| parameters `w0`, `first` | `request_word`, `first_bytes` | the rules reach parameters; register-style abbreviations say nothing |

The test locals `err`, `buf`, `w`, `i` and `n` and the two test names that said `bufferful` follow.
The callers in `components`, the kernel, `system_initializer`, `fixtures`, `system_tests` and the
`std-nife` overlay move in the same commit.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.

## What still blocks the move

The crate itself does not. What blocks the move is in nife, and in the decisions below.

| item | owner |
|---|---|
| Point `xtask/src/farm/generate.rs` and `stamp.rs` at the pinned checkout's `src/lib.rs` (from `cargo metadata`), not `crates/entropy_contract`. Thirteen other crates are copied the same way (`abi`, `counter_frequency_contract` and most contract crates among them), so one change serves them all. | move lane |
| Convert the six path dependencies to `git` and `rev` with `version = "0.1.0"`. | move lane |
| Repoint `helpers/interface_declarations_check.py`, `helpers/interface_stability.py`, `helpers/trust_boundary_crates.tsv`, `packages/contracts.package.toml`, the `script/verify` row, the mutation baseline row and `helpers/template_report_baseline.txt`. | move lane |
| The manifest's `publish = false` comment names this tree's allowlist path; the reason in the new repository is the publishing hold. | move lane |
| Run the stranger test and the template's workflows in the new repository, with the patches rewritten to its paths. | move lane |

## Decided by the maintainer, provisional

Each is a two-way door, decided 2026-11-11 (UTC). Reversing any costs one pull request.

1. The crate carries Kani harnesses before it leaves. Four, each falsified, since
   `script/crate-standalone` fails its proofs check without one.
2. `delivered_bytes` states its bound as its own rule (no reply carries a count above 8) and the
   kernel's error range as the caller's condition, not as a fact the crate owns.
3. The renames proposed above, which the architect has since ratified.
4. `#![forbid(unsafe_code)]`, so the README's "no `unsafe`" holds in the compiler.
5. The shipped text describes no consumer: the crate header, `READY`, `BRING_UP_FAILED`, `readiness`,
   `VERSION` and the test docs no longer speak of "the kernel-side tests", "three programs", "the
   three backends", `abi::Error`, `NoSuchSlot`, `random16` or a dated bench defect.

## Recommendation

Cleared to move. The architect froze the identity, ratified every name and cleared the move.
The crate is a leaf, builds alone and proves; the move's real cost is the `xtask` copy, which one change for all fourteen copied crates pays.

## Decisions for the architect

Ruled 2026-11-11 (UTC) on #3480: the architect froze the identity at `INTERFACE_ID` 12, `VERSION` 1
and no magic, and ratified the names, with `DEAD` renamed `BRING_UP_FAILED`.

The architect cleared the move to `nifeos/entropy_contract` the same day, with no tag and no
publication. The clearance is the go for creating the repository.
