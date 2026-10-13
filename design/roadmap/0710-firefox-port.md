---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: firefox-port
milestone_dependencies: 809, 801
decision_dependencies: 131, 145
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 710. Porting Firefox to nife

*Title and slug are drafts.*
Raised by calef's ruling below. It builds nothing. It is the tracked home for the browser question
so that §131 and §145's "out of reach" has somewhere to point. The file name is provisional.

## calef's ruling

2026-10-03 (UTC): "We will need a web browser some day and it is logical one to go after. However
it is low on the priority stack. We're targetting service workloads first."

**This is low priority. Service workloads come first. No lane is to be started on it until calef
says so.** The only work this proposal licenses today is the measurement under "First step", and
that waits for the same word.

## What it reopens

- [§131 (hold at rung two)](../decisions/0131-hold-at-rung-two.md): rungs three and four of
  [the display ladder](../display-ladder.md) reopen the parked competitor question, and calef
  held at rung two on 2026-08-26. A browser needs rung three (real applications, software
  rendering; WebRender has a software fallback, from general knowledge). Rung four, GPU through
  virtio-gpu Venus, is optional and would only make it faster.
- [§145 (compartmentalization at process cost)](../decisions/0145-compartmentalization-at-process-cost.md),
  which names Firefox as the software people actually run Qubes for and puts it out of reach. That
  sentence is what this proposal revisits.

## The gap beyond graphics

Every item below is unmeasured and from general knowledge.

- A POSIX-style platform: threads, `mmap` with protection changes, signals.
- Firefox's multi-process model and its IPC (shared memory, sockets, handle passing). What a
  process costs is §145's concern, and the 66 microsecond `spawn_el0` figure there is the only
  number we have.
- JIT executable memory against W^X and capability confinement. Firefox runs with the JIT disabled,
  much slower.
- Networking and TLS, fonts, audio.
- Cross-compiling the build, including its Rust and C++ dependency tree.

## First step: a measurement, before any build work

Inventory the syscalls and libc functions a headless Firefox build uses, and diff them against what
nife's std PAL and libc layer provide.

- The question it answers: how large the platform gap is, in counted functions.
- What it would decide: whether the port is one milestone or a program of milestones.

Until it is run, nothing here is an estimate.

## Done when

Under §297 (software reaches a nife system as a package `jig` installs from basalt's index, the
kernel included); calef, 2026-10-12 UTC: "They should all be installed via jig through basalt."

Firefox must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). No work toward that starts
until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block start
on packaging.").

## Index row

calef ruled that a web browser is logical to go after some day but low on the priority stack, behind service workloads. Proposed: nothing built; the tracked home for the browser question, with one measurement that waits on calef's word.
