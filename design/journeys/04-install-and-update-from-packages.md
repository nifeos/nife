---
status: PROPOSED
raised: 2026-10-29
---
# 4. Install a system composed from packages, and update it from packages

*The slug is this lane's coinage and provisional; calef names things.*

calef asked on 2026-10-29 (UTC), in the maintainer's session, for this sequence saved as a journey.
The story: a person installs a nife system composed from packages, updates it from packages, and
never builds the tree to do either. calef ruled the order on 2026-10-28 (UTC): the update path goes
first, and the first repository cut waits for its conditions. `script/journeys` reads the milestone
blocks and prints each step's state; this file holds the steps, the order, and why each is on the
path.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 911 | | the approval-file gate: a scripted install grants only what the owner's approval file allows, so composition and unattended updates run with nobody at a prompt |
| 2 | 909 | | basalt builds the base packages from the pinned nife commit and composes each architecture's image by digest, 607's item 1 with basalt as the composer, as §297 (software reaches a nife system as a package) says; it waits on the dependencies 909's block names |
| 3 | 607 | | item 5 only: an OS update is a set of packages that lands through a boot slot, each slot a full copy, as U4 in §235 (the OS is built and updated from packages) rules; the block's other items are outside this story |
| 4 | 756 | | the first cut: `process-tools`, `core-tools` and `disk-tools` leave for `nifeos/process-tools`, `nifeos/core-tools` and `nifeos/disk-tools`. Condition 4 gates it: every interface the cut links carries a checkable version, and the moved package declares what it links |

The steps run in order. Step 2's composition tests each package's asks against step 1's approval
file, step 3's update lands in the slot the composed image boots from, and step 4's conditions
include basalt taking a component's output as a package by digest, which step 2 builds.

**Adjacent, not on this journey's critical path.** Milestone 910 (a package declares a service) is
what turns a package's service ask into something install starts, boot starts and update restarts;
this journey's fixtures exercise a service ask through the approval file alone, so nothing here
waits on 910. Pull request #2852 (`proposal/version-carrying-interfaces`) carried milestone 979
(interfaces a peer can check) and the block that builds step 4's gate, milestone 956 (the linked
interfaces carry checkable versions): the manifest note lists the interfaces a package links, each
with a version a peer can check. Step 4 tracks the gate through 756's block. Milestone 858 (lab
machines update themselves through packages, and only a new kernel reboots them) rides the slot
mechanism step 3 fills: its unattended updates install only within an approval file (911's bound),
on silicon, with a person present; this journey's steps run under QEMU.

## What "done" looks like

Every step above at `BUILT`. At that point a person installs a system composed by basalt from
packages, updates it through a boot slot, and builds nothing to do either. The first three packages
live in repositories of their own, with interfaces a peer can check, so a package can release
without this repository, as §151 (the goal of the repository split is independent release) rules.
