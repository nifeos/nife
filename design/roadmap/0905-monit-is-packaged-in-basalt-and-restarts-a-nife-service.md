---
status: NOT-STARTED
raised: 2026-10-12
milestone_dependencies: 801, 809, 810, 836, 837, 838, 849, 897, 910
decision_dependencies: 135, 208, 223, 226, 264, 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 905. monit is packaged in basalt, installed with `jig`, and restarts a nife service it watches

*(The title and slug are provisional.)*

calef, 2026-10-12 (UTC): *"monit is really useful. Lets add a milestone for porting it to nife."*
And the same day: *"That milestone should require it as a package delivered via jig through
basalt."*

**In brief.** Port monit, the C process and service supervisor from Tildeslash (mmonit.com,
AGPL-3.0). Ship it the way milestone 810 (`ripgrep` is packaged in basalt and installed with
`jig`) ships `rg`. That means a recipe in basalt and a package build against the C library at
basalt's pinned nife commit. Then publication to basalt's index, and `jig install monit` at the
prompt, which starts it as a nife service. The program does not run unmodified. Its process layer
has no nife backend, it starts every program with `fork`, which §264 (`fork` is declined for good,
and spawn is the supported way to start a program) refuses, and its daemon is controlled by
signals. So this milestone is a port with three named changes, and it says so.

## What monit needs

Read from the monit 5.35.2 release tarball; the detail is in [the appendix](0905-monit-is-packaged-in-basalt-and-restarts-a-nife-service/monit-source.md). Process statistics come
from a per-OS `sysdep` layer, and adding a pair of `sysdep` files is upstream's porting path.
Starting a program is `Command_execute`, which forks. Exit status arrives only by `SIGCHLD`.
`monit reload` and `monit quit` signal the daemon found by pidfile; `monit status` and the service
actions are HTTP requests to the daemon's web server. It needs pthreads, TCP and Unix sockets,
`poll` and `getaddrinfo`, and builds with autotools; OpenSSL, PAM and zlib are optional.

The daemon's HTTP interface cannot carry reload or quit, checked for calef's question 2 ruling:
`src/http/cervlet.c`'s runtime actions are `validate` and `stop`, and `stop` stops only the web
server (`Engine_stop`). Service actions are queued and the daemon woken by `kill(pid, SIGUSR1)` on
the pidfile's pid, so without a wakeup they wait for the next cycle.

## What each need maps onto in nife

