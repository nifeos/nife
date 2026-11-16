---
status: BUILT
raised: 2026-09-17
built: 2026-09-17
---
# 310. The Miri leak check stays on, and the test pool it caught cannot leak again


`.github/workflows/undefined-behavior-check.yml` had **never once succeeded**. Five scheduled runs
since 2026-08-11, all red, with `script/cadence-check` (milestone 238) reporting it DEAD. The cause
was not undefined behavior:

```
error: memory leaked: alloc78416 (Rust heap, size: 4096, align: 4096), allocated here:
   --> crates/page_tables/src/domain.rs:425:26
error: aborting due to 5 previous errors
error: test failed, to rerun pass `-p page_tables --lib`
```

`crates/page_tables`'s domain tests back synthetic physical addresses with real zeroed host frames, so
the page-table walker has memory to walk. Those frames were never freed. `cargo test` does not care,
because the process exits; Miri checks for leaks by default and does.

## The measurement

`script/undefined-behavior-check`, the full sampled workspace, on patagonia:

```
script/undefined-behavior-check  5429.38s user  18.55s system  47% cpu  3:09:41.64 total
[exited with code 0]
```

Three hours nine minutes, and green. No failure appeared behind the leak, which is worth saying
plainly because the workflow's header warns to expect one. `cargo miri test` stops at the first
failure. The last time this job was taken apart a three-week red turned out to have three
causes. This time the first fix was the only fix.

And it passes in CI, which is the measurement that settles it. `workflow_dispatch` on this
branch, run 35256118545 on `ubuntu-24.04-arm`: success in 2:58:13, the workflow's first green
since it was written. The two figures agreeing within eleven minutes across two very different
machines is worth having, because it means three hours is the job's cost rather than this laptop's.

The 47% CPU is the load-bearing part of the local line: most of this is one interpreter thread, so
it is wall clock that more cores will not buy back. The old 141-minute figure was a *red* run that
died partway and never measured a finishing one, which is the header's point that a failing check is
also not measuring.

## The part worth reading: the guard was not missing

The obvious story is "a fixture nobody taught to free". It is wrong, and the correct story is the
reason this milestone changed a shape instead of adding a line.

`PoolGuard` was added on **2026-08-03** (`d0f74db9`, *"paging: the test harnesses free what they
allocate"*) for exactly this leak, with a `Drop` that drains the thread-local map and deallocates
every frame. It worked. What it required was that each test *remember* to write
`let _pool = PoolGuard;` as its first line.

`a_multi_page_grant_maps_its_whole_pages_and_not_the_partial_tail`, written afterwards, did not.
One test out of four, five frames, five weeks of red.

That is `AGENTS.md`'s ladder, rung four, failing the way rung four fails. The whole list of
sites confirms it: `page_frame_at` was the only function allocating, `page_frame` the only other
caller of it, and the thread-local `PHYS` had no third writer anywhere in the crate. Nothing was
subtle. Binding the missing line would have restored the identical defect for the next test
author, and the next author is exactly who this file says cannot be relied on to remember.

So the fix goes up the ladder to rung one, *make the wrong state unrepresentable*. `frame` and
`frame_at` are now methods on `FramePool`, the free functions are gone. **A test that allocates
a frame without holding a pool does not compile.** The receiver is otherwise unused, and that is the
point of it.

Three smaller things came with it, each closing a way the fix itself could be wrong:

- The layout is named once (`frame_layout()`), because `alloc` and `dealloc` must agree exactly
  and a `dealloc` with a mismatched layout is itself undefined behavior. Failing a UB check with UB
  introduced by the UB fix is an available outcome and it is now unreachable by construction.
- `FramePool::new` refuses a second live pool on one thread. Nesting would make the inner drop
  free the outer's frames, leaving live entries in `PHYS` pointing at freed memory: a use-after-free
  discovered later instead of a panic discovered now. `PHYS` is thread-local, which is what makes
  one-pool-per-thread the right invariant; the test harness runs each test on its own thread and no
  test in the crate spawns one.
- A `BUGS` section on `FramePool` itself, where a reader meets the fixture, saying why the frames
  are freed and what breaks if they are not.

## The two options that were refused, and why

calef ruled on 2026-09-17. Recording the refusals rather than only the choice, per §75:

`-Zmiri-ignore-leaks`, workspace-wide. Refused. It trades a permanent reduction in what Miri
checks for a one-time cost, which is the wrong direction on a check that had already gone five weeks
without anybody noticing it was checking nothing. This project's `no_std` crates are host-testable on
purpose, and a future host test of an allocator is precisely the thing leak checking would earn its
keep on; turning it off now spends that in advance to save an afternoon.

A `paging`-scoped ignore. Refused. Narrower, and still an ignore, but it also does not exist:
`cargo xtask undefined-behavior-check` runs one `cargo miri test --workspace` with a static exclusion
list and `MIRIFLAGS` is a process-wide environment variable, so per-crate flags would need a
mechanism built first. Paying for machinery whose purpose is to check less is the losing side of both
arguments at once.

The honest cost of what was done instead: one afternoon, one crate, 60 lines changed in one test
module, no production code touched, and no test covering less than it did. That comparison is why
the decision was not close.

## Freeing changed nothing about what the tests cover

Worth stating explicitly, because "make the leak go away" has a bad version where the frames stop
being allocated and the walker stops walking real memory. Every test allocates exactly the frames it
did before, at the same synthetic addresses, and asserts the same things. The only difference is that
the allocations are reachable from a value that outlives them and frees them. All four domain tests
and the crate's other thirty pass unchanged.

## BUGS

- **Nothing checks that a new fixture in some *other* crate does not do the same thing.** This
  milestone made one fixture's leak unrepresentable. The general shape (a test that allocates host
  memory and lets the process clean up) is caught only by the weekly Miri run, which is exactly the
  detection latency that let this sit for five weeks. `script/cadence-check` shortens the latency on
  the *job going quiet*, not on the job going red.
