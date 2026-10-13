---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: a-program-is-asked-to-give-memory-back
milestone_dependencies: 851
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 850. A program is asked to give memory back before anyone is refused

*(The title and slug are provisional.)*

calef asked for this on 2026-10-06 (UTC), ruling fork 6 of milestone 851 (a running program acquires
more memory as it needs it): "Approve Fork 6 A with both follow-ons". It is the "please shrink"
notice milestone 851's fork 4 named. In his words from fork 3 of the same milestone: "Most of my
professional work has involved
large programs with spikes in memory that would need to be returned." calef ruled all four forks
on #1786 the same day.

It depends on milestone 851's `memory_broker`, its accounts, and reserve and commit (forks 1, 4 and
5 there, ruled). Every name here is provisional: the signal, its levels, the runtime hook and the
fixture.

## Why, and its honest rank

Under milestone 851, a program's memory comes back two ways. The allocator trims its band and frees
large
regions on its own. Or the owner revokes, which kills. Neither reaches memory a program holds on
purpose: a garbage-collected heap sized for throughput, a database's buffer pool, a cache. That
memory is the bulk of a large program's footprint, and it is the part the program could give back
if asked. Without a signal, the next program's commit is refused while the memory it needed sits in
someone's cache.

On principle 1, this is behind milestone 851's first slice and milestones 801 (packages over the internet)
and 802 (the trivial install). It matters to the
first customer that runs a runtime with a collector or a cache, and to calef's own workloads. It
moves no fatal-risk verdict.

## What the tree has, read on 2026-10-06 (UTC)

- Notification objects: a word a sender ORs bits into and a waiter collects, bindable so a thread
  waiting on an endpoint also wakes for it (§101 (notification objects), `abi::notification`).
- Milestone 851's broker counts committed memory per account, refuses a commit past the account's
  ceiling
  or the per-identity cap or when the pool is empty, and gives the owner a revoke.
- No runtime here has a way to be told anything about memory.

Reuse: notification objects are the tree's. The levels borrow from cgroups and Android (below).

## The forks

Four, all ruled.

### Fork 1. When the signal fires

Ruled A, calef, 2026-10-06 (UTC): "approve A, and add PSI to the prior art."

| option | what it is | verdict |
|---|---|---|
| A | Two triggers: an account's soft limit (`high`) below its ceiling, and the pool's free pages falling under a watermark | ruled |
| B | The pool watermark only | refused |
| C | Only when a commit is about to be refused | refused |

A mirrors cgroups (from memory): `memory.high` throttles and reclaims a group before `memory.max`
refuses it. The account trigger keeps one program honest against its own ceiling. The pool trigger
asks everyone when the machine is short. B cannot tell a program it is near its own limit. C is
too late: by then a commit is waiting, and a program that frees memory in response returns it after
the refusal it could have prevented.

The soft limit defaults to 80% of an account's ceiling. The pool watermark defaults to 10% of the
pool. Both are guesses to measure, not results.

### Fork 2. How it is delivered

Ruled A, calef, 2026-10-06 (UTC): "Yes", on the condition that a program can learn pressure has
eased.

| option | what it is | verdict |
|---|---|---|
| A | A notification object per account, given to the program when the account opens; the broker signals a level bit | ruled |
| B | A message on an endpoint the program serves | refused |
| C | An upcall that interrupts the program | refused |

A costs a program nothing until it looks, never blocks the broker, and coalesces: two signals
before the program wakes read as one. A program binds it to its main endpoint if it serves one,
or polls it from its allocator. B blocks the broker on a slow program. C needs a kernel mechanism
the tree does not have, and runs arbitrary code at an arbitrary point.

Two levels, set as bits in the word: moderate (give back what is cheap) and critical (give back
what you can). Android's `onTrimMemory` levels and Linux PSI's some and full are the precedent,
from memory.

How a program learns pressure eased: by query, not a third bit. The broker signals on every change
of level, up or down, and the program then asks its account for the current level. Bits coalesce,
so a raise and a clear before the program wakes would arrive as one word that says both. The bits
say something changed; the query says what is true now.

### Fork 3. Who may send it

Ruled A, calef, 2026-10-06 (UTC): "A".

| option | verdict |
|---|---|
| A. The broker only. The owner asks through the broker. | ruled |
| B. Any holder of the account | refused |

