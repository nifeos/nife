# Gaps in the shared suite: one open, the rest closed or corrected

*Name: provisional.*

An appendix to [notes/riscv-parity-scope.md](../riscv-parity-scope.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It holds the ASID gap milestone 58 (RISC-V TLB shootdown) closed,
the 24-test
overlap still open, the leak police that never policed `user::tests`, and two misread test failures.

## Closed gap: RISC-V allocated ASIDs and then threw them away

`riscv64::mmu` did the whole ASID dance. `asid_bits()` probed the implemented width at boot,
`ttbr0_value` packed the ASID into `satp[59:44]`, `flush_asid` existed for teardown. And then
`write_satp` ended with a bare `sfence.vma`, which invalidates everything, on every address-space
switch. So the tagging cost what it cost and bought nothing: RISC-V was still swinging the
sledgehammer aarch64 put down at milestone 15.

Found while deciding whether `asid_tagging_keeps_address_spaces_apart_without_flushes` could be
ported. It could not, honestly, and that was the finding.

Milestone 58 closed it, and the sequencing is the lesson. The flush was not merely slow, it was
covering for two things. `flush_asid` was local, because `sfence.vma` does not broadcast and RISC-V
has no hardware equivalent of `tlbi aside1is`. And `satp.ASID` may be zero bits wide on conforming
hardware where aarch64 mandates eight, so `crates/address_space_identifier`'s 255 numbers rest on an
assumption that
holds on one ISA and not the other. So: the SBI RFENCE shootdown first, then the removal, gated on a
boot-time probe rather than on the specification. The aarch64 witness now runs on both ISAs, and
`an_asid_flush_reaches_the_other_cores` proves the broadcast half on both. The benchmark did not
improve, and cannot on an emulator that models no TLB-miss cost. See notes/riscv-tlb-shootdown.md.

## Open gap: `tests` and `riscv_virtio_tests` overlap by 24 tests

`riscv_virtio_tests` was written when `tests` was unreachable from RISC-V, so it re-derives the disk,
FS and network properties with its own copies of `wait_for`, the net selectors, and the transcript
plumbing. Now that `tests` runs on both, 24 of its tests are gated to aarch64 solely because the twin
exists.

The finishing pass measured the overlap rather than guessing at it, and the answer is more
encouraging than "not textually identical" suggested. Of the 24 shared names, nine bodies are
byte-for-byte identical (the five socket-contract tests, `std_net`, the two smoltcp DHCP tests, and
the FS server's stack-headroom check). Of the fifteen that differ, thirteen differ only in which
image drives the driver: aarch64 passes `init_image()`, because there the virtio driver is a role
of `hello`, and RISC-V passes `blk_image()`, the dedicated binary. The remaining two differ only in a
comment and in an assertion message. There is no behavioral divergence anywhere in the 24.

So the merge is smaller than it looked. Pick `blk` on both ISAs; a dedicated binary is the better
choice regardless, and hello would keep its role for nothing. Delete `riscv_virtio_tests`' 24 copies
along with its duplicate `wait_for`, net selectors and image helpers. And keep its two genuinely
RISC-V-only tests (`a_faulting_user_thread_is_killed_and_the_kernel_survives` and
`the_page_tables_say_u_mode_cannot_read_the_kernels_memory`, the software-walk twin of the `AT S1E0R`
test). Still not done, and deliberately: it doubles the aarch64 leg's slowest tests during the
transition and it is a change to what the *disk and network* tests prove, which is a different
subject from making the userspace suite portable. It is now a measured piece of work rather than an
estimated one.

## CLOSED 2026-08-02: `no_leaked_threads` has never policed `user::tests`

Fixed, and the scope below turned out to be right on every point. The gap is closed, the two
spinners are reaped, and the probe (now `thread_leak_police`, named to sort after `tests`) runs last
and is green on both ISAs: 216 aarch64, 215 riscv64.

Three things worth keeping from doing it, because the analysis below could not have predicted them:

- The bug bit on CI before the fix landed, on 2026-08-02, exactly as this section forecast and in
  the forecast's own words. `reclaim_frees_a_started_then_exited_childs_regions` ran 90 s against a
  budget it normally clears in under 5, tripping the watchdog on a pull request that had touched
  only `dtb` and `nifefs`. The starvation reaches a test on a branch that cannot have caused it,
  which is what made it look like a flake worth re-running. It was not a flake.
- The probe was proven to bite before being believed. Leaking the spinner on purpose fails it,
  `1 thread(s) are still runnable after the suite quiesced`, with the dump. A reordered probe that
  has never failed is not evidence it polices anything.
- The free-frame shift this section flagged as the reason it needed a full run did not materialize.
  Killing `untyped_demo` frees its frames, and every later baseline in the suite was expected to move;
  both ISAs pass unchanged. Recorded because the risk was real and correctly identified, and the
  measurement is what retires it rather than the argument.

The original analysis follows, unedited, because the plan it lays out is what was built.

---

The leak police sorts by module path, and `no_leaked_threads` sorts before `tests`. So the one module
whose whole subject is user threads has never been checked for leaving any behind, which is how four
never-exiting spinners accumulated in it unnoticed until one of them starved
`reclaim_frees_a_started_then_exited_childs_regions` off a four-hart machine.

Measured, not estimated. Moving the probe to run last (rename the module so it sorts after
`tests`) and having it report rather than assert gives, on the merged tree:

```
[PROBE] leaked runnable = 2, table = 87     # aarch64
[PROBE] leaked runnable = 2, table = 87     # riscv64
```

Identical on both legs. The two are `untyped_demo` (pc deep inside `hello`) and `interrupt_ignorer` (pc at its
entry, a tight loop), exactly the two named below. The other 85 threads in the table are Blocked,
which is the healthy steady state: they are the long-lived userspace servers earlier tests started,
waiting on endpoints. A thread dump full of Blocked user threads is not a leak, and reading it as one
sends the investigation to the wrong place.

Worth stating plainly, because the direction is counter-intuitive: de-gating the module did not make
this worse in aggregate. aarch64 carried four of these before (`interrupt_ignorer`, `untyped_demo`,
`printing_client`, `self_check_client`); making the two one-shot roles `exit()` cut it to two. RISC-V
went from zero to two, because the module did not run there at all. So RISC-V now sits in exactly the
condition aarch64 has been in for many milestones, rather than a worse one, and aarch64 improved.

Two of the four were one-shot roles with nothing left to do and now `exit()`. The other two cannot,
and that is the obstacle:

- `a_user_program_that_never_yields_is_preempted_anyway` runs `interrupt_ignorer`, whose entire point is that
  it never yields, never syscalls, and never returns. A thread that exits would not test anything.
- `a_process_spends_untyped_and_the_kernel_never_allocates` reads the kernel's free-frame count the
  instant its child reports. If the child exited there, the number read would be the teardown's
  rather than the measurement's, so the child spins to hold the state still.

Both are spawned bare (`sched::spawn` around `user::run`), not into a reclaimable region, so
there is no `reclaim_region` to arm the kill with and no other way for a test to tear down a user
thread it started. Making the module policeable therefore means giving the kernel a way to kill a
bare user thread by `ThreadId`, which is a kernel change with its own design questions, not a test
reordering. Recorded here rather than done under a test lane. Reordering the runner *without* that
change would simply turn a silent gap into a permanently red gate.

What the change would be, precisely, so it can be scoped without rediscovering it. The mechanism
already exists and is proven. `reap_region_objects` sets `t.killed = true` on every live thread in a
region, and the scheduler converts a killed thread to a corpse at its next preemption (DECISIONS §16
(object revocation)'s
armed kill, the tier §24's `^C` escalation stands on). What is missing is only the ability to name
one thread instead of a region.

1. `sched::kill_thread(tid: ThreadId)`, test-support, roughly ten lines: take `SCHED`, resolve `tid`, set
   `killed = true`. No new syscall and no change to the user-visible surface (rule 3 is about the
   syscall boundary; this is an in-kernel function for in-kernel tests).
2. The two tests above kill their subject after asserting, and wait for `is_thread_present` to go
   false.
3. `no_leaked_threads` then moves to run last, and polices the module for the first time.

The risk that makes it its own piece of work rather than a footnote: killing `untyped_demo` frees
its frames, so every later free-frame baseline in the suite may shift. That is precisely the class of
change that needs a full run on both ISAs to believe, and it is why it does not belong bolted onto a
test-portability lane.

## CORRECTED 2026-08-02: most of this was a placement bug, not host load

The section below is kept because its measurement is sound and its conclusion was wrong, and the
way it was wrong is the useful part.

`every_secondary_runs_scheduled_work` was failing because each secondary's probe was spawned with
plain `sched::spawn`, which places by §28's power of two choices: the lighter of two randomly
sampled cores, deliberately not the spawner's. The probes scattered, and any core nobody sampled
never set its `RAN_ON` slot. The test was waiting on a condition that could not become true, and
passed only when the random placement happened to cover every core. Fixed by spawning the probe with
`spawn_on(cpu::id(), ..)`, which is what the test always meant.

Why "host-load sensitive" was a believable wrong answer. A random placement moves between runs
*exactly* the way a contended runner does. On 2026-08-02 it failed three pull requests in a row on
three different RISC-V CPU models, and that wandering was read as evidence about the host. It is
equally the signature of nondeterministic placement, and nothing in the failure distinguishes them.

What distinguished them was a fix that did not work. Widening the wait from 10 s to 60 s changed
nothing. A deadline cannot fix an unreachable condition, so the failure surviving a 6x wider bound
ruled out slowness in a way no amount of re-running could have. The corroboration was in the same
logs the whole time: `a_batch_of_cpu_bound_work_reaches_every_core` and `all_secondaries_came_online`
pass in the runs where this fails, so the cores are online and running work and only the per-core
attribution breaks.

This is the second time §28 has invalidated a placement assumption in a test, and both are in
this file. The reap wait above was a yield count until the cross-core placement of §28 (SMP
placement) broke it. The shape is worth naming: a test that spawns a thread and then asserts
something about *where* it ran is relying on placement, and §28 made placement random.
`sched::spawn` is now the wrong call in any test whose subject is a particular core; `spawn_on` is
the one that means what such a test says.

The load sensitivity below is real and separately measured, and the 60 s bound was kept for it.
It was the wrong explanation for this failure, not a wrong measurement.

---

## The suite's deadline tests are host-load sensitive, on `main`, today

Worth recording separately because it was nearly misattributed. `kernel::smp::tests` uses a ten-second
wall-clock `wait_for` and asserts that work placed on a core actually ran there. Under host
contention (several QEMU instances competing for eight cores, which is the normal condition when
concurrent lanes are running), the guest's vCPU threads are starved and the assertion fails:

```
run 4: FAIL secondary cores did not run scheduled work in time
TOTAL main: pass=5 fail=1
```

That is unmodified `main`, six runs under synthetic load. The same tests pass reliably on a quiet
machine. So a failure in `kernel::smp::tests` is evidence about the host before it is evidence about
the diff, and the control worth running first is *the same load against `main`*. These tests run
before `kernel::user::*`, so nothing that module leaks can reach them; the ordering alone rules out
the tempting explanation.
