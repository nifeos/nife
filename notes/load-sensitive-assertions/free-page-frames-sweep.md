# The free-page-frames sweep, 2026-09-23

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## The sweep, 2026-09-23: all forty-six reads of the global counter, classified

The two fixes in [the unowned reds](unowned-reds.md) were each found by a red CI leg and fixed one at
a time, which is the expensive way to find the rest. This is the other way. It reads every call of
`crate::memory::free_page_frames()` in `kernel/src` and says, for each one, which of the two fixes
fits, or that neither does and why.

There are forty-six reads in the tree, and "46 sites" and "46 hazards" are different claims. One is
the definition itself (`memory.rs`). Two are prose in doc comments recording that a site once read
it. One is the companion `largest_free_run`'s cross-reference. That leaves 42 executable reads, in
nine files.

### The classification

| Kind | Count | Where |
| --- | --- | --- |
| Not an assertion (diagnostic, ledger, demo report) | 7 | `testing.rs` (2), `user.rs` (3), `arch/x86_64/mmu.rs` (2) |
| Named-frame case, fixed here | 35 | `sched.rs` (15), `user/force_kill_tests.rs` (13), `user/tests.rs` (4), `user/cpu_time_tests.rs` (3) |
| Scoped-count case (`memory_region::usage`) | 0 remaining | the one instance was `live_swap_tests.rs`, fixed 2026-09-22 |
| Neither fits | 0 remaining | the one instance was `current_cpu_tests.rs`, fixed 2026-09-23 |

Every assertion in the tree that bracketed the global counter is gone. Not one of the 35 needed a
mechanism that did not exist, which was the surprise. The expectation going in was a long tail of
category-four sites wanting a kernel API nobody had written.

(Scope, added 2026-09-24: this sweep read `free_page_frames()` only. Assertions that bracket
`memory::stats()` or `thread_count()` have the same shape and were not read; the main page's
"Where to look for the next one" has the counts.)

### Why they were all one shape, and what that shape is

Every one of the 35 was a test that created one or more root `memory_region`s, did something to them,
reclaimed them, and asserted the machine's free-frame count was back where it started. The property
each was reaching for is "this region's pages came back". A root region's pages are allocator bits:
`memory_region::create` takes them with `alloc_contiguous`, and `destroy` returns every one of them
with `memory::free`. So the region's own run of frames, asked about one bit at a time with
`memory::is_page_frame_used`, is the property itself.

That is the `current_cpu_tests.rs` fix generalized from one frame to a run. The generalization is
`testing::RegionRun` (name provisional), a test-only type in the `#[cfg(test)]` harness module rather
than a kernel surface:

```rust
let region = crate::memory_region::create(16).expect("no region for the runaway");
let run = crate::testing::RegionRun::of(region);   // names the run, and asserts every frame used
// ... start a runaway, force-kill it, reclaim its region ...
run.assert_returned("reclaiming a force-killed runaway did not return its frames");
```

`of` carries the vacuity guard the `current_cpu_tests.rs` fix established. It asserts every frame in
the run is currently marked used, so a wrong base address fails loudly at capture. Otherwise it would
read as "not used" after the reclaim and pass for the wrong reason. `assert_held` is the third method,
for tests whose claim is that pages did not reach the allocator: a split child's pages go back to its
parent, and a lent region is not the borrower's to free.

### What each fixed test gained beyond not flaking

- The failure names an object: `frame 0x4023a000 (page 3 of the 16-page run at 0x40230000)` instead
  of `left: 49899, right: 49879`. A global delta names a quantity; a bit names a page.
- A leak cannot be netted out. Somebody else freeing the same number of frames in the same window
  could satisfy the old form. A frame a reclaim never returned stays marked used forever, so the
  defect fails on every run.
- Three tests lost machinery they had only because the counter was global.
  `force_kill_tests::an_address_space_never_frees_a_region_it_was_lent` opened with a loop that
  sampled the free count until two reads a yield apart agreed, to tell its own arithmetic from a
  neighboring reap. That loop is gone, because nothing a neighbor does can move these four bits.
  Two tests in `force_kill_tests.rs` and one in `user/tests.rs` created a rendezvous before the
  baseline purely so its pages fell outside the window. A comment on one records that getting the
  order wrong cost two runs at a deterministic 32 frames. The ordering is no longer load-bearing.
- `sched::tests::destroyed_region_slots_are_reused` got stricter. It ran 320 create-and-destroy
  rounds and checked the count once at the end. It now checks each round's own page at the end of
  that round, so a leak in round 7 fails in round 7 instead of being netted out across the 320.

### The seven that stay, and why they are not the same thing

They are not assertions about a window, which is what makes a global count wrong. They are reports.

- `testing.rs`, the frame ledger (two reads): one per test boundary and one at the end of the boot.
  This is the right scope for a machine-wide number. It charges the whole run and gates on
  `SUITE_PAGE_FRAME_BUDGET`, so it still catches a leak outside any region. The 35 scoped assertions
  no longer provide that coverage individually. The sweep narrowed 35 assertions and left the
  boot-wide net in place deliberately.
- `user.rs`, `x86_userspace_demo` (three reads). A boot-tour demo that reports two rounds' frame
  deltas as evidence. It asserts nothing and runs before any other thread exists.
- `arch/x86_64/mmu.rs`, `init` (two reads). It counts the frames the kernel page tables cost, into
  `TABLE_FRAMES`, in early boot on one core with nothing else running.

### BUGS

- `RegionRun` only works for a root region. A child region's pages return to its parent rather than
  to the allocator, so its frames are marked used either way, and `assert_returned` would fail on a
  correct reclaim. `memory_region::usage` is the instrument for that case (`live_swap_tests.rs`), and
  nothing in the type stops a caller reaching for the wrong one. The refusal would want a
  `memory_region` accessor saying whether a name is a root. That is a kernel surface, and so an
  architect's call rather than this lane's.
- It walks the run one frame at a time. Sixteen frames is sixteen bitmap reads under the allocator
  lock, taken separately, so the check is not atomic with respect to another core. That is sound for
  what it asserts: a frame nobody has any name for cannot be re-allocated between two of the reads
  without a defect being present. It would be the wrong instrument for a run of thousands.
- `cpu_time_tests::the_thread_that_ran_is_the_thread_that_is_charged` lost a courtesy along with its
  flake, and it is the one place coverage genuinely narrowed. Its old
  `wait_for(|| free == frames_before)` existed partly to keep a force-killed runaway's late pages out
  of a neighboring test's window. That neighbor no longer exists, since no test in the suite
  brackets the global count any more. But the wait also happened to cover the child's non-region
  pages (the current-cpu page among them), and those cannot be named from the test once the space is
  gone. `current_cpu_tests.rs` asserts that frame directly, and the frame ledger would catch a drift,
  so the property is covered. It is no longer covered here.
