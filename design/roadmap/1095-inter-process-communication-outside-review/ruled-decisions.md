# Ruled decisions

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md). The
decisions below are the rules in force; the milestone has no open decision.

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
crate ever goes to one. Nothing outside nife links the crate, and no release of it exists.

## D3. Which crate leaves first

Background: every public signature exposes `Unqueued<T>` and `Node` from `intrusive_queue`, which
no extraction plan named. A crate that has left may not depend back into nife, and a published
crate cannot depend on an unpublished one.

Alternatives: both in one lane as two repositories (one round trip fewer, two packages to rule at
once); fold the queue into this crate (the kernel's run queue would carry a second copy of the
type).

Prior art: `generational_table` left before the kernel pinned it, and the kernel pins
`extensible_host_controller_interface` by revision. Nife differs in nothing; this is the same step
one level down. Example: after the move the manifest line reads
`intrusive_queue = { version = "0.1.0", git = "https://github.com/nifeos/intrusive_queue", rev = "<full sha>" }`.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a two-way door, which the architect may
revisit. `intrusive_queue` leaves first (milestone 1097 holds its package), then this crate pins it
by git revision on that repository's `main`. Milestone 1017's list records the pair in that order.

## D4. Two public names the gates refuse

Background: the template's name gates run at zero tolerance in a crate's own repository.
`Rendezvous::one_queue_invariant` returns `bool` and does not read as a question, where its twin
`Notification::invariant_holds` does. `futex::Key::address` is a virtual address, and the naming
rule says which kind of address a name holds.

Alternatives: keep `Key::address` with a gate exemption; `is_one_queue` for the method.

Prior art: the standard library writes its predicates as questions (`is_empty`, `contains`);
Linux's `futex_key` names the field `address` inside a union keyed by `private` or `shared`. Nife
differs: the address kind is in the name. Example: `Key { space: 7, virtual_address: 0x1000 }`.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a two-way door until the first publish.
`one_queue_invariant` becomes `one_queue_invariant_holds` and `Key::address` becomes
`Key::virtual_address`. A rename lane builds it.

## D5. The futex table's names

Background: `Futexes<T, const B: usize>` is a table of futex wait queues hashed by a `Key`. A
stranger meets four provisional names on the first screen: the plural type name, the const
parameter `B`, the count method `queued()` and the field `Key::space`.

Alternatives: keep `Futexes` and rename only `B`, `queued` and `space`; keep all four and explain
them in the documentation.

Prior art: Linux names the structure `futex_hash_bucket` and its table `futex_queues`;
`parking_lot_core` keeps a `HashTable` of buckets; the standard library's count is `len`. Nife
differs in nothing. Example: `let table: FutexTable<Thread, 64> = FutexTable::new();` and
`table.len()`.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a two-way door until the first publish.
`Futexes` becomes `FutexTable<T, const BUCKETS: usize>`, `queued()` becomes `len()` and
`Key::space` becomes `Key::address_space`. A rename lane builds it.

## D6. Review clearance and the repository

Background: clearing a crate review is the go to create its repository under nifeos. This crate
has no review package (item 1), and it waits on `intrusive_queue` leaving first.

Alternatives: clear together with `intrusive_queue`, so both repositories open in one lane; hold
until the P1 items land as well.

Prior art: `generational_table`, `block_roster` and `file_allocation_table` each moved on a cleared
review with no tag. Nothing outside nife links this crate. Example: the kernel's manifest switches
from a path to a pinned revision of `nifeos/inter_process_communication` in the move's pin pull
request.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a two-way door. A lane writes the
review package now (item 1). The architect ratifies the provisional names from it and gives the go
to create the repository once `intrusive_queue` has left. No tag and no publication.
