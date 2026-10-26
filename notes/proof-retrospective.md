# Did the proofs catch the bugs?

Milestone 191. `DECISIONS §14` promises a machine-checked core, `notes/verification.md` explains how
the harnesses work, and until this note nothing in the tree had asked whether they ever found
anything. This is that question, asked against the only evidence that cannot be arranged after the
fact: the project's own written record of real defects.

*Note name provisional, per the naming tenet. An architect names things.* The evidence this page has
no room for is in [`proof-retrospective/`](proof-retrospective/README.md).

## The finding

Amber, and the red half is structural rather than a criticism of any harness.

No Kani harness in this tree has ever caught a defect after the day it was written. Every defect
in the corpus below was found by something else: a flaky suite, a boot on real silicon, a fuzzer, a
mutation sweep, loom, a code read, or a CI lint. Not one red `script/verify` run appears anywhere in
the record.

Two real defects were caught while harnesses were being written, which is the proofs working
exactly as designed and is invisible in any defect list. `dtb::be32`/`be64` had an unchecked `at + 4`
that panicked on a near-`usize::MAX` offset out of a corrupt device tree, on the kernel's boot path,
before there is any way to report a failure. `pci::intx_interrupt_number` underflowed on a pin of 0
and panicked
in debug builds. Both were found by trying to state totality and discovering it was false, and both
were hardened rather than merely proved. That is the survivorship asymmetry milestone 191's block
predicted, and it is the reason the answer is amber rather than red.

