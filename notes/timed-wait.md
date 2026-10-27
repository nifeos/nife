# What a timed wait costs

*(Written 2026-08-17 to price the fork of milestone 106 (a wait that ends on either the interrupt
or the deadline); §147 (a timer a userspace service cannot hold) settled it;
[timer.md](timer.md) has what was built. The measurements behind each section are in
[`timed-wait/`](timed-wait/README.md).)*

Milestone 106's block says a deadline in the blocked state "means the scheduler carries a timer
wheel or an ordered deadline list, which is scheduler work the kernel does not do today". That
sentence is the only cost estimate five consumers have ever been weighed against, and every clause
of it turns out to be either wrong or beside the point. This note replaces it with numbers.

Name: provisional. `timed-wait` takes the phrase the tree uses in five places ("there
is no timed wait anywhere in the kernel", roadmap 51, roadmap 106, notes/clock.md, notes/ntp.md,
notes/pipes.md) rather than inventing a second one. The roadmap file beside it is
`0106-deadline-wait.md`, and the two words are the same idea from the caller's side and the
scheduler's. calef has not ruled on it.

## The price list

Every number below was taken from this tree between 13:00 and 13:20 on 2026-08-17. What each was
measured on, and its error bar, is in its own section.

| what | today | with a deadline | how it was measured |
|---|---|---|---|
| bytes per thread | `size_of::<Thread>()` = 744, in a 4096-byte page | 752, in the same page | compile-time probe, both ISAs |
| bytes of BSS | 0 (the pool is gone; TCBs are page-resident) | 0 | `sched.rs:81`, milestone 19c (real).2 |
| the idle tick, per core | ~491 instructions aarch64, ~400 riscv64 | +30 (aarch64), +31 (riscv64) | `llvm-objdump`, debug build, executed path |
| the idle tick, as a fraction of a core | n/a | under 3 parts per million | 30 instructions x 100 Hz against a 1 GHz core; less on a faster one |
| per-tick comparisons when nothing is due | n/a | 1, identically for all three data structures | host model, 100,000 ticks |
| interrupt-stack chain | 4352 B aarch64, 4160 B riscv64, of 16384 | 3888 B / 4208 B | `script/stack-depth-check`, prototype wired in |
| a context switch is reachable from the interrupt stack | no | still no, both ISAs | the same gate, gating mode |
| threads blocked at once, suite peak | 97 of 128 (mean 37, live peak 102) | unchanged | `#[cfg(test)]` census, 31,371 samples, full suite |
| one second of the current yield-spin | a whole core-second, 10^5 to 10^6 syscalls | ~12,400 instructions | derived from `bench/baseline-*.txt` and notes/benchmarks.md |

The two numbers that decide anything are the last two, and neither is a data structure.

## 1. A per-thread deadline is free, and the block's premise is stale

The brief for this lane said "`MAX_THREADS` is 128 and the thread pool is a static BSS array". That
was true at milestone 14 (kernel) phase B.2 and stopped being true at 19c.2. The static pool is
gone, and every `Thread` now lives at the start of one page drawn from `kmem` or from a user process's own
memory region (`sched.rs:80-93`, notes/thread-control-block.md). The table is `generational_table::Table<ThreadControlBlockPointer, 256>`, about 2 KiB of
pointers.

So the question "what does a per-thread deadline field cost in bytes" has an answer nobody has to
weigh: `size_of::<Thread>()` is 744 bytes in a 4096-byte page. Measured, not reasoned, on both
ISAs, with a compile-time probe (`let _: [u8; 1] = [0u8; size_of::<Thread>()];` and read the error).
Adding `deadline: u64` makes it 752. There are 3352 bytes of slack in that page before and 3344
after.

- It changes no size class, because the size class is a page and the page is paid by whoever owns
  the thread.
- It changes no cache behavior worth naming. A `u64` appended to a struct spanning 12
  cache lines lands in the twelfth, which the expiry walk touches and nothing else does.
- It costs no BSS at all, which is the number the stale premise would have made 1 KiB.

This is the same for all three of milestone 51's shapes. A `SYS_SLEEP`, a timer object and a deadline
on `RECEIVE` all have to record, somewhere reachable from the tick, when this thread's wait ends.
The TCB is the cheapest place in all three cases, and it is free in all three cases.

## 2. The timer wheel is the wrong question

