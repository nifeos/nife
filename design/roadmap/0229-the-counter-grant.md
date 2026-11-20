---
status: BUILT
raised: 2026-09-02
built: 2026-09-02
---
# 229. Build the cycle-counter grant DECISIONS 139 decided

This is the build work for DECISIONS §139 (who may read the cycle counter, and by what authority).

It was minted `Gate: NONE`, and that held: DECISIONS 139 was already answered, the default was made
a fact rather than an assumption by milestone 228 (the cycle counters are closed by assumption), and
nothing here needed a board. What only a board can settle is in `BUGS` below.

In brief. DECISIONS 139 is `DECIDED` in three parts, and this milestone is the first two of them:

- Option 4, a per-thread grant enforced at the context switch. A granted thread runs with the
  counter open, every other thread runs with it closed, and the kernel writes the enable on the
  switch the way it already writes the address-space root.
- The grant is a field in the spawn manifest, not a method on a live thread, so a program cannot
  acquire a timing side channel it was not given at creation. DECISIONS §28 chose the same shape for
  thread placement at ratification.
- **`x86_64` is not part of this.** It keeps its ambient counter, for reasons 139 measured rather
  than assumed, and §19 (architectural parity is a tenet) should read that as a stated exception and
  not a gap.

## Why the cost is known before the work starts

139 priced it: one comparison, and one `msr` only when the value changes. That is exactly the
shape `kernel/src/arch/aarch64/mmu.rs`'s `switch_user_root` already has, which early-returns when
`TTBR0_EL1` already holds the wanted value. If no thread is granted, the value never changes and the
whole cost is a compare.

`script/fastpath-footprint` has a 5% bound and it fired on a lane today (milestone 221's soak
counters grew the aarch64 IPC path 5.7%). The switch is not the IPC fastpath, but it is adjacent
enough that the gate should be run rather than assumed.

## What it is not

**It is not milestone 147** (a profiler that holds exactly the counters it was granted). This says
*this thread may read the counter*, not *this profiler may read that subtree's counters*. 147 wants
cross-thread authority with a named target, which neither this nor any option in 139 provides, and
139 records that explicitly so nobody reads this as having built it.

**It does not buy timing confinement**, and `notes/confinement-claims.md` now carries the row saying
so: two threads and a shared word reconstruct a 6.8 ns clock on any of the three architectures. What
this buys is accountable authority, meaning the cheap accurate path is granted rather than ambient
and the kernel knows which threads hold it. Anything this milestone prints or documents must not
imply more.

## What was built

2026-09-02. Four pieces, and the third is the one that had to be measured rather than argued.

The grant is a bool on the TCB, `Thread::cycle_counter_grant`, set by
`sched::grant_cycle_counter`, which **refuses a thread that is not an embryo**. That refusal is the
security property rather than housekeeping. `CONFIGURE` and `CAP_INSERT` make the same one, so the
things you may do to a thread before it runs are exactly the creation-time manifest DECISIONS 139
part 2 asked for, and there is no later. One-way: an embryo starts closed and nothing but this
opens it.

And there is deliberately no system call method to call it, which is the one thing about this
milestone a reader will otherwise assume somebody forgot.

The lane built one (`abi::thread_control_block::GRANT_CYCLE_COUNTER`, method 3) and calef removed
it on review, on an argument the lane did not have. DECISIONS 139 declined a first-class capability
object partly because seL4's own RFC-16 for that shape has been unmerged since 2024-02-02. The
prior art for this shape is `seL4_TCB_SetAffinity`, a per-thread property expressed as a TCB
method, which MCS deleted outright and replaced with a core that is a field of
`sched_control_cap`. seL4 could not see that corner coming. This tree can: milestone 147 (a
profiler that holds exactly the counters it was granted) is already written down, wants cross-thread
authority with a named target, and no method here would provide it. Minting a method number whose
retirement is already on the roadmap is spending an irreversible thing on a path with a visible
end.

**The alternative was never method-now against capability-now.** 139 records that 147's
target-naming has no precedent in this tree to price from, so the object is not buildable yet
either. It was method-now against **not yet**, and not-yet costs almost nothing, because the grant
has no consumer: milestone 74's aarch64 half is the first and does not exist. Whoever needs it
mints it, with a requirement in hand.

**A field on `CONFIGURE` is not the alternative either, and that was established by looking.**
`invoke` carries three argument registers; `CONFIGURE` spends all three (entry, user stack,
address-space slot) and `START` spends all three on the child's first registers. There is no spare
word, so expressing the grant through the existing methods would mean widening `invoke` itself,
which is a larger and equally irreversible change than the method being deferred. The finding is
kept because the next person needs it: it is the reason the cheap-looking door is not a door.

