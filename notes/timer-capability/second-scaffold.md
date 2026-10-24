# The second scaffold: reclamation, a kernel thread, and a riscv64 cliff, 2026-09-13

*Name: provisional. For the 2026-09-13 scaffold. calef names files.*

An appendix of [can a userspace process hold a timer?](../timer-capability.md). The parent keeps
each conclusion and its figure. This file keeps the reasoning, the four builds, the error bar and
the symbol-level working behind the `IpcTables` finding that the block of milestone 263 (can a
userspace process hold a timer) points at. The scaffold was built, compiled on all three
architectures and deleted on 2026-09-13; it was never run.

## What the holder dying with a timer armed costs, measured

Priced 2026-09-13 by a second scaffold, deleted like the first. It was built on the shape
[first-scaffold.md](first-scaffold.md) says a real implementation would take: page-resident state
and a generational registry beside `rendezvous_table`, rather than the 64-entry array. Method and
error bars are below.

The answer is 268 bytes of code on aarch64, 212 on riscv64, 276 on x86_64, and zero bytes of
data. What matters is not the size, though, it is the category. The first scaffold's write-up had
the category wrong: this is not bookkeeping that can be deferred to a later milestone.

It is a use-after-free, and it is one the timer interrupt performs. The expiry walk resolves a
registry entry to a page (`phys_to_virt(phys)`) and both reads and writes it (the disarm). A timer
page freed by `MemoryRegion::DESTROY` and handed to somebody else is therefore read and written by
`on_tick`. That happens on every core, for the life of the machine, with no syscall involved and
nothing to correlate it to the process that died. It is the same class of defect `revoke.rs`'s
header opens with: *"wiring up any reclamation before revocation exists turns those 'harmless'
dangling mappings into a use-after-free"*. And it lands on the one path that runs when nothing else
is happening.

So the shape a `Timer` needs is the shape a `Rendezvous` already has, and that is why it is cheap.
`sched::reap_region_objects` already sweeps `rendezvous_table` for objects whose page lies in the
dying region. It goes one at a time, rescanning rather than building a worklist array; that
function's own comment explains why, since it is the deepest frame in the kernel and a scratch
buffer does not fit. A timer phase is the same loop against `timer_table`, placed before the
rendezvous phase, plus one line resetting the cached earliest deadline. Measured as the delta
between two builds that differ only by that loop:

| ISA | code bytes | data bytes |
|---|---|---|
| aarch64 | +268 | 0 |
| riscv64 | +212 | 0 |
| x86_64 | +276 | 0 |

Three properties fall out of the existing design rather than needing to be built. Each is worth
naming because each is a question a reader will ask:

1. A timer armed at a notification that dies first is already harmless. The expiry signal goes
   through `sched::irq_notify`, whose own comment is *"a stale name ... is simply dropped: an
   interrupt with no live rendezvous has nowhere to go, which is not an error."* Generational
   naming does the work; nothing is owed here.
2. The cached earliest deadline may be early but never late, so the reap does not have to
   recompute it. Setting it to 0 costs one extra walk on the next tick and cannot miss an expiry.
   A hint that is wrong in the safe direction is the cheaper half of this design.
3. The registry slot comes back with the page. Without the sweep a dead process's armed timers
   hold registry slots forever. That is a denial of service on a fixed registry, which a process
   can drive by spawning and dying in a loop. The sweep closes that as a side effect of closing the
   memory-safety hole.

What is still not priced here. `PageFrame::REVOKE`'s capability-scoped question (§132 (what)) has
no analogue above. This prices reclamation (object-blind, the `DESTROY` path), not "take this
holder's authority back while the object lives". A `Timer::REVOKE` is a separate question, and this
scaffold did not ask it.

## What serving a kernel thread would cost, measured

This does not decide whether the kernel thread should be served, which is an architect's under
§101 (notification)'s carve-out and milestone 106 (deadline)'s to reopen. It says what it would
cost, because a spike that noticed the question and left it unpriced sends the decision back for a
second round.

The consumer is real and it is in the tree: `kernel/src/soak.rs`'s supervisor. Its own `BUGS` says
*"It yields in a loop rather than blocking on a timer, because this kernel has no sleep-until
primitive a kernel thread can use. That is load on the machine under test."* It is a watchdog,
which is the first item on §101's own list of kernel needs.

A userspace timer service cannot serve it, and neither can `Timer::ARM`. The reason is the same one
for both, and it is not about architecture: a kernel thread runs at EL1 (S-mode, ring 0) and cannot
issue a syscall at all. `syscall::invoke` is reached from the trap path and takes a
`&mut TrapFrame`; a kernel thread never traps. It is not for want of a cspace, which is the thing a
reader expects to be missing and is not. `Thread::spawn_into` gives every kernel thread a
`CapabilityTable::new()`, empty, commented *"it can name nothing until it is handed something."*
The authority is available and the door is not.

So what a kernel thread needs is smaller than the fourth shape, not larger, because inside the
kernel authority is not the question being asked. No object, no capability, no notification, no
syscall: a deadline word on the TCB and a walk that wakes it.

Measured the same way, as the delta of one build against the build above it:

