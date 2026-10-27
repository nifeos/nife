# `--mem` entries: nested, named, and one at a time

An appendix of [scheduled execution](../scheduled-execution.md), moved from
`components/src/timetable.rs`'s BUGS by milestone 862 (comments state the constraint as it is
now) on 2026-10-09 (UTC). Wording kept except where a sentence had to split.

The roadmap's first sketch said to split a `--mem` entry's grant out of the instance's own region
"so a single `DESTROY` still reclaims both", and that is wrong on its own terms.
`memory_regions::destroy_outcome` returns `Refused` for any region with a live child, and Kani
proves it. `sched::reap_supervised` hands that refusal straight back. So a corpse whose region
carries a nested grant can never be collected through `reap` until the grant is destroyed first, by
its own separate capability.

The nesting survives the correction for a better reason. It is the only thing that can ever pair
a death with a grant. A builder is never told its child's tid: `supervision_protocol::build_child`
hands back a TCB capability, and `abi::thread_control_block` has no method that reads one out. So
the only fact this process has about a death is the tid the kernel stamped on it.
`fire_with_grant` keeps the split untyped's own capability rather than `capability_delete`-ing it the
way it does the region and the TCB. That is what lets `collect_grant` destroy it later, by name.

What decides *how many* `--mem` instances may be outstanding at once is that correlation, and the
answer taken here is one. A generation counter or a slot table could track more. Nothing here
needs its child's tid for any other reason, so paying for one would be speculative machinery for a
document that schedules exactly one such entry. With one, the pairing needs no bookkeeping at
all. `_start` drains everything already outstanding, fires the grant-bearing instance alone, and
blocks in `collect_grant` until it dies and its grant is reclaimed, before returning to the loop.
Nothing else in the document can be firing while that wait is blocked, which is what makes the
next death on `DEATHS` unambiguous.

The cost is real and is paid by every other entry, not by `--mem` ones. While a grant is
outstanding this process is blocked in one syscall and cannot poll the clock at all. An interval
entry due during that window is not skipped; it simply runs late once the loop resumes. Several
periods elapsing during a slow instance still produce one fire on resumption, `next_after`'s
ordinary skip-not-catch-up rule, not a special case for this path. `timetable.conf`'s `at-boot
memory_grant_depleter --mem 4` fires before the first `every 150ms` tick can even become due, so
this cost is not exercised by the cross-ISA test. A document whose `--mem` entry shares the clock
with a fast interval would pay it.

## What the parent note added

These facts sat in the parent's section "A backable `--mem` grant, and why it runs alone" until
2026-10-11 (UTC), beside a restatement of everything above. The restatement became a summary and a
link; these moved here, checked against the tree that day.

- `memory_regions::split_stays_within_budget_and_progresses`' sibling proof pins the refusal.
  `sched::reap_supervised` passes it back as `NotPermitted`.
- A supervisor learns a tid and nothing else. Besides the TCB capability above, `abi::fault`'s
  five-word message carries no builder-chosen tag, so nothing in a death names the grant.
- `components/src/timetable.rs` does not lean on interpreting a refused reap. It drains everything
  already outstanding before firing the grant-bearing instance, and the grant is destroyed
  unconditionally rather than guessed at.
- An interval entry delayed by that wait is never dropped. If the wait outlasts more than one
  period, `next_after`'s ordinary skip-not-catch-up rule covers it, the same as any other stall.