The notification capability the broker keeps is the only one with `SIGNAL`. A program holds it
with `WAIT` only. A peer cannot make another program shrink.

### Fork 4. What a program does with it

Ruled A, calef, 2026-10-06 (UTC): "Yes", on two conditions. A hook must not allocate, and the
runtime enforces it. Purgeable memory is a follow-on.

| option | what it is | verdict |
|---|---|---|
| A | The runtime acts first: the allocator trims its band with no hysteresis and frees cached free increments. Then it calls hooks the program registered | ruled |
| B | Hooks only | refused |

A gives every program something for free, because milestone 851's band trimming keeps one free
increment
in hand as hysteresis, and pressure is when to drop it. Hooks are for the memory the runtime cannot
see: a collector's heap target, a buffer pool. `user_mode_runtime` gains a hook registry, and
`std` gains one under `std::os::nife`, since Rust's `std` has no portable API for this. A
collector registers a hook that lowers its heap target and collects.

While hooks run, the allocator refuses every allocation: it returns null, so `try_reserve` sees
`Err` and a plain allocation aborts. A hook that allocates fails loudly the first time it runs,
not at the moment memory is shortest.

## What it never does

Ruled by milestone 851's fork 4 already, and restated so nobody builds it: the signal is advisory. It never
kills, never revokes, and never delays a refusal past the point fork 4 sets. A program that ignores
it loses nothing it would not lose anyway. Killing stays the owner's revoke, a person's decision.

## How others do it

From memory, unchecked; the building lane owes a read of each.

- Linux: cgroup `memory.high` reclaims and throttles below `memory.max`.
- Linux PSI: measures the share of time tasks stall waiting for memory, not how full it is, and a
  process can wait on a PSI trigger. A recorded alternative trigger to fork 1's fill levels.
- Android: `onTrimMemory` delivers levels to apps, and the low-memory killer acts after.
- iOS: `didReceiveMemoryWarning` asks apps to shed caches, and jetsam kills after.
- Windows: memory resource notifications report low and high memory, and a process waits on them.
- Fuchsia: memory-pressure signals to components, and a component-level killer after.
- Every one of these has a killer behind its signal. nife keeps the signal and leaves the kill to a
  person.

## The first slice

1. The broker gains a per-account soft limit and a pool watermark, and a notification per account.
2. `user_mode_runtime` and `std` read the notification, trim at once, and run registered hooks.
3. A fixture, `cache_holder`, keeps a cache it can drop and registers a hook that drops it.

### Exit criteria a stranger could check

One `cargo xtask` gate that `script/test` runs, exiting 0 under QEMU on aarch64, riscv64 and
x86_64:

1. Account pressure. `cache_holder` fills a 32 MiB cache with its soft limit at 24 MiB. It gets the
   moderate bit, drops the cache, and its committed total, read from the broker, falls below 8 MiB.
2. Raise and clear. After `cache_holder` drops its cache, the broker signals again, and the query
   reports no pressure. A second fixture that waits on the notification wakes for both changes.
3. Pool pressure. With the pool near its watermark, a second program's commit succeeds after
   `cache_holder` gives back, where it was refused with the signal disabled.
4. Advisory. A program that ignores the signal keeps running with its memory intact. Nothing is
   revoked.
5. Only the broker signals. A program's attempt to `SIGNAL` its own or another's notification is
   refused.
6. Hooks cannot allocate. A hook that allocates gets null, and `try_reserve` in a hook gets `Err`.
7. Measured, not gated: the time from signal to pages back in the pool.
8. A `design/decisions/` section records the levels and the delivery, minted by the integrator.

## Follow-ons, proposed, unnumbered

- Purgeable memory, ruled a follow-on by calef: a program marks cache memory discardable and the
  system reclaims it without asking, as macOS purgeable memory and Linux `MADV_FREE` do (from
  memory). It needs the broker to reclaim from a running program safely, which nothing here does.

## BUGS

- The default soft limit and watermark are guesses until the first slice measures them.

## Index row

A program is asked to give memory back before anyone is refused: a notification per broker account carrying pressure levels, and runtime hooks that may not allocate. The signal is advisory and never kills; the owner's revoke stays a person's decision. Waits on milestone 851 (a running program acquires more memory as it needs it).
