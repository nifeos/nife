---
status: DECIDED
raised: 2026-10-08
decided: 2026-10-08
ratified_by: calef
---

# 264. `fork` is declined for good, and spawn is the supported way to start a program

*Beside §263 (threads are built: more than one thread in an address space).*

## The ruling

calef, 2026-10-08 (UTC):

> decline fork. support spawn. decided.

§10 (process model: capability-based, microkernel) already had "No `fork`. Spawn takes an
explicit list of capabilities" among its rules. That rule was written for nife's own programs. The
standard benchmarks brought it back as a question about foreign ones: lmbench, netperf and
hackbench reach for `fork`, and a POSIX layer could have faked it. This section answers that
question too. There will be no `fork`, real or emulated, and a foreign program that starts another
program does it by spawning one.

## Spawn, for foreign programs

Supported means a `posix_spawn`-shaped path: name a program, say what it is handed, get a handle to
wait on. On nife that is the spawner's request under §219 (how the shell names an installed program
to the spawner), plus the wait and exit status milestone 172 (a capability-native subprocess
primitive) owes. `std::process::Command::spawn` and a `posix_spawn` shim are both thin layers over
that one request. Milestone 172 is where they are built; this section does not mint a second
milestone for them.

The shim has nowhere to live yet. There is no C library: §31 (the foreign-language seam: C holds
no capabilities and makes no syscalls) gives C two symbols, `malloc` and `free`, and full POSIX,
milestone 478 (tier three: full POSIX behind the foreign-language seam), is refused. So a ported C
program that spawns depends on a decision not yet made, which the maintainer is taking to calef.
This section records the dependency and does not resolve it.

## Why `fork` is refused rather than deferred

- It copies authority by default. Under §10 a child gets the capabilities it is handed and nothing
  else; `fork` hands it everything, which is the confused deputy with a default.
- It needs copy-on-write duplication of an address space and of a capability table. Neither exists,
  and neither is wanted for anything else.
- It does not compose with threads, which §263 is now building. A forked child gets one thread and
  every lock the others held.

Prior art: Baumann, Appavoo, Krieger and Roscoe, *A fork() in the road*, HotOS 2019, argues `fork`
is a poor abstraction rather than an expensive one. It is cited here and in milestone 52 (subshells
without `fork`), and listed in [`notes/bibliography.md`](../../notes/bibliography.md).

## Consequences, accepted knowingly

- lmbench's `lat_proc fork+exit` and `fork+exec` have no nife figure. Either the cell reads "not
  applicable", or nife's spawn is reported beside Linux's fork with the caveat that a nife process
  is a lighter object than a Unix one, the standing caveat in `notes/benchmarks.md`. The lmbench
  milestone chooses which and states it. A fork emulation would measure the emulation.
- netperf's `netserver` forks per connection by default. It needs a mode that serves without
  forking, from its own options if they give one or from a patch, and Linux runs the same mode. The
  netperf milestone finds which.
- hackbench runs in its thread mode, or its process mode is ported to spawn, with Linux matched.
- A ported C program that calls `fork` either gains a `posix_spawn` call in a patch or does not run.
