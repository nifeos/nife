---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: a-program-that-asks-the-cpu-dies-without-saying-so
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 555. A program that asks the CPU dies without saying so

*(This file was renumbered on 2026-09-22 from 525 to 555 because a concurrently merged lane had taken 525.)* Promoted from the proposal `a-program-that-asks-the-cpu-dies-without-saying-so`, filed 2026-09-21, on calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited except for this paragraph: the argument is its author's and promotion is not the moment to improve it. Raised by the maintainer from two milestones' findings in the same
week, and filed rather than launched because the third option below is a decision about what a
program may assume about its machine.

The diagnostic half needs no ruling and could start today; the state half is
calef's.

## The failure, as it actually presents

On `x86_64-unknown-nife` a program that decides at run time which implementation to use, by
asking the CPU what it supports, executes an AVX2 instruction in ring 3 and dies with
`vector 6 (invalid opcode)` before printing anything. Milestone 442 (a crypto provider `rustls`
can use on all three bare-metal targets) met this in `chacha20` and carries the transcript in
`notes/cryptography-provider.md`.

**It is not a bug in the kernel and the fault is correct.** Milestone 447 (a thread's vector
registers are its own) records why: this kernel saves the `FXSAVE` area and **not** AVX, so
`CR4.OSXSAVE` stays clear, and *"every VEX-encoded instruction raises"* the fault rather than
silently corrupting another thread's registers. Faulting is the design working. What is wrong is
that the program is dead and nothing says why.

**Compile-time detection is not the same question and is already handled.** A crate that gates on
`#[cfg(target_feature = "avx2")]` compiles portable code here, because the target JSON switches those
features off. The hazard is only the crate that asks `cpuid` at run time, and the target has nothing
to say to it.

## Today's answer, and what it costs

One build flag per crate, discovered by hitting the fault. `aes_force_soft` for `aes`,
`chacha20_force_soft` for `chacha20`, and whatever the next crate needs. Each is a true fact about a
specific crate and each was found the same way: a program died, somebody read a bare vector number,
and somebody traced it.

Milestone 447 also established the trap in reasoning about this: the target flip does not retire
those flags. Saving the wider state and disabling the narrow one are different questions, and a
crate that asks the CPU stays a hazard after any flip. The rule it recorded is to count the flags a
change retires rather than the flags that exist.

## The three options

A. Keep the flags, and write down that this is the mechanism. Cheapest. Each flag is
crate-specific and honest. The cost is the failure mode: a *silent* dead program, discovered by
someone reading `vector 6 (invalid opcode)` with no idea which instruction or which crate, and a
list that only ever grows by being bitten.

B. Make the refusal legible. The fault already happens and the kernel already knows which
features it enabled for userspace. An `#UD` in ring 3 on a target whose userspace has no AVX state
is diagnosable: the kernel can say which instruction faulted and that this machine's userspace has
no state for it, instead of a bare vector number. This is the smallest thing that turns a silent
death into a message, it forecloses nothing, and it is the maintainer's recommendation for the
reversible half. It needs no decision: the failure path already exists and only its wording changes.

C. Enable the state. Set `CR4.OSXSAVE` and `XCR0`, save and restore the wider area, and the
hazard disappears along with every force-soft flag. This is the irreversible half and it is
calef's, for three reasons. It is a claim about what a program may assume about its machine, which
is the system call surface's neighborhood. It costs what milestone 447 measured for the narrow state
and more, on a switch path whose numbers are held to an icount tripwire. And `kernel/src/arch/x86_64/fp.rs`'s
own `BUGS` section already records that the day this kernel sets `XCR0`, that file needs an `xsave`
path, with nothing enforcing it.

## What would decide C

Not a preference: a workload. Nothing in this tree currently wants AVX, so the state would be saved
for nobody. The condition worth writing down is the first program that measurably loses
performance for want of it, which is a fact about a benchmark rather than an argument about
capability. Until then the flags and a legible refusal are the honest position, and B makes the
refusal cheap to diagnose when the day comes.

## Index row

On `x86_64-unknown-nife` a program that decides at run time which implementation to use, by asking
the CPU what it supports, executes an AVX2 instruction in ring 3 and dies with `vector 6 (invalid
opcode)` before printing anything.