The block offers "a timer wheel or an ordered deadline list" as if those were the options. Three
candidates were modeled on the host over 200,000 ticks: no structure at all (a per-thread deadline
word plus one cached `earliest`), a sorted deadline list, and a hashed timer wheel. The case that
dominates is churn, an armed wait cancelled before it fires: an ACK arrives and the retransmit timer
is thrown away. [`the-three-structures.md`](timed-wait/the-three-structures.md) has the table and
its error bars. Three things fall out:

- The idle tick is one comparison, the same one for all three, because every shape can cache the
  earliest deadline in a word. So "the scheduler carries a timer wheel" is not a cost the fork has
  to weigh.
- The sorted list is the worst option at every occupancy above one. Its insert is O(k), and inserts
  outnumber expiries by the churn rate.
- Scanning wins until about 64 threads hold deadlines at once, and the wheel wins past that. The
  five known consumers are on the order of one deadline each.

## 3. What the timer interrupt path costs, and what a deadline scan adds to it

This is the number that matters most, because it is paid on every tick on every core. Today's
timer tick runs about 491 instructions of Rust on aarch64 and 400 on riscv64, counted statically in
the debug build. A prototype deadline check in `on_tick` (read the counter, compare against a cached
`EARLIEST_DEADLINE`, walk only if it fired) adds exactly +30 executed instructions on aarch64 and
+31 on riscv64. That is one comparison per idle tick, under three parts per million of a 1 GHz
core. The static totals over-count, so the ratio is a lower bound and the delta is exact.
[`the-tick-path.md`](timed-wait/the-tick-path.md) has the per-function table and the method.

## 4. The interrupt-stack proof holds, and the reason it holds was already in the tree

Nothing on an interrupt stack may context-switch, and `script/stack-depth-check` proves it. A
deadline expiry keeps that proof, because a wake is an enqueue, not a switch: `irq_notify` already
wakes threads from the interrupt stack, and the switch is deferred to `preempt_if_needed` one frame
out. A prototype `expire_deadlines` was wired in and the gate still found no context switch
reachable, on both ISAs. What it does cost is the whole-machine `SCHED` lock on every tick, which is
why the cached `earliest` is load-bearing rather than an optimization: it is a word, not a wheel.
[`the-interrupt-stack.md`](timed-wait/the-interrupt-stack.md) has the gate output and the argument.

## 5. What the current spinning costs, so the comparison is not against zero

Five consumers spin today. The `std` PAL's `thread::sleep` loops on `rt::yield_now()` until its
deadline, and `net_stack`'s `wait_for_nic` does the same over every retransmit backoff. From the
tree's own icount baselines, one second of that spinning is between 10^5 and a few times 10^6
syscalls, and 100% of a hart. The same second waited on a deadline is about 12,400 instructions,
with the hart in `wfi` for the rest.

The ratio is at least 10^5 to 1, and it grows with core clock. That is the number the fork is
about, five orders of magnitude larger than any difference between the shapes or the data
structures. It is derived from measured primitives rather than measured end to end, so the
magnitude is uncertain by about ten. [`the-cost-of-spinning.md`](timed-wait/the-cost-of-spinning.md)
has the derivation.

## Where the three shapes actually differ

Sections 1 through 4 are common to all three of milestone 51's candidates: the same TCB word, the same
cached minimum, the same one-comparison tick, the same wake from the interrupt stack. Where they part
company is not the scheduler at all.

A `SYS_SLEEP` and a timer object need nothing else. The thread blocks with a deadline and no
counterparty; the expiry wakes it; it returns.

A deadline on `Rendezvous::RECEIVE`/`CALL` needs two things the other two do not, and both are concrete:

1. A targeted unlink from an endpoint's wait queue. `crates/intrusive_queue`'s `Queue` is
   singly linked: one `next` per node, `head`/`tail`/`len`, and its whole API is `push_back`,
   `pop_front`, `is_empty`, `len`. `inter_process_communication::Rendezvous` adds only
   `drain_waiters`, which drains *all* of them. Removing one specific waiter is a new method,
   O(queue length) from the head, on a crate that carries machine-checked proofs of its one-queue
   invariant, so the proofs move with it. Bounded by `MAX_THREADS`, paid only on an actual expiry,
   and the census in section 6 says a queue can hold most of the table.
2. Nothing else. The half that looks harder is already built. `thread_wake_handshake`'s undelivered-wake
   gate refuses a wake with `wait_on.is_some()` and nothing delivered (boot 8). So a timeout looks
   like exactly the wake the gate exists to stop. But `Handshake::abort()` already passes the gate for
   this reason, and `sched::set_ipc_aborted` + `wake` is already the pair that revocation uses on a
   drained waiter (`sched.rs`, the `doomed_eps` walk). A timeout is that pair with a different reason.

