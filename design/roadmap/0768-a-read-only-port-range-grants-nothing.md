---
status: BUILT
raised: 2026-10-05
built: 2026-10-17
promoted_from: a-read-only-port-range-still-drives-the-hardware
milestone_dependencies: none
decision_dependencies: 121
machine_requirements: none
specific_machine: none
needs_person: no
---
# 768. A read-only port range grants nothing

Raised 2026-10-05 (UTC) by `lane/633-outsider-2`, milestone 633 (an outside agent attacks the
confinement claim)'s second pass, and promoted from `design/roadmap/proposals/` by
`lane/port-rights-fix` the same day. The number 768 is provisional until the queue lands it.
*(Title, slug and every name below are drafts.)*

Reuse: the rights check is the one every other object's system call arm already performs
(`cap.rights.allows(Rights::WRITE)` in `kernel/src/system_call.rs`), applied at the grant's single
insert site; the test reuses `x86_port_tests::build_child` and `x86_programs::port_out_then_exit`.
`cycle_counter_grant` was considered as a model and rejected, because it gates a read-only counter.

## Index row

On x86_64 a `PortRange` capability without `WRITE` opens no I/O port. A port capability's rights
now decide what the TSS bitmap grants, and a test and a replayable falsification keep it so.

## The defect

`sched::thread_control_block_insert_from` installed `Thread::port_range_grant` for any `PortRange`
object and never read `cap.rights`, so a capability narrowed to `READ`, or to no rights, drove the
hardware exactly as a `WRITE` one did. `component_plan` maps `Serve` to `READ` and `Use` to
`WRITE`, so a supervisor handing a component an observe-only view handed it the device.

## Decided

calef ruled on 2026-10-05 (UTC), answer 1 of the proposal, quoted: "grant nothing without WRITE
since we can't grant READ without write"
([comment on #1687](https://github.com/nifeos/nife/pull/1687#issuecomment-5998739348)). The TSS
I/O bitmap has one bit per port and cannot permit `in` without `out`. Refused: declaring a port
range a rights-free object (makes `READ` on it a lie a reader must know about). Also refused: splitting it
into an `in`-only and an `out`-only kind (needs trapping every port access, the cost the bitmap
exists to avoid).

## Acceptance

- [x] The install site grants only when the capability allows `WRITE`.
- [x] `x86_port_tests::a_read_only_port_capability_must_not_grant_port_output` passes by default
  (it was opt-in and red), with a replayable falsification.
- [x] Claim 33 in `notes/confinement-claims.md`.
- [x] The ruling recorded in `design/decisions/` as an amendment to §121 (what a device capability is when the device has no page: x86 port I/O).

## Follow-on

- **Done.** The §121 amendment recording the ruling: `design/decisions/0121-port-io-capability.md`, section "A port range grants only with WRITE".
- **Recorded.** A thread handed two port ranges keeps only the last, in `kernel/src/sched.rs`
  beside `thread_control_block_insert_from`.
