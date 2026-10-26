# Unowned reds from milestone 185's lane, 2026-08-27

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## Two unowned reds, recorded rather than chased: 2026-08-27 (milestone 185's lane)

The lane for milestone 185 (sweep userspace's bounded retry loops onto a clock) named two sites at
the end of [the caretaker teardown wait](caretaker-teardown-wait.md) and moved on. This is the fuller
entry the diagnostic asks for, so a reader who meets either site does not have to re-derive what the
two-line version already knew. Neither line number had moved (checked directly against `main`,
2026-08-27), and neither site was touched: the lane's brief was to record, not to chase. A third
entry, `current_cpu_tests.rs`, was added on 2026-09-22.

### `system_tests/src/user/live_swap_tests.rs:263`, `run_swap`: "reclaiming the operator's budget returned 277 of 224 pages" (FIXED 2026-09-22)

```rust
let recovered = memory::free_page_frames() - before_reclaim;
assert_eq!(
    recovered, SWAPPER_BUDGET_PAGES as usize,
    "reclaiming the operator's budget returned {recovered} of {SWAPPER_BUDGET_PAGES} pages",
);
```

`run_swap` is the shared teardown helper all four `#[test_case]`s in this file call, not one test's
own body, so any of the four could have been the run that hit it. `SWAPPER_BUDGET_PAGES` is 224; the
observed run recovered 277, 53 pages more than the operator's own budget held.

The first diagnostic applies (the main page's "Which direction did it fail?"): a slow machine
produces a deficit, never a surplus. Host contention can only make `reclaim_region` return late or
with less than expected. It cannot manufacture 53 extra pages out of a budget that was only ever 224.
A count higher than the property being measured is the negative-direction shape this register
already names: "a wait written against something wider than the property"
(`notes/riscv-parity-scope.md`'s phrase). Here it reads as free frames from outside `budget` landing
inside the measured window. The likeliest source was a neighboring test's own teardown completing
while this one's `before_reclaim`/`reclaim_region` pair was in flight.

#### Seen a second time, 2026-09-21

The rung 2b lane of milestone 198 (a package manager, and the trivial install that makes a second
customer possible) saw it as *"returned 296 of 224 pages"*: a 72-page surplus where the first
sighting was 53. The second sighting added three things, and none of them closed it.

- It recurs rather than being one run's accident, and the surplus is a different number each time.
  That is what a neighboring teardown landing in the window would look like, and not what a fixed
  accounting error would look like.
- It is intermittent. The same binary, on the same machine, the same afternoon, passed the assertion
  twice and failed it once.
- The failing run was the loaded one: load 7.0 of 8 cores, with another lane's two runaway doctests
  pegged at 99% each. Both passing runs were quieter. That does not contradict the diagnostic,
  because load does not manufacture pages. It fits the reading that a neighbor's teardown completes
  inside the measured window, since contention is what stretches the window.

So the second sighting strengthened the negative-direction diagnosis, and still did not establish
whether the frames came from a neighbor or from a leak. It remained un-investigated and worth a
lane.

The original 2026-08-27 wording of that open question, kept because it is the question the fix
retires: this is not explained by load, and it may be a real bug. Every negative-direction case this
register had actually chased turned out to be a test written against a global counter that something
else could also move. The reaper count and the address-space frame count were both rescoped to a
narrow `ThreadId`-scoped wait in the first round (the old text said the fourth, which only confirmed
them; corrected 2026-09-24). None was a kernel defect undiscovered underneath.
Whether this one was the same shape, or something a swap-system regression put there, was not
established, because this lane's brief did not investigate it. It was proposed in this lane's report
as a milestone of its own (provisional; the integrator mints the number), rather than chased in
place.

*Correction (2026-09-24): the two paragraphs above, and the answer below, sat under the
`current_cpu_tests.rs` entry. They speak of a surplus and a swap-system regression, which are this
site's, so they were moved here. The 2026-09-23 move of the two "N of 224 pages" paragraphs had
already corrected the same misplacement once.*

#### Fixed 2026-09-22 (pull request #1101)

The diagnosis was right, and the fix follows from it. The assertion read
`memory::free_page_frames()`, a count of every free frame in the machine, so anything else
allocating or freeing between the two reads moved it. It now reads `memory_region::usage(budget)`,
which counts the pages retyped out of this region. That is the quantity the test was always trying
to assert: that the operator's budget gave back exactly what it held. A neighboring test's teardown
can no longer be mistaken for this one's result. The narrower assertion is also a stronger one. A
global delta of the right size can be reached by the wrong frames coming back; a scoped one cannot.

The first version of this fix was wrong, and CI caught what the local gates could not. It asked
`memory_region::usage(budget)` after the reclaim and unwrapped it. But `reclaim_region` destroys the
region, so the name is stale and `usage` returns `None`. The test panicked with "the operator's
budget should still exist after reclaim" on every architecture. A lane that never committed or ran
the fix had written it, and `script/lint` and `script/fmt` cannot boot QEMU, so it reached a pull
request looking clean. The assertion should say the opposite of what it said: the region's absence
is the measurement. Under §16 (object revocation: reclaim the objects a process built), the kernel
refuses to reclaim a region whose children are still carved out of it. So a stale name proves every
child was gone first.

The open question above is answered by construction rather than by investigation, and it is no
longer worth a lane. A neighbor can no longer move the assertion at all. If it ever goes red, the
answer is "a leak", with nothing else to rule out first.

### `system_tests/src/user/current_cpu_tests.rs:134`, `the_page_is_returned_when_the_space_is_dropped` (FIXED 2026-09-23)

```rust
let before = crate::memory::free_page_frames();
{ /* load a space, check it has a current-cpu page */ }
assert_eq!(crate::memory::free_page_frames(), before,
           "dropping an address space did not return its current-cpu frame");
```

Same global counter, same failure, seen 2026-09-22 on pull request #1094. That diff touches only a
maintainer script, so it cannot have caused it. It failed on the riscv64 CPU matrix while `main` was
green, which is the signature of a load-sensitive assertion rather than a regression.

It cannot take `run_swap`'s fix. The frame this test watches is the one an address space owns that
its region does not pay for. The whole reason the test exists is that `memory_region::destroy` does
not cover it and `Drop` must free it by hand. So there is no region whose usage could be queried
instead, and a scoped counter is not available the way it was for `run_swap`. Fixing this one needs
a different mechanism, and it had not been chased.

*The paragraph above records what was not available, and it is still true: there is no region to
scope to. What was missed is that a scoped quantity does not have to be a region.*

#### The fix asks about the frame, not about a count

The test holds the page's kernel virtual address while the space is alive
(`current_cpu_page_kernel_va`). That address is the frame through the direct map, and
`mmu::virt_to_phys` inverts the map, so the test can name the physical frame the allocator handed
out. After the drop it asks `memory::is_page_frame_used(frame)`, which reads that one frame's bit in
the bitmap:

```rust
let frame = {
    let space = load(current_cpu_reader_image(), 0).expect("load failed").0;
    let va = space.current_cpu_page_kernel_va().expect("...");
    let frame = PageFrame::containing(mmu::virt_to_phys(va));
    assert_eq!(memory::is_page_frame_used(frame), Some(true), "...");
    frame
};
assert_eq!(memory::is_page_frame_used(frame), Some(false), "...");
```

Why a global count could never work here, in one sentence: the quantity it measures is not the
property. `free_page_frames()` is a fact about the whole machine. Bracketing it asserts that nothing
else in the kernel allocated or freed for the length of the window, a claim this test has no business
making and cannot keep on any loaded run. The frame's own bit is the property itself: one bit, owned
by this space, set by `attach_current_cpu_page` and cleared only by `Drop`.

Leak sensitivity is not merely preserved; it is sharper. A frame `Drop` never returned stays marked
used forever, so the defect fails this assertion on every run, not only on runs where the arithmetic
happens to be visible. Unlike the old form, a leak here cannot be masked by somebody else freeing the
same number of frames in the same window (the coincidence caveat in
[the known residuals](known-residuals.md) for the `<=` sites). The vacuity guard before the drop keeps
the inversion honest. `Some(true)` proves the address names a frame this allocator owns, so a wrong
direct-map inversion fails loudly instead of returning `None` and reading as "not used".

The exposure that replaces the old one is far smaller, and it is recorded rather than hidden. A
neighbor that allocated this exact frame in the microseconds between the drop and the check would
fail it falsely. That needs an allocation, where every observed failure of the old form was a freeing
neighbor's late teardown. It also needs the allocator's linear scan to land on this one frame out of
the machine's free set.

It was proved by injection on 2026-09-23, because [the fifth round](stack-reuse-and-proxy-detectors.md)
showed that a clean run proves nothing about an assertion. Deleting the `crate::memory::free(frame)`
call from `AddressSpace::drop`, which is this test's defect exactly, turns the leg red:

```
test kernel::user::current_cpu_tests::the_page_is_returned_when_the_space_is_dropped ...
[PANIC] assertion `left == right` failed: dropping an address space did not return its
current-cpu frame at 0x4002d000
  left: Some(true)
 right: Some(false)
```

The frame's address is in the message, which the old form could not print: a global delta names a
quantity, not an object. It was reverted. The same filtered run on the clean tree reports
`test result: ok. 1 passed`, and the full aarch64 leg is green.

### `kernel/src/arch/aarch64/timer.rs:680`, `holding_a_lock_masks_the_timer`: "the timer is not ticking at all"

*Fixed on 2026-09-24: the check now waits on the property under a one-second bound. See
[the preemption window appendix](preemption-window-tick.md).*

```rust
let alive_on = crate::cpu::id();
let t0 = timer::ticks_on(alive_on);
timer::spin_for(timer::interval() * 2);
assert!(
    timer::ticks_on(alive_on) > t0,
    "the timer is not ticking at all"
);
```

This is the liveness check at the top of `holding_a_lock_masks_the_timer`, ahead of the
lock-masking assertion [the second round](measurement-windows-and-the-load-recipe.md) (2026-08-04)
already core-scoped. That fix touched the two reads bracketing the critical section. This read pair,
`t0` before `spin_for` and the assertion after it, is a different window in the same function, and
that round did not audit it.

The failure direction is positive ("not yet": `ticks_on(alive_on)` did not exceed `t0` after
spinning across two tick intervals). By the first diagnostic that is honest load sensitivity, not a
wait written against something wider than the property. `ticks_on` is already core-scoped
(`ticks_arrive_at_the_configured_rate`'s fix, reused here), so this is not the migration hazard a
bare `ticks()` would have. To fire with no kernel defect, it needs a vCPU denied the core for the
whole two-interval spin. An oversubscribed host can do that. (The old text had this sentence
inverted, "a genuine failure of this kernel rather than of the host"; corrected 2026-09-24.) Killing a
run before it even reaches the suite's userspace half is consistent with it. This leg never got far
enough to log a host-load line: `HostLoad` samples while the leg is running, and a leg that dies
output-less here gave it nothing to sample.

It was not chased. The register's rule is that the "not yet" kind of failure is the one a
clock-bounded wait or a wider margin actually fixes, and this lane's brief was to record rather than
change kernel timer tests. Whether it wants a `wait_for`-shaped rescope, or is rare enough not to, is
unmeasured. It is a sibling of the family that milestone 62 (tests that assert on time) and the
earlier rounds dispositioned, and none of them reached it.