And one hazard shape 3 carries that is worth naming before it is discovered, because it leaves the
machine the moment a program is written against it. A `CALL` that times out leaves a live `Reply`
capability naming a thread that is no longer waiting. `ipc_reply` already refuses to deliver to a
thread not parked as `WaitRole::Reply`, and its comment says why in exactly these terms ("a stale
reply whose CALL was aborted long ago, its caller re-parked elsewhere"), so the dangerous case is
covered. The residual is narrower: `Object::Reply(tid)` carries a `ThreadId` and no per-call nonce, so a
caller that times out and then issues a *second* `CALL` is Reply-parked again, and the first call's
stale reply would satisfy the second. This is latent today (revocation can produce the same
situation) and a deadline on `CALL` would make it reachable on purpose. It wants a lane of its own,
whichever shape wins.

## 6. How many threads are actually blocked at once

`MAX_THREADS` is 128 and the asymptotics in section 2 turn over between 64 and 128, so the occupancy
is worth knowing rather than assuming. A `#[cfg(test)]` census was sampled from inside `schedule()`'s
decision (which already holds `SCHED` and already has the table), every 64th call, over the whole
aarch64 suite:

```
106-pricing: blocked-census samples=31371 mean_x100=3695 max=97 live_max=102
```

**Mean 36.95 threads blocked, peak 97, against a live peak of 102 of 128.** So this suite genuinely
runs near the ceiling, and "128 is theoretical" would have been wrong.

Two caveats that matter for how the number is used:

- Blocked is not deadline-holding. These are threads parked on endpoints, overwhelmingly with no
  timed wait in sight. The deadline-list occupancy is bounded above by this and in practice would be
  the number of consumers in a timed wait, which is single digits. Section 2's table should
  be read at `occ` = 1 to 16, not at 97.
- 97 is what a targeted unlink walks past. The one place the blocked count is the *right* number
  is shape 3's `Queue::remove`, which walks an endpoint's wait queue from the head. This says that
  queue can be long, so the walk is O(97) rather than O(3) in the worst case the suite reaches.

## BUGS

- No end-to-end measurement of the spin exists. Section 5 is derived from the tree's gated icount
  baselines and its HVF medians rather than from a run that counted `SYS_YIELD` calls during a real
  retransmit backoff. The order of magnitude is uncertain by about ten; the conclusion is not.
- Section 2 is a host model, not a kernel measurement. It counts the comparisons and pointer
  writes a kernel implementation would perform, at the points it would perform them. It does not
  model cache behavior, and its wall-clock column was taken on a machine running two other lanes'
  gates (load average 9 to 25) and is therefore not reported. Two runs at different loads agreed on
  the ordering of every row.
- The 491 and 400 instruction totals in section 3 are static whole-function counts and include
  arms a timer tick does not take, so they over-count. The +30 / +31 deltas do not: those are executed
  paths of one function. Treat the ratio as a lower bound and the delta as exact.
- No release-build number is reported. Everything here is the debug build, because that is what
  this tree gates and what its icount baselines measure. The release figures would be smaller and
  nothing here turns on them.
- The prototype was thrown away. The `deadline` field, the cached `EARLIEST_DEADLINE`, the
  `expire_deadlines` walk and the census were built to obtain the numbers above and are not in the
  tree. Reproducing them means writing them again; the shapes are described precisely enough here
  that this is a morning's work, and shipping them would have settled the fork by accident, which
  milestone 51's block and milestone 106's both warn against.
- **Nothing here says which shape to choose.** That is the point: this note exists to make the fork
  decidable, and the decision is an architect's.

## A miscitation found on the way

Milestone 106's block, milestone 103's block and `notes/pipes.md` all cite the timed-wait fork as
"DECISIONS §51". `design/decisions/0051-sink-protocol.md` is the sink protocol; the fork is in
`design/roadmap/0051-wall-clock-time.md`, a milestone block rather than a decisions section. This is
exactly the failure `CLAUDE.md` warns about ("`script/decisions --check` verifies that a cited `§N`
resolves to *some* section, never that it resolves to the right one, so a well-formed wrong citation
is invisible to it") and the collision `MEMORY.md` records between `§N` and milestone N as two
numbering schemes. This lane corrected 106's block; #1291 fixed `notes/pipes.md` and #1296 fixed
103's, both 2026-09-25. `design/decisions/0043-clock-authority.md`'s "What this lane deliberately did not do" gets it
right, saying
"the milestone block's fork".
