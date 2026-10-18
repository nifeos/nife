# What is left of `procps`, and the forks it waits on

An appendix to [the process view](../process-view.md), written 2026-09-26 by the lane
`milestone/126-procps` for milestone 126 (the `procps` package: who else is running, and who is
allowed to ask). Every remaining program in the package is blocked on a fork rather than on effort.
This file holds each fork with the seven questions AGENTS.md asks of one, so that a ruling can be
made without reading anything else. Since 2026-09-26, sections 2, 4 and 5 are each a milestone or proposal of
their own (`w`, `pidwait` and `pmap` from the prompt), and milestone 126 is built. Sections 1, 3 and 4 are decided: §224 (no `pwdx`), §225 (`free`
sees the machine and your share) and §226 (`pidwait` takes tids). The rest is not. The milestone block
(`design/roadmap/0126-who-else-is-running.md`) carries the status; this carries the reasoning.

The stem `what-is-left` is a provisional name, minted with this file. Nothing here adds a kernel
method or a syscall, and nothing here was built. Where an option would
need one, the option says so and stops.

## 1. `pwdx`: the premise the block carried was wrong

Decided 2026-09-26: option A. calef: *"Yes, decline pwdx."* The ruling is §224 (no `pwdx`), and
the reason is the one below: only the shell has a working directory (`grant_plan::nav::Cwd`), so
`pwdx` has nothing to report. The fork is kept as it was written, since it is the reasoning.

The block filed `pwdx` beside `w` as "print a name for a tid", blocked on a display name. That is
not what `pwdx` does. Upstream `pwdx PID` prints the process's current working directory, read from
`/proc/PID/cwd` (recalled from the `procps-ng` manual page, not re-read). So `pwdx` never needed a
display name, and it was waiting on the wrong fork for a month.

The real question is sharper. A working directory here is not a kernel fact. It is a value,
`grant_plan::nav::Cwd`, inside the shell's own `Holdings` (`crates/grant_plan/src/command.rs`). A grep
for `Cwd` across `components/` and `crates/` finds it in `grant_plan` and the shell (`swish`) and
nowhere else. Other programs receive grants, not a position to resolve against. So on this system
exactly one kind of process has a working directory, and it can already print it with `pwd`.

What is being decided: whether `procps` ships a `pwdx` at all.

| option | what it takes | why it loses, or wins |
|---|---|---|
| A. Decline it, as `sysctl` was (§115 (no `sysctl`)) | a line in the package table | wins; see below |
| B. The shell answers a query about its own position | a request protocol the shell serves, and a capability to ask it | a new protocol two programs agree on, for a fact only its owner has any use for |
| C. The kernel keeps a cwd per thread | a path in the kernel | wrong on its face: the kernel does not resolve paths, and a `Cwd` names components inside one capability, which means nothing to anyone who does not hold it |

The tree's analogous case is `sysctl` and `pkill`: a program whose Unix job is reaching into state
the caller does not hold was declined, and the gap was recorded rather than papered over. Option B
has the same shape as `watch` before milestone 281 (`watch` holds exactly what `ps` holds): it would
answer a question the caller could answer itself. Prior art, recalled: Plan 9's `/proc/n/ns` ends
with the process's `cd` line, and Fuchsia keeps the working directory in the process's own `fdio`
state with no way for another process to read it. Neither kernel holds a path.

Cost: A is free. B is a protocol, a manifest field and a test on three ISAs, for a program with one
possible subject. Reversibility: A is reversible, since nothing is written against its absence.
Same-cost test: A still wins at equal cost, because B's subject is a process that can already
answer.

Recommendation: A. `procps` ships without `pwdx`, and `pwd` is the answer for the one process that
has a directory to report. This is reversible, so it is a recommendation rather than a list.

## 2. `w`: two things missing, and one of them is a ruling already proposed

Upstream `w` prints who is logged in and what each of them is running, with idle time and CPU time
per session. Measured against this tree on 2026-09-26:

