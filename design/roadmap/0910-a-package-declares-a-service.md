---
status: NOT-STARTED
raised: 2026-10-12
milestone_dependencies: 809, 849
decision_dependencies: 159, 208, unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 910. A package declares a service: install starts it, boot starts it, update restarts it

*(Minted 2026-10-12 (UTC) by lane `maintainer/mint-monit-milestone` from calef's ruling the same
day. The number is provisional until the merge queue lands it; the title, the slug and every name
below are drafts.)*

calef, 2026-10-12 (UTC), on question 2 of milestone 905 (monit is packaged in basalt, installed
with `jig`, and restarts a nife service it watches), PR #2022: *"a, and restart services on
update."* Option a was that a package may declare a service the progenitor starts at boot. monit
needs it because a supervisor tied to a session is not one, and its own `daemonize` forks.

**In brief.** A package's manifest can declare a service. `jig install` shows that grant with the
rest and starts the service. The progenitor starts it at every boot from the installed-programs table, and a
package update restarts it on the new bytes. `jig remove` and `jig rollback` undo it. Restart is
teardown and respawn by the progenitor, with no signals and no `kill`.

## What calef ruled

- The declaration. A package's manifest names the program, its arguments (monit runs as
  `monit -I`), the capabilities it needs and a restart policy.
- Install. `jig install` lists everything the install grants, including "runs as a service, now
  and at every boot", and one confirmation covers all of it. Install starts and enables the service
  by default; `jig install --no-start` opts out.
- The installed-programs table. The installer adds the service to the versioned installed-programs table, under
  §208 (installing a package is granting it, and the installed programs are versioned). The progenitor
  starts it at once and at every boot. `jig remove` stops it and drops it from the set; `jig
  rollback` returns to the previous set, services included.
- Restart. The progenitor tears the process down and respawns it from its installed-programs entry.
  A crash restarts it under the declared policy, from the death message §26 (the fault endpoint:
  thread death becomes a message a supervisor holds) already delivers.
- Reload and graceful stop. A service may declare a control endpoint that accepts those
  messages over IPC, the Fuchsia component lifecycle model. A service without one can only be
  restarted. The command that sends them is `service_control`, with the verbs `start`, `stop` and
  `reload` (ruled 2026-11-01 UTC; see Open questions for the reasons).
- Configuration. The package ships a read-only default, and the owner's configuration lives in
  a writable directory granted to the service.
- Update. A package update is a new installed-programs version, and the progenitor restarts the
  affected services on it automatically. `jig` lists which services will restart before the
  confirmation, which is §159 (lab machines upgrade like user machines, and only a new kernel needs
  a reboot) applied to an optional package.

## What exists, and who else touches this

Checked 2026-10-12 (UTC). Nothing on the roadmap or in the proposal pile owns a declared service.

- The progenitor builds the boot's own services from literals in `crates/system_initializer`, by
  archive entry. Nothing starts a program from the installed-programs table at boot, and nothing restarts a
  service that dies: milestone 854 (the init package holds only what init does) found no real boot
  starts `root_supervisor` or `sub_server_supervisor`.
- Milestone 858 (lab machines update themselves through packages, and only a new kernel reboots
  them) needs this for base services. Its worklist items 2 and 7 are unminted: the progenitor
  starts every base service from the live generation, and a supervisor restarts a service from new
  bytes. This milestone builds the mechanism for a package's declared service, and item 7 now
  says it extends it to the base. 858 waits on silicon and a person; this does not.
- Milestone 849 (a package declares what it needs at run time, apart from what it links) owns how
  a foreign package says what it needs. The service declaration is a sibling field, so it rides
  that work rather than a second encoding.
- Milestone 809 (the package client becomes a program, `jig`, with the verbs an index needs) owns
  `install`, `remove` and `rollback`. §270 (a package manager holds an installer endpoint, not
  the spawn endpoint) keeps the starting in the progenitor.

## The install confirmation

