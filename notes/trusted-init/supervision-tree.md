# Phase B.2: shrinking what a broken progenitor can do

An appendix to [notes/trusted-init.md](../trusted-init.md), which summarizes it.

B.1 settled *what bytes the progenitor is*. B.2 is the other half: what a compromised progenitor can still reach.

The pre-B.2 progenitor (`system_initializer`, `hello`'s init role) holds a large untyped budget for its entire life,
because it stays the system's process builder. Every process in the system is therefore one bug in
the progenitor away from being built wrong. The answer is not to make the progenitor more careful; it is to make it
hold less, and to make it hold it for less time.

## The tree

Four small portable programs (`components/src/root_supervisor.rs`, `spawner.rs`, `sub_server_supervisor.rs`, `flaky.rs`, sharing
`crates/supervision_protocol`):

```text
  root_supervisor   the root untyped + the initrd + a report endpoint       (briefly)
    |
    +-- spawner   one program image, one budget (WRITE), no initrd  (can build only that program)
    |
    +-- sub_server_supervisor    a request channel + a fault endpoint, NO memory   (cannot build anything)
            |
            +-- flaky   a report endpoint                           (the supervised sub-server)

  then: root_supervisor deletes its untyped and becomes a RECEIVE loop on its own supervision endpoint.
```

Each split is chosen so that the authority is the smallest thing that still does the job:

- The spawner holds a program image, not the initrd. root_supervisor copies `flaky`'s bytes into fresh
  read-only pages in the spawner's address space (the `blobs` field of `Endow`). So "build me program
  X" is not a request the spawner *can* honor for any other X: the only program it can name is the
  one it was handed. Compare the progenitor, which holds a 14 MB archive of every program in the system.
- The spawner's budget is WRITE without GRANT. It may spend memory; it may not lend it. Nothing
  it builds can be endowed with a budget of its own.
- Each instance is built in its own region, split off the budget, so reaping it is one
  `MemoryRegion::DESTROY` (§16 (object revocation)) and one instance's corpse cannot pin another's memory. A LIFO reap returns
  the pages to the budget (§16's return-of-pages), so the restart loop is not a leak.
- The supervisor holds no memory at all. Its entire power is to ask the spawner for a rebuild of
  one program. A compromised supervisor is a restart loop, not a foothold.
- root_supervisor deletes the budget once the two servers are running (and the wiring capabilities, and
  the spawner's budget copy). What is left cannot make a page, an address space, a thread, or an
  endpoint.

## Restart policy is userspace code, and stays there

`sub_server_supervisor` is the policy: bounded retries (two), a clean exit read as "finished" rather than "crashed",
a give-up when the budget runs out. It reaps the corpse through the spawner before rebuilding, because
§26's corpse is dead-until-reaped. None of that is in the kernel, and the kernel's whole contribution
is one five-word message. That is DECISIONS §26 working as designed: the kernel is the only witness to
a death, so it reports; everything after the report is somebody's opinion.

Note which side of the split *reaping* falls on: the supervisor decides whether to reap and
rebuild, the spawner is what can. Policy and authority, separated by an IPC boundary.

## What is proven, both ISAs

`system_tests/src/user/authority_tests.rs`:

- `the_root_supervisor_drops_its_construction_authority_and_cannot_build_again`. After the handoff, root_supervisor tries the
  two primitives that build things (retype a page, retype a kernel object). It reports the result
  from *inside* the process, because what matters is what the holder can do and only the holder can
  ask.
  Both fail, and they fail with `NoSuchSlot` (there is nothing there), not `NotPermitted` (there is
  something there and you may not use it). That distinction is the whole difference between "we asked
  the progenitor not to" and "the progenitor cannot".
- `a_dead_sub_server_is_restarted_by_its_supervisor_not_by_the_root`. The sub-server runs as attempt 0 and
  faults on a load from an unmapped address. The supervisor receives `FAULT`, reaps the corpse
  through the spawner, and asks for attempt 1. Attempt 1 runs and exits cleanly. The supervisor
  receives `EXIT` and does not restart it. Exactly five reports arrive and the endpoint then has no parked
  sender, which is how "and then nothing else happened" is asserted without a blocking receive.

"Without the progenitor's involvement" is an authority argument, not a timing one. The progenitor cannot retype a
page by then, and a process that cannot retype a page cannot have built the replacement. Scheduling
order is not the evidence; the empty capability slot is.

## The bug this work found

The supervision-tree tests enter more processes per run than anything before them. That surfaced a
pre-existing race in the exception-return path on both architectures. Staging `SPSR_EL1`/`ELR_EL1`
(and `sepc`/`sstatus`) for the `eret` is not atomic with respect to a nested exception. So an
interrupt in a two-instruction window could return a brand-new process to its entry point *at EL1*. Fixed by
masking interrupts at the top of the restore. Full account in notes/exceptions.md; it is written up
there rather than here because it is an exception-path fact, not a progenitor fact.

## Two design forks found and deliberately not built through

1. Reaping requires the same right as building. `MemoryRegion::DESTROY` needs `WRITE` on the region,
   and so does `RETYPE`. So a root supervisor that could restart a dead tier-one server is a root
   supervisor that can build processes, which is exactly the authority we set out to give away.
   root_supervisor therefore chooses to be *unable* to build, and its policy for a dead tier-one server is
   "report and stop", the fail-closed floor pushed as high and as small as it goes. Splitting a
   reap-only right out of `WRITE` (a rights bit, or a distinct `Untyped::REAP`) would let a root
   supervisor recover without regaining construction authority. That is a kernel surface change and a
   rights-model change, so it is an architect's call, not a thing to slip in.
2. A supervisor cannot turn a tid into a handle. The death message names the dead thread by tid
   (§26 (the fault endpoint), item 5), and nothing turned a tid into something a builder holds.
   The options were `Tcb::NAME`, per-child fault endpoints, or the builder reporting the tid.

Both are answered. Fork one by §32 (a supervisor may collect a corpse without being able to build
one) and §148 (a supervisor restarts by asking)'s spawner pattern. Fork two by §148 as amended
2026-10-04: the death message carries the builder's label, and `sub_server_supervisor` tells two
children apart by it. See milestone 105 (the two forks).
