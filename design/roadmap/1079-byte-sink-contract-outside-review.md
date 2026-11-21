---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: 151, 242
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1079. `byte_sink_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review lane that met `crates/byte_sink_contract` as a stranger would, before
the crate leaves for a repository of its own under milestone 1017 (the contract crates leave in this
order), step 5. It changes nothing; it lists what the move lane and the lanes after it change.

The review package for this crate is `notes/crate-reviews/byte-sink-contract.md`. The baseline this
review takes is what the tree records. That is the crate
name (`_contract` per `design/naming.md`), the ratified `INLINE_MAXIMUM`, and the two opcodes with
`OPERATION_BYTES` at zero (notes/sink-protocol.md). It includes the severity bits that §242 (a
system log) placed in the first word, and the interface id 6 and version 1 of milestone 956 (the
linked interfaces carry checkable versions). It also includes the standing rulings that a moved
crate carries no `Name:` block and mentions neither nife nor a nife program. None of those is reopened here, except where a later
naming rule contradicts a name, and the text says so where it does.

Reuse: the template `nifeos/crate-workflows` and its scaffold and helpers, Kani, cargo-mutants,
`helpers/lint_externalized_crates.py`, and the existing in-tree consumers as the regression suite
(the kernel-side `sink_tests`, `system_log`, the shell's pipeline reader, std's PAL). No new tool.

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

The P0 items are in [the appendix](1079-byte-sink-contract-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1079-byte-sink-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1079-byte-sink-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Each gives the background, then the recommendation, then prior art and one concrete example.
A, B and D are two-way before the first tag, and the maintainer has decided them, provisionally.
C has its ruling. The review clearance that creates the repository is under
[Open decisions](#open-decisions).

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

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may revisit. No such operation exists and nobody has built against one, and a later `VERSION`
can add one. The contract has no wire version operation. The pair (`INTERFACE_ID`, `VERSION`)
declared in the program's `INTERFACES` note is the check, made by the spawner, and the `VERSION`
documentation says so. Item 2 is the work, and the `VERSION` documentation states it.

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

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may revisit. No publish happens while the publishing hold stands, and keeping the fixtures out
is the stricter choice. `fixture` (`src/lib.rs` lines 282 to 309) moves to an in-tree crate,
`byte_sink_fixture` (name provisional), as `filesystem_fixture` is for `filesystem_contract`. Item 3
is the work.

### C. The second and third words of an end-of-stream message are reserved

Ruled 2026-11-14 (UTC), option A: a writer sends zero in both, and a sink refuses an end of stream
with either word non-zero as `Msg::Malformed`. `unpack` decodes `Msg::Eof` only from the exact
message `(eof(), 0, 0)`, so a first word with any bit set below the opcode fails the same way,
which is what its documentation says the low 32 bits are. A Kani harness proves that equivalence
over every three words, with a falsification patch.

`rm` reports its exit status and its count on a verdict endpoint of its own, slot 2
(`filesystem_contract::rm::VERDICT_SLOT`), after its report stream ends. A `SEND` returns only once
a receiver takes its message, so the verdict cannot overtake the text, and a reader drains the report
before receiving it. The spawn service leaves that slot empty, so at the prompt the kernel refuses the
send and `rm` exits; the kernel's granted-directory and granted-set wirings grant it when a test asks.

Prior art: Unix delivers a producer's exit status through `wait(2)` to its parent, never through the
pipe; `set -o pipefail` reads statuses from the supervisor, not the stream. HTTP chunked transfer
defines an explicit trailer after the last chunk; a defined trailer would need a new `VERSION` here.

Example: `rm -rv logs | wc`: `rm` sends its lines and `(eof(), 0, 0)`, `wc` counts them and sees a
well-formed end, and the verdict goes to slot 2, which nothing at the prompt holds. A guest test
that spawns `rm` directly reads the same report and then the verdict.

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

Prior art for `first_word`: the ABI note names the registers `w0..w4` and describes a system call by its words. `filesystem_contract` and `line_editor` put their opcodes in the same bits and the tree calls
that "the first word" in prose throughout.

Example for `INLINE_MAXIMUM`: `MAXIMUM_NAME_BYTES` versus `MAXIMUM_NAME_LEN` in `design/naming.md`
is this case; `INLINE_MAXIMUM` is a byte count and its documentation says so in its first words.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may revisit. The table above holds with one row changed: `INLINE_MAXIMUM` becomes
`MAXIMUM_INLINE_BYTES`, not `INLINE_MAXIMUM_BYTES`, because a limit opens with `MAXIMUM_`, which is
the table's own reason. `req` becomes `first_word`, `Msg` becomes `Message` and `ret` becomes
`returned`. `w0`, `w1`, `w2`, `OPERATION_EOF`, `eof`, `Msg::Eof`, `pack`, `unpack`, `classify`,
`severity`, `with_severity`, `kind`, `len`, `operation`, `Sent` and its variants keep their names.
No old-name alias. Item 9 is the work.

## Done when

The checks are in [the appendix](1079-byte-sink-contract-outside-review/done-when.md).

## Finding status

- P0-1, the crate stands alone: Open. The crate-root documentation is rewritten as the specification
  ([#3565](https://github.com/nifeos/nife/pull/3565)). Left: a README, both licenses, a literal
  license and lints, `rust-version` and a `publish` comment a stranger can act on. That comment
  points at `crates/publish-allowlist.txt`. Also left: the scaffold and the `Name:` strip, and a
  test payload other than `b"nife"` (`src/lib.rs` line 394, not a wire literal).
- P0-2, the `VERSION` text: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P1-3, `fixture` in the public API: Done (maintainer provisional, decision B, 2026-11-14 UTC),
  [#3873](https://github.com/nifeos/nife/pull/3873). The module is the in-tree crate
  `byte_sink_fixture` (name provisional, `publish = false`), and its consumers changed one `use`.
- P1-4, the `abi` dev-dependency: Done, [#3565](https://github.com/nifeos/nife/pull/3565). The pin
  of `GONE` to `abi::Error::Gone` is a `const` assertion in `system_tests/src/user/sink_tests.rs`.
- P1-5, proofs: Done, [#3565](https://github.com/nifeos/nife/pull/3565). Five harnesses, each with a
  falsification patch.
- P1-6, the decoder ignores `kind`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P1-7, British spellings: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P1-8, end-of-stream words reserved: Done. `unpack` refuses either word non-zero and `rm`'s verdict
  has its own endpoint ([#3581](https://github.com/nifeos/nife/pull/3581)); the crate root's "What
  it does not carry" states the rule ([#3873](https://github.com/nifeos/nife/pull/3873)).
- P1-9, names a later rule reopens: Done (maintainer provisional, decision D, 2026-11-14 UTC),
  [#3873](https://github.com/nifeos/nife/pull/3873). `MAXIMUM_INLINE_BYTES`, `first_word`,
  `Message` and `returned` replace the old names, with no alias; the names then go to the crate's
  release name review.
- P1-10, the zero-count contradiction: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- P2-11, `len` on a narrow `usize`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
  `unpack` compares the count as `u64`.
- P2-12, `req` drops high bits: Done (maintainer provisional, two-way door),
  [#3880](https://github.com/nifeos/nife/pull/3880). `first_word` takes typed `operation: u8` and
  `len: u32` parameters, so a caller cannot pass bits the word drops.
- P2-13, `pack` returns a bare four-tuple: Done (maintainer provisional, two-way door),
  [#3880](https://github.com/nifeos/nife/pull/3880). `pack` and `unpack` take and return
  `MessageWords`; the struct's name stays provisional and joins the release name review.
- P2-14, equivalent mutants: Open. Four exemptions with their reasons go into the new repository's
  `.github/mutants-exempts`.
- P2-15, severity bits on any first word: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Decision A, no wire version operation: Done (maintainer provisional, 2026-11-14 UTC), built as
  P0-2, [#3565](https://github.com/nifeos/nife/pull/3565).
- Decision B, `fixture` out of the published crate: Done (maintainer provisional, 2026-11-14 UTC),
  built as P1-3, [#3873](https://github.com/nifeos/nife/pull/3873).
- Decision C, end-of-stream words reserved: Done (ruled 2026-11-14 UTC, option A),
  [#3581](https://github.com/nifeos/nife/pull/3581).
- Decision D, the names: Done (maintainer provisional, 2026-11-14 UTC), built as P1-9,
  [#3873](https://github.com/nifeos/nife/pull/3873).
- The move: Done as a pin. nifeos/byte_sink_contract exists with `check` and `proofs` green, and
  nife depends on it at commit 8a6c4599882e505d219829fd106d9e572d56f6e9 (`version = "0.1.0"`,
  `publish = false`). No tag and no publish; a pre-release tag moves the pin later.
  `MAXIMUM_INLINE_BYTES` replaced the ratified `INLINE_MAXIMUM` and needs re-ratification.

## Open decisions

### Clear the review, which creates nifeos/byte_sink_contract

Background: `byte_sink_contract` is the one framing every "write these bytes there" destination
speaks (a terminal, a file, a pipe), so a writer holds any of them in one capability slot. A crate
that leaves nife gets a review package under `notes/crate-reviews/`, which an architect reads;
clearing it is the go to create the crate's repository and move the crate. The package is
`notes/crate-reviews/byte-sink-contract.md`.

The in-tree work has landed: `fixture` is the crate `byte_sink_fixture` (P1-3), the end-of-stream
words are reserved and the crate root says so (P1-8), and the four renames are applied (P1-9). The
manifest, README, licenses and exemptions (P0-1, P2-14) come with the move itself.

Recommendation: clear it. The names table in the package, including `MAXIMUM_INLINE_BYTES`, which
replaced the ratified `INLINE_MAXIMUM`, goes to the architect's name review with it.

Door: one-way. The template's workflows and nife's pin act on a repository and a moved
crate from the first day; moving back is a second move.

Prior art (recalled, not read): the Rust project moved `libc` out of the compiler's tree into a
repository of its own, and the compiler consumes it back by version. Where nife differs: a review
package checks that the crate stands alone before it leaves, not after. Example:
`jh7110_entropy`, whose review the architect cleared on 2026-11-01 (UTC), lives in nifeos/jh7110_entropy,
and nife depends on it at a pinned revision.

What it blocks: the move, which is milestone 1017's step 5, and so the first tag and the first
publish.

## Follow-on

- **Outstanding.** The Open lines under [Finding status](#finding-status) and the items under
  [Open decisions](#open-decisions).

## BUGS

- The Kani figures come from a standalone copy with a literal manifest, not from the crate in this
  tree, whose workspace does not run Kani over this crate.
- Item 11 came from reading, not from a build for a 16-bit target; the fix is cheap enough to land
  without one.
- This review did not check the prior-art figures marked unverified. Confirm or drop them before the README cites any of them.
- The review did not run the QEMU-side `sink_tests`; the indifference proof they carry stands on the
  tree's own gates.

## Index row

An outside review of `byte_sink_contract` before it leaves found two findings that block the move. The crate does not stand alone, and its `VERSION` text specifies a wire operation that does not and cannot exist. It found eight to land before the first release. They are the fixture module in the public API, the `abi` dev-dependency, no proofs, a kind byte the decoder ignores, a British spelling, undefined end-of-stream words, names a later rule reopens, and a zero-count contradiction. It found five for later.
Four decisions: the architect ruled the end-of-stream words reserved, and the maintainer decided,
provisionally, no wire version operation, `fixture` out of the published crate, and the renames.
The in-tree findings have landed and the review package awaits the clearance.
