# Review package: `inter_process_communication`

*Name: provisional, minted by lane 1095-ipc-rulings-built. The `inter_process_communication`
package of [crate-reviews.md](../crate-reviews.md), in its own file as every package is.*

## In one paragraph

The kernel's blocking primitives, as plain state machines over intrusive queues. `Rendezvous` is a
synchronous endpoint where a send and a receive meet or one waits, with a pending-signal count for
a bound interrupt. `Notification` is a word with waiters. `Timer` is a one-shot timer with the
helpers that keep a cached earliest deadline. `FutexTable` holds futex wait queues. It is `no_std` with no allocator and one dependency, `intrusive_queue`. Sixteen Kani harnesses
prove the one-queue invariant, the token discipline and the delivery rules. The timer and the futex
table stay in this crate (maintainer, provisional, 2026-11-14 UTC, D2); the README the move writes
states the shared reason (the same token discipline and proof shape) in its first line. The
record is [milestone 1095](../../design/roadmap/1095-inter-process-communication-outside-review.md) (`inter_process_communication`: what an outside maintainer would fix before it leaves).

## Identity

| item | value |
|---|---|
| crate | `inter_process_communication`, ratified 2026-09-18, the crate name only |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; none in the manifest, the move writes it |
| license | the nife workspace supplies it today; the move writes `MIT OR Apache-2.0` and both license files |
| `publish` | `false`; nothing publishes until every outside-review milestone is delivered |
| size | 2447 lines in seven files under `src/` (library, `tests.rs`, and `verification.rs` under `cfg(kani)`) |
| dependency | `intrusive_queue`, by `path` in nife. It leaves first (milestone 1097, D3); this crate then pins it by a full revision |
| consumers in nife | `kernel` only |

## Public API

| module | items |
|---|---|
| root | `Rendezvous`, `SendVerdict` (`Rendezvous`, `Blocked`, `Refused`), `ReceiveVerdict` (`Signal`, `FromSender`, `Blocked`), `RendezvousCounts` |
| `notification` | `Notification`, `SignalVerdict` (`Woke`, `ToBound`, `Counted`, `Empty`), `WaitVerdict` (`Word`, `Blocked`), `NotificationCounts` |
| `timer` | `Timer`, `ArmVerdict` (`Pending`, `DueNow`), `NEVER`, `lowered`, `is_due`, `next_due`, `earliest` |
| `futex` | `FutexTable<T, const BUCKETS: usize>`, `Key { address_space, virtual_address }` |

What a stranger may find surprising:

- Every operation answers with a verdict enum and hands back the caller's `Unqueued<T>` token when
  its node did not queue. The kernel owns the nodes; this crate only links and unlinks them.
- `Rendezvous` holds two queues and a pending count, and the proofs say at most one queue is
  non-empty. A single queue with a tag would hold that by construction (milestone 1095, P2-11,
  open after the move).
- `Timer::deadline` answers `NEVER` (`u64::MAX`) for no deadline (P2-13, open).
- `debug_counts` on `Rendezvous` and `Notification` answers a struct with field names.

## Proofs and falsification patches

Sixteen harnesses in `notification.rs`, `timer.rs` and `verification.rs`, each with one patch under
`falsifications/`. Each harness covers one claim:

- The invariant holds after every operation.
- A signal loses no bit, and a waiter wakes before the bound receiver.
- A wait answers a non-zero word or blocks.
- The timer cache is never later than an armed deadline, and a walk fires exactly the due timers.
- A cancelled or re-armed timer never fires stale.
- A bound endpoint never yields a sender.
- A send rendezvouses exactly when a receiver waited.
- A token comes back exactly when its node did not queue.
- Removing a waiter keeps the invariant, a pending signal drains first, and the endpoint forgets a collected sender.

The futex table has host tests and no
harness. The proofs are inductive steps over one waiter per queue.

## Names

The crate name is the one ratified name. `script/crate-standalone` counts 60 unratified names.
The ones this package asks the architect to ratify, all provisional with a `Name:` block:

| name | ruling | what was considered |
|---|---|---|
| `SendVerdict`, `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict`, `ArmVerdict` | D1 (2026-11-14) | `Sent` and `Received`; `SendOutcome`; keeping the bare names, which makes `Send` shadow `core::marker::Send` |
| `Rendezvous::one_queue_invariant_holds` | D4 (2026-11-19) | `is_one_queue`; the question shape would read as a question about the queues |
| `Key::virtual_address` | D4 | the bare `address`, which the register gate refuses |
| `FutexTable<T, const BUCKETS: usize>` | D5 | keeping `Futexes`, which named a collection of objects when a futex is a key |
| `FutexTable::len`, `FutexTable::is_empty` | D5 | `queued`; `is_empty` is what clippy asks of a public `len` |
| `Key::address_space` | D5 | `Key::space` |
| `RendezvousCounts`, `NotificationCounts` | P2-16b | keeping the tuples |

The remaining names (the crate's modules, `Rendezvous`, `Notification`, `Timer`, `Key`, every method
and `NEVER`) carry an older provisional block or none, and the move writes none.

## Naming rules

Against the template's own checks (`vendor/crate-workflows/helpers/`, run with `--manifest` over
`src`; each judged 539 names):

| rule | result |
|---|---|
| units spelled out (`unit_symbols.py`) | 0 findings |
| spelled-out words (`spelled_out.py`) | 0 findings |
| question shape (`question_shape.py`) | 0 findings |
| British spelling (`british_spelling.py`) | 0 findings |
| register names (`register_names.py`) | 2 findings, the ruled `address_space` (field `Key::address_space` and the test helper's parameter): the gate reads every `address` as a location and wants `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS`. An address space is not an address, so the gate misreads the ruled name; see the open question |
| aliases of an old nife spelling | none |

## License

The manifest inherits `license` from the workspace. The crate adapts no source, so the move writes
`MIT OR Apache-2.0` with no `AND` term.

## What the release still needs

| item | owner |
|---|---|
| `intrusive_queue` leaves first, then this crate pins its revision | milestone 1097 (`intrusive_queue`: what an outside maintainer would fix before its first release) |
| The move: own manifest (`license`, `lints`, `rust-version`, description, repository), both license files, README and `docs/design.md` in place of the links to `notes/inter-process-communication.md`, citations and `Name:` lines stripped | the move lane |
| A full mutation run in the new repository, with the three survivors triaged (P1-9) | the move lane |
| Ratify the provisional names above, then the go to create the repository | calef |
| The externalization list names the pair, `intrusive_queue` first | milestone 1017 (the contract crates leave in this order) |

## Proposed items

None beyond the milestone's open items (P0-3, P1-6, P1-7, P1-9, P2-11, P2-13, P2-15).

## Open questions

1. How does the template's `register_names` gate treat `address_space`? Two options: add
   `space` handling to the template's address rule (a change to `crate-workflows`, not to this
   crate), or rename the field. The ruling of D5 names it `address_space` and the gate runs only
   in the new repository, so this package raises the question and renames nothing.

## Result of `script/crate-standalone`

A run with `--no-kani` fails exactly the checks the move resolves: 60 unratified names, the path
dependency, the workspace `license` and `lints`, no `rust-version`, no README, no license files.
The citations fail too (12 section, 12 path and 9 milestone lines, in patch headers and rustdoc). The build is
skipped because the dependency is outside the copy. Host tests, clippy with warnings denied, the
bare-metal builds and Kani run green in nife.

## Recommendation

Ready for review.
