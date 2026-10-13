---
status: REFUSED
raised: 2026-09-20
refused_by: 49, 448
---
# 480. A server that logs which channel a request arrived on

Refused by milestone 49 (design/roadmap/0049-users-and-attribution.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '49. Users, login, and attribution: what identity is for once it stops being authority', under
`## Follow-on`:

> The second half of DECISIONS §109, a server logging which channel a request arrived on. No
> server in this tree needs it: every multi-client server either serves exactly one principal by
> construction (`fs_subtree_caretaker`) or is anonymous by design (the credential service).
> Building it now would be a mechanism with no consumer to shape it.
>
> -- design/roadmap/0049-users-and-attribution.md

## Why it is here rather than only there

This is the second half of §109 (attribution is a property of a channel, not of a capability), a decision this tree made and built only one half of. A server that
serves several principals could attribute a request to the channel it came in on, which is what
makes an audit trail possible without trusting the caller. The survey found no such server: every
multi-client server here either serves exactly one principal by construction, as
`fs_subtree_caretaker` does, or is anonymous by design, as the credential service is.

## Revisit

- **Condition.** A multi-client server that serves more than one principal and is not anonymous by
  design. The refusal's argument is that building it now would be a mechanism with no consumer to
  shape it, so the first such server is both the trigger and the specification.

## Index row

Half of a recorded decision, deliberately unbuilt because the survey found no server that could use
it. The condition is a consumer, and the refusal is explicit that the consumer would shape the
mechanism rather than merely justify it.
