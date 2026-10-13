---
status: BUILT
raised: 2026-10-08
built: 2026-10-11
milestone_dependencies: 812
decision_dependencies: 271
machine_requirements: none
specific_machine: none
needs_person: no
---
# 840. The scheduler file is split along its seams

*(The title and slug are provisional.)*

`kernel/src/sched.rs` is 8,921 lines at `ea7a58d`. Milestone 41 (dead-code triage) measured it at
3,166. Its header says "a round-robin scheduler, and the preemption that makes it mean something",
and that is about a quarter of it. The rest is everything that takes the `IPC_TABLES` lock.
Milestone 98 (the scheduler that stopped scheduling: name what `SCHED` actually guards) already
found this for the lock; this milestone does the same for the file.

The boundaries were an architect's call. calef ruled them on 2026-10-11 (UTC): *"Accept all four
sched split recommendations."* §271 (the scheduler file splits into submodules of `sched`) records
the four answers: submodules, capability deletion stays, region reaping alone, and the size target
in *Done when* below. Every module name still ships provisional. The lane waits on milestone 812
(`std::thread::spawn` runs real threads in one address space), which reshapes this file and already
cut `sched/configure.rs`, `sched/futex.rs` and `sched/process.rs` as submodules beside `sched.rs`.

Reuse: not applicable; this moves code and adds none. The precedent is `kernel/src/user.rs` beside
`kernel/src/user/`, where each service grew its own file.

## What is in the file

Measured on `ea7a58d` from the file's own top-level items. Line ranges are approximate at the edges,
where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 923 | 923 | header, per-CPU current thread and capabilities, the thread table, `IpcTables` |
| 924 to 1166 | 243 | `mod trace`, twice |
| 1167 to 1337 | 171 | soak-test counters, boot stage |
| 1338 to 1547 | 210 | `mod canary`, twice, and its two wrappers |
| 1548 to 2884 | 1,337 | init, spawn, placement, stealing, idle, exit and fault, the tick, `schedule`, `finish_switch` |
| 2885 to 2975 | 91 | interrupt routes: `bind_irq`, `irq_route`, `irq_notify` |
| 2976 to 3105 | 130 | rendezvous creation |
| 3106 to 3471 | 366 | notifications, including binding one to a thread |
| 3472 to 3639 | 168 | timers |
| 3640 to 3766 | 127 | wake placement and `wake` |
| 3767 to 4737 | 971 | send, receive, call and reply, capability transfer, stranding callers |
| 4738 to 4960 | 223 | deleting page-frame, device-frame and port-range capabilities |
| 4961 to 5130 | 170 | the current thread's capability lookup, `grant`, delegation |
| 5131 to 5178 | 48 | creating a thread control block |
| 5179 to 5818 | 640 | region reaping, `reclaim_region`, supervised reap and survey |
| 5819 to 6199 | 381 | configuring and starting a thread control block, binders |
| 6200 to 6831 | 632 | accessors, most of them hooks for `system_tests` |
| 6832 to 8921 | 2,090 | `mod tests`, 33 `#[test_case]` functions |

`mod trace` (`:925` and `:1115`) and `mod canary` (`:1349` and `:1507`) each appear twice. Each
pair is a real module under `cfg(not(feature = "bench"))` and its no-op twin under
`cfg(feature = "bench")`, written so call sites need no `cfg` of their own. They are cfg variants,
not duplicates, and each pair moves as one.

The brief this was filed from listed IPC rendezvous, notification binding, timers, capability
grants, device-frame capability deletion and region reaping. All six are here. It left out SMP
placement and stealing, interrupt routing, thread-control-block construction, the supervision
survey and the test hooks.

## A proposed cut

Every name here is provisional. `ipc` is not available: §154 (the acronym test is whether the
phrase is spoken, applied recursively) deratified it.

| module, provisional | from the table | about |
|---|---|---|
| `sched` (`sched/mod.rs`) | 1 to 923, 1548 to 2884, 3640 to 3766, boot stage | 2,400 |
| `sched::trace` | 924 to 1166 | 240 |
| `sched::canary` | 1338 to 1547 | 210 |
| `sched::rendezvous` | 2976 to 3105, 3767 to 4737 | 1,100 |
| `sched::notification` | 3106 to 3471 | 370 |
| `sched::timer` | 3472 to 3639 | 170 |
| `sched::interrupt_route` | 2885 to 2975 | 90 |
| `sched::capability` | 4738 to 5130 | 390 |
| `sched::thread_control_block` | 5131 to 5178, 5819 to 6199 | 430 |
| `sched::region_reaping` | 5179 to 5818 | 640 |
| `sched::inspection` | 6200 to 6831, soak counters | 800 |

Tests move beside the code they test, each module growing its own `mod tests`.

## What an architect had to rule

Answered 2026-10-11 (UTC) in §271: the recommendation on each of 1 to 3 was accepted, and 4 ships
provisional. The questions stay here as they were asked.

1. Submodules of `sched`, or siblings of it. Milestone 98's argument points at siblings: a
   rendezvous is not scheduling. The recommendation is submodules. Every seam above takes
   `&mut IpcTables` under one lock (76 `IPC_TABLES.lock()` sites), so as siblings the table's
   fields become `pub(crate)` and the whole kernel can reach them. As submodules they stay
   `pub(super)`. That reason holds at equal cost, so this is not a recommendation about effort.
2. Whether capability deletion belongs in `sched` at all. `kernel/src/revoke.rs` is its largest
   caller after `syscall.rs` and a candidate home. It stays in `sched` here because it walks every
   thread's capability table under the scheduler's lock.
