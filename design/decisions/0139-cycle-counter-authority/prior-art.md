# §139 appendix: prior art, and the clock a closed counter cannot stop

Evidence for [§139 (who may read the cycle counter, and by what
authority)](../0139-cycle-counter-authority.md), what seL4, Linux and L4Re do, and why a grant is
not confinement. The main page is complete without this one, which exists to verify or challenge it.

## Prior art, read rather than recalled

seL4, which is the one that matters, has both answers and ships the weaker one. Its build option
`KernelArmExportPMUUser` is documented on `docs.sel4.systems/projects/sel4/configurations.html`
(read 2026-09-02) as:

> Grant user access to the performance monitoring unit. While useful for benchmarking, this option
> opens the possibility of timing channels.

It defaults off, and the same page records that `KernelVerificationBuild` excludes options of this
kind. So **seL4's published 413/426-cycle numbers are produced by a configuration seL4 does not
verify and does not recommend for production**, which is worth knowing before treating them as the
standard to match.

**seL4's own community has proposed exactly option 2 and has not landed it.** RFC-16, *"New
capability for the PMU"*, Krishnan Winter, proposed 2024-02-02, is `seL4/rfcs` PR #22, still open,
file `src/proposed/0160-pmu.md`. Read in full 2026-09-02. It says:

> Present profiling support uses the PMU through an ad-hoc interface that is designed for debugging
> and is consequently only available in a specific benchmarking configuration of the kernel. The
> same interface cannot be used in a production system as it is inherently insecure.

and

> Obviously the PMU presents a covert channel that exposes information about execution of user-level
> components (as well as the kernel). Therefore, PMU access needs to be explicitly authorised, which
> means we need an access-control model for the PMU.

and, on the current ARM situation:

> Additionally, on ARM systems, the only way to get access to the PMU from user-space is to
> configure the kernel to export access to the PMU registers, making the PMU an uncontrolled
> resource.

Its shape is a new object `seL4_PMU` with badged capabilities, the badge naming which counters
are authorized, and a blocking invocation. Its own unresolved questions include "How will the PMU
object affect verification? Initially it will not be available in verification builds of seL4".

Linux has both answers too, and the arm64 one is the interesting half. The global answer is
`perf_event_paranoid`, documented at `kernel.org/doc/html/latest/admin-guide/sysctl/kernel.html`
(read 2026-09-02) as controlling "use of the performance events system by unprivileged users
(without CAP_PERFMON)", default 2, with `-1` allowing "(almost) all events by all users". It is a
global sysctl, not a per-target grant, which is the criticism milestone 147 already makes of it.

The arm64 half is closer to what this decision needs. The commit *"arm64: perf: Enable PMU counter
userspace access for perf event"* (lkml.rescloud.iu.edu archive `2105.2/02527.html`, read
2026-09-02) enables `PMUSERENR_EL0`'s `ER` and `CR` bits per task, on the context-switch hook,
and states its reason:

> Only support user access when explicitly requested on open and only for a thread bound events.
> This avoids some of the information leaks x86 has and simplifies the implementation.

Two things fall out of that sentence and both bear on this decision. Per-thread, opt-in, maintained
at context switch is the mainstream modern answer, not an exotic one. And the "information leaks
x86 has" that Linux is avoiding are the consequence of the always-on `rdtsc` that this tree has
inherited on `x86_64` by the same default.

**L4Re: I could not source this.** The searches returned a virtualization paper and secondary
summaries rather than an L4Re or Fiasco.OC authority on PMU access control. Recorded as not
established rather than paraphrased.

## The thing that does defeat a closed counter, and it defeats it on all three architectures

A program with two threads and a shared page does not need a counter instruction. One thread
increments a word in a loop; the other reads it. Schwarz, Maurice, Gruss and Mangard, *Fantastic
Timers and Where to Find Them: High-Resolution Microarchitectural Attacks in JavaScript*, FC 2017,
read 2026-09-02 at `gruss.cc/files/fantastictimers.pdf`, built exactly this inside a browser:

> We implemented a clock with a parallel counting thread using the SharedArrayBuffer. An
> implementation is shown in Listing A. . The resulting resolution is close to the resolution of the
> native timestamp counter. On our Intel Core i test machine, we achieve a resolution of up to 2 ns
> using the shared array buffer.

Reproduced on cordoba in the same throwaway spike as the [HPET comparison](x86-clock-sources.md),
in C, with no privileged instruction of any kind. The spike counted 1.692 ns per increment and a smallest observed step of 4 increments, so 6.8 ns of usable resolution
from two ordinary threads, on a machine under load average 3.6.

This does not change the recommendation and should not be read as an argument against option 4. It
changes what option 4 is *for*, and the document already says the true thing in its
[fatal-risk section](../0139-cycle-counter-authority.md#does-this-touch-fatal-risk-7). nife makes no timing-isolation claim, and a per-thread counter grant buys comparable
measurement and accountable authority, not confinement against a program that wants to measure
time. Anything that can spawn a second thread and share memory with it reconstructs a nanosecond
clock, on aarch64 and riscv64 as much as on `x86_64`. The row that section already asks for in
`notes/confinement-claims.md` should say this, since it is the concrete reason the claim is not made.