The reason for the red half was one sentence, in `script/verify`'s own header: `cargo kani -p
<crate>` never compiles the kernel, the user programs, or xtask. So 64,818 lines of `kernel/src`,
holding the scheduler, IPC, trap frames, timer, arch trees and resource accounting, were out of
reach by construction. That is, without exception, where the expensive defects were. The harnesses
were not weak; they were not pointed at the code that broke. *(Corrected 2026-09-25: false since
that day, when `kernel` became a harness crate; [`kernel-proofs.md`](kernel-proofs.md) has the
reach now.)*

And the corpus is not evenly distributed over kinds of defect. Of the eighteen entries below,
five are concurrency, four are a hardware or firmware contract, three are resource accounting across
a program's control flow, three are a specification that was never written down, and three are
records rather than code. Kani, as this tree uses it, is aimed at none of those five classes. It is
aimed at the sixth, panic-freedom and algebraic correctness over hostile input. In that class the
record shows zero escapes: nothing that a shipped harness states has ever afterwards turned out
to be false in the code it covers.

That is the honest shape of the result. The proofs are a net with a known, documented perimeter, and
every fish caught in this project swam around the outside of it.

## The numbers

Prose in this tree quotes "112+ harnesses" (the roadmap) and "over 100 across more than 20 crates"
(`notes/fuzzing.md`, correctly hedged as a floor). Neither was the tree's number. At base `f1f138a8`
on 2026-08-30 the tree held 145 harnesses, 143 of them under `crates/` and two in `vendor/redoxfs`,
across 25 packages. `script/verify` ran 140 of them. `kernel/src` was 64,818 lines that no harness
then compiled, out of 206,728 lines of Rust. There were 19 `kani::cover!` vacuity guards, in four
crates. [`the-counts.md`](proof-retrospective/the-counts.md) has every figure, the command that
produced it, and the re-derivation.

## The method

Four fixed questions per defect, from milestone 191's block:

1. What was the defect, as a property that was false? Not "the board hung". The invariant.
2. Was that property provable at all? A property about what a U74 does with a store buffer is not
   a property of our source and no prover here will ever see it.
3. Did a harness exist that covered it? A harness that existed, passed, and let the defect through
   is the most interesting outcome available, and it gets its own section.
4. What would it have cost to have one? This is what makes the study a worklist.

Then the reverse pass, which is not optional: which harnesses prove something that could plausibly
have been false, and which prove a tautology of the operators they use. Without that pass a study of
proofs can only confirm.

Every counterfactual below is a judgment, not a measurement.

## The defect corpus

Sources, all in-tree: `notes/exceptions.md`, `notes/arch-audit.md`, `notes/intrusive-queues.md`,
`notes/interleaving.md`, `notes/visionfive2.md`, `notes/instruction-clock.md`, `notes/fs-server.md`,
`notes/fuzzing.md`, `notes/mutation-testing.md`, `notes/load-sensitive-assertions.md`,
`notes/citations.md`, `design/decisions/0076-roadmap-status-versus-tree.md`, and `git log`.

Column 4 is a judgment in every row.

| # | Defect | The false property | Provable? | Harness existed? | Found by |
|---|---|---|---|---|---|
| 1 | aarch64 exception-return race | staged `SPSR_EL1`/`ELR_EL1` survive to the `eret` | no (assembly, hardware semantics) | none possible | a flake, 1 run in 4 |
| 2 | riscv64 `trap_return` sibling | same, for `sepc`/`sstatus` | no | none possible | reading, after #1 |
| 3 | riscv64 `trap_entry` `sscratch` window | no user value is a `&TrapStash` | no | none possible | the arch audit; left documented |
| 4 | PLIC enable-bit lost update | concurrent `enable`/`disable` on one word preserve each other | by loom, not by Kani | none | the arch audit |
| 5 | wake-before-switch-out race | a thread is never queued while a core still executes it | by loom, and now is | Kani harness existed, structurally blind | a flake, 10/10 to 8/10 |
| 6 | `clock_protocol` seqlock missing `fence(Release)` | a reader never sees a torn `(state, offset)` pair | by loom | none | loom, on its first run |
| 7 | timer re-arm drift (milestone 6 (threads)) | the deadline advances one interval per delivered tick | yes, and the proof now exists elsewhere | none | measurement, 100 Hz became ~70 Hz |
| 8 | FS-server stack 528 bytes short | the grant exceeds the deepest handler recursion | no (a property of codegen) | none | a wall-clock ceiling, misread for a day |
| 9 | `nifefs::write_image` accepted a NUL name | every accepted name reads back as itself | yes, cheaply | 2 harnesses, neither states it | the fuzzer, under a minute |
| 10 | `dtb::node_reg` indexed past its cell stack | the walker is total | no (the BMC wall, named in advance) | 4 harnesses, all on the leaves | reading, while writing the fuzz target |
| 11 | `dtb::Region::end` overflowed | `start + size` does not wrap on a hostile memory map | yes | none for `end()` | the fuzzer, ten minutes cold |
| 12 | `gpt::parse` accepted a backup-boundary equality | no accepted table puts a usable block in the backup array | yes | 9 harnesses, none states it | mutation testing |
| 13 | `login::mint()` cspace slot leak | `mint` returns the cspace to its entry occupancy | yes, after an extraction | none | a 9th login answered DENIED |
| 14 | `login::connect()` rendezvous leak | a served channel leaves no global registry slot behind | yes, after an extraction | none | an unrelated later test, "512 live at once" |
| 15 | x86_64 `boot_cpu_id` answered "which core am I" | `boot_cpu_id` is the core that booted, on every arch | no (a cross-arch contract; rung 1 territory) | none | a test failing half the time |
| 16 | x86_64 PVH `pci::place_bars` trusted any nonzero BAR | a BAR is usable only inside the window this kernel mapped | yes, and a proof would have proved the wrong predicate | 5 pci harnesses | attaching a real NVMe device |
| 17 | nine roadmap statuses wrong in both records | the roadmap describes the tree | not a code property | the gate compared two records; the tree is a third | a sweep, after a lane burned its budget |
| 18 | a retracted benchmark quoted as current | an attributed block quote still exists in its source | not a code property | no gate read quote targets | `script/citations`, built for it |

Six rows deserve more than a cell, and [`defect-corpus.md`](proof-retrospective/defect-corpus.md)
gives each its long form. What each one found:

- Row 7, the timer drift, is the one property this study can price exactly. `crates/timetable`'s
  `next_after` already proves the law, so a `rearm` built on it makes the defect unrepresentable.
  The original form (`TVAL` against `CVAL`) sat a register below the arithmetic, out of a proof's
  sight; the second form, which milestone 62 (tests that assert on time) injected, would have gone red.
- Rows 5 and 6 are races, which Kani as this tree uses it cannot see. `notes/verification.md` said
  so before either was found: *"Concurrency is the sharpest edge of this limit: every queue and
  endpoint proof here is single-threaded, and the wake-before-switch-out race
  (notes/intrusive-queues.md) lived precisely in the SMP interleaving those proofs cannot see. Green
  harnesses and a real race coexisted; the flaky test found it."* The seqlock is the corpus's one
  unambiguous green for a formal method, and the method was loom.
- Rows 9, 11 and 12 sat in crates that already had harnesses. `nifefs` proved the reader and not
  the writer, `dtb` proved `be32` and nobody looked at `end()`, and `gpt`'s harness restated
  `parse`'s own inequality. A proof proves what you asserted, not what you meant.
- Rows 13 and 14, `login`'s two leaks, are resource accounting: every path returns the resource to
  its entry occupancy. That is provable once the slot bookkeeping is extracted into a crate, and
  nothing here proves it for any function.
- Row 16 is the defect a proof would have got wrong: a harness on `place_bars` would have encoded
  the believed predicate.
- Rows 17 and 18 are records, and fail the same way one level out: a checker compares two
  artifacts and the truth lives in a third.

## What the proofs did catch, and the reverse pass

[`the-reverse-pass.md`](proof-retrospective/the-reverse-pass.md) holds the other half of the corpus.
Beyond the two catches in `dtb` and `pci`, one proof obligation closed a wrap in
`paging::domain::grant_pages` (totality, not a fix), and in `crates/glob` the prover falsified the
harness's own specification. Nothing a shipped harness states has afterwards turned out to be false,
which is either a strong result or an unfalsifiable one. The reverse pass tells them apart:

- The harnesses that could not have failed are stated over a single operator or expression:
  `capability::subset_is_reflexive` is a tautology of `&` and `!`. The ones that could fail are
  stated over a composition, such as a table, a walk, a round trip or a second implementation.
- The 145 count over-represents distinct properties by about twelve, through per-ISA parity.
- What separates a load-bearing harness is a recorded falsification, and only a minority of the 145
  have one. No gate requires the record and no convention marks it.
- The second separator is non-vacuity. Only four crates carry `kani::cover!`, so for most harnesses
  that assume, nothing in this tree would say so if one were vacuous.

## Three harnesses that nothing ran

Of the five unrun harnesses, `vendor/redoxfs`'s two are unrun on purpose. `crates/jh7110_trng`'s
three had never been added to the hand-kept shard table, the accident milestone 125 (a number in the prose is a claim) found for
`multicast_dns_protocol` on 2026-08-16. Milestone 193 (put `kernel/src` within reach of the prover) ran
them (all pass), added the row and gave `script/lint` a check comparing the two sets, on 2026-08-30.
The crate is `jh7110_entropy` now. [`the-unrun-harnesses.md`](proof-retrospective/the-unrun-harnesses.md)
has the account.

## The worklist

Each item names the defect that motivates it. The first two are the ones this study would spend its
next lane on.

| # | Harness or gate | The defect it answers | Shape | Cost |
|---|---|---|---|---|
| 1 | A gate comparing the harness-crate set against `script/verify`'s shard table | `multicast_dns_protocol` 2026-08-16, `jh7110_trng` today | a check in `script/lint`, plus the missing row once it is known to pass | small, and it is rung two |
| 2 | Extract the timer re-arm law and point both ISAs' `rearm` at it | timer drift, milestone 6: 100 Hz became 70 Hz | Phase-2 extraction; `crates/timetable`'s three harnesses already state the law | small, and the proof is written |
| 3 | `every_accepted_name_reads_back_as_itself` in `nifefs` | `write_image` accepted a NUL name, 2026-08-02 | one harness over the writer, which is the half currently unproved | small |
| 4 | `no_accepted_table_puts_a_usable_block_in_an_entry_array` in `gpt` | `Gpt::parse`'s backup-boundary wrong-accept | state the property absolutely, not relative to `create` | small |
| 5 | `region_end_is_total` in `dtb`, and a sweep for its siblings | `Region::end` overflowed on a hostile memory map | the `be32` hardening applied to every `pub` arithmetic helper on blob-derived fields | small |
| 6 | Lift `login`'s cspace bookkeeping into a crate and prove occupancy is restored | `mint()`'s 8-login ceiling; `connect()`'s rendezvous leak | a real extraction, the largest item here | a lane |
| 7 | A loom crate over the PLIC enable-bit read-modify-write | the lost update the arch audit found and fixed by inspection | the `work_steal_slot` pattern, which is precedented | modest |
| 8 | A convention that marks a harness's falsification record | the reverse pass could not enumerate them | naming or an attribute; **an architect's call**, since it is a convention | a decision, not a lane |
| 9 | `cover!` guards on the assuming harnesses that have none | twenty of twenty-four harness crates carry no vacuity guard | mechanical, one per harness that assumes | modest |

Item 1 is done (milestone 193, above). Item 8 is the only one that is an architect's rather than a
lane's: it is a tree-wide convention, and this note has no authority to mint one.

## EXAMPLES

Re-derive the counts rather than quoting them. Run from the repository root. Harnesses, the way to
count them that is not wrong:

```
$ grep -rn "^\s*#\[kani::proof\]" --include="*.rs" . | grep -v target | wc -l
     145