The standing note lives in `crates/abi`'s `thread_control_block` module, where a reader looking for
the method meets the reason it is absent. **`Thread::cycle_counter_grant` and every arch name here
are provisional.**

The switch writes it beside the address-space root. `sched::schedule` reads the incoming
thread's bool out of the same locked block that reads its `ttbr0`, and calls
`arch::timer::set_cycle_counter_grant` immediately after `mmu::switch_user_root`, which is the
`adopt_address_space` shape one register over. The arch call compares before it writes, so a
machine where nothing is granted never writes the register at all.

Two honest implementations, not a shared abstraction, which is the question the block below
left open. The two registers disagree on both halves, and each disagreement decides one
implementation:

- aarch64 caches what it last wrote. `PMUSERENR_EL0` does not exist without FEAT_PMUv3, so
  "read back what the hardware holds" (which is exactly how `switch_user_root` compares
  `TTBR0_EL1`) is not available on the architecture. A per-core `AtomicBool`, plus a per-core
  record of whether the register exists at all, which `timer::init` already had to work out for
  milestone 228's write. The grant is `CR` (bit 2) alone: `EN` would open every PMU register and
  `ER` the event counters, and 139 granted neither.
- riscv64 reads `scounteren` back and changes one bit. The CSR always exists, so the read is
  available; and it is *required*, because `TM` shares the word, is open for every thread by
  design, and `crates/user_rt`'s `now()` breaks if it is ever cleared. A cached implementation
  would have had to model `TM` too, which is the more moving parts, not fewer.
- `x86_64` is an `#[inline(always)]` empty function. 139 part 3, and the measurement below
  says it costs literally nothing.

One call site, three implementations behind one name, which is what `switch_user_root` itself
already is. A trait would have added a vocabulary without removing a difference.

### What it costs, measured

`script/fastpath-footprint`, this branch against `8a418981` built in a separate worktree:

| ISA | `ipc_fastpath` | `system_call_entry` |
|---|---|---|
| aarch64 | 5852 -> 5988 (+136 bytes, +2.3%) | 3304 -> 3304 (**unchanged**) |
| riscv64 | 5132 -> 5148 (+16 bytes, +0.3%) | 1870 -> 1870 (**unchanged**) |
| `x86_64` | 6687 -> 6687 (**unchanged**) | 1637 -> 1637 (**unchanged**) |

The gate is green on all three. `system_call_entry` is byte-identical everywhere, which is what
minting no method means measured rather than asserted, and `x86_64` is byte-identical on both,
which is the no-op compiling to nothing rather than a coincidence. That is the closest thing to
"an ungranted kernel is unchanged" that is actually true: on the other two the switch mechanism has
to exist in the image, and what an ungranted thread pays at runtime is a load, a compare and a
return.

