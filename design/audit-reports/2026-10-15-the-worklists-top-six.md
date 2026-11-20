# Documentation sweep, 2026-10-15: the worklist's top six, read for what a reader would act on

*Documentation audit, run 2026-10-15 (UTC) at base `8fad33799` by the lane of milestone 427 (the
documentation sweep the worklist already ranks). The procedure is
[notes/documentation-audit.md](../../notes/documentation-audit.md); the index is
[README.md](README.md). The file name is provisional, like every audit report's.*

**Kind:** documentation. **Lens:** the staleness worklist's top six, read for the claims a reader
would act on: a path to type, a count to trust, a tense that names the present, and a named lane
that already built the thing a note still argues for. **Findings:** fixed 9, minted 0, accepted 0.

## Why this lens

## Why this lens

Every count trigger fired: milestones built 288 to 417 (fires at 10), components 197 to 222 (fires
at 10), ABI constants 76 to 6 (fires at 1). The calendar had not fired. The worklist's head, at
the base:

```
 moved/cited  commits  last edit   document
   21/27          184  2026-10-06  notes/comment-cost-2026-10-07.md
   16/19           36  2026-10-11  notes/shared-page-audit.md
   15/59           31  2026-10-12  design/roadmap/0902-.../helpers-classes-1-and-2.md
   13/16           34  2026-10-11  notes/memory-ordering.md
   11/13           37  2026-10-07  design/roadmap/0407-...-agree-on.md
   11/11           36  2026-10-11  notes/verification/core-harnesses.md
```

### The two uncountable triggers, answered

Has a subsystem been rewritten inside its existing crate? Yes, and it is the window's whole shape:
the scheduler file, the kernel crate root, the net_stack root and swish's boot check were all split
along their seams, which moves no count. The worklist caught it, which is the answer its design
hopes for: three of the six documents above cite `kernel/src/sched.rs` and were stranded by the
split.

Has a decision superseded a plan a note still prescribes? Not in this scope, but its inverse
turned up twice: a built milestone that a note still argues for as future work (finding 4), and a
proposal's counter-example that moved crates and kept its stale address (finding 7).

## What came back clean

The 0902 helpers appendix's rows, where its files still exist: spot checks came back exact at the
base (`interface_stability.py` 1,047 lines, `packages.py` 749, `cap_abbreviation.py`'s reader
graph), and 64 of the 68 paths it cites resolve today. `notes/memory-ordering.md`'s known
fence-count rot needed nothing new: the previous sweep already minted milestone 622 for it and left
a dated pointer in the note, and the drift since (the gate counts 23 fences, against 19 at that
sweep) is 622's business, not a second finding. That note's `user/src/kbd.rs` transcript is a
captured gate output, kept under the rename rule. The 0407 block's measurement table is a dated
snapshot whose own premise paragraph translates the three crate renames, and it stays as measured.
The comment-cost note's forks (a) and (b) are genuinely undecided: `helpers/prose_only.py` treats a
changed `.rs` file as code exactly as before, and no path gate has landed.

## Findings

### 1. FIXED: the comment-cost note said today what its base held

`notes/comment-cost-2026-10-07.md` measured the tree at base `47a1d85f3` and then asserted the
results in the present tense. Re-measured with the note's own `q3.py`: comment bytes are 54.6% of
Rust, not 57%; the kernel runs 0.86 comment lines per code line, not 0.92; 1,225 Rust files where
the base had 669; median file 9.2 KB against 14.3; the tree 4.6 million tokens against 4.3. The
comment mass itself is unchanged at 2.5 million tokens, which the fork that rests on it (the long
blocks, not the density) survives. `kernel/src/sched.rs`, 481 KB at the base and the note's exhibit
of a file no agent reads whole, has been split into `kernel/src/sched/`; the sentence now says so.

### 2. FIXED: the same note's dead-path census, re-counted

All 9 `design/roadmap/proposals/` paths still cited from Rust comments are dead, every slug behind
them having been promoted since; the note said 10 of 13. The renamed crates it names as still
cited hold 7, 2 and 3 dead sites against its 8, 7 and 3. And #1799, which the note called "still
open", merged on the day the note was written. The direction of every claim survives the
re-measure; the numbers do not, and they are the reader's.

### 3. FIXED: the shared-page audit claimed the present for its own day

Its summary sentence, in bold, said nothing found "is" a live privilege escalation "on `main`
today". It holds its own day now (2026-08-04), with the four security audits since named as the
current word. The same note's premise item said the inbound socket half (`LISTEN`/`ACCEPT`) "is
not on `main`"; it has since landed in `crates/socket_contract`, and a reader auditing sockets
today would have skipped half the contract on this note's word.

### 4. FIXED: a built milestone still argued for as a case

