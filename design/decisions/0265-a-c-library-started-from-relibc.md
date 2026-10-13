---
status: DECIDED
raised: 2026-10-08
decided: 2026-10-08
ratified_by: calef
---

# 265. A C library started from relibc, whose Rust platform layer holds the capabilities

*The file name is a lane's coinage.*

## The ruling

The benchmark ruling on PR #1854 (nife is measured with the field's standard benchmarks) requires
the standard benchmarks to run unmodified. The nine programs it mints as milestones 826 to 834 are
all POSIX C. nife had no C library that runs one: §31 (the foreign-language seam) stops at tier two,
`malloc` and `free`, and milestone 478 (tier three: full POSIX behind the foreign-language seam) was
REFUSED until "a component somebody needs that cannot be adapted to the narrower tiers". Its nine
are that component, nine times over, because adapting a benchmark is what that ruling forbids.

Four options were put to calef: A, keep 478's refusal and adapt the benchmarks; B, relibc, Redox's C
library written in Rust; C, musl; D, picolibc or newlib. calef, 2026-10-08 (UTC):

> B, although I suspect we end up building our own with a similar approach. We have different
> constraints than they do.

A loses to the benchmark ruling directly. C and D are C libraries whose system-call layer is C, so
the C would make the syscalls, which is the thing §31 exists to prevent. Giving them a Rust layer
underneath means writing relibc's `Pal` seam again for a codebase not designed around one. relibc is
the only one of the four built on that seam already.

## What the evaluation found, and the shape it gives

notes/c-library.md measured relibc against nife's constraints before anything was built. Its
recommendation, adopted here as the shape of the work (reversible, and calef's expectation made
concrete):

- Take relibc's architecture and its OS-neutral code once, then own the result. relibc's header
  modules (about 38,000 lines: `stdio`, `string`, `stdlib`, `time`, `ctype` and the rest) call the
  OS only through a `Pal` trait. That code and that seam are seeded into an in-tree crate nife owns,
  at a recorded relibc commit with its MIT notice and provenance in each seeded file.
- Do not track upstream as a vendored engine. Redox does not accept LLM-generated contributions
  (redox-os/redox `CONTRIBUTING.md`, March 2026), so a nife platform layer can never go upstream.
  The divergence (the whole platform layer, startup, TLS, 149 `cfg(target_os)` sites outside the
  layer) would outgrow the code it patches. Upstream fixes are ported by hand from reading, and a
  bug nife finds in relibc goes back as a bug report.
- Leave behind `redox-rt` (fork, signals and process management emulated in userspace, 7,485 lines),
  the Redox and Linux platform layers, the dynamic linker, dlmalloc, and every dependency a stage
  does not use. Each dependency a stage keeps is a §46 (thin primitives or whole subsystems) ruling
  named in its `Reuse:` line.
- No fork, ever (the fork ruling, PR #1856). `fork` returns `ENOSYS` and says why in its `BUGS`;
  `popen`, `system` and `forkpty`, which call fork from relibc's generic code, are rewritten on
  `posix_spawn` or refused. `posix_spawn` is already a `Pal` primitive in relibc and maps onto
  milestone 172 (a capability-native subprocess primitive).
- Staged by consumer. Stage 1, files, clock and memory, is milestone 835 (a C library, stage 1:
  files, clock and memory), first consumed by ioping (834), SQLite's speedtest1 (831) and STREAM
  (832). Stage 2, threads, is milestone 836 (a C library, stage 2: threads), with milestone 812
  (`std::thread::spawn` runs real threads in one address space), which PR #1856 rules is to be
  built. Stage 3, sockets, is milestone 837 (a C library, stage 3: sockets), for iperf3 (828) and
  netperf (829). `posix_spawn` is milestone 838 (a C library: `posix_spawn`, and no fork), with
  milestone 172.

## §31 rule 1, amended

§31 rule 1 read: *"The C makes no syscalls and holds no capabilities."* It now reads:

> **The C makes no syscalls directly; the C library's platform layer, in Rust, holds the
> capabilities.**

The C calls `open`. The library's Rust platform layer turns that into a `filesystem_protocol`
request on the directory capability the process was granted. No C translation unit contains a
syscall instruction. relibc's `syscall()` function and `<sys/syscall.h>` are not built for nife.
The platform layer speaks the same protocol crates `std` speaks (notes/std.md), so the two runtimes
cannot disagree about a wire format.

### Why confinement still holds, and a correction to how §31 said it

§31 justified rule 1 as *"a property of what it can name: a syscall needs a capability slot, and the
C never sees one."* That overstated it, and the record is corrected here on purpose. A slot is a
small integer, and the C shares an address space with the Rust code that holds the slots: a memory
corruption bug in the C can overwrite the shim's state or reach its `svc` instruction. That was true
of `c_seam` in July and it is true of a C library now.

What actually confines the C is the kernel, and it does not depend on the language or on the C's
good behavior: **a process can do exactly what its capability table allows, and nothing else.** That
is what §31's tests proved, from outside the faulting address space: the witness pages outside the
grant were unchanged. Moving `open` behind a C library changes none of it. The worst a hostile or
broken C program can do is everything its own process was granted, which is the same bound every
Rust program on nife lives under. So the rule that carries the confinement is the one that was
always carrying it: **grant a C program only what it needs**, by its program manifest
(notes/program-manifest.md) and the slots its parent fills.

What rule 1 still buys, and why it is kept rather than dropped:

- The syscall surface does not widen. The library speaks existing capability protocols; no
  syscall number or method is added for C. This is §4 (kernel shape) rule 2, the syscall surface
  stays narrow, and §10 (process model: capability-based, microkernel). §31 cites this as rule 3,
  which is its number in CLAUDE.md's list rather than in §4. A C program is a client of the same
  servers as a Rust one.
- One place to audit. Every way C reaches the outside world is a Rust function in one crate's
  platform layer, reviewable and testable on the host.
- Errors map by meaning. The layer translates protocol results to `errno`, the way `std` maps
  them to `io::ErrorKind`, so a missing grant is `EACCES` or `ENOSYS`, not a crash.

## What this does not decide

- The crate's name, the note's name and this file's name are provisional; names are calef's
  (design/naming.md).
- Where the C library's crate lives and how clang finds its headers are milestone 835's to decide,
  as is whether cbindgen is taken as a dependency or the headers are written by hand.
- Whether STREAM's number is meaningful before the userspace targets stop being softfloat. It is
  not, and that is milestone 534 (the soft-float targets could now be flipped), not this library.
- Signals. relibc emulates POSIX signals; nife has none, and stage 1 provides `raise` and `abort`
  only. A benchmark that installs a signal handler gets a `sigaction` that records it and never
  fires, with a `BUGS` entry, and anything more is a separate fork for an architect.