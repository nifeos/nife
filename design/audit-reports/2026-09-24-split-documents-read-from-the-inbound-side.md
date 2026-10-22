# Documentation sweep, 2026-09-24: split documents, read from the inbound side

*Documentation audit, run by a maintainer lane on 2026-09-24 at base `edfb4cd6b`. The procedure is
[notes/documentation-audit.md](../../notes/documentation-audit.md); the index is
[README.md](README.md).*

## Why this lens

The two earlier documentation sweeps read names and numbers inside documents the worklist ranked
(2026-08-16), then the ABI surface as the prose describes it (2026-08-17). Both read a document
against the tree. Neither read the files that point *into* a document.

That gap mattered this week. On 2026-09-23 and 2026-09-24 eight documents were split into a short
main page plus appendices under §212 (a prose budget): `design/fatal-risks/README.md`, `design/naming.md`,
`notes/benchmarks.md`, `notes/load-sensitive-assertions.md`, `notes/mutation-testing.md`,
`notes/stranger-test.md`, `notes/README.md` and `AGENTS.md`. The six with appendix directories are
cited from 141, 142, 132, 70, 61 and 23 other files. A split moves sections and leaves every inbound
pointer aimed at the main page. §212's own `BUGS` names the risk: a reader now opens several files
instead of one.

`script/audits --worklist` could not have picked this scope. It ranks a document by how much of the
code it cites has moved since the document was last edited. Every split document was edited today,
so none of the six appears in its top 30. The rot here is in the citing files, and the worklist does
not look at them.

## Scope

- Every markdown link with a `#fragment`, checked against the headings of its target.
- Every appendix of the eight split documents, checked for a link from its parent.
- Every appendix and main page, measured against §212's 3,000-word cap.
- Every line outside a split document that names that document together with a section: a quoted
  heading, a backticked `##` heading, a date followed by "section", "entry" or "heading", or a
  noun followed by "section". 98 candidate lines, each read.
- Every line anywhere that names a path and then a quoted phrase: 121 citations, each resolved.
- Every quoted phrase attributed to `AGENTS.md`, checked against it and against `design/tenets/`.

### What came back clean

- All 37 anchored links resolve.
- No appendix of the eight documents is orphaned. Each is linked from its parent.
- Five of the six appendix directories are within the cap. One file was over (finding 4).
- `design/fatal-risks/README.md` "vocabulary section" citations still resolve: the section kept its place
  under a longer heading.
- The notes index condensation broke nothing `script/lint` could see, and nothing this sweep found.

### What was deliberately not examined

- Whether a split dropped a finding. Each split lane checked that itself, line by line. This sweep
  checked reachability, not content.
- The substance of the twelve `design/tenets/` pages against the old `AGENTS.md`.
- Pointers that name a section without any of the shapes above ("see the note on X"). No pattern
  finds those without reading the whole tree.
- A section name wrapped onto a line two lines below the path. The scan read two lines.

## Findings

Counts: fixed 4, minted 1, accepted 3.

### 1. FIXED: 39 citations named a section of a split document and sent the reader to its main page

| split document | sites | where they were |
|---|---|---|
| `notes/benchmarks.md` | 18 | 9 roadmap blocks, `kernel/src/bench.rs`, `xtask/src/bench.rs` (4), `script/fastpath-footprint` (2), §121 (x86 port I/O), §130 (the CMOS RTC) |
| `design/naming.md` | 9 | two crates, a component, a fixture, `script/lint`, `script/names`, 3 roadmap blocks |
| `notes/mutation-testing.md` | 8 | 3 roadmap blocks (6 sites), a `design/fatal-risks/` appendix, §202 (mechanical work goes to a cheaper model) |
| `notes/load-sensitive-assertions.md` | 4 | 2 roadmap blocks (3 sites), `kernel/src/user/cpu_time_tests.rs` |

Each site now names the appendix that holds the section. The dated sections were the most exposed:
every dated section of `notes/benchmarks.md` and `notes/mutation-testing.md` left the main page, and
the 2026-09-19 mutation section now spans two appendices. One commit per split document. The three
sites in `design/decisions/` are a separate commit, because lanes do not normally edit that
directory; the maintainer can drop it and apply the three one-line changes at merge.