Case A of the shared-page audit asks for a frame per filesystem client channel and names milestone
599 as its lane. 599 built on 2026-09-27: badged endpoints, by calef's ruling, with the witness
proving isolation on all three architectures. The case read as open work; it now records the
build, and what actually remains (the production progenitor's pool, 599's own "What is left").

### 5. FIXED: four of the tooling appendix's rows lost their subjects three days after the snapshot

`design/roadmap/0902`'s helpers appendix was measured 2026-10-12 against `e3cc3f128`. On
2026-10-13 and 2026-10-14 basalt took over the foreign-program builds, and four inventoried files
were deleted: `build-ioping.sh`, `build-ripgrep.sh`, `build-speedtest1.sh` and
`ripgrep-ignore-walk.patch`. A dated note at the top says the rows and the cross-references naming
them describe the base, not the tree a reader stands in.

### 6. FIXED: two atomics-table rows pointed at a file that is now an index

`notes/memory-ordering.md`'s table sent a reader to `kernel/src/sched.rs` for the IRQ routing
table and the work-steal request slot. The scheduler split moved them to
`kernel/src/sched/interrupt_route.rs` and `kernel/src/sched/placement.rs`; `sched.rs` re-exports
and defines nothing.

### 7. FIXED: the 0407 block's counter-example moved, and four citations kept pre-rename spellings

The block's central exhibit, a test that re-declares constants a crate already publishes, moved
with the system tests to `system_tests/src/user/live_swap_tests.rs`. The re-declaration is still
there, so the argument stands; three citations now point where it lives. Four references to
`swap_proto` and `credential_proto` keep the spellings milestone 265 (`_proto` is a truncation,
and it collides with the other word it could be short for) renamed away from, in a live
proposal a reader would act on; they name the current crates.

### 8. FIXED and gated: five of the core-harnesses appendix's nine counts were wrong

`notes/verification/core-harnesses.md` said 8, 2, 8, 6 and 3 harnesses for capability,
memory_regions, paging, inter_process_communication and generational_table; the tree holds 14, 4,
37, 15 and 4. The counts are corrected, and the conversion the procedure asks of every sweep went
in with them: each of the nine per-crate counts now carries a `<!--count:...-->` marker, with one
derivation per crate in `script/lint`'s registry, so the next harness lands with its number or not
at all. The same edit fixed the two pointers the scheduler split invalidated (the IPC functions
are in `kernel/src/sched/rendezvous.rs`; the type is the crate's `Rendezvous`) and the
memory_regions proofs' move to `src/proofs.rs`. The gate caught two mistakes in its own markers
before landing, a marker on the line after its number and a cardinal past twenty, which is the
mechanism working on its author.

### 9. FIXED: the ABI-constants trigger had gone blind to most of the ABI

The drop that fired this sweep's trigger, 76 to 6, is not the surface shrinking. `script/audits`
counted `pub const NAME: u64` in `crates/abi/src/lib.rs` only, and the ABI crate grew per-object
modules (`rendezvous.rs`, `usage.rs`, `rights.rs`, `survey.rs`, and more); the crate holds 95 such
constants today, 6 of them in `lib.rs`. A new system call number landing in its own module would have
fired nothing. The derivation now walks the crate's `src/`, and this audit's baseline row is the
first taken under it, so the next delta mixes scopes by one discontinuity, recorded here rather
than papered over.

## What was deliberately not examined

- The worklist below its top six; 506 documents cite moved code and are the next sweep's scope.
- The ABI's doc comments, the 2026-08-17 lens, and every counted marker and structural link, which
  are the gates' output.
- `design/decisions/`, out of a lane's edits and out of the worklist.
- Whether the 23 harnesses `crates/paging` gained are the right harnesses. That is verification's
  question, and the appendix now points at the parity series rather than adjudicating it.
- The full re-derivation of the comment-cost note's roughly 60 dead source paths. Its argument was
  re-checked on the subset above, which is where the reader-actable citations live.

## What wants a lane of its own

- Milestone 622 (the fence inventory is re-adjudicated) is more owed than when it was minted: the
  gate counts 23 fences against the 13 this note's table adjudicates.
- Milestone 383 (a gate for a backticked in-tree path that does not resolve). The proposal-path
  corpses this sweep re-counted (9 dead sites, every slug promoted) are that gate's exact
  population, and the note that counts them is the worklist's head.
- Milestone 0407 (one definition of the numbers a kernel test and its program agree on) itself.
  Its sweep is mechanical, its crate-name question is priced, and nothing has started it.

## Process notes on the mechanism itself

The worklist ranked six documents and every one of them held a finding, which is not a verdict on
the ranking, only the window's density: 184 commits under the head document. Its recorded blind
spot held once more in the other direction, that a top-ranked document can be true: the shared-page
audit, second on the list for the second sweep running, gave up its findings to reading, not to
its rank. And the trigger that fired this sweep fired on a defect in its own counter, which is a
shape worth remembering: the counts are derivations, and a derivation rots the way prose does.
