---
status: NOT-STARTED
promoted_from: every-supervisor-dispatches-on-known-event-codes
raised: 2026-10-19
milestone_dependencies: none
decision_dependencies: 300
machine_requirements: none
specific_machine: none
needs_person: no
---
# 938. Every supervisor dispatches on the event codes it knows

Raised by the 2026-10-19 documentation sweep
([decisions read forward](../audit-reports/2026-10-19-decisions-read-forward.md), finding 12).

[§300 (the paused state, a TCB verb, silent)](../decisions/0300-tcb-suspend-resume-the-paused-state.md)
keeps a stopped event out of the message of §26 (fault endpoint). It rests that on a rule:
unknown event codes are never acted on, so a later `EVENT_STOPPED` is additive rather than a
migration across every supervisor.
The section says every consumer already dispatches on `EVENT_FAULT` and `EVENT_EXIT`. Two do not.
`components/src/job_undertaker.rs` collects the tid on any event it receives, and
`components/src/sub_server_supervisor.rs` reaps on any event and reads every code that is not
`EVENT_FAULT` as a clean exit. Either would reap a paused child as dead the day a third code exists.

Nothing is wrong today, because the kernel sends only the two codes. The work has three parts.
Enumerate every receiver of the supervision endpoint (`receive_fault` and its peers). Decide per
consumer what an unknown code does; ignoring it and receiving again is the reading §300 implies,
and a consumer that cannot ignore one says why. Then add what keeps it true, ideally one shared
dispatch in `supervision_protocol` that a consumer cannot bypass, so the rule is a type rather than
a review.
The correction to §300's sentence about the tree is the integrator's.

Reuse: none exists; the dispatch is on this tree's own §26 wire, and `supervision_protocol` is where the shared shape would live.

## Index row

§300 rests on supervisors ignoring event codes they do not know, and two of them (`job_undertaker` and `sub_server_supervisor`) act on any code. Each would reap a paused child as dead once a third code exists, so the rule needs a shared dispatch a consumer cannot bypass.
