---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: a-userspace-program-cannot-learn-which-cpus-are-online
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 564. A userspace program cannot learn which cpus are online

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `a-userspace-program-cannot-learn-which-cpus-are-online` on 2026-09-22, filed 2026-09-21. Raised by the `SURVEY` selector lane, which shipped a record
reporting the cpu a thread was placed on. It found that a reader has no way to interpret the set of
ids it collects. Nothing in this tree gives userspace `smp::online_harts_mask` or anything derived
from it.

A lane could start today. What it needs first is a ruling on which mechanism carries
it, which is what the fork below is for.

## The gap

The kernel knows exactly which cpus are online, as a bitmask, and `crates/cpu_set` exists because
**that set is not `0..n`**. On QEMU `virt` it is contiguous from zero by coincidence. On the
VisionFive 2 it is `{1, 2, 3}`, since slot 0 is an M-mode monitor core with no MMU. Treating the
count as an index put `init` into a parked core's inbox on first silicon. It took three boots to
diagnose.

Userspace has none of it. Not the mask, not a count, not a way to ask. A program that wants to reason
about cores has exactly two options today, and both are wrong. It can hardcode a number, or infer the
set from what it happens to observe.

## Why it became worth writing down now

The placement record makes the temptation concrete rather than theoretical. A userspace supervisor
tallying placements over a domain gets a correct census. The ids it observes are by construction a
subset of the online set. What it cannot do is show an online core with nothing on it. Nor can it
tell that case apart from a core that is not online.

The obvious workaround is the bug. `for cpu in 0..count` is exactly the shape `cpu_set` was written
to kill. A reader who reaches for it will be right on the emulator and wrong on the board. So the
gap is not merely a missing convenience. It is a missing fact whose absence pushes readers toward a
known failure.

## The fork a lane must bring to calef rather than settle

**This is not a survey record**, and that much the selector lane did settle. A survey answers
questions about a *domain*, and the online set is a fact about the *machine*. Smuggling it out as a
per-thread record would repeat the same value on every entry. It would also put a machine fact behind
a capability that names a supervision subtree.

What remains open is which mechanism does carry it, and the candidates are not equivalent:

- A method on some existing machine-facing capability, which keeps the no-ambient-authority
  property. A program learns the topology because somebody handed it the right to ask.
- A read-only page, on the shape calef's 2026-09-21 selector ruling chose for a thread reading
  its own cpu. Cheap. The objection that ruling raised against a mapped statistics page
  (ambient authority outliving revocation) is weaker here, because the online set is not a secret
  and barely changes.
- Part of whatever a program is already told about its machine at start. The set changes so
  rarely that a value handed over once may be the honest shape. That makes this a question about
  process startup rather than about a new method.

Each is a system-call-surface or wire decision, which is the irreversible category. So a lane brings the
costs and calef picks.

## BUGS

- "Online" is not a constant, even though this tree currently treats it as one. Nothing here
  brings a cpu up or down after boot, so a value handed over once is correct today. A mechanism that
  bakes that assumption into a wire is a mechanism that has to be un-shipped if hotplug ever
  arrives. Whichever option is chosen should say which of the two it is promising.
- This is not affinity, and the distinction matters because the two look alike from userspace.
  Whether a program may *choose* where its threads run remains open. Reading the topology is
  explicitly not a step toward it. seL4 refuses migration by design and this kernel does not
  implement it.

## Index row

The kernel's online cpu set is a bitmask that is `{1, 2, 3}` on real silicon and `0..n` only on
QEMU, and userspace cannot see it at all. The placement record makes that absence load-bearing.
The workaround a reader reaches for is exactly the range that `crates/cpu_set` exists to refuse.
Not a survey record (a survey answers about a domain, this is about the machine), so which
mechanism carries it is a system-call-surface fork for calef.