| monit needs | nife today | owner |
|---|---|---|
| A C library | relibc-seeded `c_library/`, files, clock and memory: BUILT | milestone 835 (a C library, stage 1: files, clock and memory) |
| pthreads | stage 2, BUILT on its branch (#2009) and in the merge queue | milestone 836 (a C library, stage 2: threads) |
| TCP and Unix sockets, `poll`, `getaddrinfo` | stage 3 promises TCP, `poll` and literal-address `getaddrinfo`; no Unix sockets | milestone 837 (a C library, stage 3: sockets) |
| Starting a service | `fork` returns `ENOSYS` (`c_library/src/platform/nife.rs`) and always will | §264; milestone 838 (a C library: `posix_spawn`, and no fork) on milestone 172 (a capability-native subprocess primitive) |
| Process ids | `getpid` is 0, `kill` names only the caller, and `getpgid` answers 0 for any number, so every pid looks alive | milestone 838, as ruled (question 3): a pid is a tid in a held domain |
| `SIGCHLD`, `SIGHUP`, `SIGTERM` | none delivered; `sigaction` records a handler and nothing fires it; reload and stop arrive on milestone 910's control endpoint instead | §265 (a C library started from relibc, whose Rust platform layer holds the capabilities), "What this does not decide" |
| A process table | `endpoint::SURVEY` over one supervision domain, records `STATE` and `PLACEMENT`; no command line, CPU time or memory | §223 (the process view is the supervision domain); milestone 126 (the `procps` package, now `process-tools`) |
| Keeping monit itself running | a declared service, started at install and every boot, restarted on crash and update | milestone 910 (a package declares a service: install starts it, boot starts it, update restarts it) |
| Ending a service | no `kill`; ending a child is its region holder's | milestone 455 (`kill`, `pkill`, `skill` and `snice`: the signaling stratum of `procps`), REFUSED |
| Learning of a death | the kernel's death message to the spawner's supervision endpoint | §26 (the fault endpoint: thread death becomes a message a supervisor holds) |
| Grants (files, network, a domain to watch) | `grant_plan::Manifest` (`domain`, `network`, a directory) in an ELF note | milestone 849 (a package declares what it needs at run time, apart from what it links) for a foreign package |
| Uid checks on the control file | `geteuid` and `st_uid` are both `NOBODY`, so the check passes | none needed |

The sysdep a port writes reads `SURVEY` where Linux's reads `/proc`. Over the domain monit is
handed, it can answer existence and the main thread now, CPU placement as a bonus, and nothing
about memory, CPU time or command lines until the survey grows records for them. Those are kernel
surface, an architect's call, and not in this milestone. So this port's process checks are
existence and restart; `cpu` and `memory` rules report "not available" through the
`available_statistics` path monit already has.

## Where monit overlaps nife's supervision, and what it adds

The overlap is the mechanism. nife already turns a death into a message (§26) and keeps restart
policy in userspace. `sub_server_supervisor` restarts `first_attempt_crasher` per label, from
milestone 22 (trusted init: verify it, and shrink what a broken one can do). monit's restart, done
right on nife, is the same shape: monit spawns the service, so the service's death reaches monit,
and monit spawns it again.

What monit adds is everything above the mechanism, and today nife has none of it on a booted
system. Milestone 854 (the init package holds only what init does) found that nothing on today's
boot starts `root_supervisor` or `sub_server_supervisor`; only tests do. So monit would be the
first restart policy a customer can configure. It brings a declarative file of services, start and
stop programs, timeouts and retry limits. Its health checks look past "is it alive": a port that
answers, a protocol that speaks, a file's checksum or age, a filesystem's free space. And it has
alerts by mail, a status page and a CLI. None of that should be rebuilt in the kernel or the progenitor.

monit's model of a stranger's process is a pid read from a file, the ambient naming §223 refuses.
On nife it sees only the services it spawned or a domain it was granted: a narrower monit, on
purpose. And monit is itself a service under milestone 910, so the progenitor keeps monit running
and monit keeps its own services running, each with the policy it holds.

## The modification question

§262 (nife is measured with the field's standard benchmarks, run unmodified, against Linux) sets
the rule for benchmarks: the program's source is upstream's at a pinned version, and the layer
under it is ours. Risk 1 uses the same word. monit is not a benchmark, but the standard is the one
this tree applies to foreign software, and monit cannot meet it.

- A `sysdep_NIFE.c` pair is a modification by that wording: it is new source compiled into the
  program. It is also the seam upstream built for exactly this, the change least likely to rot and
  the one upstream could take. The alternative that leaves monit untouched is a C library serving a
  `/proc`-shaped directory over the granted domain, so a Linux-host build's `sysdep_LINUX.c` runs
  as is. That puts a compatibility filesystem in the C library to avoid one file in monit.
- A nife-only `Command_execute` on `posix_spawn` (milestone 838) is a patch. It is selected the
  way monit selects a `sysdep`, so `fork` stays the path on every other system. It is §264's stated
  consequence for a C program that forks. It also stops trusting `SIGCHLD` and collects status with
  `waitpid(WNOHANG)` on its poll, or every start program waits out its timeout.
- A nife-only daemon control, from question 2's ruling. The daemon serves milestone 910's control
  endpoint: reload sets `Run_DoReload` and stop sets `Run_Stopped`, the flags the signal handlers
  set today. `monit reload` and `monit quit` send to it instead of `kill_daemon`, because monit's
  HTTP interface carries neither (above). The same change wakes the daemon in-process for HTTP
  service actions, where `do_wakeupcall` signals a pid.
- What does not work and is not patched: `ping` (raw sockets), PAM, and `getifaddrs` link checks.
  Each goes in the block's `BUGS` when built.

So monit reaches nife as a port with three named changes, the ripgrep precedent: `rg`'s own source
untouched, a ten-line patch in a dependency, and the patch nife's to maintain (#1892).

Ruled by calef, 2026-10-12 (UTC), on question 1 (#2022): *"monit 1 of 4, ship it patched as you
recommend."* basalt's recipe carries the changes, they are offered upstream as a platform port,
and monit is recorded as a port with named changes. calef's question 2 ruling added the third.

The comment above `Command_execute` gives six reasons for `fork` over `posix_spawn`: `setsid`,
`setuid`, close-all, `chdir`, `umask`, and what a `clone` or `vfork` child may do. Each assumes a
child that inherits its parent's state and must shed it. A nife child inherits nothing it is not
granted, so none applies; [the appendix](0905-monit-is-packaged-in-basalt-and-restarts-a-nife-service/monit-source.md) takes them one at a time.

## Delivery: a package from basalt

calef's second sentence decides the route. The deliverable is not a binary on a data disk and not a
base-image program (§241 (a threadbare base: the boot slot holds the kernel and what boots and
repairs, and every other program is a package)). It is a package `jig` installs.

1. basalt carries a recipe naming monit 5.35.2's tarball by SHA-256, with the `sysdep` pair and
   the nife patch as files beside it. Its configure line is `--without-pam`, `--without-ssl`
   (question 4), `--without-zlib` unless basalt packages zlib, and the `ARCH` the patch's
   `configure.ac` case selects. A download that does not match fails the build (§195 (a reviewed
   recipe vouches for a package, and the machine's owner may overrule it)).
2. basalt builds it for aarch64, riscv64 and x86_64 against `c_library` at the pinned nife commit,
   with `helpers/c-library-cflags.sh`'s flags. Milestone 897 (SQLite is packaged in basalt, and its
   tests run in basalt's package build) builds that path for the first C package; this is the
   second.
3. The manifest note, for a foreign program, is milestone 849's: the network for its web server
   and checks, the spawn request, and `ENUMERATE` on the domain its services are born under. It
   declares a service under milestone 910: `monit -I`, restart on crash, a control endpoint, a
   read-only default `monitrc` in the package and the owner's `monitrc` in a writable directory.
4. One archive per architecture, kind `optional`, under §197 (a package is one archive file,
   named and vouched for by its recipe). It is published to basalt's index, under
   §250 (an image names its distribution's package index, at `basalt.nifeos.org`, and a package's
   bytes may live anywhere). Milestone 810 builds basalt's first publish job.
5. `jig install monit` installs it. Its `install <name>` is built by milestone 809 (the package
   client becomes a program, `jig`, with the verbs an index needs). It fetches the index by name
   over HTTPS, whose QEMU half is built by milestone 801 (packages over the internet: a host name,
   HTTPS and the distribution's index). monit is served from the opt-in copyleft repository
   (below), so the test first runs `jig add-repository`, milestone 809's unbuilt item 7.

The order across repositories: the nife pieces (`posix_spawn`, sockets, process ids, the manifest
support) land first through nife's queue. basalt then bumps its pin in a separate pull request
citing the landed commit (`nifeos/nife@<sha>`) and adds the recipe; that pull request records the
dependency.

## The license, as facts

- monit's `COPYING` is the stock GNU AGPL v3. Each source file adds an exception permitting
  linking with OpenSSL. libmonit carries MIT-licensed parts from Hanson's CII library.
- Statically linking `c_library` (MIT, from relibc) into monit is permitted by MIT; the linked
  binary is distributed under the AGPL.
- §135 (running GPL software is aggregation, the capability boundary is what makes it so, and
  packages are how it arrives) was amended on 2026-09-27. Copyleft never enters the image or the
  `base` kind. It arrives only in `optional` packages from a repository the user opts into through
  basalt. The distributor publishes corresponding source beside each binary and ships the license
  text in the package. Whether that repository lives under nifeos or a sibling
  organization is still open under milestone 198 (a package manager).
- Shipping the binary obliges basalt, under the AGPL's section 6, to offer the corresponding
  source: the tarball, the `sysdep` pair, the patches and the build scripts.
- Section 13 applies to a modified version, and this one is modified. Whoever runs it must
  prominently offer users who interact with it over a network (its web UI is that interaction)
  the source of that version. Upstream's footer links only to mmonit.com.
- Ruled 2026-10-12 (UTC), question 4, *"a"*: the nife patch adds a footer link to basalt's
  published source for that exact build. basalt publishes the corresponding source beside the
  binary, and monit builds `--without-ssl` until OpenSSL is packaged.

## Done when

On aarch64, riscv64 and x86_64 under QEMU, in `script/swish-check`, from a nife at basalt's pinned
commit:

1. `jig install monit` fetches monit from basalt's index over HTTPS, and the archive's digest
   matches the recipe.
2. The install listed "runs as a service", and monit's daemon is running with no further command.
   Its `monitrc` declares one service: a fixture that listens on a port and faults when told to
   over it, since nothing can `kill` it (milestone 455). monit starts it.
3. `monit status` typed at the prompt prints the service as `OK` with a pid that names it.
4. The test tells the fixture to fault. Within two monit cycles the daemon reports it gone,
   starts it again, and `monit status` at the prompt shows it `OK` under a new pid.
5. `monit reload` at the prompt makes the daemon reread its `monitrc`; after a reboot monit is
   running again and restarts the fixture. The web UI's footer links to the build's source.
6. The recipe's tarball checksum, altered, fails basalt's build.

Every change to monit's source is listed in the recipe and in this block. A lane that finds a
fourth change it needs stops and asks rather than adding it.

## Which fatal risk it informs

Risk 1 (`1-only-software-written-for-nife.md`): only software written for nife runs on nife. monit
is a stranger's C daemon that uses threads, sockets, process creation and a process table. That is
more of the POSIX surface than speedtest1 or ioping touch. It informs the risk honestly only if it
is reported as a port with three changes, not as an unmodified run. As such it is evidence that a
real C daemon needs a `sysdep` and two small patches, and that `fork` and signals are the cost.

## Questions calef ruled

1. **Ship monit patched?** Ruled 2026-10-12 (UTC): yes, as recommended. See *The modification
   question*.
All four on #2022, 2026-10-12 (UTC).

2. **How does the daemon outlive the prompt?** *"a, and restart services on update."* A package
   declares a service; that mechanism is milestone 910.
3. **What is a pid in the C library?** *"a"*. A pid is a tid within a supervision domain the
   process holds, as under §226 (`pidwait` takes tids and composes with `pgrep`, because a program
   does one thing), with no global namespace. It is milestone 838's scope, including the `getpgid`
   fix, recorded in its block.
4. **The section 13 offer.** *"a"*. A footer link to basalt's source for the build, from the
   opt-in repository, and no SSL until OpenSSL is packaged. See *The license, as facts*.

Reuse: monit 5.35.2 itself, with upstream's porting seam; relibc's `posix_spawn` file actions
through milestone 838; `crates/ps`'s survey walk for the sysdep; milestone 810's and 897's basalt
packaging. Writing a nife-native supervisor instead was considered: `sub_server_supervisor` is the
mechanism, and what calef asked for is monit's policy, configuration and checks, which nothing in
the tree has.

## Index row

The first foreign service supervisor on nife, installed by `jig` from basalt. A stranger's C daemon
with threads, sockets and process creation, ported through its own `sysdep` seam and patches off
`fork` and signals. A nife service itself, it restarts a service it spawned and reports it at the
prompt.
