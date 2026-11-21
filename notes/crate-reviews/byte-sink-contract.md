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
first word, the bytes in the other two) and gets no reply; the one fact `SEND`'s return carries is
whether the sink is absent (`NoSink`, write into the void) or destroyed (`Gone`, the writer must
end). The crate is `no_std`, has no dependency and no `unsafe`, and six Kani harnesses prove the
decoder and the helpers. One module, `fixture`, is this tree's test scaffolding and still ships in
the public API.

## Identity

| item | value |
|---|---|
| crate | `byte_sink_contract`, ratified |
| identity values | `INTERFACE_ID` 6 and `VERSION` 1, row 6 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word and no wire version operation (the maintainer's provisional ruling on decision A); the pair in a program's `INTERFACES` note is the check, made by the spawner. Both constants still carry provisional `Name:` blocks; see Names |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, none in the manifest (the move writes it) |
| license | inherited from the nife workspace; no license file in the crate (the move writes both) |
| `publish` | `false`; milestone 1017 keeps moved contract crates unpublished until their own release milestone |
| dependencies | none, of any kind |
| size | `src/lib.rs` 548 lines (library, unit tests and `fixture`) and `src/verification.rs` 84 lines |
| files | 9 tracked, six of them falsification patches |
| mutation baseline | not measured by this lane; the census rows of milestone 1079's appendix (79 tested, 73 caught, 4 unviable, 2 missed, 0 timed out) predate the later changes and are not re-counted here |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency with a `version` key | `components`, `crates/primitive_benchmark_contract`, `crates/system_initializer`, `crates/system_log`, `fixtures`, `kernel`; `system_tests` as a development dependency; `fuzz` with a path and no `version` key |
| source copied into `std` | `cargo xtask std-src` copies `src/lib.rs` into the patched standard library as `sys/pal/nife/sinkproto.rs` (`xtask/src/farm/copied.rs`), so `println!` and every sink share one definition. The move must keep that source reachable (see What still blocks the move) |
| prose and tables | `crates/externalization-list.txt`, `packages/contracts.package.toml`, `helpers/trust_boundary_crates.tsv`, `helpers/interface_stability.py`, the comments in `uefi_loader/Cargo.toml`, `system_tests/Cargo.toml` and `crates/std_runtime_contract/Cargo.toml` |
| users of `fixture` | `fixtures/src/sink_transcript_writer.rs`, `file_sink.rs`, `file_source.rs`, and the kernel-side `system_tests/src/user/sink_tests.rs` |

Milestone 1017's table still lists a development dependency on `abi` for this crate. The crate has
no dependency now (P1-4 landed), so the row no longer matches the tree; this package leaves it alone.

## What was run

All commands ran in this lane's worktree, at `main` with this lane's one added file.

| check | result |
|---|---|
| `script/crate-standalone byte_sink_contract` | eight failures, all expected before a move: `names` (27 names not ratified, the first `VERSION`), manifest inherits `license`, manifest inherits `lints`, no `rust-version`, no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, and one path into the nife tree (`src/verification.rs` line 3, a falsification patch path in a comment). Build for the host and the three bare-metal targets passes; Kani: 6 of 6 harnesses verify; all six falsification patches turn their harness red |
| `cargo test -p byte_sink_contract` | 12 unit tests and 3 doctests pass |
| `cargo clippy -p byte_sink_contract --all-targets -- -D warnings` | no warning |
| `cargo doc -p byte_sink_contract --no-deps` with warnings denied | no warning |
| the template's naming checks (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`, `question_shape`, run from a checkout of `nifeos/crate-workflows` over `src/lib.rs` and `src/verification.rs`) | 122 names judged, 0 refused by each |
| the template's `abbreviations` (report only) | 8 components are not words: `eof` (in `OPERATION_EOF`, `eof`, `Eof` and a test name), `req`, `msg` (in `Msg`), `ret`, and `src` (a local in a test). The unit rule is not mechanical: nothing refuses `INLINE_MAXIMUM`, which lacks its unit |
| `script/names byte_sink_contract` | the crate name and `verification` ratified; `INLINE_MAXIMUM` ratified (before the units rule); `VERSION` and `INTERFACE_ID` provisional; every other item has no block |
| `python3 helpers/lint_externalized_crates.py --count` | one hit for this crate: the test payload `b"nife"` (`src/lib.rs` line 394), which is not a wire literal |
| `script/crate-review-packet byte_sink_contract` | facts above; no foreign type in any public signature, no `unsafe` site |
| `script/lint` | see the pull request's checks |

Not run, and so not claimed: the stranger test, since there is no README to follow. Also not run:
`cargo-mutants`, whose census belongs to its own window, and Miri. A first release has no baseline
for `cargo-semver-checks`, and no repository exists for the template's workflows. The lane ran
neither `cargo package`, the QEMU-side `sink_tests` nor a build for a 16-bit target.

## Public API

| kind | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| opcodes | `OPERATION_BYTES` (0), `OPERATION_EOF` (1), `OPERATION_SHIFT` (56) |
| first-word fields | `SEVERITY_SHIFT` (32), `KIND_SHIFT` (40), `KIND_TEXT` (0), `INLINE_MAXIMUM` (16) |
| first-word functions | `req(operation, len)`, `operation(w0)`, `len(w0)`, `eof()`, `severity(w0)`, `with_severity(w0, level)`, `kind(w0)` |
| codec | `pack(bytes)` returning `(w0, w1, w2, taken)`, `unpack(w0, w1, w2, out)` returning `Msg::{Bytes(n), Eof, Malformed}` |
| send outcome | `GONE` (-11), `Sent::{Ok, Gone, NoSink}`, `classify(ret)` |
| `fixture` (item 3) | `TRANSCRIPT`, `SINK_NAME` (`"sinkout"`), `READY`, `DONE`, `BAD`, `code(sent)` |

No nife type appears in a signature. What a stranger may find surprising:

- A zero-count bytes message is legal and a sink treats it as nothing, as `write(2)` does for a
  zero-byte write (P1-10, done).
- An end of stream is exactly the message `(eof(), 0, 0)`; `unpack` returns `Malformed` for any
  other, including a first word with any bit set below the opcode (decision C, built).
- `unpack` returns `Malformed` for a bytes message whose body kind is not `KIND_TEXT`, so every decoder refuses a future
  structured body and none prints it as text (P1-6, done).
- `with_severity` and `severity` act on any first word, including an end of stream; no consumer reads severity
  on anything but a bytes message (P2-15, done).

## What landed, and what did not

Verified on `main` by reading the source, not by trusting the roadmap block. The three items the
milestone says the clearance waits for:

| item | on `main` | evidence |
|---|---|---|
| P1-3, `fixture` out of the published crate | **not landed** | `pub mod fixture` is still in `crates/byte_sink_contract/src/lib.rs` (line 325), exporting six items documented against nife files; no `byte_sink_fixture` crate exists under `crates/` |
| P1-8, end-of-stream words reserved | **mostly landed, one sentence left** | `unpack` returns `Eof` only for `(eof(), 0, 0)`, with a doctest and a Kani harness (`an_end_of_stream_decodes_exactly_from_its_one_message`); `rm`'s verdict has its own endpoint (`filesystem_contract::rm::VERDICT_SLOT`, slot 2). The crate root's "What it does not carry" (line 34) still says a sink ignores the two words, which contradicts the decoder |
| P1-9, names a later rule reopens | **not landed** | `INLINE_MAXIMUM`, `req`, `Msg` and `ret` keep their old names; `MAXIMUM_INLINE_BYTES`, `first_word`, `Message` and `returned` do not exist |

The other P1 and P2 items, by the same reading:

| item | state |
|---|---|
| P0-1, the crate stands alone | open; the move carries it (README, both licenses, literal `license`, `lints` and `rust-version`, the scaffold, the `Name:` strip, the `b"nife"` payload) |
| P0-2, `VERSION` text | done |
| P1-4, `abi` dev-dependency | done; the crate has no dependency |
| P1-5, proofs | done, six harnesses each with a patch that fails it |
| P1-6, `kind` refused | done |
| P1-7, British spellings | done; `british_spelling` refuses none |
| P1-10, zero-count rule | done |
| P2-11, narrow `usize` | done; `unpack` compares the count as `u64` |
| P2-12, `req` typed parameters | open; `req` still takes `u64` for both |
| P2-13, a words struct for `pack` | open; `pack` still returns a bare four-tuple |
| P2-14, mutant exemptions | open; they go into the new repository's `.github/mutants-exempts` |
| P2-15, severity on any first word | done |

## Names

The architect ratified the crate name, and `verification` is a standing ratified module name.
Everything else below is the maintainer's provisional ruling on decision D (2026-11-14 UTC, a
two-way door) or has no review yet. This lane ratifies nothing; the architect ratifies, changes or
refuses each row. Nothing below is built: milestone 1079's P1-9 is the work.

