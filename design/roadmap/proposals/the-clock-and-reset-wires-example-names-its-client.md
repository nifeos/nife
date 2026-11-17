---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: none
decision_dependencies: 320
machine_requirements: none
specific_machine: none
needs_person: yes
---
# §320's example names who starts the clock and reset server and calls it

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-radon-device-authority`, as finding 5 of the
security audit [radon's device authority](../../audit-reports/2026-11-17-radon-device-authority.md).

Reuse: nothing new; the change edits one paragraph.

## The problem

§320 (the JH7110 clock and reset wire)'s concrete example says the progenitor starts the server
and badges its endpoint to start the GMAC's control process. In the tree the kernel does both:
`kernel/src/user/jh7110_clock_and_reset_service.rs` spawns the server, keeps the endpoint in a
kernel static, and calls it through `sched::ipc_call_badged` from the kernel's starters. No program
holds a capability on the endpoint. A reader judging the server's exposure from §320 would place a
userspace client on it that does not exist. The section is an architect's to edit, so a lane only
proposes it.

## The work

- The example names the kernel as the server's spawner and its only client today, or says that the
  progenitor is the client milestone 1062 (the device control planes leave the kernel) intends, if
  that is the ruling.

## Done when

§320's example and the server's spawner agree on who calls the server.

## Index row

§320's example names who starts the clock and reset server and calls it.
