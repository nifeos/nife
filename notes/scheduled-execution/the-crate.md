# The timetable crate: the detail behind its doc comments

An appendix of [scheduled execution](../scheduled-execution.md). It holds the facts that
`crates/timetable`'s doc comments point at and have no room to carry. Each section names the module
it serves.

## The four answers, by example

`lib.rs` names the four answers registration gives. One entry each, against a scheduler that holds
only a budget:

| answer | example entry | why |
|---|---|---|
| `Admission::Fires` | `every 5s least_authority_demo 7` | planned, backed and armed |
| `Admission::Refused` | `every 5s memory_grant_depleter` | it exists to spend a budget, and none was named |
| `Admission::Unbacked` | `every 5s date` | a clock is a capability, and the scheduler has none |
| `timetable::Error` | `every fortnight least_authority_demo 7` | the document does not parse |

A Unix crontab line runs as its user and can do whatever that user can do. Nothing in the line
bounds it. Here an entry cannot name what its program's manifest does not declare, and cannot be
backed by authority the scheduler was not given.

## The page

`registration.rs` puts control in a shared page rather than in a message. The registrar bumps the
request word and the timetable notices on its next pass, which costs it one load per pass. The
compositor's per-client control page (`components/src/compositor.rs`, `ctl::SEQ`) is the same shape
in this tree.

The reason is that the timetable yield-polls its clock and a process has one blocking wait point. A
timer object exists now: milestone 106 (a wait that ends on either the interrupt or the deadline)
built it. The timetable has not adopted it, and milestone 674 (the last yield-spinners sleep on a
timer) is the work that would. Once it does, the sequence word can become a notification, and
nothing else in the protocol changes.

## The phase, sampled

`proofs.rs` leaves phase preservation to the host tests, for the reason
[next-after-proofs.md](next-after-proofs.md) gives.
`next_after_is_strictly_in_the_future_and_keeps_its_phase` asserts the congruence at values up to
`u64::MAX / 4`, across four periods, among them `1` and a large prime. That is sampling, and it says
so.

The firing arithmetic is machine-checked rather than sampled because its wrong answers are quiet. A
lost phase, for one, drifts an entry a few nanoseconds per hour and shows up as a schedule nobody
can explain a month later. A wrong parse is a document that does not load, which is loud, so the
parser is left to the host tests.
