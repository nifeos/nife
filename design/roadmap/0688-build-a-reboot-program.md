---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: build-a-reboot-program
milestone_dependencies: none
decision_dependencies: 243
machine_requirements: none
specific_machine: none
needs_person: no
---
# 688. Build a reboot program

*Title and slug are drafts.*
Raised by the lane for §243 (notices for people), pull request #1424. Its own worked example, a
reboot warning shown on every session, has no publisher: "No program reboots the machine" today
(§243, "What the tree has today"). calef ruled §243's six questions on 2026-09-27 (UTC), so this
program now has somewhere to publish once it exists.

## The finding

§243's capability shape (Q3, decided) already names this program's badge: whoever spawns it
registers "every display, may bypass" with `notice_board`, the one bypass right §243 reserves for
critical urgency. Nothing in the tree today initiates a shutdown or a restart. This proposal is
scoped to the program that would hold that badge and publish the warning §243 works through end to
end, not to the reboot mechanism itself (what actually powers the machine down or restarts it). That
mechanism is a separate finding for whoever picks this up.

## The shape

- Publishes class `system.shutdown`, key `reboot`, urgency critical: text such as "Rebooting at
  14:05 UTC", republished under the same key as the deadline nears, withdrawn by a cancel. §243's
  worked example has the full sequence.
- Its badge is registered by whichever spawner starts it. If that spawner is the progenitor,
  §243's "What it costs" already prices the slot and names the blocker: the table is at 23 of 24
  once the system log service takes its own slot. A progenitor-spawned reboot program therefore
  waits on pull request #1360 raising `CAPABILITY_TABLE_SLOTS` to 32.
- The reboot mechanism needs its own finding: a spawner, and a trigger (a command, a timer, or
  both).

## What it unblocks

§243's first worked example, a reboot warning shown on every display, gets a real publisher
instead of a hypothetical one.

Name unminted; "a reboot program" describes it rather than names it.

## Index row

No program reboots the machine, so DECISIONS §243's worked example, a reboot warning shown on every session, has no publisher. Proposed: the program that holds the critical-urgency bypass badge and publishes it.