3. Whether region reaping belongs with thread control blocks or alone.
4. Every module name in the second table.

If the answer to 1 is siblings, the cut is the same and only the paths change. If it is "do not
split", the file keeps growing at its current rate and nothing else breaks.

## What it is not

A file split, not a lock split. `IPC_TABLES` stays one lock at one rank and no function changes
behavior. Splitting the lock is a design change with a lock-order argument behind it, and not this
milestone.

## What moving the code breaks

These were found with `grep`, and a lane has to carry each one.

- `script/fastpath-footprint` matches two regexes on mangled symbols such as
  `kernel5sched8ipc_send`. The mangled name follows the defining module. A function moved to
  `sched::rendezvous` no longer matches, even behind a `pub use`.
- 16 falsification patches under `kernel/falsifications/` and `system_tests/falsifications/` apply
  diffs to `kernel/src/sched.rs`. Each must be regenerated and shown to fail its test again. The one
  under `kernel/` carries its test's module path in its file name.
- 51 citations of the form `sched.rs:NNNN` across the tree's prose and comments go wrong at once.
  Replacing each with a function name is cheaper than renumbering it.
- `helpers/cap_abbreviation.py` keys two allowances on the path `kernel/src/sched.rs`.
- `script/stack-frame-check` names `kernel::sched::init`, which stays put.
- Codegen units are partitioned by module, so a split can change what is inlined across `schedule`,
  `finish_switch` and the send and receive paths. That is from memory, not read. Measure it with
  `script/fastpath-footprint`, `script/icount` and `script/bench`.

## Done when

1. `kernel/src/sched.rs`, the parent (812's layout keeps it beside `kernel/src/sched/` rather than
   as `sched/mod.rs`), is under 3,000 lines, and no new file is over 1,500 (§271, part 4).
2. Every `pub` path callers use today still resolves, through `pub use` where needed, so no Rust file
   outside `sched` changes. The tooling, patches and citations listed above are the exception.
3. `script/test` passes on aarch64, riscv64 and x86_64 with no test body changed.
4. `script/fastpath-footprint` and `script/icount` are unchanged, or each difference is explained.
   `script/bench` floors hold.
5. All 16 falsification patches apply and fail their tests.
6. Each new module carries a `//! Name:` block marked provisional.

## What was built

Built 2026-10-11 (UTC) on `ddd38f0b0`, the merge of milestone 812. `kernel/src/sched.rs` went from
8,888 lines to 1,889, under the ceiling of §266 (a Rust source file stays under 2,000 lines), so
its file-length row is gone. Every name below is
provisional.

| file | lines | holds |
|---|---|---|
| `sched.rs` | 1,889 | `IpcTables`, init, spawn, exit and fault, the tick, `schedule`, `finish_switch`, `wake` |
| `sched/tests.rs` | 1,219 | the scheduling tests and the wait helpers the others share |
| `sched/rendezvous.rs` | 1,109 | creation, send, receive, call, reply, capability transfer, stranding |
| `sched/rendezvous/tests.rs` | 717 | the rendezvous tests |
| `sched/region_reaping.rs` | 831 | the reap verdict, `reclaim_region`, supervised reap and survey, with their tests |
| `sched/inspection.rs` | 826 | soak counters, boot stage, preemption counts, accessors, `system_tests` hooks |
| `sched/capabilities.rs` | 427 | lookup, `grant`, delegation, and the deleters (§271 part 2) |
| `sched/thread_table.rs` | 406 | `Threads`, the peak and per-slot tick accounting |
| `sched/notification.rs` | 387 | notifications and binding |
| `sched/placement.rs` | 324 | spawn placement, idle, inboxes, stealing, wake targets |
| `sched/thread_control_block.rs` | 292 | creating, inserting into and starting a thread control block, binders |
| `sched/trace.rs`, `sched/canary.rs` | 183, 153 | the real modules; each bench twin stays inline beside its `mod` line |
| `sched/timer.rs` | 173 | timers |
| `sched/interrupt_route.rs` | 102 | interrupt routes |

`capability` was the planned name, but a module of that name shadows the `capability` crate inside
`sched`, so the child is `capabilities`. Each child takes `use super::*` and the parent re-exports
each whole, so no path outside `sched` changed and no test body changed. Private items became
`pub(super)`.

Measured against `ddd38f0b0`. `script/icount` reads identically on both ISAs and `script/bench
--check` passes on all three. `script/fastpath-footprint` moves at most 12 bytes on aarch64 and
x86_64. On riscv64, `riscv_trap_dispatch` now inlines `preempt_if_needed` (146 to 360 bytes), so
`syscall_entry` grew 11% while the executed code stayed the same. That baseline was re-recorded with
the reason. The 18 falsification records that patched `sched.rs` were regenerated at the lines' new
homes, and the sweep shows each one still turns its test red.

## Follow-on

- **Decision.** Every module name is provisional and is calef's to rule, per
  `design/decisions/0271-the-scheduler-file-splits-into-submodules-of-sched.md`.
- **Done.** The riscv64 `syscall_entry` growth is recorded in `bench/fastpath-riscv64.txt`.
- **Refused.** Rewriting the `sched.rs:NNNN` citations in `notes/` here. They cite line numbers that
  were stale before this split, since the file had grown from 3,166 lines. §277 (cite a heading or a
  symbol, never a line number) now holds them in its code ratchet, and its sweep repoints them.

## Index row

`kernel/src/sched.rs` grew from 3,166 lines to 8,921, and three quarters of it is rendezvous,
notifications, timers, capability deletion, region reaping and test hooks rather than scheduling.
This splits it into submodules of `sched` under one unchanged lock, as §271 rules. Every module
name is provisional.