- Who is logged in: `components/src/login.rs` runs one session at a time on one terminal ("The
  terminal: single-session, deny cleanly"). A `w` would always print one row.
- What they are running: a tid has no name. That is exactly DECISIONS §164 (whether the kernel resolves
  a tid it already sent), `PROPOSED` in `design/decisions/0164-resolving-a-tid-a-supervisor-holds.md`. §164's option A is the block's old
  "kernel-resident name", and its option C is the block's "userspace-resident name". The two forks
  were the same fork, written twice.
- CPU time: now exists, since milestone 282 (a thread's CPU time) built
  `abi::survey::record::CPU_TIME`.

So the block's own display-name fork is withdrawn in favor of §164, which is the one written where
rulings live. What 126 adds to §164, and what §164's "what is blocked" section does not yet list:
`w`'s `WHAT` column and `ps`'s missing `CMD` column are both consumers. The authority answer the
block already gave still stands for whichever option wins. A viewer holding `ENUMERATE` on a domain
learns each member's tid and state, and a name is more information about a member already named, so
it needs no stronger right.

Recommendation: no `w` until §164 is ruled and a second session can exist. Building a one-row `w`
now would demonstrate nothing a reader could not see by looking at the terminal.

## 3. `free`, `vmstat`, `slabtop` and `tload`: machine-wide statistics

Decided 2026-09-26. calef: *"Rule 3."* A third shape that combines the two options below: option 1's
region method for the caller's share, plus a read-only machine memory page held as a capability,
granted to every login by default and withholdable by the owner. `free` prints a machine line, and
a "yours" line when the caller holds a region. The ruling, its reasons and the prior art checked
against primary sources are in §225 (`free` sees the machine and your share). The fork is kept as it
was written. Built the same day: `free`, `vmstat`, `slabtop` and `top`'s machine line, recorded in
[the machine and your share](the-machine-and-your-share.md).

The block's 2026-08-26 fork covered `free` and `vmstat` and missed the other two members of the row.
Re-checked 2026-09-26: `kernel/src/memory.rs`'s `stats()` and `free_page_frames()` are still read
only by the boot summary and by kernel tests (`self_test.rs`, `testing.rs`, `sched.rs`, `user.rs`),
with no path to userspace.

The fork, restated in one line: does "how much memory is free" carry the region-scoped authority
everything else here carries, or is it the second machine-wide ambient fact after monotonic time?

1. Per-region: a new method on `MemoryRegion`, gated by `ENUMERATE`, answering how much of the
   region behind this capability is committed. `pmap`'s shape one object type over, §114
   (`ENUMERATE` extends to the address-space object). `Rights::ENUMERATE`'s own rustdoc already
   names `MemoryRegion` as the next object to grow it. It is a new kernel method, so it stops here.
2. Machine-wide: a figure nobody holds a capability to. It needs a new kernel-to-userspace channel,
   and it is the first fact about memory a program could learn without holding any.

This touches the syscall surface either way, so it gets options and no winner.

The two members the block missed each change shape under this fork:

- `slabtop` has no subject. Milestone 14 (kernel objects from untyped) removed the kernel heap and
  its slab allocator; `kernel/src/sync.rs` records that the `HEAP` and `SLAB` lock ranks left with
  them. Kernel objects are carved from regions their holders own. So the question `slabtop` asks,
  which kernel caches are using memory, becomes option 1's question asked by object type. Under
  option 2 it has nothing to read at all.
- `tload` draws a load-average graph. No decaying load figure exists, as `crates/uptime`'s `BUGS`
  already say. A domain-scoped one could be computed in userspace from `SURVEY`'s state record,
  needing no new kernel state. But that program holds exactly `ps`'s three slots, so milestone 281's
  rule (two programs are two programs when they hold different authority) says it is not a program.
  If anything, it is a line in `top`'s summary. A machine-wide load is option 2 again.

Blocked on this: `free` and `vmstat` entirely, and whatever `slabtop` and `tload` become.

## 4. `pidwait`: `pgrep`'s authority, waiting

Decided 2026-09-26, and not as recommended below. calef: *"D."* `pidwait` takes tids and composes
with `pgrep`, as `pidwait $(pgrep foo)`, on his principle that a program does one and only one
thing. It holds different authority from `pgrep`, so milestone 281's rule allows two programs. The
refusals, the prior art and what the building lane owes are in §226 (`pidwait` takes tids). The
recommendation is kept as it was written.

`pidwait` blocks until every matching process has exited. It asks the same question `pgrep` asks,
over the same domain, and so holds the same three slots. By milestone 281's rule it is a mode of
`pgrep` rather than a program. Whether that rule applies is calef's, as it was for `watch`.

How it would wait, checked in `kernel/src/syscall.rs`:

- Receiving the death message is `rendezvous::RECEIVE`, which needs `READ`. A viewer holds `ENUMERATE`
  only, and a receive would also take the message from the supervisor it was meant for. So that
  route is closed, correctly.
- Polling `SURVEY` until the tid is gone works today with no new authority. With no timed wait in
  the kernel, it is a yield-spin, the same cost that got `watch` cut.

And there is nothing to wait for from the prompt yet: `pgrep` cannot be given a pattern at all,
since its manifest is `ProgramArgumentDeclaration::Forbidden` until milestone 47 (navigation and naming) grows
positional arity.

Considered and refused: no `pidwait` program, only a wait mode on `pgrep` once milestone 106 (a wait
that ends on either the interrupt or the deadline) gave it a real sleep and milestone 47 gave it an
operand. That was this section's recommendation before calef ruled. §226 refused it as option A
(`pgrep --wait`): disorienting to users, since a program named for finding would also block.
`pidwait` ships as its own program instead, taking tids and composing with `pgrep`.

### After the ruling: how `pidwait` would see an exit (options, 2026-09-26)

§226 (`pidwait` takes tids and composes with `pgrep`) chose a separate program that is named tids.
Checked against the tree the same day by `milestone/126-free`:

- The shell has pipes and no `$( … )`, so the composable form is `pgrep | pidwait`, with `pidwait`
  reading decimal tids on its input. That needs nothing new.
- The tid `pgrep` prints is the full generational name (`crates/ps`'s `write_thread_id`), so a
  reused slot gets a different tid and cannot alias one `pgrep` already printed. A 32-bit generation
  can wrap, which is not a practical race.
- A new finding: `pgrep | pidwait` puts both programs in one domain, so `pgrep` prints `pidwait`'s
  own tid. A `pidwait` that cannot recognize itself waits for itself forever. No program here can
  learn its own tid, so whatever primitive is chosen has to refuse, or skip, the caller's own tid.
- Nothing lets a program observe a named tid's exit with less authority than `pgrep` holds.
  `RECEIVE` needs `READ` and steals the supervisor's message; `SURVEY` needs `ENUMERATE`, which is
  `pgrep`'s authority, and using it would undo the reason §226 made this a separate program.

So `pidwait` needs a new kernel primitive, which is the syscall surface. Options only, no winner:

| option | shape | cost and objection |
|---|---|---|
| 1 | A method on the supervision endpoint that blocks until a named member has exited, gated by a new right below `ENUMERATE` | a new right bit and a method; the kernel needs a queue of exit-watchers beside the supervisor's death message. A holder can still test a guessed tid for membership, one bit per call, but cannot list. Refuses the caller's own tid |
| 2 | The same method under `ENUMERATE` | refused by §226's own reason: it is `pgrep`'s authority |
| 3 | A per-child exit capability the spawner retains and hands on | Fuchsia's shape, recalled rather than re-read: a process handle with a wait right. Capability-exact, but it cannot compose with `pgrep`'s output, which is bytes, not capabilities |
| 4 | Notification objects (§101 (notification objects), decided and unbuilt), signaled by the supervisor on each death | builds §101 first, and `pidwait` would still need a way to tell which tid died without `ENUMERATE` |

Every option blocks in the kernel, so none needs milestone 106's timed wait. Nothing about
`pidwait` is built until one is chosen. The input side (reading tids from a pipe) is small and
waits with it, because a `pidwait` that cannot wait is not worth shipping.

## 5. `pmap` from the prompt

Unchanged since 2026-08-23 and re-checked: `ThreadControlBlock::CONFIGURE` still removes a space
from the registry `address_space::LIST` reads (`take_user_address_space` in `kernel/src/user.rs`),
so every live space reads as empty to any capability that named it. `crates/grant_plan` still has no
`pmap` program variant.

What has moved is where the fix would live. DECISIONS §142 (what a spawner retains over a child
after `START`) made retention a declared field on `ChildEndowment`, and declared it empty. A shell
that ran `pmap` on its own child would have to declare a retained, `ENUMERATE`-narrowed view of that
child's space. That is two changes the tree has not made: a non-empty retention, which §142 left for
a later ruling, and a space that stays listable after `CONFIGURE`, which is a change to the object's
lifecycle. Both are architect's calls. `pmap <tid>` also needs an operand, so milestone 47 again.

## 6. Where the process view comes from, a decision owed

The block recommended deriving the view from the supervision tree, and the tree took that option by
construction when `ps` shipped over `rendezvous::SURVEY`. No file under `design/decisions/` records
it, so a non-subtree view (a monitor watching two unrelated services) is neither built nor refused.
A lane may not write that section, so this was draft text for an integrator. It is now minted as
§223 (the process view is the supervision domain), with the seven questions answered there, and
calef decided it 2026-09-26: "A, and refuse B." Two corrections came with it: the view is one supervision domain rather than every
descendant, and a monitor can watch unrelated services by holding each one's endpoint.

The process view is the supervision subtree. A viewer holding `ENUMERATE` on a supervision endpoint
sees that endpoint's domain and nothing else. A set of processes that is not a subtree is expressed
by a supervisor whose purpose is to be their common parent. Refused: a separate process namespace
with its own capability, which can express any set but can also disagree with the tree.
