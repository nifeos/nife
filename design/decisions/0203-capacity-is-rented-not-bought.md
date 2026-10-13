---
status: AMENDED
raised: 2026-09-20
decided: 2026-09-20
ratified_by: calef
---

# 203. Capacity is rented rather than bought, and what each of the three benches is still for

calef, 2026-09-20, after a conversation that re-derived a hardware decision he
had already converged on elsewhere and which this tree had never recorded. *(Section number
provisional until the merge queue lands it.)*

The goal, in his words: *"unleash capacity and move the roadmap 2-3x faster"*, with the budget
freed by reducing inference spend rather than increased.

## The ruling

**Rent. Do not buy a machine.** Rented capacity is elastic, API-driven, needs no capital, and bare
metal you install your own operating system on is open enough for nife to run on it.

## Why buying lost, with the numbers that decided it

A bought machine was weighed seriously and priced against what this tree measures. Two candidates
were considered: a top-end Framework Desktop (Strix Halo, 128 GB unified, open platform, x86_64,
which calef had converged on in an earlier conversation) and a faster closed appliance. Neither
survives the goal above.

- **Inference alone does not pay for hardware.** The mechanical load is roughly 2 million tokens a
  day; hosted rates for open-weight models put that in tens of dollars a month, so a machine bought
  to avoid it takes years to repay. calef's limit is a rate limit rather than a bill (§202 (mechanical work goes to a cheaper model)), and renting relieves it this week.
- A closed appliance takes nife off its own hardware. The only reason to own rather than rent is
  a machine nife can boot, and that needs a documented boot path, not speed. An appliance with vendor
  firmware cannot deliver it at any price.
- **The machines already owned are not the constraint.** Measured 2026-09-20: cordoba is a 2013
  four-core i5-4670 with 3.6 GB of its 23 GB available, because Immich holds 15 GB serving the
  family's photos; its GPU is a GT 1030. It is doing a real job well and is not a build host. The
  actual ceiling is patagonia, an 8-core M3 with 16 GB, which is why `VERIFY_JOBS` is 4, why two
  lanes gating at once has killed a session, and why the lane count tops out at three or four.

## What runners buy, ranked by measured effect

Measured from this session's CI: a full run on real code takes 23 to 29 minutes; runs on
documentation-only branches finish in 2 to 3 because nearly every job skips.

1. Lane concurrency, which is the large one. Lanes gate locally today: three architectures of
   `script/test`, `script/verify` at about 3.5 GB a harness, mutation sweeps, all on one laptop. That
   is the three-to-four lane ceiling, the out-of-memory kill, and the 1.9 GB-free scare. Renting
   moves that work off the machine a person is also using.
2. Hardware-gated milestones become gate-able. Milestone 16 (real hardware and IOMMU-backed
   driver isolation) says its own gate is *"not the 'no board' kind: the board is on the desk. The
   remaining work needs somebody sitting at it"*. Rented metal with a power API and a serial console
   removes the person from that loop.
3. Merge throughput, which is real but third. The queue already batches up to five and group
   builds finish in minutes on ordinary changes. GitHub's concurrency and a 16 GB hosted runner that
   forces `VERIFY_JOBS=2` bind on heavy branches rather than on a typical day.

This ranking was amended on 2026-09-24 by [§214 (the Team plan buys runner
concurrency)](0214-the-team-plan-buys-runner-concurrency.md). Once lanes gated in CI, merge
throughput bound on an ordinary day, and calef upgraded the organization to GitHub Team for its 60
concurrent jobs. The ruling above is unchanged; §214 has the measurements and the new order.

Milestone 488 (a self-hosted CI runner) is refused twice and its security half is answered rather
than ignored: a runner restricted to this repository's own branches, with pull requests from
outside staying on hosted runners. That is the shape that clears the objection, and it is a condition
on any runner this decision buys.

## The three benches, ranked by what only they can do

calef's ranking, confirmed against the records:

- argon is the most valuable and cannot be replaced by rental. Milestone 127 (the seL4 machine:
  a Jetson TX1) is titled *"The seL4
  machine: a Jetson TX1, so identical silicon referees the comparison"*. Renting gives you *a*
  machine; the point is *the same* machine as the reference. `sel4bench` also times single operations
  through a PMU cycle counter that neither QEMU-TCG nor HVF provides.
- radon is next, for access rather than comparison. riscv64 silicon is hard to reach, and that
  access is the whole of the argument. `notes/visionfive2.md`'s fifth bench stop identifies radon's
  "receiver woken with nothing delivered" dumps as the terminal state of a completed tour, and
  `notes/scheduler.md` records that the gate built against it has never fired on a field failure.
  Risk 5 has no confirmed silicon-only defect, so
  radon's value here rests on reaching an architecture, not on evidence it has produced.
- xenon is the least valuable bench to sit at, and the reason is sharper than "x86 is common": a
  rented machine does its job *better*, because it comes with a power API and a console instead of
  needing a person at a null modem.

One correction to that ranking, and it is why xenon is not retired. xenon is a Core i5-7500T and
the tree holds per-machine measurements keyed to it, including the fastpath footprint at 8,404 bytes
against its 32 KB L1i, 26%. A rented machine of another microarchitecture is not like-for-like, and
re-baselining costs more than the box is worth. It is the least valuable bench to sit at, not the
least valuable record.

## The asymmetry this creates, stated because parity is a gate

Moving hardware legs to rented runners covers aarch64 and x86_64 and leaves riscv64 bench-bound
unless rental exists. Under DECISIONS §19 (architectural parity is a tenet) parity is a gate rather than an
aspiration, so a plan that
quietly covers two of three architectures reads as full coverage six weeks later. It is written here
instead.

And riscv64 rental does exist, which was checked rather than assumed. Scaleway's Elastic Metal
RV1 is a T-Head TH1520 (C910, RV64GC), 4 cores at 1.85 GHz, 16 GB, 128 GB eMMC, at €0.042 an hour or
€15.99 a month, with a serial console over SSH, command-line provisioning, and support for installing
*"the most exotic operating systems"*. It is a Labs product with no SLA.

**It is not a radon substitute and is better than one in a different way.** The TH1520 is a different
SoC from radon's JH7110, so booting nife there is a port (a UART at another address, its own device
tree, whatever its firmware hands over) rather than a switch. That makes it a second RISC-V
implementation, which is what catches an assumption baked into one vendor's silicon, and it means
baseline continuity stays with radon. The port is proposed as its own milestone rather than assumed
here.

## What is not decided

Which provider, and the spend split between inference, runners and bare metal. calef has said any of
the three architectures is acceptable, which settles the constraint this section needed.

The riscv64 provider was decided on 2026-09-25: [§215 (the second RISC-V machine is a rented
Scaleway Elastic Metal RV1)](0215-the-second-risc-v-machine-is-a-scaleway-rv1.md). aarch64, x86_64
and the spend split remain open.
