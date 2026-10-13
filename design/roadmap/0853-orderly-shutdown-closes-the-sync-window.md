---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: orderly-shutdown-closes-the-sync-window
milestone_dependencies: 805
decision_dependencies: 251
machine_requirements: none
specific_machine: none
needs_person: no
---
# 853. Orderly shutdown closes the sync window

*(The title and slug are provisional.)*

Reuse: the design is borrowed from the prior art below; no code is taken, because nothing
outside a service manager can own the order, and nife's would be written here.

calef asked for this on 2026-10-06 (UTC), ruling on #1783, milestone 805 (`reboot` at the prompt).
He approved the sync-only binding (option A in §251 (restarting the machine is a kernel object the
progenitor hands out)'s amendment) and asked that option D be filed as follow-on work.

## The gap

`reboot` sends `fs::SYNC` on its sync-only capability, waits for the reply, then invokes the reboot
object. A write another job makes after that reply and before the reset is lost on a device with a
volatile write cache. `components/src/reboot.rs`'s `BUGS` records it. The window is short, and it
only holds a write from a job somebody left running in the background, but no sync issued by one
program can close it. Something has to stop the writers first.

## What closes it

An orderly shutdown. Before the reset, the reboot path tells every stateful server to stop taking
writes, sync and acknowledge. Only after every acknowledgement does it invoke the reboot object.
That is the only option in §251's amendment that closes the window rather than narrowing it.

It needs an owner for shutdown order, and neither `reboot` nor the progenitor is the right one.
`reboot` should not know which servers exist. The progenitor holding that list is the god-process
drift calef ruled against on the same pull request. The owner is a service manager: the process that
knows what was started, in what order, and what depends on what. nife has none yet. Its supervision
tree (`root_supervisor`, `sub_server_supervisor`) restarts what dies and does not order a stop.

So this proposal has two halves, and the first may be its own milestone:

1. A service manager owns start and stop order for the stateful servers (the file server, the
   block servers, the system log once it is durable).
2. Shutdown is a request to it. `reboot` asks it to stop the system, it stops servers in reverse
   dependency order and waits for each to sync and acknowledge, and then the reboot object is
   invoked. The sync-only capability stays as the last step, or goes, depending on whether the
   file server's own stop includes the sync.

## Does publish-subscribe fit?

calef asked on 2026-10-06 (UTC) whether publish-subscribe, common in distributed systems, applies
here. It does, in one shape: publish-subscribe with acknowledgment, a deadline and ordered phases.
Plain fire-and-forget fails three ways:

- The publisher must know every subscriber finished before the reset, and a broadcast says
  nothing back.
- The file system must sync after the writers stop. A flat broadcast cannot order that.
- A hung subscriber must not block the reboot forever.

The capability mapping:

- Subscribing is holding a badge on a shutdown notification, which the service manager hands out.
- Publishing needs the reboot capability. `reboot` publishes, waits for every acknowledgment or the
  deadline, then resets. Neither `reboot` nor the progenitor knows who holds state.
- Phases stand in for a dependency graph: for example 1, writers stop; 2, servers sync; 3, the
  reset. A subscriber declares its phase.

Prior art for this shape, recalled, not re-read:

- Kubernetes sends `SIGTERM`, waits `terminationGracePeriodSeconds`, then `SIGKILL`, with `preStop`
  hooks: a broadcast plus a deadline.
- Windows sends `WM_QUERYENDSESSION`, then `WM_ENDSESSION`, with timeouts. That is two phases, and
  the veto in the query phase is mostly grief.
- Android broadcasts `ACTION_SHUTDOWN` and waits a few seconds. It is pure fire-and-forget, and apps
  lose data.
- Linux's kernel reboot notifier chain is priority-ordered and synchronous: priorities as ordering.
- macOS `launchd` sends `SIGTERM` to everything, waits about 20 s, then `SIGKILL`.
- systemd and Fuchsia's `component_manager` stop in dependency order: ordering without
  publish-subscribe, which needs a service manager that knows the graph.

## Prior art for the ordering (recalled, not re-read)

- Fuchsia's `component_manager` stops components in dependency order, and `fshost` flushes on
  `fuchsia.process.lifecycle` `Stop` before power control resets.
- systemd stops units in reverse dependency order, remounts file systems read-only, syncs, then
  resets.

## Open questions for whoever builds it

- Who may ask the service manager to stop the system, and is that the reboot object's holder or a
  separate capability?
- What a server that does not acknowledge in time gets: a timeout and a forced reset, or a refusal.
- Whether power-off (excluded from milestone 805) rides the same request.
- Checked at promotion against milestone 850 (a program is asked to give memory back before anyone
  is refused): that signal is advisory, coalescing and unacknowledged. Shutdown needs every
  server's acknowledgement in order, so this stays its own protocol rather than one more level on
  that mechanism.

## Index row

Orderly shutdown closes the sync window `reboot` leaves open: a service manager owns stop order, `reboot` asks it to stop the system, servers stop writers, sync and acknowledge, and only then is the reboot object invoked. A hung server meets a deadline, not a hang.