### 2. FIXED: two citations described `design/naming.md` in a way that stopped being true

`crates/serial_console/tests/fixtures/README.md` and milestone 297 (soak becomes soak-test) said the
fabricated transcript is what `design/naming.md` "opens its rename section with". After the split
the rename section opens with three links, and the transcript is recorded in
`design/naming/rename-where-names-hide.md`. Both now say so. A stale pointer that describes its
target is worse than one that only names it, because the reader trusts the description.

### 3. FIXED: six code comments quoted four headings their notes do not have

`notes/cfi-unwind.md` has no "trap entries"; the section is "The hard case: a trap is not a call"
(`vectors.s`, `trap.s`). `notes/interrupts.md` split "Testing it with no device on RISC-V" into two
headings (`ns16550.rs`). "the same five blocks every time" was never in `notes/fs-server.md`; it is
in `notes/benchmarks/read-path-block-contract-and-metadata-cache.md` (two sites in
`redoxfs_server`). `AGENTS.md` never contained "other lanes running in parallel"; the claim is its
lane-count section (`kernel/src/arch/x86_64/mod.rs`). Only the first predates today's splits.

### 4. FIXED: one appendix was over the cap

`design/fatal-risks/the-hal-and-the-next-machine.md` measured 3,020 words. Four sentences lost
restatement and it is now 3,000. No claim, date or citation changed.

### 5. MINTED: nothing checks a phrase quoted after a path

Findings 2 and 3 are one class: a path, then a quoted phrase the file no longer contains. Of 121
such citations, 14 failed, 9 of them rot. `script/citations` already checks the two neighboring
shapes, and this one is the shape a split breaks. Proposed as [a quoted phrase after a path must be
in that file](../roadmap/0644-a-quoted-phrase-after-a-path-must-be-in-that-file.md), a ratchet
on added lines, with the measurement.

### 6. ACCEPTED: both marked prose-budget exceptions are past their granted counts

`design/fatal-risks/README.md` was granted 4,235 words on 2026-09-24 and measures 4,440, after a correction
to risk 2 landed the same day. `AGENTS.md`'s marker says 6,279 and the file measures 6,292. Raising
a grant is calef's call and cutting a correction to fit would be worse, so nothing was changed. It
is recorded in milestone 586 (a prose ratchet in lint)'s block, as a design note: the marker's
number should be the baseline the gate holds. It is not added to either file's `BUGS`, because that
would grow a file already past its grant.

### 7. ACCEPTED: eight provenance lines cite "AGENTS.md's naming section"

`kernel/Cargo.toml`, `script/lint` and six others record where a name's provenance was written, and
quote "standard terms a reader already knows from outside". That text moved to `design/naming.md`
under §155 (the naming conventions move out of the constitution). A provenance line records where a
ruling was made at the time. Rewriting it is the sweep that destroyed a refusal once
(`design/naming/rename-what-moves.md`: a quotation never moves).

### 8. ACCEPTED: three quotations record an old wording on purpose

Milestone 188 (the IPC fastpath) quotes the `SEND`, `RECV` claim it corrected. Milestone 234
(the project's own numbers) quotes the stale "40% comments" line it reports. `notes/register-of-measures.md`
quotes a heading as it was before a date was added. Each is about the old text.

## The class converted into a check

Two classes, neither built in this lane:

- Finding 5 is a proposal against `script/citations`, with the false-positive rate measured first.
- Finding 6 is folded into milestone 586, which will build the word-count gate anyway.

The 37 date-or-noun section pointers ("the 2026-09-21 section") cannot be gated without reading
them. They stay with this sweep.

## What the mechanism learned about itself

The worklist ranks documents whose cited code moved. A split is the opposite case: the document
moved and its citers did not. So the next split should come with its own inbound sweep, and the
cheapest place for it is the split lane itself, which already knows every heading it moved.
`notes/documentation-audit.md`'s `BUGS` now says the worklist is blind to a moved document, and how
to sweep the inbound side.
