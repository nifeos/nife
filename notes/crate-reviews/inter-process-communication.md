# Review package: `inter_process_communication`

*Name: provisional, minted by lane 1095-ipc-rulings-built. The `inter_process_communication`
package of [crate-reviews.md](../crate-reviews.md), in its own file as every package is.*

## In one paragraph

The kernel's blocking primitives, as plain state machines over intrusive queues. `Rendezvous` is a
synchronous endpoint where a send and a receive meet or one waits, with a pending-signal count for
a bound interrupt. `Notification` is a word with waiters. `Timer` is a one-shot timer with the
helpers that keep a cached earliest deadline. `AddressWaitTable` holds address wait queues. It is `no_std` with no allocator and one dependency, `intrusive_queue`. Sixteen Kani harnesses
prove the one-queue invariant, the token discipline and the delivery rules. The timer and the address wait
table stay in this crate (maintainer, provisional, 2026-11-14 UTC, D2); the README the move writes
states the shared reason (the same token discipline and proof shape) in its first line. The
record is milestone 1095 (`inter_process_communication`: what an outside maintainer would fix before it leaves), at
[its roadmap file](../../design/roadmap/1095-inter-process-communication-outside-review.md).

## Identity

| item | value |
|---|---|
| crate | `inter_process_communication`, ratified 2026-09-18, the crate name only |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; none in the manifest, the move writes it |
| license | the nife workspace supplies it today; the move writes `MIT OR Apache-2.0` and both license files |
| `publish` | `false`; nothing publishes until every outside-review milestone is delivered |
| size | 2398 lines in eight files under `src/` (library, `tests.rs`, `test_node.rs` under `cfg(any(test, kani))`, and `verification.rs` under `cfg(kani)`) |
| dependency | `intrusive_queue`, by `path` in nife. It leaves first (milestone 1097 (`intrusive_queue`: what an outside maintainer would fix before its first release), D3); this crate then pins it by a full revision |
| consumers in nife | `kernel` only |

## Public API

| module | items |
|---|---|
| root | `Rendezvous`, `SendVerdict` (`Rendezvous`, `Blocked`, `Refused`), `ReceiveVerdict` (`Signal`, `FromSender`, `Blocked`), `RendezvousCounts` |
| `notification` | `Notification`, `SignalVerdict` (`Woke`, `ToBound`, `Counted`, `Empty`), `WaitVerdict` (`Word`, `Blocked`), `NotificationCounts` |
| `timer` | `Timer`, `ArmVerdict` (`Pending`, `DueNow`), `NEVER`, `lowered`, `is_due`, `next_due`, `earliest` |
| `address_wait` | `AddressWaitTable<T, const BUCKETS: usize>`, `Key { address_space, virtual_address }` |

What a stranger may find surprising:

- Every operation answers with a verdict enum and hands back the caller's `Unqueued<T>` token when
  its node did not queue. The kernel owns the nodes; this crate only links and unlinks them.
- `Rendezvous` holds two queues and a pending count, and the proofs say at most one queue is
  non-empty. A single queue with a tag would hold that by construction (milestone 1095 (`inter_process_communication`: what an outside maintainer would fix before it leaves), P2-11,
  open after the move).
- `Timer::deadline` and `earliest` answer `Option<u64>`, `None` when nothing is armed. `NEVER`
  (`u64::MAX`) is only the word a kernel stores for its cached earliest (P2-13; maintainer, provisional, 2026-11-20 UTC, D7).
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

The address wait table has host tests and no
harness. The proofs are inductive steps over one waiter per queue.

## Names

The architect ratified every public name on 2026-11-21 (UTC), with two renames. `Notification::poll`
is now `try_wait`, a non-blocking `wait`, which is the shape the Rust standard library gives its
`try_` calls. `AddressWaitTable::waiters` is now `waiter_count`, a count of the waiters under one
key, which the bare plural noun did not say. No alias of an old nife spelling exists. The move
strips the `Name:` blocks.

| name | status | what was refused |
|---|---|---|
| `inter_process_communication` | ratified 2026-09-18 | `ipc` |
| `SendVerdict`, `ReceiveVerdict`, `SignalVerdict`, `WaitVerdict`, `ArmVerdict` and their variants | ratified 2026-11-21 (UTC) | `Sent` and `Received`; `SendOutcome`; the bare names, which make `Send` shadow `core::marker::Send` |
| `Rendezvous::one_queue_invariant_holds`, `Notification::invariant_holds` | ratified 2026-11-21 (UTC) | `is_one_queue`; the question shape would read as a question about the queues |
| `Key`, `Key::virtual_address`, `Key::address_space` | ratified 2026-11-21 (UTC) | the bare `address`, which the register gate refuses; `Key::space` |
| `AddressWaitTable<T, const BUCKETS: usize>` with `new`, `park`, `take`, `remove`, `len`, `is_empty`, `waiter_count` | ratified 2026-11-21 (UTC) | the plural noun, which named a collection of objects when an address wait is a key; `queued`; `waiters` |
| `RendezvousCounts`, `NotificationCounts` | ratified 2026-11-21 (UTC) | keeping the tuples |
| `Rendezvous` with `new`, `send`, `receive`, `signal`, `bind_to_interrupt`, `is_bound_to_interrupt`, `is_idle`, `debug_counts`, `remove_sender`, `remove_receiver`, `drain_waiters` | ratified 2026-11-21 (UTC) | |
| `Notification` with `new`, `signal`, `wait`, `try_wait`, `is_idle`, `word`, `debug_counts`, `remove_waiter`, `drain_waiters` | ratified 2026-11-21 (UTC) | `poll` |
| `Timer` with `new`, `arm`, `cancel`, `expire`, `deadline`, `is_armed`; `NEVER`, `lowered`, `is_due`, `next_due`, `earliest` | ratified 2026-11-21 (UTC) | |
| the modules `address_wait`, `notification`, `timer` | ratified 2026-11-21 (UTC) | |
| `Refused` (a variant of `SendVerdict`) | ratified 2026-11-21 (UTC) | |

The one private test module, `test_node`, keeps its provisional block.

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
| The externalization list names the pair, `intrusive_queue` first | milestone 1017 (the contract crates leave in this order) |

## Proposed items

None beyond the milestone's open items (P0-3, P1-6, P1-7, P1-9, P2-11).

## Open questions

1. How does the template's `register_names` gate treat `address_space`? Two options: add
   `space` handling to the template's address rule (a change to `crate-workflows`, not to this
   crate), or rename the field. The ruling of D5 names it `address_space` and the gate runs only
   in the new repository, so this package raises the question and renames nothing.

## Result of `script/crate-standalone`

A run with `--no-kani` fails exactly the checks the move resolves: one unratified name (the private test module), the path
dependency, the workspace `license` and `lints`, no `rust-version`, no README, no license files.
The citations fail too (12 section, 12 path and 9 milestone lines, in patch headers and rustdoc). The build is
skipped because the dependency is outside the copy. Host tests, clippy with warnings denied, the
bare-metal builds and Kani run green in nife.

## Recommendation

Cleared on 2026-11-21 (UTC), with the names ratified as above. The move waits for `intrusive_queue`.
