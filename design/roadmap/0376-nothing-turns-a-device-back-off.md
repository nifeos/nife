---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: none
decision_dependencies: 304
machine_requirements: riscv64 silicon (the JH7110 STG domain is the measured thing)
specific_machine: radon (the JH7110 board; the measurement is its power draw)
needs_person: yes
---
# 376. Nothing turns a device back off

Filed as a proposal on 2026-09-04 by the milestone 220 lane, which wrote
the first code in this tree that turns a device on and deliberately did not write its inverse.
It was promoted by milestone 433 (drain the proposal pile to zero) on 2026-09-19. Checked against the tree that day and nothing has moved.
`crates/jh7110_clock_and_reset` exposes `discover`, the offset and mask arithmetic, and the two
read-back predicates (`clocks_running`, `was_already_up`), with no path that gates a clock or
asserts a reset. `kernel/src/drivers/jh7110_clock_and_reset.rs` still offers no teardown. The
gate is decided:
[§304 (who may turn a device off)](../decisions/0304-who-may-turn-a-device-off.md), 2026-10-15:
measure before building. This milestone's next work is the bench run, radon with the STG domain
ungated and gated, the power difference recorded. What gets built, if anything, is considered
against that number. The kernel-reclaim shape (per-job off
[§40 (a supervisor's death is its subtree's death)](../decisions/0040-no-reaper-of-last-resort.md),
refcounted per shared line, no new surface) is the named candidate. No capability variant
appears without a driver that needs one.

The mechanism is small; the authority question waits for the number. §304 leaves a driver-asks
capability undecided, and what counts against it is a cost rather than a refusal: surface in the
category [§10 (process model: capability-based, microkernel)](../decisions/0010-capability-microkernel.md)
and [§16 (object revocation)](../decisions/0016-object-revocation.md) keep narrow. Nothing is built
before the number.

What the work is. `crates/jh7110_clock_and_reset` can enable a clock and release a reset. It cannot gate a
clock or assert a reset, and `kernel/src/drivers/jh7110_clock_and_reset.rs` offers no teardown at all. So a
driver process that exits, faults, or is revoked leaves its device clocked and running forever, and
nothing in the system can reclaim that. On a board with one TRNG this costs nothing measurable. On
the same `SoC`'s USB, PCIe and DMA blocks it is the whole of device power management.

Why the lane refused to just add it. Asserting a reset is not the mirror image of releasing
one. The JH7110's TRNG reset is documented as shared upstream
(`devm_reset_control_get_shared`): the same line resets the PL080 DMA engine at `0x1600_8000`. So
"turn my device off" is, in the hardware, "reset a block my neighbour is also using", and a
teardown written without that in view would interrupt a stranger mid-transaction. The refcounting
Linux gets for free from its clock framework does not exist here.

The three questions it has to answer, and they are the interesting part:

1. **Who may gate a clock?** Milestone 220 kept the whole controller in the kernel on the argument
   that granting it would *widen* a driver's authority rather than confine it (the STG window
   covers USB, both PCIe root ports and the DMA engine). A teardown that a driver can *ask* for is
   a different question from a controller a driver can *hold*, and the first may well be
   answerable without new system call surface: the kernel already knows when a spawned service dies.
2. What is the unit of reclamation? Per-device, if something refcounts shared lines. Per-job,
   if it hangs off §40's subtree death, which is where §92's caretaker-lifetime question landed
   for an analogous reason. The second is more elegant and needs the refcount anyway.
3. **Is it worth anything before there is a power budget?** This tree measures rather than
   argues, and nobody has measured what an ungated STG domain costs radon. A milestone justified
   by tidiness rather than by a number would be the *implementation convenience* tenet running
   backwards.

**What is already recorded, so this proposal is not the only trace.**
`notes/jh7110-clock-and-reset.md`'s `BUGS` carries the limitation beside the feature, which is the
FreeBSD posture working as designed: a reader who meets the driver meets the fact that it has no
off switch. This proposal exists because the *decision* about authority has no home in a `BUGS`
entry.

Related. DECISIONS §86 (whether an NVMe driver can leave the kernel, and what capability would
let it) is the argument milestone 220 reused and the one this would extend. §40's subtree death is
the reclamation mechanism question 2 points at.

## Index row

`crates/jh7110_clock_and_reset` can enable a clock and release a reset and cannot do either in
reverse, so a driver process that exits, faults or is revoked leaves its device clocked and running
forever with nothing able to reclaim it. On a board with one TRNG that costs nothing measurable; on
the same SoC's USB, PCIe and DMA blocks it is the whole of device power management. The lane
refused to just add the inverse, for a reason in the hardware: the JH7110's TRNG reset is documented
as shared, the same line resets the PL080 DMA engine, so "turn my device off" is "reset a block my
neighbor is using". The refcounting Linux gets from its clock framework does not exist here.
Three questions make it calef's. Who may gate a clock, since a teardown a driver can ask for is a
different question from a controller a driver can hold and the first may need no new system call
surface. What the unit of reclamation is, per-device with a refcount or per-job off §40's subtree
death. And whether it is worth anything before somebody measures what an ungated STG domain costs
radon, because a milestone justified by tidiness rather than a number is the tenet about
implementation convenience running backwards. §304 ruled the third question on 2026-10-15: measure
first. The bench run is this milestone's next work and needs a person at the board; what gets
built is considered against the number, and no capability variant appears without a driver that
needs one.