```

[`the-counts.md`](proof-retrospective/the-counts.md) has the rest: the packages no shard proves (as
captured on 2026-08-30, before the crate rename), the looser pattern that
over-counts, the source-line counts, every harness name and the vacuity guards by crate.

## BUGS

**A retrospective cannot prove a counterfactual, and every judgment here is marked as one.** "A proof
would have caught this" is an argument. The nearest thing to a measurement in the whole note is item 7
of the corpus, where the property is already proved over already-written code in `crates/timetable`.
Even that carries a qualification: the defect's original form was a register choice below the
arithmetic, and a pure-function proof would not have seen it.

**Survivorship runs both ways and the reverse pass only partly fixes it.** Defects the proofs
prevented never entered the record, which is why the two harness-writing catches (`dtb`, `pci`) are
the tip of an unmeasurable quantity. Defects nobody has found yet are not in the corpus either. This
note can say what the proofs did not catch; it cannot say what they did.

**The corpus is what the tree wrote down, and the tree writes down what it noticed.** Eighteen entries
is not eighteen defects; it is eighteen defects somebody found interesting enough to record. The
selection bias runs toward defects that were hard, because a defect fixed in five minutes leaves no
note. That bias runs *against* the proofs' case: easy defects are the ones a harness is most likely to
have caught silently.

**Ten of the eighteen entries are single-sourced.** Where the corpus rests on one note's account of
itself (`notes/visionfive2.md`'s bench stops most of all, whose fourth stop's conviction was overturned
by its own fifth stop the next day), this study inherits that note's confidence and no more.

The reverse pass is a reading, not a measurement. "Could this property plausibly have been false"
has no gate behind it. A proper version is mutation testing pointed at the harnesses, which
`notes/mutation-testing.md` explicitly excludes today (`mod proofs` and `mod verification` are excluded
from the mutation run, correctly, because they are not the test suite's to defend). Mutating the
*subject* code and asking which harness goes red would answer this question mechanically, and nothing
in the tree does it.

This note has no gate and produces no artifact the build checks, which is milestone 191's own
`BUGS` entry and remains true. Its worklist is nine rows of prose, and prose is rung four. The one
thing that stops it going the way milestone 94's inventory went is that item 1 is a gate, item 8 is a
decision for an architect, and the rest name a crate a reader can grep for.

The counts will drift. They were taken on 2026-08-30 at base `f1f138a8`. The EXAMPLES section and
`the-counts.md` exist so the next reader re-derives them instead of quoting this note, which is the
failure `notes/fs-server.md` records about its own injector counts and the roadmap records about
"112+".

## See also

- `notes/verification.md`, which explains the harnesses and already stated most of these limits.
- `notes/interleaving.md` for the class Kani cannot reach and loom can, including the seqlock defect.
- `notes/fuzzing.md`, whose "What fuzzing finds that Kani does not" asks this question of parsers.
- `notes/mutation-testing.md` for the third tool and the gpt defect.
- `notes/arch-audit.md` for the compensating control where no prover reaches at all, and for the two
  defects it cleared and the one it fixed.
- The corpus sources, listed under [the defect corpus](#the-defect-corpus), for the defects themselves.
- `design/decisions/0014-project-direction.md` for the thesis this is measured against, and
  `design/decisions/0076-roadmap-status-versus-tree.md` for the record-level defect with the same shape.