Added 2026-10-12 (UTC). The 892 lane found (#2027) that no milestone builds `jig install`'s
confirmation, the list of everything an install grants under §208. This milestone's service line
needs it. So does milestone 892 (a std program at the prompt holds the network and a name
resolver): calef ruled (c) there, that the manifest asks for resolver zones and the owner grants
them at install. So it is built here, once, for every grant:

- Each grant on its own line: the service ("runs as a service, now and at every boot"), each
  resolver zone, each directory with its rights, and every other capability the manifest asks for.
- A root-zone ask, which can resolve any name, is flagged apart rather than listed like the rest.
- One confirmation covers the list, and what was confirmed is kept with the installed-programs entry.
  That record is the one the scripted-installs milestone in flight (#2026, approval files) reuses,
  so an approval file answers the same list a person would.

## The work

1. The declaration in the manifest, beside 849's run-time needs: program, arguments, grants,
   restart policy (`never`, `on-crash` with a limit and backoff), optional control endpoint.
2. The install confirmation above, and its record. The installer records the service in the
   installed-programs table's entry, with `--no-start`.
3. The progenitor starts every enabled service in the live generation, at once on install and at
   every boot, each under a supervision endpoint the progenitor holds.
4. Teardown and respawn on a crash, by policy, and on an update, for each service whose package
   changed; `jig` prints the list before it asks.
5. `remove` stops and drops; `rollback` restores the previous set and restarts what changed.
6. The `service_control` command and the control endpoint's messages: `reload` and `stop`, a stop
   that falls back to teardown, and `start` for a service that `--no-start` left stopped.

## Done when

On aarch64, riscv64 and x86_64 under QEMU, in `script/swish-check`, with a fixture service package
that reports its version:

1. `jig install` of the fixture lists each grant: "runs as a service, now and at every boot", a
   directory with its rights, and a resolver zone. After one confirmation the fixture is running,
   as `ps` shows over its domain, and the confirmed list is in its installed-programs entry.
2. A second fixture whose manifest asks for the root zone is flagged apart in its confirmation.
3. After `reboot` (milestone 805 (`reboot` at the prompt: a person restarts the machine without
   touching its power)), it is running again with no command typed.
4. The fixture faults on request; it is running again under a new tid, by its policy.
5. Installing version 2 lists the fixture among services that will restart; afterwards it reports
   version 2.
6. `jig rollback` leaves it running and reporting version 1. `jig remove` leaves it stopped, and
   after a reboot it is still stopped.

## Open questions

Name: ratified 2026-11-01 (calef, ruling on this block). `service_control`, with the verbs `start`,
`stop` and `reload`. The name is a software name, so the BSD-first rule for hardware register,
field and constant names does not apply; it says what the command does and follows the tree's own
pattern. `jig install --no-start` leaves a service stopped and the block named no way to start it
later. `start` goes to the progenitor, which holds the supervision endpoint (§270 keeps spawning
out of `jig`). A stop that falls back to teardown also goes to the progenitor. So the command is a
progenitor client in any case, and `start` adds no new kind of authority. `restart` is not a
verb: restart is the progenitor's teardown and respawn on a crash or an update. `enable` and
`disable` are not verbs: boot persistence belongs to the installed-programs table under §208.

The restart policy's spellings and limits are drafts for the lane to propose.

BUGS. Whether `jig install --no-start` also clears the enabled flag for boot is not settled by this
block. The lane that builds 910 asks before choosing: a service that is stopped now but enabled
would start again at the next boot.

Reuse: the policy takes `sub_server_supervisor`'s restart loop, from milestone 22 (trusted init:
verify it, and shrink what a broken one can do). The set and its generations are §208's, and the
encoding is 849's. systemd units, launchd property lists, s6 and Fuchsia's component manager were
the models read for the declaration and the lifecycle; none runs here. The shape taken is
Fuchsia's: the manager tears down and restarts, and a component may serve a lifecycle protocol.

## Index row

The missing half of a package manager for long-running programs: a package can say "I am a
service", and install, boot, crash, update and rollback each start, restart or stop it from the
installed-programs table, with no signals.