| item now | proposed | status | rule or reason |
|---|---|---|---|
| `INLINE_MAXIMUM` | `MAXIMUM_INLINE_BYTES` | ratified before the units rule; the proposal reopens it | a limit opens `MAXIMUM_` and ends with its unit |
| `req` | `first_word` | proposed | a `SEND` has no request and no reply; the function packs the first of three words |
| `Msg` | `Message` | proposed | spelled out |
| `ret` (parameter of `classify`) | `returned` | proposed | the naming rules cover parameters; `return` is a keyword |
| `VERSION`, `INTERFACE_ID` | keep | provisional `Name:` blocks; the architect ratified these two names for `block_roster_contract` | the identity pair every contract carries |
| `w0`, `w1`, `w2` | keep | proposed | the ABI's own words for the message registers |
| `OPERATION_EOF`, `eof`, `Msg::Eof` | keep, with "end of stream" opening the documentation | proposed | std's spelling (`UnexpectedEof`) and the term every reader searches for; the abbreviations report lists `eof` |
| `pack`, `unpack`, `classify`, `severity`, `with_severity`, `kind`, `len`, `operation` | keep | proposed | `len` and `with` are std spellings; the rest are plain words |
| `OPERATION_BYTES`, `OPERATION_SHIFT`, `KIND_SHIFT`, `KIND_TEXT`, `SEVERITY_SHIFT`, `GONE`, `eof`, `Sent` and its variants | keep | no block, passes the checks | plain words; `GONE` is the kernel's `Gone` return restated |
| `fixture` (module) | the crate `byte_sink_fixture` | provisional (decision B) | moves out as `filesystem_fixture` did for the filesystem contract |

