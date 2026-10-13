# §139 appendix: the premise was half false

Evidence for [§139 (who may read the cycle counter, and by what
authority)](../0139-cycle-counter-authority.md), the four checks that reframed the question of
milestone 75 (who may read the cycle counter). The main page is complete without this one, which
exists to verify or challenge it.

Milestone 75's block frames this as a decision about whether to open something that is closed.
Three checks say the framing is wrong in three different directions, and each one changes what the
decision has to cover.

## 1. On `x86_64` the counter is already open to every program, and nothing decided that

`kernel/src/arch/x86_64/boot.s:178` and `:385` are the only writes to `CR4` in the tree, and both
are `or eax, 1 << 5`, which is `PAE`. `TSD` is bit 2 and is never touched, so it holds its reset
value of clear, and ring 3 may execute `rdtsc`. `notes/x86-port/user-mode-runtime.md` states this in its own words and
is the record that it was noticed rather than overlooked:

> `now()` is `rdtsc`, and ring 3 may read it because `CR4.TSD` is clear at reset and this kernel does
> not change it. That is the same shape as aarch64 needing `CNTKCTL_EL1.EL0VCTEN` and RISC-V needing
> `scounteren.TM`, with the difference that here the permissive state is the default and the kernel
> would have to act to *close* it.
>
> -- notes/x86-port/user-mode-runtime.md

So one of the three supported architectures has already answered milestone 75 with option 1, by
inheritance. **A decision that says "closed unless granted" is not a decision to open something; on
`x86_64` it is a decision to close something programs already use.**

## 2. And closing it on `x86_64` would take the clock away, because there is only one register

`crates/user_rt`'s `now()` on `x86_64` is `rdtsc`. There is no coarse alternative on that
architecture the way `CNTVCT_EL0` is the coarse alternative on aarch64. So on `x86_64` the §10
clock exception (§10 says there is "no ambient authority", and notes/abi.md records the counter as
its one eyes-open exception) and this milestone's question are the same register. Setting
`CR4.TSD` today would break `Instant`, `thread::sleep`, the random seed, smoltcp's timestamps, and
the benchmark harness, all at once.

Checked, and it held: a search for a second source on 2026-09-02 found none. The evidence is its own appendix, [`x86-clock-sources.md`](x86-clock-sources.md).

This is not an argument for leaving it open. It is the statement of what closing it costs. The
shape of the fix already exists in this tree. §43 (reading the clock is a page) put the wall clock
in a page rather than a register, and a coarse monotonic value published in a page is the same move
one axis over. Nothing here proposes building that; it is named so the `x86_64` row is a decision
with a price rather than an exception with no plan.

## 3. On aarch64 and riscv64 the tree does not *establish* that the counter is closed, it assumes it

This is the finding worth acting on regardless of which option wins.

aarch64. Nothing in the tree writes `PMUSERENR_EL0`; the grep for it returns milestone 75's own
block, milestone 147's, and nothing else. Arm's register description says of every field in it,
`EN`, `CR`, `SW` and the rest:

> On a Warm reset, this field resets to an architecturally UNKNOWN value.

and of the cycle counter with `CR` and `EN` both 0, that EL0 reads "are disabled" and "generate an
exception to EL1, or to EL2 when EL2 is implemented and enabled for the current Security state".
Both quotations are from the `PMUSERENR_EL0` page of the Arm system-register reference at
`https://arm.jonpalmisc.com/latest_sysreg/AArch64-pmuserenr_el0`, read 2026-09-02. The two sentences
together say the trap is conditional on a value this kernel never sets.

Linux hit exactly this and fixed it by writing the register explicitly. Its commit
*"arm64: kernel: enforce pmuserenr_el0 initialization and restore"* (lkml.iu.edu archive
`1601.3/03556.html`, read 2026-09-02) says:

> The pmuserenr_el0 register value is architecturally UNKNOWN on reset.

and describes the exposure as platforms where "the pmu is not probed, therefore the pmuserenr_el0
register is not reset in the kernel, which means that its value retains the reset value that is
architecturally UNKNOWN".

Under QEMU this is almost certainly zero and the trap almost certainly fires. On argon, the
Jetson TX1 that milestone 127 (the seL4 machine) is about, it is whatever TF-A and the boot ROM
left, and nobody here has looked. This lane did not run a spike to find out; see BUGS.

riscv64. `kernel/src/arch/riscv64/timer.rs:182` opens the time CSR with
`csrs scounteren, TM`, a set of bit 1 and nothing else. The comment four lines above it says:

> CY (cycle) and IR (instret) stay closed.

Nothing clears them. They stay closed only if firmware left them clear, which is the identical
mistake that file's own comment records having found and fixed two paragraphs earlier. `user_rt`
documented U-mode `rdtime` as working "because the kernel sets scounteren.TM"; the kernel never set
it, and it worked on OpenSBI's default. The same sentence, about the same register, is now true of
`CY` and untrue of `TM` only because somebody went and looked. This is a claim stated in a comment
that the code does not establish, which is rung four wearing rung one's clothes.

What follows from all three. Part of milestone 75 is not a decision at all. Whatever authority
model wins, the kernel has to write these registers rather than inherit them, or the answer is
firmware's on every board. That part is a defect fix and this document
[recommends it outright](../0139-cycle-counter-authority.md#recommendation).

## 4. And 74's aarch64 half is blocked on more than this

Milestone 127's other prerequisite, the EL2 to EL1 entry drop, is built (PR #650): `boot.s`'s EL2
drop writes `MDCR_EL2 = 0`, clearing `MDCR_EL2.TPM`, the trap that would otherwise catch every EL1
access to `PMCCNTR_EL0`. So the EL2 half of the path exists.

## BUGS

- No spike was run. The claim that an EL0 `mrs x0, pmccntr_el0` traps today under QEMU is
  inferred from Arm's register description plus the absence of any write to `PMUSERENR_EL0` in this
  tree; it was not observed. It was not run for two reasons. The answer that matters is on argon, where
  the reset value is UNKNOWN and no emulator can report it, and the aarch64 EL0 read is the one
  measurement that a QEMU run would answer least usefully. The `x86_64` claim was not spiked either
  and rests on reading `boot.s`'s two `CR4` writes and on notes/x86-port/user-mode-runtime.md's own statement.
- The riscv64 `mcounteren` half is untested. Even with `scounteren.CY` set, U-mode reads of the
  `cycle` CSR require `mcounteren.CY` from firmware, which is OpenSBI's on radon and is not ours.
  Whether it is set there is unknown and is a bench check, the same shape as milestone 127's
  "`PMCCNTR_EL0` readable at EL1" item.
