---
status: BUILT
raised: 2026-09-01
built: 2026-09-01
---
# 214. A test that prints "skipping" and returns is counted as passed

A test that prints "skipping" and returns counts as passed. Milestone 164 (x86_64 userspace can't
build `aes`) showed it: eleven tests moved from the skip column to the pass column without running
anything.

It was minted with no gate, on the grounds that this is a sweep of test code in this repository
and nothing outside it is involved. That held: no hardware, no external dependency, and the whole
of it was made on patagonia.

## What it needed

`kernel/src/testing.rs` has a `skip!` macro. It stores a reason and returns, and the runner prints
`skipped: <reason>` and counts the test in a separate column, which is exactly right: a suite that
reports "200 passed, 55 skipped" is telling the truth about what it proved.

Sites all over the tree did not use it. They `crate::println!("    (no RedoxFS disk attached;
skipping)")` and `return`, and a test that returns without setting a skip reason is
indistinguishable from one that passed. The line is in the transcript, so a human reading the
scrollback can see it. The counts cannot, and neither can anything that reads the counts.

## The real count, and how it was taken

This block's minting note said 46 sites across 16 files, which was one lane's grep of one
architecture. The merged tree holds more, and the pattern that found them is worth recording
because AGENTS.md already carries a scar about a count taken with a grep that only matched
single-line forms.

Three passes, each over `#[test_case]` function bodies located by brace matching rather than by
line:

1. Print-and-return. Every `println!`/`print!` invocation inside a `#[test_case]` body whose
   arguments contain `skip`, paren-matched so a multi-line invocation counts once: 70 sites in
   11 files, plus 5 more in helper functions those tests call.
2. Silent return. Every bare `return;` inside a `#[test_case]` body, 87 of them, minus the
   ones the first pass already had: 5 genuine early exits with no line at all, which is the
   same defect with the evidence left out.
3. Outside the harness, checked and deliberately left alone: `kernel/src/main.rs`'s boot tour
   (8 sites) and `kernel/src/bench.rs` (13). Neither is a `#[test_case]`; the tour was always
   meant to run on a machine of unknown provisioning, and a benchmark reports its own line.

80 sites, 18 files. The three counts are separate because they are three different judgments.
The second pass is the one the minting note's grep could not have found.

## What each site turned out to be

| | sites | what was done |
|---|---|---|
| Uniform `let ... else { println!("(R; skipping)"); return; }` | 68 | `skip!("R")`, same words |
| An `Option`-returning helper printing the line itself | 4 | print deleted; the 9 `#[test_case]` call sites skip in their own frame |
| A `()`-returning helper doing the same one level deeper | 1 | `fs_service::crash_disk_present` (provisional name) lets its 2 callers ask first |
| Silent early return, no line | 5 | `skip!` with the reason the comment already gave |
| Partial: some of the claim was proved | 3 | `skip!`, with a reason naming which half ran |

Seventy-seven were genuine skips and three were partial. None turned out to be a
legitimate early exit on a satisfied condition. The partials are the judgment this milestone
owed:

- `sink_tests::one_reader_two_sources_and_the_same_answer` runs the pipe arm, asserts it
  against the transcript's own numbers, and then finds no disk for the file arm. It skips, and the
  reason says the pipe arm ran and the file arm did not, because the claim in the test's *name* is
  that two sources agree and one source cannot agree with anything. The pipe arm's assertions
  still execute and would still fail the run.
- `a_client_resolves_a_real_dns_name_when_the_host_resolver_answers` (both ISA twins) skips
  when the resolver does not answer. Its own name is conditioned on an answer, so when none comes
  the claim was never put to the test.

Two reasons were stale rather than absent, and were corrected in passing: `memory.rs`'s
`initrd_is_reserved_if_present` documented itself as passing "trivially" when there is no initrd,
which is precisely the thing being swept. Both timers' `ticks_arrive_at_the_configured_rate`
printed a paragraph beginning `UNMEASURED` and then returned as a pass.

