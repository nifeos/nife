---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
milestone_dependencies: 624
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 628. The x86_64 swish-check leg costs what the others do

Number provisional (the 628 lane, 2026-10-03 UTC): minted by the integrator at merge. calef raised
it on 2026-10-03, because swish-check gates every pull request. Its x86_64 leg was about 18 of the
job's 21 minutes on the branch of milestone 624 (the x86_64 paint path stops repainting the world).
That is run 37083676625: 7.7 s a line against aarch64's 0.2 s. The pull request's base is
`milestone/624-paint-path` until 624 lands.

The leg was paced by the timer, not by emulation. After the hand-over, the x86_64 boot thread
halted in a loop while still runnable. Every pass of the round-robin that reached it stopped the
core until the next 10 ms tick, and the polling input driver kept the rotation turning. The boot
thread now exits instead. Under TCG on patagonia the first boot went from 4.09 s a line to 1.07 s.
The second boot's eight lines went from 23.8 s to 0.9 s. On CI (run 37089120726) the leg is 0.26 s
a line under KVM, against aarch64's 0.19 s, and 2.85 s under TCG on arm64.
`notes/benchmarks/swish-check-x86-leg.md` has the method and each lead measured one change at a
time.

What ships:

1. `kernel/src/lib.rs`: the boot thread calls `sched::exit()` after `x86_hand_over`.
2. The x86_64 leg runs in its own job, `swish-check-x86_64` (provisional), on an x86_64 runner. It
   boots under KVM whenever `/dev/kvm` opens; `helpers/qemu-uefi-x86_64.sh` takes `NIFE_ACCEL=kvm`.
   The aarch64 and riscv64 legs stay on the arm64 runner, whose weakly ordered host is why they are
   there. An x86_64 guest has no such property to lose.
3. `script/ci-build` gains the two `ci` rows the jobs run. The `local` row is unchanged.
4. A local run without `std_exerciser` skips `wc < args.txt` along with the line that writes its
   input.
5. `SWISH_CHECK_X86_LINE_SECONDS`, the bound for the leg under TCG, goes from 90 s to 45 s: 3.2x CI's
   slowest emulated line. Under KVM the leg holds the other legs' 30 s.

The swish-check job took 21m07s on 624's branch. Its arm64 half now takes about five minutes and
the x86_64 job about three, in parallel.

Three leads measured as not levers. `-cpu max` costs the same as a named model. Dropping the IOMMU
moved 10 percent, inside run-to-run noise, and the confinement depends on it. The harness polls at
100 ms on every leg.

## BUGS

- The new job does not gate a merge until the ruleset names it. The merge queue requires
  `swish-check (the interactive shell, three architectures)`, which now runs two legs. Adding
  `swish-check-x86_64 (the interactive shell, x86_64 under KVM)` is an admin act, and calef's.
  Until then an x86_64-only regression at the prompt merges green.
- The input driver still polls, so an x86_64 core never idles at a prompt. That is
  `components/src/input.rs`'s named follow-up, and no longer where the leg's time goes.

## Follow-on

- **Recorded.** The ruleset edit that makes `swish-check-x86_64` a required check is calef's, in
  this block's BUGS. The input driver's polling is `components/src/input.rs`'s own follow-up.
- **Recorded.** aarch64 and riscv64 also end their boot in `arch::halt()` on a runnable thread,
  and `interrupt_ignorer` is the slowest line on every leg; both are unexamined, in
  `notes/benchmarks/swish-check-x86-leg.md`.

## Index row

BUILT on `milestone/628-x86-swish-parity` (PR #1487). The x86_64 swish-check leg was tick-paced by
a halted boot thread on the run queue. The boot thread now exits, and the leg runs on an x86_64
runner under KVM at 0.26 s a line against aarch64's 0.19 s. The new job gates nothing until the
ruleset names it.