The gate fired once, on the ABI that was then removed, and the number is worth keeping because
it is an argument. Written as an ordinary arm inside `invoke`, the method put 224 bytes on
riscv64's `system_call_entry`, 12% against the 5% bound, because `invoke` inlines into the dispatcher
every system call fetches. Extracting it `#[inline(never)]` the way milestone 156 (`system_call_entry`'s measured size) extracted the other
spawn-path bodies brought it to +1.5%; removing the method brought it to zero. A method on this
object is not free even when nothing calls it, which is one more thing to weigh when somebody
mints the real surface.

### How it was verified

`script/test` green on all three architectures, with three new tests.

- A live thread cannot be granted the counter (`sched::tests`). It asks on its own behalf,
  which is the strongest available form of the question: the caller is `Running` by definition of
  executing the line, and holds every authority a kernel thread has. `WrongObject`, the same
  refusal `CONFIGURE` makes on a started thread.
- The write is idempotent and does not disturb its neighbors (`sched::tests`). Four calls
  where two change anything, then on riscv64 the assertion that carries this milestone's asymmetry
  argument: `scounteren.CY` is clear and `scounteren.TM` is still set.
- A granted EL0 thread reads the counter, and an ungranted one is killed for trying
  (`user::tests`), both halves in one test because each is the other's control. The same program
  is run twice: granted it reports, ungranted the read traps and `USER_FAULTS` counts it. This is
  the closed-side test an earlier draft of this block said it had not earned; removing the ABI is
  what made it available, because the thread now runs from the kernel side where both cases are
  observable. The counter values are carried and not checked, because QEMU leaves `PMCR_EL0.E`
  clear and `PMCCNTR_EL0` reads zero there forever.

What that test buys through a back door, and what the back door costs. With no ABI there is no
honest userspace route to a granted thread, so the grant is applied by a `#[cfg(test)]` function,
`sched::grant_cycle_counter_to_current`, which deliberately breaks the embryo rule and cannot exist
in a shipped kernel. Same spirit as the `soak` and `fastpath_pad` affordances this tree already
carries. What is therefore not exercised end to end is the embryo-only path a real ABI would go
through; `sched`'s own refusal test covers that rule directly, and the two together are what the
one EL0 test used to cover alone.

The negative half does not run on `x86_64`, where `rdtsc` is ambient and an ungranted read is
not an error. That skip is DECISIONS 139 part 3 showing up in a test rather than a gap in one.

## BUGS

- Nothing verifies the granted state on silicon. QEMU's `PMUSERENR_EL0` is almost certainly zero
  either way, which is the same blindness milestone 228 recorded, so a green run says little. The
  bench item belongs beside 127's existing `PMCCNTR_EL0` one.
- The riscv64 half is `scounteren.CY` and is not symmetrical with aarch64's, because one CSR
  carries both the coarse and fine permissions there while aarch64 splits them across two registers.
  Answered above, with the reason: two implementations behind one call site, because the registers
  disagree about whether they can be read back and about whether anything else lives in the word.

- No userspace can use this. There is no system call method, by decision (above), so the only
  caller of the grant in a shipped kernel is nothing at all, and `sched::grant_cycle_counter`
  carries a `not(test)` dead-code allow to say so out loud. A reader who wants the counter opens
  the question of what the surface should be, which is milestone 74's or 147's to answer.

- The EL0 test grants through a `#[cfg(test)]` back door, so the embryo-only rule is proven by
  a unit test rather than through the path a program would take. Nothing is proven about an ABI
  that does not exist, which is the honest state of it.

- **Every name here is provisional**: `Thread::cycle_counter_grant`,
  `arch::timer::set_cycle_counter_grant`, `cycle_counter_grantable`,
  `sched::grant_cycle_counter` and its test-only twin. Names are an architect's.
- No consumer exists yet, and that is still true after this. The only exerciser is the kernel's
  own test. Milestone 74 (cycle counters) is the first real consumer, and
  it is also the owner of a portable userspace read. The raw `mrs`/`csrr` here lives in the one test
  program that needs it rather than in `crates/user_rt`, so that 74 designs that API instead of
  inheriting one from a test vehicle.

## Follow-on

- **Milestone 127.** Verifying the granted and ungranted states on real silicon. QEMU's
  `PMUSERENR_EL0` is almost certainly zero either way, so a green run says little, and this block
  puts the bench item beside 127's existing `PMCCNTR_EL0` one.
- **Milestone 74.** The userspace surface, and the first consumer. There is deliberately no system call
  method, so in a shipped kernel nothing calls the grant at all. 74 is the first real consumer and
  also owns a portable userspace read, which is why the raw `mrs`/`csrr` here lives in the one test
  program that needs it rather than in `crates/user_rt`. Milestone 147's cross-thread,
  target-naming authority is the other claimant, and neither this nor any option in DECISIONS 139
  provides it.
- **Recorded.** `design/roadmap/0229-the-counter-grant.md`: the riscv64 half is `scounteren.CY` and
  is not symmetrical with aarch64's `CR` bit, because one CSR carries both the coarse and the fine
  permission there while aarch64 splits them across two registers.
- **Recorded.** `design/roadmap/0229-the-counter-grant.md`: the EL0 test grants through a
  `#[cfg(test)]` back door that breaks the embryo rule and cannot exist in a shipped kernel. The
  embryo-only path a real ABI would take is proven by a unit test rather than end to end.
- **Recorded.** `design/roadmap/0229-the-counter-grant.md`: every name here is provisional and names
  are an architect's, including `Thread::cycle_counter_grant`,
  `arch::timer::set_cycle_counter_grant`, `cycle_counter_grantable` and
  `sched::grant_cycle_counter`.

## Index row

the kernel half is complete and the system call method is deliberately not minted: a grant on the TCB,
written at the context switch beside the address-space root, aarch64 `PMUSERENR_EL0.CR` cached,
riscv64 `scounteren.CY` read back so `TM` survives. `x86_64` is a no-op that leaves its fastpath
byte-identical
