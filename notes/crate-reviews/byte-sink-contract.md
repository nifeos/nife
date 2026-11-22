# Review package: `byte_sink_contract`

*Name: provisional, minted by lane milestone/1079-byte-sink-contract-review-package. This is the
`byte_sink_contract` package of [crate-reviews.md](../crate-reviews.md). It has its own file because
the packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 5
of [milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` leaves it. The findings and their statuses are in
[milestone 1079 (`byte_sink_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1079-byte-sink-contract-outside-review.md);
this package does not restate them, and checks which have landed.

## In one paragraph

The one framing every "write these bytes there" destination speaks, so a terminal, a file and a
pipe are substitutable in one capability slot. A sink is a `Rendezvous` with `WRITE` only. A writer
`SEND`s up to 16 bytes in three message registers (the opcode, body kind, severity and count in the
first word, the bytes in the other two) and gets no reply. `SEND`'s return carries one fact: whether
the sink is absent (`NoSink`, so write into the void) or destroyed (`Gone`, so the writer must
end). The crate is `no_std`, has no dependency and no `unsafe`, and six Kani harnesses prove the
decoder and the helpers. The test scaffolding is the separate in-tree crate `byte_sink_fixture`, which
is never published, so the public API holds the wire and nothing else.

## Identity

| item | value |
|---|---|
| crate | `byte_sink_contract`, ratified |
| identity values | `INTERFACE_ID` 6 and `VERSION` 1, row 6 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. Frozen as built; ruled 2026-11-21 (UTC). No magic word and no wire version operation; the pair in a program's `INTERFACES` note is the check, made by the spawner. Both constants carry ratified `Name:` blocks |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, none in the manifest (the move writes it) |
| license | inherited from the nife workspace; no license file in the crate (the move writes both) |
| `publish` | `false`; milestone 1017 keeps moved contract crates unpublished until their own release milestone |
| dependencies | none, of any kind |
| size | `src/lib.rs` 639 lines (library and unit tests) and `src/verification.rs` 97 lines |
| files | 9 tracked, six of them falsification patches |
| mutation baseline | not measured by this lane; the census rows of milestone 1079's appendix (79 tested, 73 caught, 4 unviable, 2 missed, 0 timed out) predate the later changes and are not re-counted here |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency with a `version` key | `components`, `crates/primitive_benchmark_contract`, `crates/system_initializer`, `crates/system_log`, `fixtures`, `kernel`; `system_tests` as a development dependency; `fuzz` with a path and no `version` key; the in-tree crate `byte_sink_fixture` |
| source copied into `std` | `cargo xtask std-src` copies `src/lib.rs` into the patched standard library as `sys/pal/nife/sinkproto.rs` (`xtask/src/farm/copied.rs`), so `println!` and every sink share one definition. The farm reads the source from the pinned checkout cargo fetches (`copied_package_directories`) |
| prose and tables | `crates/externalization-list.txt`, `packages/contracts.package.toml`, `helpers/trust_boundary_crates.tsv`, `helpers/interface_stability.py`, the comments in `uefi_loader/Cargo.toml`, `system_tests/Cargo.toml` and `crates/std_runtime_contract/Cargo.toml` |
| users of `byte_sink_fixture` | `fixtures/src/sink_transcript_writer.rs`, `file_sink.rs`, `file_source.rs`, and the kernel-side `system_tests/src/user/sink_tests.rs` and `system_log_tests.rs`; `fixtures` and `system_tests` depend on it in their manifests |

Milestone 1017's table still lists a development dependency on `abi` for this crate. The crate has
no dependency now (P1-4 landed), so the row no longer matches the tree; this package leaves it alone.

## What was run

All commands ran in this lane's worktree, at `main` with this lane's edits to documents only.