| ISA | code bytes | data bytes |
|---|---|---|
| aarch64 | +312 | 0 |
| riscv64 | +282 | 0 |
| x86_64 | +400 | 0 |

The data figure is the interesting one, and it is exact. `notes/timed-wait.md` predicted a
per-thread `deadline: u64` at zero bytes, on the argument that a TCB is page-resident with slack.
Measured on this tree: `size_of::<Thread>()` is 1,152 bytes with the field, in a 4,096-byte page,
so 2,944 bytes of slack remain. The prediction holds. (`notes/timed-wait.md` quotes 3,352 bytes of
slack, which was true when written; the TCB has grown since, and the conclusion has not changed.)

The whole primitive is `sched::sleep_until(deadline)`. Under `IPC_TABLES` it writes the deadline,
does `handshake.park` on no rendezvous (which is what a `CALL` caller already does), lowers the
cached earliest, releases, and calls `schedule()`. The expiry walk gains a second loop that wakes
threads whose deadline has passed, `serve()` first so the boot-8 wake gate lets it through. It
shares the cached earliest with the object path, so an idle tick still costs one comparison whether
or not anything is sleeping.

And it was applied to the real consumer, which is what makes this a measurement rather than a
sketch. `soak.rs`'s beat loop, a `now()`/`wrapping_sub` deadline comparison wrapped around
`sched::yield_now()` in six lines, becomes `sched::sleep_until(due)`. The kernel builds clean with
`--features soak_test` (spelled `--features soak` when this was measured on 2026-09-13; milestone
297 renamed it the following day). That closes the `BUGS` entry quoted above rather than merely
addressing it.

What this does not tell you. Nothing was run: the scaffold builds and is deleted. So there is no
evidence the supervisor wakes on time, only that the code the wake would run compiles and that its
cost is four hundred bytes or less. The parent's `BUGS` section carries that.

## Method and error bars, both scaffolds

Four kernels were built, each differing from the one above it by exactly one change. Each was built
release for all three ISAs and measured with `llvm-nm --print-size`, splitting symbols by type: `t`
is code, and everything else (`d`, `b`, `r`) is data.

| build | what it adds | aarch64 code | riscv64 code | x86_64 code |
|---|---|---|---|---|
| B0 | nothing; `origin/main` at `7317cdff` | 193,984 | 163,990 | 140,842 |
| B1 | the fourth shape, page-resident | 199,148 | 165,830 | 146,618 |
| B2 | the reap sweep | 199,416 | 166,042 | 146,894 |
| B3 | `sleep_until` and the thread walk | 199,728 | 166,324 | 147,294 |

The error bar is zero bytes. B1 was built, reverted to B0, and rebuilt from the same patch, and both
B1 runs produced byte-identical figures on all three ISAs. Nothing here is within noise of
anything, because there is no noise.

`ipc_fastpath` does not move, reproducing the 2026-09-05 measurement exactly: 7,028 / 5,936 / 8,122
before and after, on `script/fastpath-footprint`. `syscall_entry` moves by +12 B on aarch64 (+0.8%)
and +96 B on x86_64 (+5.9%). Both are identical to the figures recorded on 2026-09-05, a
reproduction across a week of unrelated commits and a differently-shaped scaffold. riscv64 came out
at +68 B (+3.7%) here against +158 B then. The page-resident shape is the cheaper one on that ISA,
and the earlier figure is not wrong; it priced a different structure.

## One finding that is not about timers at all

B1's data figures did not behave, and the reason belongs where somebody pricing the next addition
to `IpcTables` will find it. The registry itself is arithmetic. A
`generational_table::Table<u64, 512>` is 512 `Option<u64>` (16 bytes each, no niche) plus 512
`u32` generations plus two `usize`, so 10,256 bytes. `IPC_TABLES` grew from 15,432 to 25,696 bytes
on every ISA, which is that plus the cached-earliest word. Expected.

riscv64 then paid for it a second time. In B0 that ISA's largest `.rodata` symbol is three bytes.
In B3 it carries a 25,680-byte anonymous `.rodata` aggregate, which is `EMPTY_TABLES` materialized
as a template to copy from rather than a struct initialized in place. aarch64 and x86_64 have no
such symbol in either build. So riscv64's data delta is +37,061 bytes against aarch64's +10,980 for
identical source. It is a cliff rather than a slope: the struct crossed a size the backend treats
differently. The next field added to `IpcTables` may cost that ISA nothing, or another whole copy
of the struct.

Nothing in this tree records that, and it is not specific to timers: it prices every future
addition to the scheduler's tables. It is left here rather than acted on, because a lane does not
choose `MAX_TIMERS`, and because the right response may be to shrink the registry rather than to
chase the backend.

And it makes `MAX_TIMERS` a real decision rather than a constant copied from `MAX_RENDEZVOUS`. At
512 it is 10 KiB of kernel data, doubled on riscv64. At 64 (the 2026-09-05 scaffold's number) it is
1.3 KiB. This lane picked 512 to mirror the rendezvous registry and has no evidence that is right.
(Since then, verified 2026-10-11 UTC: milestone 106 built the timer with `MAX_TIMERS = 256` in
`kernel/src/sched.rs`.)
