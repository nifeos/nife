# The process view: a listing is a capability, not a fact about the machine

Milestone 126 (the `procps` package), the view stratum. `ps`, `pgrep` and `top` work here, and none of them can enumerate
the machine. What each can
see is one supervision domain, because somebody handed it the endpoint that supervises one. There is
no `/proc` to open, no pid space to scan, and no call in a program's reach that takes a process id it
was not shown.

This note is why that shape, what it costs, and the one place the authority is currently wider than
the job.

## The problem, which the reader already agrees with

`ps aux` on Linux reads `/proc`, and `/proc` is ambient: any process gets it with no grant from
anyone. So the listing is every process on the box, including the ones with secrets in `argv`. Nobody
defends that design; they live with it, and `hidepid` exists because enough people stopped wanting
to.

That is what makes this a good first demonstration of the argument milestone 121 makes for
directories. Enumeration is a larger power than reading something you were handed, and the claim
needs no setup: the reader already knows the Unix behavior is wrong.

## The design: a view over a supervision domain

The scope is the supervision domain, because the kernel already maintains it (§223 (the process view
is the supervision domain)). A thread's
supervision endpoint is recorded at `START` (`Thread::fault_ep`, DECISIONS §26) and never changes.
The set of threads whose deaths arrive on one endpoint is therefore a set the kernel keeps for its
own reasons, exactly maintained, and it costs nothing to read.

So the domain a viewer sees is the endpoint it holds, one level deep: a grandchild appears only if
its spawner named the same endpoint at `START`. A monitor over unrelated services holds `ENUMERATE`
on each one's endpoint. Same move `rm -r` makes with a directory it was handed: authority is held,
not global. A scope the system already keeps cannot drift out of
agreement with reality, which is the property a registry would not have had.

The wide grant is not forbidden. It is nameable. An operator's monitor over the whole machine is
a program handed the endpoint that supervises the whole machine, and `caps ps` prints which one that
is *before* anything is spawned. On Linux there is no such distinction to print, and that is the
entire difference. This is Fuchsia's job-handle shape: their `ps` needs a handle to the root job to
see everything, and a handle to a smaller job to see less.

## The surface: one new method, no new syscall

`abi::endpoint::SURVEY`, method 6 on an endpoint capability. A new method inside the established
capability model, not a new syscall number.

```text
invoke(cap, SURVEY, cursor, record, 0) -> (next_cursor, tid, word)
```

- `next_cursor` returns in x0 (a0 on RISC-V), `tid` in x1, and the selected record's word in x2.
- Start with `cursor = 0`. Feed each `next_cursor` back. `abi::survey::DONE` (zero) means finished.
- A negative first word is an `abi::Error`.
- Needs `ENUMERATE`, and pointedly not `READ`. `READ` on a supervision endpoint is what `RECEIVE`
  and `endpoint::REAP` take, so a viewer holding it could reap a child; a domain names its members
  and does not act on them (calef, 2026-08-17). See `capability::Rights::ENUMERATE` and the first
  `BUGS` entry in [process-view/limitations.md](process-view/limitations.md), which is the finding
  this right came from.

`state` is one of `abi::survey`'s four codes: `READY`, `RUNNING`, `BLOCKED`, `DEAD`. Only four,
because a supervised thread cannot be found in the other two. `Embryo` has not run, so it has no
recorded supervision endpoint yet; `Finished` is what *un*supervised death looks like, and a
supervised thread dies into `DEAD` and waits for its supervisor.

### The rest of the method, in brief

The third argument, `record`, is a selector: it names which per-thread fact comes back in x2, and a
new fact is a new value rather than a fourth return register (calef, 2026-09-21). `record::STATE` is
0, so every caller written before the selector still reads what it read. `record::PLACEMENT` (1)
is the cpu the thread was placed on when it started, not where it runs now. Treat that cpu id as a
name and never iterate `0..count`, because the online set is not a range. An unknown record is
`BadMethod`, refused before the walk; the rights check runs first.

Membership is `fault_ep == invoked_ep` and nothing else, proved both ways by Kani harnesses: a
stranger is never shown, and a member is never hidden. The scheduler lock is given back between
entries, so a survey is a sequence of snapshots with `readdir`'s bargain. The selector's design,
the placement cost argument, the proofs and the cursor's safety argument are in
[process-view/survey-design.md](process-view/survey-design.md).

## Refused, empty, and populated are three answers