- The weekly run's cost is now measured and still unjudged. Three hours nine minutes locally and
  2:58:13 in CI, and the workflow's header asks for its 240-minute budget to be tightened once `compositor` is
  sampled. This milestone supplies the number the header was missing and does not answer whether the
  cadence is worth it. See the follow-on.
- `crates/page_tables/tests/mapping.rs` still has the opt-in shape this milestone removed from
  `domain.rs`. `TableGuard` must be bound by hand; every test in the file does bind one today, so
  nothing leaks, and nothing keeps that true. Recorded in a `BUGS` section at `TableGuard` rather
  than fixed, because converting it touches twenty-one call sites in a file that is not failing.
- **`FramePool` is a provisional name** (a private type inside `#[cfg(test)] mod tests`, not a public
  surface). It replaces `PoolGuard`, which named the `Drop` half of a thing that is now mostly an
  allocator.

## Follow-on

- **Done.** The weekly workflow can go green for the first time since it was written, and
  `script/cadence-check` stops reporting it DEAD. What that buys is not the leak: it is every other
  thing Miri checks in this workspace, aliasing and provenance and uninitialized reads, which have
  been unchecked on `main` since 2026-08-11 because one error message hid all of them.
- **Milestone 428.** Is a three-hour weekly Miri run worth what it costs? The workflow's header
  raises it and nobody has answered; the cadence is an architect's call, and the measurement, the
  likely answer and what still has to be measured before it is one are in that block. The short
  version: the cost is concentrated in a few crates whose expensive tests are breadth over in-memory
  input, which is exactly what Miri cannot judge, so the lever is running less of it rather than
  running it less often. Nothing is blocked on the answer, since the job is green and inside its
  budget.
- **Milestone 428.** Nothing here measured which crates dominate, and that is the prerequisite for
  the bullet above being more than an argument: a per-crate wall clock from one instrumented run.
  `cargo miri test` prints per-target timings already and nobody has collected them. It is step 1 of
  that block, which is why the two bullets share a number.
- **Recorded.** `crates/page_tables/tests/mapping.rs`'s `TableGuard` is the same opt-in shape this
  milestone removed from `domain.rs`, and it is a `BUGS` section at the guard rather than a fix, for
  the reason stated there: twenty-one call sites in a file that is not failing. `script/lint` cannot
  check it, because "a test that allocates and forgets a guard" is not a grep.
- **Recorded.** An edit outside this lane's own roadmap block, named here because AGENTS.md says a
  lane edits its own block and only that. `notes/undefined-behavior.md`'s item 3 said the `paging`
  leak was "fixed". That stopped being true five weeks ago and is the §76 (what catches a milestone
  status that is wrong in both places) shape of a record
  describing a system that no longer exists. Corrected in place, with the recurrence and why the
  second fix went up a rung.

## Index row

`.github/workflows/undefined-behavior-check.yml` had **never once succeeded**, five scheduled runs
red since 2026-08-11, and the cause was never undefined behavior: `crates/page_tables`'s domain tests
leaked five zeroed host frames, which `cargo test` ignores and Miri's default leak check does not.
The interesting part is that the guard was not missing. `PoolGuard` had been added on 2026-08-03 for
exactly this leak, and a test written afterwards simply did not bind one, which is AGENTS.md's rung
four failing the way rung four fails. Binding the missing line would have restored the identical
defect for the next author. So allocation moved onto the pool itself and a test that allocates a
frame without holding one no longer compiles. calef refused both alternatives on 2026-09-17,
`-Zmiri-ignore-leaks` globally and a `paging`-scoped ignore, on the ground that a permanent
reduction in what Miri checks is the wrong trade for a one-time cost; the refusals and their reasons
are in the block. Measured: three hours nine minutes, exit 0, full sampled workspace, with no
second failure hiding behind the first, which supplies the honest cost figure the workflow's own
header says is "not yet known", and confirmed green in CI at 2:58:13.
