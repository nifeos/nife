---
status: NOT-STARTED
raised: 2026-10-08
milestone_dependencies: 835, 172
decision_dependencies: 223, 226, 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 838. A C library: `posix_spawn`, and no fork

*(The title and slug are provisional.)*

The fork ruling on PR #1856 declines fork permanently and supports spawn. §265 (a C library started
from relibc) follows it. relibc already routes `posix_spawn` through a platform call rather than
through fork and exec. This stage writes that call on milestone 172 (a capability-native subprocess
primitive). It also rewrites the three generic functions that call fork (`popen`, `system` and
`forkpty`) on `posix_spawn`, or refuses each in `BUGS` with its reason.

Reuse: relibc's `spawn` header and its `posix_spawn` file-action types (MIT), over milestone 172's
primitive.

## What this has to decide

`posix_spawn` names a program by path and passes file actions. On nife a child holds only what its
parent grants, so the file actions become grants: an `addopen` is a directory or file capability
the child receives, and an `adddup2` moves one. Whatever the parent does not pass, the child does
not have. That mapping is a design fork for an architect when this milestone is taken, not before.

## Process ids, ruled 2026-10-12 (UTC)

Added by lane `maintainer/mint-monit-milestone`. calef ruled question 3 of milestone 905 (monit is
packaged in basalt, installed with `jig`, and restarts a nife service it watches) on #2022 with one
word, *"a"*. In this C library a pid is a thread id within a supervision domain the process holds,
as `pidwait` already takes tids under §226 (`pidwait` takes tids and composes with `pgrep`, because
a program does one thing). A process can ask about a pid only inside a domain it was granted, and
there is no global pid namespace. `posix_spawn` returns the child's tid in the spawner's domain.

So this milestone's scope grows by one item: `getpid`, `getppid`, `getpgid`, `kill(pid, 0)` and
`waitpid` answer from the held domain through `endpoint::SURVEY`, under §223 (the process view is
the supervision domain). A number outside every held domain is `ESRCH`. Today `getpid` is 0 and
`getpgid` answers 0 for any pid (`c_library/src/platform/nife.rs`), so every pid looks alive; that
is fixed here, not in 905. monit is the first consumer: the services it watches are the domain its
service manifest requests.

## First consumers

netperf's server in a spawn mode, if 829 wants one; any C program that runs a helper. No standard
benchmark needs it to run, which is why it is last.

## Done when

A C program spawns a child with `posix_spawn`, waits for it with `waitpid`, and collects its exit
status, on all three architectures; `popen` and `system` either work on it or are refused with a
reason. The child's pid answers `getpgid` while it runs and `ESRCH` once reaped, and a number
outside the held domain answers `ESRCH`.

## Index row

Process creation for nife's C library: `posix_spawn` on milestone 172, and `popen` and `system`
rewritten on it, because fork is never coming.
