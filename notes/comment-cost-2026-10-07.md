# What Rust comments cost, 2026-10-07 (provisional name)

Measured 2026-10-07 (UTC), read-only, at base `47a1d85f3`, by lane/comment-cost-measurement. calef
asked for it after #1799 ran the whole suite, CPU matrix included. calef: "We should also consider if the code comments are
just too verbose. We know there is a cost to that verbosity." This note measures; the forks at the
end are calef's. The scripts are in [comment-cost-2026-10-07/](comment-cost-2026-10-07/) and run
from the repository root (`OUT=<dir>` sets where they write).

Result.

- Of the last 300 merged pull requests (#1487 to #1794, 2026-10-03 to 2026-10-07), 115 touched Rust
  and 18 of those touched Rust only in comments. 7 of the 18 were otherwise prose that
  `helpers/prose_only.py` skips, so they ran the full suite for nothing a build reads: 1,638
  runner minutes on the jobs a prose-only change skips, about 230 per pull request.
- 205 of 1,109 Rust file edits (18%) changed comments and nothing else. Most rode along with code.
- The leading cause of a comment-only edit is a fact going stale (24 of 58 pull requests), not a
  moved reference (4). But path citations rot: 10 of the 13 `design/roadmap/proposals/` paths cited
  in Rust comments at the base pointed at nothing; at the re-measure all 9 still cited do, every
  slug since promoted.
- Comments are 54.6% of the bytes in this tree's Rust and 37.3% of its non-blank lines, and the
  kernel runs 0.86 comment lines per code line (re-measured 2026-10-15 UTC; the base's figures are
  the tables below). seL4's C runs 0.15, Redox's kernel 0.09, Linux's `kernel/` 0.33,
  and Rust's `core` 1.22.
- In a uniform sample of 30 comment blocks, 24 keep, 4 cut and 2 move. The blocks that hold the
  bytes are long ones, and a line-weighted sample of 15 found 3 that belong in a note.

## 1. Comment-only Rust changes, and what CI spent on them

Method. For each merged pull request, the diff of its merge commit against the commit's first
parent (what the pull request landed). Every changed `.rs` file was lexed before and after by a
small Rust lexer ([rustlex.py](comment-cost-2026-10-07/rustlex.py): nested block comments, raw
strings, char literals versus lifetimes). A file edit is *comment-only* when the code token stream
is identical once comments are removed. An added or deleted `.rs` file counts as code. The non-Rust
files of each candidate were then run through `prose_only.classify` against the tree at its merge
commit, exactly as CI would.

| | pull requests |
|---|---|
| merged, last 300 | 300 |
| touched Rust | 115 |
| Rust edits all comment-only | 18 (16% of Rust pull requests) |
| ... and every other file prose nothing reads | 7 (#1794, #1771, #1756, #1679, #1539, #1512, #1507) |
| had at least one comment-only Rust file | 58 |
| already prose-only, no Rust (skipped today) | 84 |

The other 11 of the 18 also changed a note some code names (`notes/e1000e.md`, the fatal-risks
README), so they would run everything regardless. #1799, merged 2026-10-07 UTC, is an eighth of the
first kind.

Doctests. None of the 18 changed the code inside a doc comment's fence once the fenced blocks
were extracted and compared ([q1c.py](comment-cost-2026-10-07/q1c.py)). One (#1512) edited a line
inside a fenced diagram in `components/src/line_editor.rs`, a binary crate whose diagram no doctest
runs. A coarser rule, "the edited doc block contains a fence anywhere", would have excluded 4 of the
7, leaving 3.

Runner minutes ([minutes.py](comment-cost-2026-10-07/minutes.py)). For each pull request, every
`ci.yml` and `verify.yml` run on its branch (`pull_request`) and its merge group (`merge_group`).
Job wall time is rounded up to the minute per job. It is split between the jobs a prose-only change
still runs (`draft gate`, `clippy`) and the rest.

| pull request | runs (PR + group) | skippable minutes | of which merge group |
|---|---|---|---|
| #1794 | 6 + 2 | 151 | 75 |
| #1771 | 2 + 2 | 163 | 83 |
| #1756 | 12 + 2 | 327 | 81 |
| #1679 | 4 + 2 | 269 | 128 |
| #1539 | 8 + 2 | 231 | 119 |
| #1512 | 2 + 2 | 231 | 114 |
| #1507 | 6 + 4 | 266 | 130 |
| total | | 1,638 | 730 |

All 18 comment-only pull requests spent 5,918 such minutes, 2,662 of them in the merge queue; the
other 11 needed a full run for their notes.

Caveats. Earlier pushes may have carried code a later push removed, so the pull-request half
overstates; the merge-group half (730) is the firm figure. A merge group that
held another pull request's code would have run everything anyway. The repository is public and
hosted minutes are free, so the cost is pool contention and queue latency, not money. The prose
gate exists because twelve prose pull requests on 2026-10-05 got required jobs cancelled for want
of a runner.

## 2. Why comments change

Each of the 58 pull requests with a comment-only Rust edit was classified by hand from a word-level
diff of the changed comments, by the cause that drove the edit ([q1.py](comment-cost-2026-10-07/q1.py)
writes the diffs).

| cause | pull requests | examples |
|---|---|---|
| a fact went stale: code, a ruling or a measurement moved under the comment | 24 | #1761 (the PCI survey settled which I219), #1739 (argon is a TX1), #1692 (§249 (a running address space stays nameable) changed what is consumed) |
| new content: a `BUGS` entry, a measurement, a finding | 13 | #1687 (eight `BUGS` from the outsider pass), #1794 (GUID measurement) |
| a record's status: name ratified, falsification state | 8 | #1771, #1701, #1730 |
| a rename sweep of an identifier | 6 | #1584 (`recv` to `receive`), #1599 (`OP_` to `OPERATION_`), #1773 |
| a path reference moved | 4 | #1734 and #1527 (proposals promoted), #1543 (a script renamed), #1552 |
| style or spelling | 3 | #1549 (backticks), #1539, #1610 (a counted number) |

So reference churn as a *cause* is small. As a *passenger* it is larger: 38 comment-only file edits
in 10 pull requests replaced a file path. And the paths that remain are rotting where nobody looks:

| reference in a Rust comment | lines citing it | gated? | dead today |
|---|---|---|---|
| `milestone N` | 6,051 | yes, `script/roadmap --check` and `script/citations` | 0 |
| `§N` | 2,834 | yes, `script/decisions --check` and `script/citations` | 0 |
| a source path (`crates/...`, `kernel/...`, `script/...`) | 2,214 | no | about 60 (regex; some are deliberate history) |
| a note or design path (`notes/...md`, `design/...md`) | 1,599 | no | 16 |
| ... of which `design/roadmap/proposals/` | 13 | no | 10 |
| a pull request number | 182 | no | not checked |

The dead source paths are mostly rename sweeps that stopped at code: `crates/slots` (7 sites at
the 2026-10-15 UTC re-measure, now `generational_table`), `crates/regions` (2, now
`memory_regions`), `crates/wake_handshake` (3, now `thread_wake_handshake`). The proposal paths are dead by design, since promotion renames the file.
The chain that produced #1799 is typical: #1507 cited `proposals/the-install-gates-run-nowhere.md`,
and #1527 rewrote it to milestone 712 (the install gates run nowhere) eight hours later; #1756 cited
`proposals/the-package-client-becomes-a-program.md`, and #1799 rewrites it to its new milestone number a day
later. The
gated citation kinds have zero dead references, which is the argument for preferring them.

## 3. How dense, and what a reader pays

Lines are physical lines: a line holding only comment counts as comment, a line with any code
counts as code, blank lines neither. `vendor/` and `patches/` are excluded ([q3.py](comment-cost-2026-10-07/q3.py)).

| area | files | code lines | comment lines | comment per code | comment share of bytes |
|---|---|---|---|---|---|
| `kernel/` | 136 | 39,758 | 36,728 | 0.92 | 65% |
| `components/` | 65 | 15,847 | 12,490 | 0.79 | 63% |
| `crates/` | 233 | 91,929 | 56,615 | 0.62 | 54% |
| other | 123 | 19,653 | 11,457 | 0.58 | 54% |
| `xtask/` | 29 | 13,098 | 6,469 | 0.49 | 50% |
| `system_tests/` | 83 | 20,580 | 9,917 | 0.48 | 49% |
| all | 669 | 200,865 | 133,676 | 0.67 | 57% |

79% of comment lines are doc comments (`///`, `//!`).

Reference points, measured with the same lexer on shallow clones taken 2026-10-07. C is lexed as
if it were Rust, which is close enough for comments, and license headers count as comment.

| tree | comment per code | comment share of bytes |
|---|---|---|
| seL4 `src/` (C) | 0.15 | 18% |
| seL4 `include/` (C) | 0.30 | 30% |
| Redox kernel `src/` | 0.09 | 12% |
| Linux `kernel/` (C) | 0.33 | 30% |
| Rust `library/std/src` | 0.55 | 36% |
| Rust `library/alloc/src` | 0.88 | 45% |
| Rust `library/core/src` | 1.22 | 52% |
| nife `kernel/` | 0.92 | 65% |

The kernel is commented about six times as densely as seL4's C and three times Linux's `kernel/`.
It is in the range of Rust's `alloc` and `core`, whose comments are the published API documentation
for every Rust user, examples included. seL4's thin comments are not thrift: its specification and
proofs live in a separate repository, [l4v](https://github.com/seL4/l4v) ("formal specifications
and proofs for the seL4 microkernel", including a Haskell model "kept in sync with the C code").

The top 20 files by comment lines:

| file | comment | code |
|---|---|---|
| `kernel/src/sched.rs` | 3,981 | 4,073 |
| `crates/system_initializer/src/lib.rs` | 2,959 | 3,169 |
| `crates/filesystem_contract/src/lib.rs` | 2,812 | 2,637 |
| `crates/grant_plan/src/lib.rs` | 2,673 | 4,088 |
| `components/src/swish.rs` | 1,952 | 3,690 |
| `kernel/src/user.rs` | 1,888 | 1,737 |
| `xtask/src/swish_check.rs` | 1,563 | 2,628 |
| `kernel/src/lib.rs` | 1,384 | 1,436 |
| `redoxfs_server/src/lib.rs` | 1,351 | 3,122 |
| `components/src/login.rs` | 1,329 | 886 |
| `system_tests/src/user/tests.rs` | 1,315 | 1,885 |
| `crates/abi/src/lib.rs` | 1,154 | 358 |
| `crates/swish/src/lib.rs` | 1,116 | 2,573 |
| `crates/video_terminal/src/lib.rs` | 996 | 2,054 |
| `kernel/src/arch/x86_64/mmu.rs` | 995 | 1,061 |
| `crates/user_mode_runtime/src/lib.rs` | 953 | 489 |
| `kernel/src/testing.rs` | 938 | 499 |
| `kernel/src/user/fs_service.rs` | 856 | 1,381 |
| `kernel/src/arch/riscv64/mmu.rs` | 849 | 876 |
| `crates/machine_discovery/src/acpi.rs` | 838 | 1,969 |

`components/src/login.rs` opens with a 689-line module doc. 75 files carry more than
100 lines of `//!`, 11,076 lines between them.

Reading cost, at the rough rate of 4 bytes per token (an estimate, not a tokenizer count). At the
base the median Rust file was 14.3 KB, 56% of it comment; the mean 25.9 KB. Re-measured
2026-10-15 (UTC), after the split series: median 9.2 KB, mean 15.0 KB, 1,225 files where the
base had 669. `kernel/src/sched.rs` was 481 KB at the base, about 120,000 tokens; no agent could
read it whole in one call. It has since been split into `kernel/src/sched/`. The tree's Rust
grew from 4.3 to 4.6 million tokens; the comment mass is unchanged, 2.5 million. Comment
lines sit mostly in long blocks: 48% of them
are in blocks over 10 lines, 20% in blocks over 40.

Milestone-title parentheticals are only 0.5% of comment bytes.

## 4. Thirty comment blocks, judged against CLAUDE.md's rule

A block is a run of adjacent comment lines of one kind. 28,536 blocks; median 2 lines, mean 4.8.
Thirty were drawn uniformly with seed 20261007 ([q4.py](comment-cost-2026-10-07/q4.py)). Move means
the block is a finding, history or design essay that a note should hold; keep and cut are
AGENTS.md's own rule.

**Keep 24 (80%), cut 4 (13%), move 2 (7%).**

- Keep: `uefi_loader/src/arch/aarch64/mod.rs:53`, why `ALLOCATION_CEILING` is `0x7fff_ffff` (the
  boot map's 1 GiB block). `components/src/virtio_net_transport.rs:281`, why a bounded wait guards
  the transmit ring. `crates/coremark/src/lib.rs:215`, why no iteration can be hoisted.
- Cut: `crates/serial_console/src/watch.rs:127`, "How far the boot got, and what it announced." on a
  field named `progress`. `crates/inter_process_communication/src/lib.rs:823`, a test doc restating
  `senders_queue_fifo`. `tools/redoxfs_host/tests/recovery.rs:192`, "The comparison." Two of the
  four are on public items, where `missing_docs = "warn"` in the workspace `Cargo.toml` demands a
  line, so they are lint-forced rather than chosen.
- Move: `std_exerciser/src/main.rs:1016`, the history of `remove_dir_all` being `Unsupported` for
  two milestones (a finding). `kernel/src/arch/x86_64/exceptions.rs:472`, a tutorial on x86's two
  kernel entries whose last sentence is the only constraint.

A uniform draw over blocks favors two-line comments, so 15 more were drawn weighted by length,
which is where the bytes are. 12 keep, 3 move, 0 cut. The moves:
`components/src/login.rs:1` (a 689-line design document that already has `notes/login.md` beside
it), `components/src/rmle.rs:1` (the naming search, a record), and `components/src/uptime.rs:1`
("the finding worth stating"). The keeps were long for a reason, such as
`crates/memory_regions/src/table.rs:173`, whose doctests are a gate on a type's traits.

The finding: almost nothing in this tree's comments restates the code. The cost is volume of
legitimate explanation plus history and findings that accumulate in place, which is what "correct
yourself loudly" produces when the correction is written at the site and never retired.

## Caveats, together

- Four days of pull requests; a rename-heavy week would move the causes table.
- Comment-only is decided by a hand-written lexer; all 18 diffs were read to check it.
- Runner minutes overstate on the pull-request side, as above.
- Causes, dead-path triage and keep, cut or move calls are one agent's judgment.
- Tokens are bytes / 4, an estimate.

## Forks for calef

### (a) Should `helpers/prose_only.py` treat comment-only Rust changes as prose?

- Options. (1) No change. (2) A changed `.rs` file counts as prose when its code tokens are
  unchanged and no fenced doc-comment code changed; anything the lexer cannot parse runs everything.
  (3) As (2), but any edit in a doc comment runs everything, plain `//` only.
- What it buys. 7 of 300 pull requests (2.3%), about 230 runner minutes each, 730 in the merge
  queue alone over four days. Option 3 keeps none of it: every one of the 7 edited a doc comment.
- Risks, and the fail-toward-running answer. A doc comment is an attribute, so comment text can
  fail a build three ways: `missing_docs` (a warning, and clippy runs `-D warnings`), rustdoc's
  `broken_intra_doc_links`, and a doctest. The first two already run on a prose-only change, in
  `script/lint` under the `clippy` job, which never skips. The fence rule covers doctests. What
  remains is the gate itself: the lexer is new code deciding what is tested, and a wrong skip is a
  red `main`.
- Prior art. Bazel and Buck key on file content hashes, not token streams, so neither does this
  (from memory); Cargo itself rebuilds on any byte change. This would be unusual.
- **Recommendation: option 2**, with the lexer's self-test in `script/lint` beside the gate's own,
  and the decision printed per file like every other. Reversible: a one-line revert. The saving is
  modest, so if calef prefers not to grow a tokenizer in a gate, option 1 is defensible, and fork (b)
  removes the most predictable class anyway.

### (b) Should comments stop citing mutable locations?

- Options. (1) No rule. (2) Gate the paths: a lint check that every file path in a Rust comment
  resolves, like check 1 does for Markdown links. (3) Prefer stable names: cite `§N`, `milestone N`
  and crate or item names (`generational_table`, rustdoc links), not file paths, and never a
  `proposals/` path; enforce (2) for what remains.
- Measured. The two gated citation kinds have 8,885 citing lines and zero dead. The ungated
  path kinds have 3,813 and 78 regex hits on dead paths, of which roughly a third are fragments or
  deliberate history. Proposal paths are 77% dead, by construction.
- Prior art. Rust API guideline C-LINK ("Prose contains hyperlinks to relevant things") is about
  rustdoc intra-doc links, which `rustdoc` checks at build time; a path in plain text is checked by
  nothing.
- **Recommendation: option 3.** It also makes fork (a) matter less, because a proposal promotion or
  a file rename no longer touches `.rs` at all. A proposal not yet minted has no stable name; cite
  it by its slug in words, or by the milestone once minted. The gate in (2) catches the next
  `crates/slots`. Reversible.

### (c) Should CLAUDE.md's comment-density rule change?

- Options. (1) Keep "far more heavily than production code would be, deliberately". (2) Keep the
  density, but cap a comment block, as §212 (a prose budget) caps a document. A block over a
  threshold (say 40 lines, which holds 20% of comment lines) may not grow, and a new one links to a
  note instead. (3) Adopt a
  production-style rule: comments say what and why, design rationale lives in `notes/`.
- Measured. The sample says the rule is being followed: 80% of blocks keep and few restate. The
  cost is the long tail of essays, findings and corrections that accrete in place, 2.5 million
  tokens of comment in a 4.3 million token tree, and files like `kernel/src/sched.rs` that no agent
  reads whole.
- Prior art. Linux's coding style: "Generally, you want your comments to tell WHAT your code
  does, not HOW", "try to avoid putting comments inside a function body", and "there is also a
  danger of over-commenting". Google's Python guide: "never describe the code. Assume the person
  reading the code knows Python ... better than you do." seL4 keeps its rationale in proofs and a
  separate specification. Rust's `core` is as dense as this kernel, but its comments are published
  documentation.
- **Recommendation: option 2**, because the density measured is not the problem and the long blocks
  are; it reuses §212's ratchet rather than inventing a rule. This one is calef's rule, so the data
  is here and the call is his. If calef wants the rule itself to move, option 3's wording is Linux's,
  quoted above. 2026-10-15 (UTC): `design/roadmap/0929-kernel-comments-in-the-linux-style.md`
  now proposes option 2's shape for `kernel/`.
