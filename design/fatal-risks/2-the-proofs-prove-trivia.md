---
experiment_status: RUN
experiment_run: 2026-08-30
---
# 2. The proofs prove trivia, and the real bugs live where Kani cannot reach

*Risk 2 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there.*

The claim: the verification half of DECISIONS §14 is real but narrow, and narrow in the direction
that does not matter.

**The experiment:** milestone 191 (did the proofs catch the bugs?), against this project's own defect
history, plus a reverse pass asking which harnesses prove a property that could plausibly be false.

Re-read 2026-10-03. AMBER (calef, 2026-10-03, #1286). The red half is that no standing proof has caught a
regression: every defect a proof caught was caught while its harness was being written (rule 1's
survivorship asymmetry). The second reason is reach. Eight harnesses prove kernel
code on all three architectures; none passes `asm!`, fixed-address MMIO or an `arch/` subtree its
host skips. Files with `asm!` hold 15,966 of `kernel/src`'s 86,528 lines, about 18%
([`notes/kernel-proofs.md`](../../notes/kernel-proofs.md)). Reworded 2026-09-25 on the architect's
ruling; it said "the red half is structural", naming a crate boundary milestone 193 (put
`kernel/src` within reach of the prover) removed on 2026-08-30
([`notes/proof-retrospective.md`](../../notes/proof-retrospective.md); PR #589). Dated 2026-10-03
(§216, from #1286): the riscv64 kernel harnesses are checked against the host's machine model
(`arm64`/`macos` on patagonia; `arm64`/`linux` in CI, observed 2026-10-03 by run 37108539047). All
seven gave identical verdicts and SAT counts under both models, so this is a gap waiting to bite,
not a hole. The fix and its gate are milestone 635
(riscv64 proofs check against the riscv64 model); see [its block](../roadmap/0635-riscv64-proofs-check-against-the-riscv64-model.md).

*Measured 2026-10-04 (UTC) by milestone 741 (does a standing proof notice a regression), under §216:*
across 189 harnesses in 26 packages (glob and calendar not measured), 178 of the 184 that reach a
cargo-mutants mutant kill at least one. In reach, the proofs kill 1,668 of 2,626 viable mutants (64%),
and the census tests catch 91%. Six harnesses reach mutants and kill none, four of them panic-freedom
proofs. Four `inter_process_communication` harnesses prove `unsafe fn`s, which cargo-mutants never
mutates, so they reach no mutant and go unscored. 17 mutants are killed by a proof alone.
[`notes/kani-reach-2026-10-04.md`](../../notes/kani-reach-2026-10-04.md).

*Measured 2026-10-06 (UTC), under §216 (fatal-risk facts are correctable, and verdicts are the architect's):* the falsification backlog is done. `unfalsified` fell from
56 harnesses to 0 across #1697, #1701 and #1713; 227 of 229 carry a record (220 replayable, 7
attested). None is `unfalsifiable` (§134 (a harness carries a machine-replayable falsification record, or it is not evidence), amended 2026-10-06): `inter_process_communication::signal_preserves_the_invariant` carries a patch that replays, a plain increment of the pending count that overflows where the saturating one does not. `direct_memory_access_validator::a_descriptor_mutated_after_validation_cannot_reach_the_device`
was falsifiable all along: its memory model was a fixed table, so a double fetch (validate one read, copy a second) could not show. A racing-driver model (`ChainMem::racing`) now turns it red under a double-fetch patch. The proof held only because its model of memory was too kind, this risk's failure mode, found by falsification work. A record shows a
harness *can* go red, not that one *has* after the day it was written. AMBER re-affirmed by calef on
2026-10-06 (#1730); green needs a standing harness in CI to catch a defect nobody planted, and every
file out of reach to be reached by stubbing or listed as a named assumption with a test that can
break it. [Appendix](proofs-and-their-reach.md#added-2026-10-06-216-the-falsification-backlog-is-done).

*Measured 2026-10-06 (UTC):* across 9,794 verify runs (2026-08-03 to 2026-10-06), no standing Kani harness failed verification on a pull request, and none was found weakened. [`notes/kani-catches-2026-10-06.md`](../../notes/kani-catches-2026-10-06.md) (provisional name).

The first x86_64 proof went red on a latent defect, the first of the class this risk asks about. The
claim: proofs over the pure crates and slices of a mostly unverified kernel. [Appendix](proofs-and-their-reach.md).