| check | result |
|---|---|
| `script/crate-standalone --no-kani byte_sink_contract` | seven failures, all move-time items. They are a manifest that inherits `license`, a manifest that inherits `lints`, no `rust-version`, no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, and one path into the nife tree. That path is a falsification patch path in a comment (`src/verification.rs` line 3). The build for the host and the three bare-metal targets passes. Kani was not run in this lane; the six harnesses and their six falsification patches are in the tree and unchanged in number |
| `cargo test -p byte_sink_contract -p byte_sink_fixture` | 11 unit tests and 3 doctests in the contract crate and 1 unit test in the fixture crate pass |
| `cargo clippy -p byte_sink_contract -p byte_sink_fixture --all-targets -- -D warnings` | no warning |
| `cargo doc -p byte_sink_contract --no-deps` with warnings denied | no warning |
| `script/names byte_sink_contract` and `byte_sink_fixture` | the crate name, `verification`, `VERSION`, `INTERFACE_ID`, `MAXIMUM_INLINE_BYTES`, `first_word` and `Message` ratified; every other item has no block; `byte_sink_fixture` ratified |
| `python3 helpers/lint_externalized_crates.py --count` | one hit for this crate: the test payload `b"nife"` (`src/lib.rs` line 360), which is not a wire literal |
| `script/lint` | see the pull request's checks |

Not run, and so not claimed: the stranger test, since there is no README to follow. Also not run are
the template's naming checks and `abbreviations` report, which need a checkout of
`nifeos/crate-workflows`. The renames of P1-9 apply what that report listed, and the test local `src` remains. Also not run:
`cargo-mutants`, whose census belongs to its own window, and Miri. A first release has no baseline
for `cargo-semver-checks`, and no repository exists for the template's workflows. The lane ran
neither `cargo package`, the QEMU-side `sink_tests` nor a build for a 16-bit target.

## Public API

| kind | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| opcodes | `OPERATION_BYTES` (0) and `OPERATION_END_OF_STREAM` (1), both `u8`; `OPERATION_SHIFT` (56) |
| first-word fields | `SEVERITY_SHIFT` (32), `KIND_SHIFT` (40), `KIND_TEXT` (0), `MAXIMUM_INLINE_BYTES` (16) |
| first-word functions | `first_word(operation: u8, len: u32)`, `operation(word_zero)`, `len(word_zero)`, `end_of_stream()`, `severity(word_zero)`, `with_severity(word_zero, level)`, `kind(word_zero)` |
| codec | `MessageWords { word_zero, word_one, word_two }`, `pack(bytes)` returning `(MessageWords, taken)`, `unpack(words, out)` returning `Message::{Bytes(n), EndOfStream, Malformed}` |
| send outcome | `GONE` (-11), `Sent::{Ok, Gone, NoSink}`, `classify(returned)` |
| `byte_sink_fixture` (a separate in-tree crate, never published) | `TRANSCRIPT`, `SINK_NAME` (`"sinkout"`), `READY`, `DONE`, `BAD`, `code(sent)` |

No nife type appears in a signature, and the fixture crate is not part of this surface.

What a stranger may find surprising:

- A zero-count bytes message is legal and a sink treats it as nothing, as `write(2)` does for a
  zero-byte write (P1-10, done).
- An end of stream is exactly the message `(end_of_stream(), 0, 0)`; `unpack` returns `Malformed` for any
  other, including a first word with any bit set below the opcode (decision C, built).
- `unpack` returns `Malformed` for a bytes message whose body kind is not `KIND_TEXT`, so every decoder refuses a future
  structured body and none prints it as text (P1-6, done).
- `with_severity` and `severity` act on any first word, including an end of stream; no consumer reads severity
  on anything but a bytes message (P2-15, done).

## What landed

The three items the milestone says the clearance waits for, verified by reading the source:

| item | state | evidence |
|---|---|---|
| P1-3, `fixture` out of the published crate | landed ([#3873](https://github.com/nifeos/nife/pull/3873)) | no `fixture` module in `crates/byte_sink_contract/src/lib.rs`; `crates/byte_sink_fixture` holds the six items, has `publish = false`, and depends on the contract crate; the five consumers use it |
| P1-8, end-of-stream words reserved | landed | `unpack` returns `EndOfStream` only for `(end_of_stream(), 0, 0)`, with a doctest and a Kani harness (`an_end_of_stream_decodes_exactly_from_its_one_message`); the crate root's "What it does not carry" and `OPERATION_END_OF_STREAM` say the words are reserved and refused; `rm`'s verdict has its own endpoint (`file_system_contract::rm::VERDICT_SLOT`, slot 2) |
| P1-9, names a later rule reopens | landed | `MAXIMUM_INLINE_BYTES`, `first_word`, `Message` and the `classify` parameter `returned` replace the old names, with no alias |

The other P1 and P2 items:

| item | state |
|---|---|
| P0-1, the crate stands alone | done; the move commit in nifeos/byte_sink_contract carries the README, both licenses, literal `license`, `lints` and `rust-version`, and the scaffold, with no `Name:` block ([#3900](https://github.com/nifeos/nife/pull/3900)) |
| P0-2, `VERSION` text | done |
| P1-4, `abi` dev-dependency | done; the crate has no dependency |
| P1-5, proofs | done, six harnesses each with a patch that fails it |
| P1-6, `kind` refused | done |
| P1-7, British spellings | done |
| P1-10, zero-count rule | done |
| P2-11, narrow `usize` | done; `unpack` compares the count as `u64` |
| P2-12, `first_word` typed parameters | done (ruled 2026-11-21 UTC); `first_word` takes a `u8` opcode and a `u32` length, and the opcodes are `u8` |
| P2-13, a words struct for `pack` | done (ruled 2026-11-21 UTC); `pack` returns `(MessageWords, taken)` and `unpack` takes the struct |
| P2-14, mutant exemptions | waiting on nifeos/byte_sink_contract#1, which lists the four survivors the repository's first mutation run names |
| P2-15, severity on any first word | done |

## Names

The architect ratified every name below on 2026-11-21 (UTC), except `MessageWords`, which waits for the release review.

| item | status | rule or reason |
|---|---|---|
| `MAXIMUM_INLINE_BYTES` | ratified | a limit opens `MAXIMUM_` and ends with its unit |
| `first_word` | ratified | a `SEND` has no request and no reply; the function packs the first of three words |
| `Message` | ratified | spelled out |
| `returned` (parameter of `classify`) | ratified | the naming rules cover parameters; `return` is a keyword |
| `byte_sink_fixture` (crate) | ratified | follows `file_system_fixture` and `socket_fixture`; never published |
| `VERSION`, `INTERFACE_ID` | ratified, values frozen | the identity pair every contract carries |
| `MessageWords` (struct and `new`) | provisional, for release review | names the three words a `SEND` carries; the fields take the names of the parameters they replace |
| `word_zero`, `word_one`, `word_two` | ratified | the three message registers, spelled out; doc comments keep the register names where they help |
| `OPERATION_END_OF_STREAM`, `end_of_stream`, `Message::EndOfStream` | ratified, each with `#[doc(alias = "eof")]` | spelled out; `eof` is the standard library's and C's spelling |
| `pack`, `unpack`, `classify`, `severity`, `with_severity`, `kind`, `len`, `operation` | ratified | `len` and `with` are std spellings; the rest are plain words |
| `OPERATION_BYTES`, `OPERATION_SHIFT`, `KIND_SHIFT`, `KIND_TEXT`, `SEVERITY_SHIFT`, `GONE`, `Sent` and its variants | ratified | plain words; `GONE` is the kernel's `Gone` return restated |

The std copy (`sys/pal/nife/sinkproto.rs`) is copied from `src/lib.rs` by `cargo xtask std-src`.

## The move

The crate lives in nifeos/byte_sink_contract, and nife depends on it by `git` and `rev`
([#3900](https://github.com/nifeos/nife/pull/3900)). The repository's `check` and `proofs` workflows
pass on its main branch.

## Recommendation

The architect cleared the crate on 2026-11-21 (UTC). The lane built the three conditions the milestone sets (P1-3, P1-8 and P1-9). The seven
failures of `script/crate-standalone` are the ones a move writes: the manifest, the README, both
licenses and the path in a comment. The crate has no dependency and no
`unsafe`, its public API holds the wire and nothing else, six proofs each have a falsifying patch,
and the decoder enforces what the documentation says about the wire.

## Decisions for the architect

1. The architect cleared the move on 2026-11-21 (UTC).
2. The architect ratified the Names table on 2026-11-21 (UTC), apart from `MessageWords`.
3. The architect froze the identity pair `INTERFACE_ID` 6 and `VERSION` 1 as built on 2026-11-21 (UTC).
4. The architect ruled P2-12 and P2-13 on 2026-11-21 (UTC), and the lane built both. `MessageWords`
   is the one name left for the release review.

Nothing here asks for a tag, a publication or a repository.