## The numbers, before and after

`script/test`, same tree, same runners.

| | before | after |
|---|---|---|
| aarch64 | 310 passed, 3 skipped | 310 passed, 3 skipped |
| riscv64 | 314 passed, 2 skipped | 314 passed, 2 skipped |
| x86_64 | 212 passed, 44 skipped | 187 passed, 69 skipped |

Twenty-five tests moved out of the pass column on x86_64 and nothing ran that had not run
before, which is the outcome this milestone wanted. A change here that made the numbers look
better would have been the tell that something was wrong. The two legs that did not move are the
control: they attach every fixture, so none of the rewritten branches is taken, and their being
identical is what says the sweep changed reporting rather than behavior.

The 25 are not spread evenly. 24 of them skip with "no RedoxFS disk attached", one reason,
waiting on one thing:
milestone 215 (design/roadmap/0215-x86-64-pci-interrupt-routing.md), on a PCI
function's interrupt reaching nothing on x86_64.
Its block already sized the disk arm at 36 tests; this makes that number visible in the final
line instead of only in a lane's report.

Nothing in the tree tracks a test count as a `<!--count:-->` claim, so nothing had to move. The
counted-claims registry derives Kani harnesses, harness crates, loom harnesses, shell scripts,
system calls, rights bits and security audits, and no test total. Were one ever added, it would want
`<!--count-at-least:-->`. This milestone is the second time in a week the number moved for a
reason nobody was tracking.

## The mechanism, and what it will get wrong

The sweep is a one-time fact. The shape came back once already (milestone 145 shipped `skip!`, and
the tree then wrote 80 sites that ignored it), so it needs something that does not depend on
anyone remembering.

Rung two of AGENTS.md's ladder: a gate that fires without being remembered. It lives in
the harness rather than in `script/lint`. `console::CountedWrites::write_str` already sees every
fragment the kernel prints, so in test builds it tells `testing::note_printed` when one contains
`skip`. `Testable::run` clears that flag *after* printing the test's name, so a test whose own
name carries the word cannot accuse itself, and checks it once the test returns. A test that said
"skip" and left no skip reason panics, naming `testing::skip!` and this file.

It fires on what the machine actually printed rather than on what the source looks like, which is
why it needs no allow-list and cannot be defeated by a format string a grep sees in pieces.
Verified by negative control: reverting one site to `println!` + `return` fails that test with the
message, and restoring it goes green.

**Why not rung one.** Rung one is a harness in which an unreported skip cannot be written down:
milestone 145's first candidate, a `#[test_case]` returning `Result<(), Skipped>` so `?` carries
an absent fixture out of a helper and into the runner. It is the better mechanism on paper and it
would have caught the helper cases structurally rather than by sweep. It is also a return-type
change on every one of the tree's `#[test_case]`s and an `Ok(())` on the end of each, for a defect
this catches at the moment it happens. Recorded as refused with its reason rather than not
considered; if the helper shape recurs, that is the argument for paying for it.

**Why not `script/lint`.** A source-level check would have to know which `println!`s are inside a
`#[test_case]`, which is brace matching in POSIX shell, and it would fire on the boot tour and on
`bench.rs` unless it carried a file allow-list. AGENTS.md is explicit that three checks have been
deleted from that script for exactly the signature of a rule that rejects legitimate work.

**The honest false-positive shape**: a test that prints "skip" for some reason of its own and then
passes. There are none in the tree today, measured on all three architectures rather than
asserted, and a test that genuinely wants the word can assert on the message instead of printing
it. **The honest blind spot is the other direction**. A test that returns early having proved
nothing and printed nothing is invisible to this, which is why pass 2 above went looking for those
by hand and why nothing here claims to have found the last one.

## BUGS

