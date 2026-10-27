# Processes: what threads share, and how they end together

*Written 2026-10-10 (UTC) by milestone 812 (`std::thread::spawn` runs real threads in one address
space)'s lane. The rulings are §269 (how threads share a process) forks 1, 3 and 5, and calef's four
answers on pull request #1892 the same day. The wire contract is `abi::process`, `abi::objtype::PROCESS`,
`abi::rights::BIND` and `abi::SYS_EXIT_THREAD`; the kernel half is `kernel/src/sched/process.rs`.*

## The object

A process is one page retyped from its creator's region, so the region pays for it. It holds what
its threads share:

- the capability table, after the page's header;
- the address space, which it holds as the registry's one holder of that space;
- a member count, which every pointer into the page depends on;
- how it ended, for the supervisor.

`RETYPE_OBJ(region, PROCESS, space)` creates one, consuming the space's `WRITE` capability the way
`CONFIGURE` consumes one. That second word is this lane's choice of how a process gets its space.
The ruling says a process holds one and leaves the creation shape open. A process born with its
space is Zircon's `zx_process_create`.

A thread joins by `CONFIGURE` given the process capability with `BIND`, which is not consumed.
Whatever the embryo's own table holds moves into the process's at the same slots, so a loader
endows a thread before it joins exactly as it always has; a slot already taken in the process's
table refuses the join. A thread configured with a space is a process of its own, with no
capability, exactly as before.

## Who holds a space

One holder per space: a thread that is a process of its own, or a process. §249 (a running address
space stays nameable)'s amendment (b) refused a second *thread*. That refusal retires, as calef
ruled, because threads now share a space by joining its process. A second *holder* is still refused
(`WrongObject`), so two processes cannot share one space. This is my reading of "the bound mark
retires". §269 fork 5 notes that memory can be shared for reasons other than being one process; this
tree shares it through frames. Lifting it later means counting holders in the registry.

## How it ends

A process ends on any of these:

- a member's `SYS_EXIT`;
- a member's fault;
- its last running member's `SYS_EXIT_THREAD`;
- `Process::DESTROY`;
- destruction of its region;
- its last member leaving the thread table.

The last of these is what keeps revocation sound. The sweeps reach tables through threads, so a
table with no member must never be joined again.

Ending it ends every other member. A blocked member is finished in place, and reaped as soon as the
member that ended the process is (or by `DESTROY` itself), since nothing will switch it out again.
A ready or running one is marked killed and dies at its next preemption, the §16 (object
revocation) amendment's mechanism. A killed member that parks before that preemption is finished
by `schedule()` instead of sleeping. One caught mid-switch-out when the end came is finished by the
same sweep that reaps the others, which waits briefly for its core to let it go. CI's `thead-c906`
leg found that last case through `rayon`'s workers, which spin and then sleep.

Supervision stays on the thread, as it was. The first member's `START` takes the reserved fault slot
out of the shared table, so a sibling started later is unsupervised. When the process ends, the
supervised member reports how it ended, whoever ended it: from `depart` if it is the one leaving,
from the killed-thread conversion, or from the teardown of a blocked member. `REAP` therefore still
names the thread the supervisor built.

The space is released when the process has ended and its last member is reaped, by `DESTROY` if no
member remains, or by the region sweep once the process's name is gone.

## How it is proved

`system_tests/src/user/process_tests.rs`, on all three architectures, each test with a replayable
falsification:

- two hand-written threads share a space, a table and a futex, one of them ending itself with
  `SYS_EXIT_THREAD`;
- a sibling's `exit` ends the supervised member, and its supervisor hears `EVENT_EXIT`;
- every refusal of the contract;
- a destroyed region ends its process and a member that lives in another region.

`std_threads_tests::rayons_parallel_sum_runs_on_every_online_core` proves the reaping of blocked
members: `rayon`'s idle workers are asleep on a futex when its `main` returns. `a_member_stranded_killed_on_a_futex_is_finished_and_reaped` stages the mid-switch case's
outcome, since the race itself cannot be staged on demand.

## BUGS

- **A killed member runs on until its next preemption**, so "ends every thread atomically" (§269
  fork 5) holds for state, not for the last few microseconds of execution. A ready member is not
  pulled off its run queue; it is converted the next time it is switched in and preempted.
- **The TCB page of a thread that ended is not reused.** A `std` program spends one page of its
  heap region per thread it has ever spawned, until that region is destroyed.
- Teardown walks every thread to find members, `MAX_THREADS` at most. Fine at 256; a member
  list in the page is the shape if that grows.
- Exit codes are not carried. `SYS_EXIT`'s code was discarded before this milestone and still
  is; a process records its end's event, not a code.
