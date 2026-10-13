# The HVF leg: the aarch64 suite on the physical core

*(Milestone 81. `script/ci-build`, `cargo xtask test --hvf`, `helpers/qemu-runner-aarch64.sh`.
The leg rode `script/gates` from 2026-08-04 until milestone 286 retired that script into
`script/ci-build`'s table on 2026-09-13; every measurement below was taken under the old name.)*

Every test this project had ever run ran under TCG, where QEMU translates each aarch64 instruction
into host code. That is the right default (deterministic, identical on any host), but it is not a
real machine: no real caches, no real store buffer, and four "cores" that QEMU round-robins between
rather than four cores running at once. Apple's Hypervisor.framework runs the same kernel at guest
EL1 on the physical Apple Silicon core, four vCPUs on four host threads, and until the RISC-V board
arrives it is the only real silicon this project can test on at all.

## The leg runs again on a GICv3 (milestone 227)

Read this before the rest of the page: everything below it describes runs from QEMU 11.0.2 on a
GICv2, and that machine no longer starts here.

```
qemu-system-aarch64: HVF does not support GICv2 emulation
```

That is QEMU 11.1.1 refusing `virt,gic-version=2,accel=hvf`, reproduced on 2026-09-02 with no nife
kernel involved. From then until milestone 227 the leg had no machine to run on, because the kernel
drove GICv2 only; `script/ci-build` skipped it out loud (milestone 222) and this machine had no
accelerated coverage at all.

**Since milestone 227 (2026-09-19) the kernel drives GICv3 too**, and the runner asks HVF for one
(`gic-version=3`; TCG keeps 2). [interrupts.md](interrupts.md) has the driver. Measured on the first
runs, an Apple M-series core, QEMU 11.1.1:

| | result |
|---|---|
| boot, four cores over PSCI | yes; `interrupts : GICv3 ...`, ticks on every core |
| the kernel suite, full run | **322 passed, 3 skipped** on the final run, exit 0; one test hung in two earlier runs |
| the scanout and inbound checks | pass (compositor, terminal text, test pattern; both listeners) |
| `script/bench --real` | runs to completion |

Two things failed on the way there, and neither is the GIC:

- **An EL0 cycle-counter read is not refused under HVF** with `PMUSERENR_EL0` at zero. The
  hypervisor answers it, so the negative half of
  `user::tests::a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults` skips on an
  Apple core now, and `arch::timer::set_cycle_counter_grant`'s BUGS carries the measurement. This is
  milestone 74's finding as much as this leg's: an HVF run is not evidence about that register.
- **`user::tests::a_std_program_serves_a_granted_listening_port` hung in two of three full `--hvf`
  runs**, and passed when run alone. It was not HVF-only: the riscv64 CPU matrix hit it under TCG
  twice (notes/net/the-inbound-check.md). Milestone 514 (the HVF std listener hangs in some full
  runs) fixed it. The prober's third connection, opened within a
  millisecond of the hand-written listener's second answer, could complete its handshake into the
  socket `net_stack` re-arms after every `ACCEPT`. The hand-written client's `CLOSE` then dropped
  that socket without a reset. So the prober held an established connection to nobody while the std
  listener timed out and aborted. `net_stack` now resets such a peer. The block has the forced
  reproduction and the runs.

**What still happens when the machine will not start** (milestone 222's machinery, unchanged):
`script/ci-build` skips the leg and says so; `script/test --hvf` fails with a paragraph saying the
breakage is not yours; `cargo xtask test --hvf` asks before standing up its referees. All of it
comes from one probe in `helpers/qemu-runner-aarch64.sh` that starts the runner's own machine
string paused and quits it, so nothing tests a QEMU version number.

## How to run it

```sh
script/ci-build         # every local check, cheapest first, with this leg last
script/ci-build hvf     # just this leg, through the table
script/test --hvf       # just this leg, with no probe and no loud skip
```

`--hvf` is aarch64 only and says so if you ask for anything else: Hypervisor.framework runs the
host's own ISA, so `--arch riscv64 --hvf` is refused, and so is `--cpu`, because the guest runs the
physical core and `-cpu host` is the only answer.

It skips the host-logic crates, the vendored RedoxFS round trip and the `redoxfs_server` core, and
prints one line saying so. Those are host code on the host; no accelerator exists on that path, so
re-running them costs about 30 seconds and proves nothing the TCG leg has not. What the leg does
re-run is everything an accelerator can change: the 234 kernel tests under QEMU, the host-side
scanout referee, and the post-run image checks.

## What it costs (measured 2026-08-04, M-series, warm build)

| command | what ran | wall clock |
|---|---|---|
| `script/test --arch aarch64` | host crates + the 234-test aarch64 kernel suite under TCG + image checks | 58 s, 52 s |
| `script/test --hvf` | the same 234-test suite on the physical core + image checks | 16 s, 12 s |
| `cargo test --workspace --exclude kernel --exclude user --exclude user_mode_runtime` | the host crates alone | 14 s |

The counts here read 232 when the timings were taken, and are 234 in the merged tree: milestone 86's
`time` brought two kernel tests with it. The number is a fact about the whole suite rather than
about this leg, so it is taken at the merge (CLAUDE.md), and the wall clocks are left as measured
because two tests do not move a twelve-second figure.

Two samples each, on a machine that was not otherwise idle (another lane was building). Subtract
the 14 seconds of host crates that only the TCG row contains. The comparison is then about 40 s
against about 14 s for the same fixtures, the same 234 tests, and the same post-run image checks.
That is roughly a 3x win on the leg, and more on the booted suite alone, since a fixed few seconds
of both numbers is cargo and image building.

So the leg adds 12 to 16 seconds to the local run. Timing it before adopting it was the condition
milestone 81 (an HVF leg: the test suite on the physical core) set. The `--full` flag the block
named as the fallback is not needed, because native execution beat TCG by the margin the block
hoped for.

## Why the local tier and not a workflow

GitHub's hosted macOS arm64 runners are themselves virtual machines and do not expose nested
virtualization, so HVF does not exist there. A self-hosted runner on the dev machine would close
that gap and is deliberately not part of this: it couples CI to a laptop that sleeps.

`script/ci-build` with no arguments is the one command a person or an agent runs before pushing,
which makes it the place where every lane on this machine picks the leg up for free. The leg is a
`local` row in its table, and there is no CI job for it at all. When the host cannot supply HVF the
leg skips loudly. It names the reason (not macOS, not Apple Silicon, `kern.hv_support` off, or a
QEMU built without the accelerator), and it says in plain words that nothing in the run executed on
a physical core. A silent omission would let a Linux CI transcript read exactly like one from this
laptop, and the whole point of the leg is that those are not the same evidence.

## Semihosting is not answered under HVF, and this is what that looks like

The test harness reports its verdict by asking the host to exit through semihosting. QEMU 11.0.2
does not intercept ARM semihosting on the HVF path. The `hlt` trap is not swallowed: it raises an
Unknown-reason exception in the guest, the panic handler tries to exit the same way, and four cores
write interleaved garbage forever. So the host ends the run. `hvf_kernel_leg` reads the transcript
for three markers printed before the exit (`test result: ok. N passed`, `[PANIC] `, `WATCHDOG:`),
relays 200 more lines after a failure, and stops. The probe, the exception and why the relay is
bounded are in [`hvf-leg/semihosting.md`](hvf-leg/semihosting.md).

## The SMMU correction

`helpers/qemu-runner-aarch64.sh` attaches `iommu=smmuv3` on the HVF path as well as TCG. smmuv3
emulation alongside HVF acceleration is not a fragile combination: the suite on the physical core
runs green with the SMMU attached, including `a_backing_outside_the_grant_is_refused_by_the_iommu`,
the test that asserts the *hardware* faults a DMA leaving its domain.

Without the SMMU under HVF the compositor test fails outright ("a virtio-gpu is present but the
IOMMU is not active"), and the DMA-confinement tests would have no hardware to assert about. Both
paths are where the flag belongs on principle too. The accelerator chooses how CPU instructions execute, while the
SMMU sits in front of the PCIe root complex and translates *device* traffic that QEMU emulates in
the host process either way. This is a correction to the record, in the sense CLAUDE.md means: the
machine overruled the comment.

## What the physical core found: five assertions about "has it happened yet"

Every failure belonged to the milestone-78 family (`notes/load-sensitive-assertions.md`), found from
the opposite direction. A slow machine produces a deficit, and HVF is a *fast* machine that produces
the same deficit for the mirror-image reason. A yield count is not a duration in either direction.
They arrived one per run, over five runs, so the leg is a sampler of this defect, not a detector.

The five were the revoked waiter, the worker that made no progress, the corpse that never parked,
a supervision reclaim with no assertion, and the spinner that never ran. Each yield count became a
`wait_for` on the state it was waiting for, and the spinner's vacuity guard now waits for the
spinner to have spun. None of these was a timer-behavior failure, which is the finding worth
stating plainly. Each test, its failure and its fix are in
[`hvf-leg/five-assertions.md`](hvf-leg/five-assertions.md).

## Timer behavior: what did NOT differ, and why

Under HVF guest time is host time (`CNTVCT_EL0` is passed through at the host's 24 MHz) and there is
no icount instrument, so the milestone expected the wall-clock assertions to be the casualties. None
of them failed. The reason is milestone 78 (the load-sensitive assertions). The timer-drift twins
are aimed at the re-arm law: over a window in which
`MISSED_TICKS` did not move, the deadline advanced by exactly one interval per delivered tick. That
law is a statement about state the kernel owns, not about how fast the clock runs. A test that
measures the right thing does not care which accelerator it is on.

The only wall-clock timer exposure this leg inherits is `ticks_arrive_at_the_configured_rate`'s
retry budget, which reports `UNMEASURED` instead of failing (milestone 62 (tests that assert on
time: make a red run mean something)). HVF makes it no worse in principle: a deschedule of a host
thread running a vCPU produces the same missed tick either way. The local tier runs `script/icount`
before `script/test`, so this leg is preceded by an instrument the accelerator cannot influence at
all.

### The settle windows the leg makes weaker, not flakier

Three sites spend 400 yields to "let it settle" and then assert that nothing more happened. A
shorter real settle window makes that negative assertion easier to pass, so the leg proves *less*
there rather than going red. They are left alone deliberately, because turning a settle window into
a duration is a design question. The three sites are named in
[`hvf-leg/five-assertions.md`](hvf-leg/five-assertions.md).

## BUGS

- Most of the measurements below were taken on QEMU 11.0.2 and a GICv2, before milestone 227
  (a GICv3 driver) put the leg back on a machine QEMU 11.1.1 will start. When the probe cannot start
  that machine, `script/ci-build` skips the leg out loud, which is milestone 222 (a leg that fails
  instead of skipping), and a loud skip is a record of a gap and not a substitute for one.
- **A timing assertion that fails on every HVF run can sit on `main` unseen**, because nothing
  runs this leg but a person. The tell is a red `script/ci-build hvf` on a commit whose CI is green.
- The scanout referee can sample too late on a loaded host. Measured 2026-09-24 over six runs, on a
  host held at a 1-minute load of 14 to 25 on 8 cores: one run passed the whole suite and then
  failed the display-terminal and display-pattern checks while the guest's own tests passed.
- One failure ends the suite, on both legs. The custom test framework does not unwind, so a
  panicking test is the end of the run under TCG too; what differs here is only the exit. The scanout
  and inbound checks still report afterwards and can fail because the guest never reached their
  tests, so the leg now says to fix the first failure before reading them.
- A failing run leaves an exception storm behind it. The kernel has no way to know its
  semihosting exit will not be answered, so any failure under HVF ends in an unbounded panic loop
  on four cores. The host stops reading and kills the child, so the cost is bounded in practice.
  But a transcript from a failed HVF run ends in interleaved garbage after the 200-line budget, and
  anyone driving the runner by hand under HVF (rather than through `xtask`) will get a QEMU
  that never stops. Use `helpers/qemu-bounded.sh` for that. A guest-side fix is not built here:
  recognizing the semihosting trap in the Unknown-reason handler and parking in `wfi` instead of
  panicking. It would touch the exception path for a test-only benefit. On its own it would make
  the leg *worse*, because `hvf_kernel_leg` stops reading after 200 more lines, and a guest that went
  quiet would leave it blocked on the transcript until the outer bound killed it. The two halves
  would have to land together, the host stopping on a marker the parked guest prints.
- **The leg is not a CI gate and cannot be one.** Nothing enforces that it ran. `script/ci-build`'s
  no-argument path is the enforcement, and running that is a convention.
- One machine, one model, no variation. `-cpu host` is mandatory under HVF, so this leg says
  nothing about other aarch64 implementations; that job stays with `script/cpu-matrix` (which is
  riscv64's) and with the second board.
- The leg samples the yield-count defect rather than detecting it. One of the four found here
  passed three consecutive HVF runs before failing. So a green HVF leg is evidence, not proof, and
  a red one on an unchanged tree should be read as this family before anything else.
- The suite still exits through semihosting on the TCG leg, so the two legs report their
  verdicts by different mechanisms. A change to the harness's final line
  (`test result: ok. N passed`) would silently turn this leg into one that always fails, and no
  test asserts that string. It is matched in `hvf_kernel_leg`.

## See also

- design/roadmap/0081-hvf-leg.md (the block)
- design/roadmap/0222-hvf-leg-fails-silently.md (milestone 222, why the leg skips rather than fails)
- notes/scripts.md (`script/ci-build` and `script/test`, and where the leg sits in them)
- notes/load-sensitive-assertions.md (milestone 78: the family both failures belong to)
- notes/benchmarks.md (`--real`, the other HVF caller, and the exit trick this leg reuses)
- notes/semihosting.md (the mechanism that is not answered here)
- notes/framebuffer-contract.md (the scanout the referee checks, and why the guest cannot)
