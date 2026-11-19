# Ruled decisions

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md). The two
decisions below are the rules in force; the milestone's "Open decisions" holds the ones still
calef's.

## D1. The names of the verdict enums

Background: the crate hands back what an operation decided as an enum per operation: `Send`,
`Receive`, `Signal`, `Wait`, `Arm`. The prose in the same files calls these "verdicts". `Send` at
the crate root is also the name of the standard library's marker trait, in the same namespace, so
a user who imports it loses the trait in that module (item 4). Once published, a rename is a
breaking change for every user; before, it costs one commit in the kernel.

Recommendation: `SendVerdict`, `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict`, `ArmVerdict`.
One suffix across the four modules, the word the documentation already uses, and no collision
with anything in `core`.

Alternatives: `Sent`/`Received` (shorter, but a past participle reads as a fact, and `Blocked` names
nothing that anyone sent). Keep `Send` and tell users to import by path (the footgun stays for a
glob import). `SendOutcome` (fine, but the tree says "verdict" everywhere it explains these).

Prior art: Rust's `std::cmp::Ordering`, `std::ops::ControlFlow` and `std::task::Poll` are the
standard library's verdict enums, each a noun that names the kind of answer; none shares a name
with a trait. seL4's equivalents are not types at all (a C function's side effects).

Concrete example: today, `use inter_process_communication::{Rendezvous, Send}; fn park<T: Send>()`
fails to compile with "expected trait, found enum `Send`". With the recommendation it compiles,
and `match ep.send(me) { SendVerdict::Blocked => ..., }` reads as the documentation already does.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The five take the `Verdict` suffix: `SendVerdict`,
`ReceiveVerdict`, `SignalVerdict`, `WaitVerdict` and `ArmVerdict`, with no alias. Item 4 is the
work.

## D2. Whether the timer and the futex table stay in this crate

Background: the crate has its ratified name. Its contents grew: the rendezvous and the notification
are inter-process communication; the timer and the futex wait queues are not, and arrived later
because they share the token discipline and the proof shape. While nife is the only user, moving
a module out is a two-way door. The first publish makes the crate's contents a claim the name
makes to strangers (item 10).

Recommendation: keep the four together through the move, publish nothing (publishing stays on hold in
any case), and settle the scope at the publish review with the measured demand in hand. State the
shared reason in the README's first line so a stranger is not surprised. If the scope is then
split, `timer` and `futex` leave as their own crates before the first tag, which nobody outside
nife will have pinned.

Alternatives: split now (two more repositories to review and move before this one can; the
timer is 150 lines); rename the crate to say what it holds (reopens a ratified name for a
question the publish review can answer later).

Prior art: seL4 keeps endpoints and notifications in one kernel module family and timers in
another; Linux keeps `futex` and `hrtimer` apart; Zircon keeps channels, futexes and timers as
separate dispatcher types. No kernel bundles the four under a communication name; all four bundle
them under one lock, which is what this crate mirrors.

Concrete example: a crates.io search for "futex wait queue" finds `parking_lot_core` and
`futex`, never a crate named for inter-process communication. A search for "rendezvous endpoint
seL4" would find this one, and `timer::NEVER` in the sidebar would surprise the reader.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. The four modules stay together, and the scope is settled at the publish review if this
crate ever goes to one. Nothing is published and nothing outside nife links the crate.