This is the deliverable, not a detail. A monitor that reports nothing because it *could not look*
reads exactly like a quiet machine, which is the worst failure this tool has available. `filesystem_contract`
chose `EPERM` over an empty listing for the same reason (milestone 108's shape).

| what the viewer holds | answer |
|---|---|
| the endpoint with `ENUMERATE`, domain has members | the rows |
| the endpoint with `ENUMERATE`, domain is empty | `DONE` on the first call: an answer |
| the endpoint **send-only** (`WRITE`, no `ENUMERATE`) | `NotPermitted` |
| the endpoint with `READ` but not `ENUMERATE` (a supervisor that was never widened) | `NotPermitted` |
| nothing in the slot | `NoSuchSlot` |

`pgrep` adds a fifth that is not about authority at all: a selector that matched nothing in a
domain that really has members. See the `pgrep` section below.

The send-only case is the interesting one and it is a real relationship in this tree: a peer that
reports *to* a supervisor holds exactly that. It may send here and it may not look, and the kernel
says so rather than answering with a plausible nothing.

The fourth row is the one the 2026-08-17 rights split added, and it is the direction a reader is
least likely to expect. `READ` is the *stronger* right on this object (it unlocks `RECEIVE` and `REAP`)
and it still does not unlock the view. That is deliberate. The two are not ordered, because
receiving deaths and naming members differ in kind rather than in degree, so a holder that wants
both is granted both, which is what the kernel tests' `hold_supervisor` does.

Nothing this system ships holds both, and that fell out of the split rather than being designed:
`system_initializer` endows `job_undertaker` with `READ` on `deaths` and nothing else, and a `ps`
with `ENUMERATE` on `deaths` and nothing else. So the program that can free a job's memory cannot
enumerate the jobs, and the program that lists them cannot touch one. Before the split there was one
bit and both would have held it.

All four are asserted in one kernel test on both ISAs
(`kernel/src/user/survey_tests::a_viewer_without_the_domain_is_refused_rather_than_shown_an_empty_list`),
and the empty case is in the same test on purpose: neither claim means anything without the other.

## What `ps` is, concretely

Two halves at the IO boundary, which is the crate-and-program pair convention.

- `crates/ps` is the listing: the cursor walk, the buffer, the columns, the refusal catalog.
  Host-tested in milliseconds, nine tests, and total for *every* reader including one that never
  advances its cursor.
- `components/src/ps.rs` is the syscall and two sinks, about sixty lines.

The kernel's survey tests drive `ps::collect` against the real `endpoint::SURVEY`, so the cursor
protocol is proved end to end rather than by a second copy of the walk written in a test.

Collect first, complain second, print third. The rule of DECISIONS §67 (a program's second stream is a declaration) is that a
program says everything it has to complain about and closes its second stream before it writes a byte of output.
The reader drains diagnostics to end-of-stream first. A survey cannot know its complaints up
front (an endpoint can die mid-walk), so the whole domain goes into a buffer, then diagnostics, then
the table.

The buffer is the caller's, and that was a gate's doing. It began as a `[Row; MAX_ROWS]` local,
which made `collect`'s frame 4,336 bytes. That is larger than the 4,096-byte guard page under every kernel
thread stack. So one call could move `sp` past the guard in a single step and land in a neighboring
thread's stack without ever faulting. `script/stack-frame-check` failed the build and named the
shape, which is the second time that gate has caught a `[T; MAX]` local wearing the clothes of a
bound. A caller-provided slice is the fix it recommends and is better anyway: a program that sizes
its own listing knows where the memory came from, and `ps` sizes it at `MAX_ROWS` while the kernel's
tests size it at eight.

A compile-time assertion in the kernel's survey tests keeps `ps::MAX_ROWS` at least as large as
`sched::MAX_THREADS`, so the shipped program has no truncation case. A caller with a shorter buffer
does, and it is not silent: `Survey::is_complete` is false, nothing is printed, and diagnostics
say the domain has more in it. Same rule as the refusal: a monitor never reports less than it saw
without saying so.

## Where it comes from at the prompt

`Manifest::domain` is a declaration, `Manifest::clock`'s twin: a process domain is not a name a
person types, so there is no token to place and no refusal to write. What the field does is tell
The progenitor which children to endow, and tell a person reading `caps ps` that the authority exists.

Init places the endpoint in `grant_plan::DOMAIN_SLOT` (seven) with `ENUMERATE`, using the same named-slot
mechanism §67 gave the diagnostics stream, for the same reason. How many low slots a child gets
depends on what else the line granted it, and a program that probes a fixed number needs that number
not to move.

The endpoint it places is `deaths`, which is what supervises every job the progenitor spawns for the shell.
So `ps` at the prompt lists this shell's jobs, including itself, and nothing else. Init, the shell,
the terminal, the filesystem server, the compositor, the net stack and every driver are outside it,
which is why `ps | wc` at the boot gate counts a handful of lines where a `/proc`-shaped listing
would count dozens.

## EXAMPLES

At the prompt:

```text
$ ps
         TID  STATE
           5  blocked
           9  running

$ ps > running.txt        the table lands in the file
$ ps | wc                 the table is counted, and no /proc was read to make it
$ caps ps                 the scope, printed before anything is spawned
  ps would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 7  endpoint  domain   ENUMERATE. the processes this shell's jobs are
                              supervised by, and no others. it can name them and
                              do nothing to them: not receive their deaths, not
                              collect them, and not learn anything about a process
                              outside this domain but that it exists
    ...
```

The refusal, which has no Unix equivalent because on Unix there is no domain to be outside of:

```text
$ ps        ps: this process holds no process-domain capability
```

From a host test, with no kernel at all:

```rust
let mut reader = |cursor: u64| match cursor {
    0 => (1, 7, abi::survey::RUNNING),
    _ => (abi::survey::DONE as i64, 0, 0),
};
// The row buffer is the caller's, because a `[Row; MAX_ROWS]` local outgrew the guard page; see
// below. `ps` sizes it at `MAX_ROWS`, the kernel's tests at eight.
let mut rows = [ps::Row::default(); ps::MAX_ROWS];
let survey = ps::collect(&mut rows, &mut reader);
assert_eq!(survey.rows()[0].tid, 7);
```

## `pgrep`, and the `pkill` that cannot exist

On Unix, anything that can find a process can end it, because `kill(pid)` turns a name into an
action. Here a domain names its members and does not act on them (calef, 2026-08-17). A survey
returns a tid, which is a name and not a capability, and `abi::tcb` has no method that takes a tid
and ends the thread. Killing stays with the shell that spawned the job, which holds the region. So
`pkill` cannot be assembled out of a view, and `procps` is ported without its signaling stratum.
A kernel test asserts that the capability that printed a corpse's tid is refused the reap.

`crates/pgrep` is the filter and `components/src/pgrep.rs` the syscall, with `ps`'s three grants
exactly. It tells four answers apart where upstream's exit 1 tells none: matched, none matched,
empty domain, and refused. Its pattern is a glob over the four run-state words, because a process
has no name. The prompt cannot deliver a pattern today, so the shipped program is `pgrep '*'`. The
whole argument is in [process-view/pgrep.md](process-view/pgrep.md).

## BUGS

Each entry is in full, with its mechanism and history, in
[process-view/limitations.md](process-view/limitations.md).

- Userspace cannot learn the machine's online cpu set, so a placement census cannot show an idle
  core. Do not work around it with `0..count`.
- A placement can be stale: it is where the thread was put, not where it is.
- A thread observing itself was ruled to get a per-thread page, and that page is not built.
- Holding a domain with `READ` was more authority than looking needs. Fixed 2026-08-17 by
  `Rights::ENUMERATE`.
- The cursor and the tid are machine-wide slot indices, so a viewer can count threads outside its
  domain, though it can never name one. Recorded-accepted, with a proposed milestone.
- A process has no name, so there is no `CMD` column.
- A survey is a sequence of snapshots.
- A child that is built but not yet started is not in its domain.
- The comparison against Linux is not apples to apples. Ours lists a domain; theirs lists a machine.
- `pgrep`'s selector cannot be typed at the prompt, it matches glob rather than a regular
  expression, and it has no exit status to report with.
- `ps` lists itself, and in a pipeline it may or may not list its own reader.
- A doomed thread does not say so.
- `pmap` cannot be reached from the interactive prompt against any real process.
- `pmap` shows one row per mapped page, and cannot tell a device mapping from ordinary memory.
- A resumed `LIST` cursor can land on a recycled log slot.

## `pmap`, and `ENUMERATE` extended to the address-space object

`pmap` works on both ISAs over `abi::address_space::LIST`, the same split one object type over:
`MAP_INTO` needs `WRITE`, and `LIST` needs `ENUMERATE`. The listing reads the space's own
revocation log, so it costs nothing new and cannot disagree with what a revoke would find. But
`ThreadControlBlock::CONFIGURE` removes a space from the registry the instant it binds to a thread, and nothing
delegates an address-space capability to a second program. So no program but a space's own builder
can hold a live view of it today, and `pmap` runs only in kernel tests. Fixing that is a
spawn-protocol question. The delegation audit and the finding are in
[process-view/address-space-enumeration.md](process-view/address-space-enumeration.md); the pricing of `pmap` from the prompt
is in [process-view/pmap.md](process-view/pmap.md).

## `watch` was built, and then cut

`watch` went on 2026-09-13 with `crates/watch`. It held exactly `ps`'s grants and was `ps`'s own
loop. In a capability system, two programs are two programs when they hold different authority, so
identical capability tables mean the split draws no boundary. The live view came back as `top`,
ranked by per-thread CPU time. The account is in [process-view/watch.md](process-view/watch.md).

## What this does not build

`w`, `pidwait` and `pmap` from the prompt, each now a proposal of its own. `sysctl` is declined
(§115 (no `sysctl`)), so is `pwdx` (§224 (no `pwdx`)), and the signaling stratum is refused
(milestone 455 (the signaling stratum of `procps`)). The machine-wide statistics are built beside
this view, under §225 (`free` sees the machine and your share): see
[the machine and your share](process-view/the-machine-and-your-share.md). The forks are written up
in [what is left](process-view/remaining-procps-programs.md).

See `design/roadmap/0126-who-else-is-running.md`, notes/glob.md (the matcher `pgrep` reuses),
notes/supervision.md (the mechanism this reads),
notes/pipes.md (the second stream), and notes/program-manifest.md (how the grant is declared).