The old names take no alias (the maintainer's ruling with decision D). The renames touch the std
copy, so the lane that applies them regenerates it.

## What still blocks the move

| item | owner |
|---|---|
| P1-3: move `fixture` to `byte_sink_fixture` and change its four consumers' `use`. | a lane, then this package updates |
| P1-9: apply the four renames across every caller and regenerate the std copy. | a lane, then this package updates |
| P1-8: correct the crate root's sentence that a sink ignores the end-of-stream words. | a lane |
| P0-1: README in the template's section order, both licenses, literal `license`, `lints` and `rust-version`, the scaffold, the `Name:` strip, the `b"nife"` test payload, the path in `verification.rs` line 3. | move lane |
| The `std` farm must read the pinned checkout's source; until then the move breaks the platform layer's build. | maintainer |
| Convert the seven path dependencies (and `fuzz`'s) to `git` and `rev` with `version = "0.1.0"`, and read every file `script/crate-move --dry-run` lists. | move lane |
| The four mutant exemptions (P2-14). | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Recommendation

Not ready to clear. Two of the three conditions the milestone sets (P1-3 and P1-9) have not landed
and the third (P1-8) has one sentence left that contradicts the decoder. Clearing now would make the
architect's name review and the repository's first commit act on a crate whose public API still
carries test scaffolding and four names the maintainer has ruled to change. The crate is otherwise
in good order: no dependency, no `unsafe`, six proofs each with a falsifying patch, the naming checks
pass, and the decoder enforces what the documentation says about the wire.

The package is complete as a reading aid. Once the three items land, the lane that lands the last
one updates the What was run table and this recommendation, and the architect reads it then.

## Open questions

- Should the architect ratify the two-way provisional rulings (decisions A, B and D) now, so the
  lane that applies them builds against ratified names, or after the build? Ratifying first avoids
  a second rename.
- P2-12 (typed `req` parameters) and P2-13 (a words struct) change the public signatures before the
  release. Nobody has built or ruled either. Both are two-way until the first release and one-way after it.

## Decisions for the architect

1. Clear the move. Recommendation: not yet; wait for P1-3, P1-8 and P1-9 to land.
2. Ratify the names table above when the build lands, or now. Provisional names stay provisional
   until the architect rules.
3. Ratify the identity pair `INTERFACE_ID` 6 and `VERSION` 1 as frozen, as for the other contract
   crates.
4. Decide P2-12 and P2-13 before the first release, or leave them for the maintainer's provisional
   ruling.

Nothing here asks for a tag, a publication or a repository.
