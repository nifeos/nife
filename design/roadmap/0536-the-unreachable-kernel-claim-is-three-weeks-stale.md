---
status: BUILT
raised: 2026-09-20
built: 2026-09-25
promoted_from: the-unreachable-kernel-claim-is-three-weeks-stale
---
# 536. Two records still say the prover cannot see `kernel/src`, and it has been able to since 2026-08-30

Built 2026-09-25 (UTC), under calef's ruling of that day recorded as §216 (fatal-risk facts are
correctable, and verdicts are the architect's). Risk 2's stale sentence and size are corrected in
`design/fatal-risks/README.md`, its appendix `design/fatal-risks/proofs-and-their-reach.md`, and
`notes/proof-retrospective.md`, each citing its source, and `script/verify`'s header agrees (PR
#1276). Risk 2's status and color are unchanged, because §216 leaves them with calef. The line
counts below are this block's 2026-09-20 measurement; the corrections carry the 2026-09-25
re-measure (86,528 lines, 15,966 in files calling `asm!`, eight harnesses).

§216 answers the gate. Its reasoning: `design/fatal-risks/README.md` is an architect's file and risk
2's text is theirs to amend. AGENTS.md puts the falsification list outside a lane's reach, and the
edit to it by the lane for milestone 64 (enough `std` to run somebody else's crate) is recorded as
an exception rather than a precedent. `notes/proof-retrospective.md` is an ordinary note and needs
no decision, but the two say the same sentence and should be corrected together or the reader meets
a tree that disagrees with itself.

## The claim

`design/fatal-risks/README.md` risk 2 (the proofs prove trivia, and the real bugs live where Kani cannot
reach), verbatim:

> The cause is one line of `script/verify`'s own header, verified rather than inferred:
> *"`cargo kani -p <crate>` never compiles the kernel, the user programs, or xtask."* So 64,818
> lines of `kernel/src` are out of reach by construction, and that is exactly where every
> concurrency, hardware-contract and resource-accounting defect lived.

`notes/proof-retrospective.md` carries the same sentence, which is where risk 2 got it.

## Why it is wrong now

Three milestones landed after it was written:

- Milestone 193 (put `kernel/src` within reach of the prover, because today the proofs cannot see it),
  2026-08-30. Four changes, three of them one-line `cfg`s.
- Milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it, and none of it is proved),
  2026-09-04. Two harnesses inside `arch/aarch64/iommu.rs`.
- Milestone 304 (`cargo kani -p kernel` only ever compiled one architecture, and it was the runner's),
  2026-09-16. Two harnesses inside `arch/x86_64/irq.rs`, and a second verify host.

`notes/kernel-proofs.md` is the record and this lane re-ran it. Six kernel harnesses exist; four run
on any given host. **`script/verify`'s header line is itself stale** and is the root of both copies.

## What the corrected claim looks like

The reach is now bounded by three things rather than by the crate, and this lane measured each
(`notes/verus.md` has the commands and the verbatim tool output):

| what stops a harness | how it fails | scale |
|---|---|---|
| a call graph reaching `asm!` | loud: `TerminatorKind::InlineAsm is not currently supported by Kani`, and it reaches through a *dependency* (`aarch64-cpu`) | **15,001** of `kernel/src`'s 81,413 lines are in files containing an `asm!` call, and every one is under `arch/` |
| a fixed-address MMIO read | loud: `dereference failure: invalid integer address` | the rest of `arch/` |
| an architecture the host does not compile | **silent**: `no harnesses matched the harness filter` | two of three subtrees per run; `riscv64`'s is compiled by nothing anywhere |

So the honest sentence is narrower and better: **DECISIONS §4 (kernel shape) rule 1,
architecture-specific code under `kernel/src/arch/`, has held the
unreachable residue to under a fifth of the kernel.** And it is exactly the fifth you would expect.
Both defects risk 2 leans on are still on the far side of it, so **the risk's
color does not obviously change**. They are the timer re-arm drift from milestone 6 (threads, the context switch,
and preemption) and the VisionFive 2 undelivered wake. What changes is that the cause is two Rust constructs rather than
a crate boundary, and that the fix is writing harnesses rather than moving a wall.

`64,818` is also stale as a size. `kernel/src` is 81,413 lines today, 40,953 of them non-blank
non-comment.

## What is blocked until it is answered

Nothing is blocked. This is a correctness problem in the record, and the reason it is worth a
proposal rather than a passing mention is the one AGENTS.md states. A lane briefed from a stale
premise spends its budget discovering that, and this one did. The next lane sent at risk 2 will do
the same.

## What this does not propose

Not that risk 2 should go green. It should not, on this evidence: no Kani harness in this tree
has still ever caught a defect after the day it was written, which is the finding, and reach was
only ever half of the explanation for it. See `notes/verus.md`, which asked whether a different
verifier would extend the reach and found that it stops at the same boundary.

## Follow-on

- **Milestone 432.** Proving `arch/riscv64/iommu.rs` from an aarch64 host, which PR #1276 priced.
  Milestone 432 (the RISC-V IOMMU driver has no counterpart to the SMMU's proofs) built it 2026-09-25. Whether the corrected facts move risk 2's verdict was put to calef in PR #1282,
  and he ruled the same day to keep AMBER and reword the red half.

## Index row

Risk 2 said `cargo kani` never compiles the kernel for three weeks after milestone 193 made it
compile. The fatal-risks entry, its appendix and `notes/proof-retrospective.md` now state the reach
as measured: `asm!`, fixed-address MMIO and an uncompiled `arch/` subtree, not a crate boundary.
