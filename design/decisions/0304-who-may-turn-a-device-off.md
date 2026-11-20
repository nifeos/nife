---
status: DECIDED
raised: 2026-10-15
decided: 2026-10-16
ratified_by: calef
---

# 304. Who may turn a device off, and what the unit of reclamation is

Raised by [milestone 652 (twelve `DECISION` gates cite only sections that are already ruled)](../roadmap/0652-twelve-decision-gates-cite-only-ruled-sections.md). Milestone 376 (nothing turns a device back off) was filed by
[milestone 220 (this kernel drives no clock or reset controller)](../roadmap/0220-jh7110-clock-and-reset.md)'s
lane. That lane wrote the first code in this tree that turns a device on, and deliberately did not
write its inverse. A driver process that exits, faults or is revoked leaves its device clocked and
running forever, and nothing in the system can reclaim that.

## The decision

**Measure before building.** The next work under milestone 376 is the bench run. Radon runs with
the STG domain ungated and gated, and the power difference gets measured and recorded; nothing in
this tree turns a device off until that number exists. Nothing is built now.

What the measurement decides is open on purpose. The kernel-reclaim shape is the candidate, not a
commitment. The shape: a spawned job's enables are recorded where they are made, and the kernel
undoes exactly those enables at subtree death, refcounted per shared line, with no new system call
surface. If the cost of an ungated domain is real, that is the shape to build. The ruling says
"then consider", and consider is what happens.

One thing the ruling settles: measure first, on radon, and nothing in this tree turns a device off
until the number exists.

## What this tree already does in the analogous case

Milestone 220 kept the whole clock-and-reset controller in the kernel. The recorded argument:
granting it would widen a driver's authority rather than confine it. The STG window covers USB,
both PCIe root ports and the DMA engine, not one device. The crate
(`crates/jh7110_clock_and_reset`) exposes `discover`, the offset and mask arithmetic, and the two
read-back predicates. It offers no gate and no assert. Its own doc for the TRNG reset line says
why the asymmetry is the silicon's. The line is *shared* (`devm_reset_control_get_shared`
upstream): the same line resets the PL080 DMA engine. The doc's words: "deasserting it is safe;
asserting it would reset a neighbor, which is why nothing in this crate offers an assert". Linux's
clock framework buys its way out with refcounting this tree does not have.

[§86 (whether an NVMe driver can leave the kernel)](0086-el0-nvme-driver.md) is the tree's own
precedent for the shape of the answer. Its words fit: "the irreversible part of this decision is
not in 2a. It costs zero new system call surface". The choice between the cheap answer and the
capability was "settled by measurement". Prior art, read rather than recalled. The shared-reset
fact and the refcounting are recorded in `notes/jh7110-clock-and-reset.md`, from Linux's source.
The note's `BUGS` section carries the limitation beside the feature: "nothing here can turn a
clock off or assert a reset, deliberately. ... It also means there is no teardown: a service that
dies leaves its device clocked."

## Why measure first

The block's own reasoning, which is three facts rather than a mood. The teardown a driver asks
for is a different question from a controller a driver holds, and the first has no consumer in
this tree. The per-job unit is the more elegant of the two reclaim shapes. It needs the refcount
anyway, and so does the capability's mechanism. The kernel-reclaim shape contains what the
capability would have needed. And a milestone justified by tidiness rather than a number is the
implementation-convenience tenet running backwards. The effort test, stated because the skill
asks. The kernel-reclaim shape is more work than the capability's minimal ask, and it would still
be the choice. It spends reversible bookkeeping on a path the kernel already walks at every death,
where the capability spends irrevocable surface on nobody.

## The options

| | shape | what it costs |
|---|---|---|
| **A** | A driver may ask: a capability variant on the system call surface for gating and asserting. | Not decided; waits for the measurement. New surface in the category §10 and §16 keep narrow, granted against a window that reaches devices the holder has no business resetting. |
| **B** | The kernel reclaims at job death: a spawned job's enables are recorded where they are made, and the kernel undoes exactly that job's enables, refcounted per shared line, when its subtree dies. | The candidate, not a commitment. Bookkeeping in the kernel's controller path and a refcount, and no new surface at all; the driver never learns that power management happened. |
| **C** | Measure before building: radon runs with the STG domain ungated and gated, and the number decides whether anything downstream of this question is worth a milestone. Chosen. | A bench session and a person at the board, and the honest possibility that the answer is "not yet worth it", which is a real answer. |

## Whether the premise is true

Checked 2026-10-15. The crate still offers no gate and no assert, and the shared-reset doc still
says why. The note's `BUGS` still carries the no-teardown limitation. Nothing in `design/decisions/`
took up who may turn a device off before today. The three questions were open exactly as the block
left them. The ruling answers the third: measure. The first two wait for the number.

## How reversible it is

The measurement is a bench run. The kernel-reclaim shape is bookkeeping on a path that already
exists, removable without a trace. The capability is the system call surface: the one irreversible
thing in this question, and the part with no consumer. The ruling takes the options in
reversibility order. The surface stays out of reach until a measured number and a real driver both
ask for it.

## What this leaves open

- **Whether a capability variant appears.** The driver-asks shape (option A) is not decided. The
  measurement, and then calef's call, decide it. What counts against it today is a cost, not a
  refusal. It spends irrevocable surface on a consumer that does not exist, since no driver in this
  tree power-manages a device the kernel's controller did not enable. That surface is in the
  category [§10 (process model: capability-based, microkernel)](0010-capability-microkernel.md) and
  [§16 (object revocation)](0016-object-revocation.md) exist to keep narrow.
- **The unit question stays open, with its lean recorded.** Per-device with a refcount, or per-job
  off [§40 (a supervisor's death is its subtree's death)](0040-no-reaper-of-last-resort.md)'s
  subtree death. Decided when something is built. The lean is per-job. The reason is the one
  [§92 (a caretaker is supervised by the client it serves)](0092-caretaker-lifetime.md) landed the
  same way. §40's subtree death is the one reclamation hook that already fires. Both units need
  the shared-line refcount anyway.

The build decision stays open, explicitly. Kernel reclaim at job death, versus nothing: considered
against the measured number. The per-job unit is the recorded lean. The shared-line refcount is
needed either way. The `BUGS` entry beside the driver stays either way, since it records a fact
about the hardware the crate serves.
