---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: 151, 242
machine_requirements: none
specific_machine: none
needs_person: yes
---
## Goal

A stranger who finds `byte_sink_contract` on crates.io can write a conforming sink or a conforming writer from the crate's own documentation, with no access to this tree. Every statement the documentation makes about the wire is one the decoder enforces. The repository's gates are green
for the crate's own reasons and red only for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1079-byte-sink-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1079-byte-sink-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a published name, a published wire behavior, a layout, a dependency, a license). Everything else is two-way, and the maintainer decides it.

### P0: blocks the move or any publish

1. The crate does not stand alone, and the documentation that explains it does not travel.
   Finding: there is no README, no `LICENSE-MIT` or `LICENSE-APACHE`, no `rust-version`. The manifest inherits `license` and `lints` from the workspace, and its `publish = false` comment points at `crates/publish-allowlist.txt`, a file that will not exist in the new repository. The
   crate root documentation sends the reader to notes/abi.md, notes/sink-protocol.md ("the module
   note", twice), §16, §242, milestones 613 and 956, `filesystem_contract`, `line_editor::proto`, `filesystem_fixture`, `cargo xtask std-src` and two files under `fixtures/src/`. It also names the architect in four `Name:` blocks. The reasons for the shape (register-only rather than a
   shared page; `SEND` rather than `CALL`; no per-write error; what the protocol deliberately does
   not carry) live only in the note. A stranger holding the published crate cannot learn why the contract has its shape, and the template's `standalone` gate refuses every one of those
   references.
   Evidence: `crates/byte_sink_contract/Cargo.toml` lines 5 to 6, 8 to 13, 16 to 17, 20 to 26. `src/lib.rs` lines 4, 8, 10 to 11, 15 to 16, 22, 26, 32, 35, 58, 61, 66, 153 to 158, 162, 248, 278 to 281, 285, 291, 297. Also the `standalone` findings above, and the two `lint_externalized_crates` hits (`line_editor` at line 16, `b"nife"` at line 360).
   Change: copy the scaffold (README with the template's section order, both license files, `CHANGELOG.md`, `_typos.toml`, `deny.toml`, `.github/`). Write `license`, `rust-version` and the three lints as literals. Delete the `publish` line in the release that publishes, and until then give it a comment a stranger can act on. Strip the `Name:` blocks (ruled for every moved crate). Rewrite the crate root documentation as the specification. It carries the wire table and the two shape decisions with their reasons. It carries the `Gone` versus `NoSink` split with std's mapping (`BrokenPipe` versus `Unsupported`) as the worked example. It also lists what the contract does not carry, in the crate's own words with no path into this tree. Rename the test string at line
   360.
   Door: two-way (the license is already ruled). Effort: M.

2. The `VERSION` documentation specifies a bind-time wire check that does not exist and cannot
   exist in this shape.
   Finding: the comment says a writer `CALL`s `OPERATION_VERSION` and binds only when its own
   version is in the served set. No such opcode exists in this crate or anywhere in the tree. Proposal 1000 (message contracts carry bind-time version checks), not started, reserves opcode 0 for it "where the protocol leaves it free", and this contract gives opcode 0 to `OPERATION_BYTES`. A sink is `SEND`-only by the contract's own first paragraph, so there is
   nothing a writer could `CALL`, and the one receiver that would answer is a pipe's ordinary
   reader. The check that does exist today is static: the program's `INTERFACES` note carries
   (6, 1), and `interfaces_note` says the progenitor compares only the syscall ABI's entry at spawn
   and carries the others. An outside implementer who trusts the comment reserves opcode 0 for a
   version reply and collides with every bytes message.
   Evidence: `src/lib.rs` lines 19 to 27 (the comment) and 46 (opcode 0 taken);
   `crates/interfaces_note/src/lib.rs` lines 4 to 7; `design/roadmap/1000-*.md` line 23.
   Change: the comment states what is true: a program's note declares the pair (`INTERFACE_ID`, `VERSION`). The contract has no wire operation for versions and, being one-way, cannot have one without a reply. A framing change bumps `VERSION`, and a sink that serves two versions does so by accepting both framings, which is what "serves N and N-1" means for a stream. Whether a
   wire version operation is ever added is decision A.
   Door: the comment is two-way; the wire question is one-way, decision A. Effort: S.

### P2: later

The P2 items are in [the appendix](1079-byte-sink-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1079-byte-sink-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. Each gives the background, then the recommendation, then prior art and one
concrete example. The maintainer brings them one at a time.

### A. Does the byte-sink contract ever get a wire version operation?

Background: every message contract is to carry a bind-time version check, and the proposal's sketch
reserves one opcode for a `CALL` that returns the served set. This contract is one-way: a sink is a
`Rendezvous` with `WRITE` only, a writer `SEND`s and gets no reply, and a pipe's reader is any
program that receives. There is no reply path on which a served set could travel, and opcode 0 is
`OPERATION_BYTES`. Today's check is the `INTERFACES` note read at spawn, which carries this pair
without comparing it.

Recommendation: no wire version operation for this contract. The declared pair in the note is the
check, performed by whoever spawns and wires the program, and the `VERSION` documentation says so.
A framing change bumps `VERSION`; a sink that serves two versions accepts both framings. If anyone wants the spawn-time comparison of non-ABI interfaces, it belongs to the spawner, not to this wire.

Prior art: Unix pipes carry no version and negotiate nothing; the shape of the bytes is the
agreement. Fuchsia FIDL versions methods at compile time with `@available` and binds nothing at run
time on a one-way channel (unverified detail). seL4 protocols on a one-way endpoint carry no
version and rely on the capability's provenance.

Example: a sink implementer who followed the current comment would answer opcode 0 with a version
word. Every `println!` from a std program sends opcode 0 with a byte count, and that sink would
treat each line as a version query.

### B. Does `fixture` ship in the published crate?

Background: item 3. The module is six items for four in-tree consumers, documented against nife
file names. A published crate's surface is a one-way door: after the release, removing a module is
a breaking change.

Recommendation: no. Move it to `byte_sink_fixture` (provisional name) in this tree, as
`filesystem_fixture` already is for the filesystem contract. The published crate then holds the
wire and nothing else.

Prior art: `embedded-hal` keeps its test doubles in `embedded-hal-mock`, a separate crate;
`tokio-test` is separate from `tokio`; in this tree `filesystem_fixture` is separate from
`filesystem_contract`. A crate that ships its own fixtures (`proptest`'s `test-strategy` shapes) does
so behind a feature, which keeps the names in the documentation.

Example: on docs.rs, `byte_sink_contract::fixture::SINK_NAME` would read `"sinkout"` with a sentence
about a file created on a test image by a program the reader cannot find.

### C. What are the second and third words of an end-of-stream message?

Background: item 8. The contract defines `w0` for `OPERATION_EOF` and is silent on `w1` and `w2`.
`unpack` discards them. `rm` carries its exit status and a count there, read only by kernel-side
fixture tests; the shell drops them past a `|`.

Recommendation: reserved. A conforming writer sends zero in both and a conforming sink ignores them, which is what every decoder in the tree does. Either `rm` records its use as a private agreement outside this contract, or `rm` moves its verdict to a channel of its own. The stricter reading, `Malformed` on a non-zero word, breaks `rm` today for no reader's benefit. The looser one, a defined
status trailer, puts a supervision fact on a stream, which the `OPERATION_EOF` documentation argues
against in its own words. Reserving them keeps a trailer possible under a later `VERSION`.

Prior art: Unix delivers a producer's exit status through `wait(2)` to its parent, never through the
pipe; `set -o pipefail` reads statuses from the supervisor, not the stream. HTTP chunked transfer
defines an explicit trailer after the last chunk, which is what a defined status word would be, and
it is a defined part of that framing, not an unused field.

Example: `rm -rv logs | wc` today: `rm` sends `eof()` with status and count, the shell's pipeline
reader matches `Msg::Eof` and returns, and `wc` sees an end of stream and no status. The same words
reach the fixture test that spawns `rm` directly. One message, two meanings, decided by who
receives.

### D. The names

Each is a published name at release and so one-way. Each predates the rules this review measures it against. Recommendation first in every row.

| name | recommendation | why |
|---|---|---|
| `INLINE_MAXIMUM` | `INLINE_MAXIMUM_BYTES` | the units rule: a limit opens `MAXIMUM_` and ends with its unit; ratified before the rule, so the release review reopens it |
| `req` | `first_word` | a `SEND` has no request and no reply; the function packs the first of three words, and the decoder's `operation` and `len` read that word; `request` (the report's suggestion) would claim a reply |
| `Msg` | `Message` | spelled out; the abbreviations report expands it; nothing in std spells it `Msg` |
| `ret` (parameter of `classify`) | `returned` | the naming rules cover parameters; `return` is a keyword |
| `w0`, `w1`, `w2` | keep | the ABI's own words for the message registers (notes/abi.md) and the kernel's; a contract that mirrors the ABI keeps its spelling |
| `OPERATION_EOF`, `eof`, `Msg::Eof` | keep, with the doc comment opening "end of stream" | std's spelling (`UnexpectedEof`), and the term every reader searches for; the thing is a stream, which the text says |
| `pack`, `unpack`, `classify`, `severity`, `with_severity`, `kind`, `len`, `operation` | keep | `len` and `with` are std spellings; the rest are plain words |
| `Sent`, `Sent::Ok`, `Sent::Gone`, `Sent::NoSink` | keep | plain words that carry the contract's one distinction |

Prior art for `first_word`: the ABI note names the registers `w0..w4` and describes a syscall by its words. `filesystem_contract` and `line_editor` put their opcodes in the same bits and the tree calls
that "the first word" in prose throughout.

Example for `INLINE_MAXIMUM`: `MAXIMUM_NAME_BYTES` versus `MAXIMUM_NAME_LEN` in `design/naming.md`
is this case; `INLINE_MAXIMUM` is a byte count and its documentation says so in its first words.

## Done when

The checks are in [the appendix](1079-byte-sink-contract-outside-review/done-when.md).

## BUGS

- The Kani figures come from a standalone copy with a literal manifest, not from the crate in this
  tree, whose workspace does not run Kani over this crate.
- Item 11 came from reading, not from a build for a 16-bit target; the fix is cheap enough to land
  without one.
- This review did not check the prior-art figures marked unverified. Confirm or drop them before the README cites any of them.
- This review checked by search only whether `rm`'s verdict words have a reader outside the kernel-side fixture tests. A reader that obtains them another way would change item 8's weight, not its
  finding.
- The review did not run the QEMU-side `sink_tests`; the indifference proof they carry stands on the
  tree's own gates.

## Index row

An outside review of `byte_sink_contract` before it leaves found two findings that block the move. The crate does not stand alone, and its `VERSION` text specifies a wire operation that does not and cannot exist. It found eight to land before the first release. They are the fixture module in the public API, the `abi` dev-dependency, no proofs, a kind byte the decoder ignores, a British spelling, undefined end-of-stream words, names a later rule reopens, and a zero-count contradiction. It found five for later.
Four one-way doors for the architect: no wire version operation, `fixture` out of the published
crate, end-of-stream words reserved, and the renames.