- The check reads a substring, not a structure. `note_printed` matches `skip` anywhere in a
  console fragment, so a test that prints an unrelated sentence containing the word and then
  passes fails the run with a confusing message. That is the deliberate trade: the alternative is
  a source-level rule with an allow-list, and this tree has deleted three of those.
- A skip reason spanning two `write_str` fragments is missed. `println!("... {}", x)` splits
  around its arguments, so a message that only says "skip" across the seam does not set the flag.
  Every site in the tree today puts the word in a literal, and the failure direction is a missed
  catch rather than a false alarm.
- **The block of milestone 164 (x86_64 userspace can't build `aes`) records "211 passed, 44 skipped"
  for x86_64 and is now two counts stale.** It is another milestone's block, so this lane did not
  edit it; the table above is the current reading. The same goes for
  `notes/load-sensitive-assertions/caretaker-teardown-wait.md`'s "300 passed, riscv64 303, x86_64
  189", which was already a dated snapshot of a different tree.

## Follow-on

- **Milestone 215.** The 24 x86_64 tests that now skip with one reason, "no RedoxFS disk attached".
  They are 24 of the 25 that left the pass column, and they wait on a PCI function's interrupt
  reaching nothing on x86_64. This block makes 215's own sizing of the disk arm visible in the
  suite's final line instead of only in a lane's report.
- **Refused.** Rung one, a `#[test_case]` returning `Result<(), Skipped>` so `?` carries an absent
  fixture out of a helper and into the runner. It is the better mechanism on paper and it would have
  caught the helper cases structurally. It is a return-type change on every `#[test_case]` in
  the tree plus an `Ok(())` on the end of each, for a defect the console check catches at the moment
  it happens. If the helper shape recurs, that is the argument for paying for it.
  This refusal is milestone 471 (design/roadmap/0471-a-skip-that-returns.md), which carries it with
  the condition that would change it.
- **Recorded.** `design/roadmap/0214-print-and-return-skips.md` records the blind spot in the other
  direction: a test that returns early having proved nothing and printed nothing is invisible to a
  check that reads what the machine printed. Pass 2 went looking for those by hand and nothing here
  claims to have found the last one.
- **Recorded.** `design/roadmap/0214-print-and-return-skips.md`'s BUGS: the check matches the
  substring `skip` anywhere in a console
  fragment, so a test that prints an unrelated sentence containing the word and then passes fails
  the run with a confusing message. That is the deliberate trade against a source-level rule with an
  allow-list.
- **Recorded.** `design/roadmap/0214-print-and-return-skips.md`'s BUGS: a skip reason split across
  two `write_str` fragments by a
  format argument does not set the flag, so the failure direction is a missed catch rather than a
  false alarm.
- **Recorded.** `design/roadmap/0214-print-and-return-skips.md`'s BUGS: milestone 164's block still
  records "211 passed, 44 skipped"
  for x86_64 and is two counts stale, as is `notes/load-sensitive-assertions.md`'s snapshot. Neither
  was edited here because both belong to other work, and nothing in the tree tracks a test total as
  a counted claim that would have caught the drift.
- **Recorded.** `design/roadmap/0214-print-and-return-skips.md` records
  `fs_service::crash_disk_present` as a **provisional** name.
  Public function names are an architect's under milestone 160 (review the public function names),
  and the block says so where the helper is introduced.

## Index row

A test that printed a skip line and returned counted as passed. `kernel/src/testing.rs` has a
`skip!` macro that sets a reason and gets its own column; **80 sites across 18 files** used the
print-and-return shape instead, 5 of them returning early with no line at all. 77 were genuine skips
and 3 partial (one arm of a two-arm claim proved, which is neither column and says so). x86_64 reads
**187 passed / 69 skipped**, with nothing running that did not run before; aarch64 and riscv64 are
unchanged, which is the control. Kept from recurring at rung two, in the harness rather than in
`script/lint`: the console tells `Testable::run` when a test printed "skip", and a test that said it
and returned without a skip reason fails the run.
