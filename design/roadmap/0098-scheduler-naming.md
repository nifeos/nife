---
status: BUILT
raised: 2026-08-04
built: 2026-08-23
---
# 98. The scheduler that stopped scheduling: name what `SCHED` actually guards

Built (2026-08-23), both ISAs. Raised 2026-08-04 by calef as an abbreviation question
(`sched` or `scheduler`?), and rewritten the same hour when the question "what does `sched`
schedule?" turned up a better answer: increasingly, nothing.

What landed, and everything below this paragraph is the argument that produced it, kept as written.
`struct Scheduler` and `static SCHED` became `struct IpcTables` and `static IPC_TABLES` throughout
`kernel/src/sched.rs`. So did `kernel/src/sync.rs`'s `rank::IPC_TABLES` constant (still rank 60,
still between `ASPACES` and `INBOX`/`MAPPINGS`/`KMEM`, only the name moved). So did every doc comment
across `kernel/src/` and `crates/` that named the lock by identifier (`thread.rs`, `cpu.rs`,
`interrupt_stack.rs`, `untyped.rs`, `kmem.rs`, `user.rs`, `drivers/plic.rs`,
`arch/aarch64/mmu.rs`, `user/tests.rs`, `user/force_kill_tests.rs`, `user/survey_tests.rs`,
`crates/moho_abi`, `crates/steal_request`, `crates/wake_handshake`). A handful of comments that said "the
scheduler" to mean this specific lock, not the `sched` module, were reworded to name it directly.
They were `sync.rs`'s rank commentary, `untyped.rs`'s reap/revoke notes, and similar. That followed
this file's own instruction to check that every call site still reads correctly under the new name.
`notes/sched-lock-inventory.md` followed last, renamed to `notes/ipc-tables-lock-inventory.md` with
its content brought current and a provenance line pointing back at the old name for a reader who
remembers it. Verified as a pure rename per the proof obligation of milestone 69 (split
`kernel/src/user.rs` by service). Every changed `.rs`
line, reverted mechanically (`IPC_TABLES`→`SCHED`, `IpcTables`→`Scheduler`), reproduces the
pre-rename files byte for byte. The deliberate prose edits are the only lines that don't round-trip,
and each is accounted for above. `script/fastpath-footprint` measured 0% delta on both aarch64
and riscv64, confirming the renamed symbols compile to identical code. Full gate suite green on both
ISAs.

Left alone, and worth recording as a scope call rather than an oversight: this entry and §118 were
the only `design/` files touched. Other `design/roadmap/*.md` and `design/decisions/*.md` entries
that cite `SCHED` as a historical fact about the tree at the time they were written were not
rewritten. That matches this milestone's own measured scope and the rule that a developer lane does not
edit `design/` beyond its own entry. Likewise, `notes/*.md` files other than
`notes/sched-lock-inventory.md` that mention `SCHED`/`Scheduler` in passing (there are more than a
dozen) were left as written; only that one note was in this milestone's named scope.

The finding, in the struct's own words. `Scheduler`'s comment says it outright: "Neither the run
queue nor `current` live here any more: both moved to per-CPU storage (`cpu::PerCpu`, §11 steps 3a
and 3b)... What stays is genuinely whole-machine: the thread table and the endpoints." The
scheduling state left in §11's per-CPU migration. What the type holds today is a thread table and an
endpoint registry, which is an object registry, not a scheduler.

Two names, and only one of them is wrong.

- The module `sched` is fine. `schedule()`, the preemption, and the round-robin policy the
  module doc describes all live there and all genuinely schedule. Keeping `sched` also keeps a word
  every kernel reader arrives knowing (POSIX ships `sched.h`; Linux keeps `kernel/sched/`), which is
  the guard rail that spared `elf`, `pci` and `dtb`.
- The type and the static are misnamed. `SCHED` guards threads and endpoints, which is why
  notes/sched-lock-inventory.md classified the lock's hot path as IPC rather than as scheduling.
  That note's real finding was "this is not a scheduler lock", and the naming consequence went
  unnoticed when it was written.

It also explains an oddity that reads as bizarre until you know. Capability operations
(`grant`, `current_cap`, `delete_current_cap`) take the *scheduler* lock, for no reason connected to
scheduling: CSpaces live inside thread-table entries, and the table is in the registry. Under the
right name that stops being a puzzle.

Measured, and the rewrite shrank it by an order of magnitude. The abbreviation question would
have touched **915 `sched::` call sites across 70 files**. This one touches **88 `SCHED` references
inside `kernel/src/sched.rs`, 12 `Scheduler` mentions, and one `rank::SCHED`**, because the module
path does not change. Roughly a hundred sites in one file, not nine hundred across seventy.

**The naming question was calef's**, and it was a real one because the thing is a pair rather than
one concept. It is a thread table and an endpoint registry, held together only by both being
whole-machine and both being under one lock. `ObjectRegistry` claimed more generality than the type
has (there are
two kinds, not any kind); `Objects` was vague in the way §39 (A component is named for what it is)
warns about. `IpcState`, considered
next, had the same weakness one level down (says *that* IPC-relevant data lives here, not *what*).
Decided (DECISIONS §118 (`Scheduler`/`SCHED` rename to `IpcTables`/`IPC_TABLES`)): `IpcTables`. It
names the pair honestly and matches this tree's own
`notes/sched-lock-inventory.md` finding ("the hot set is IPC"). Mach uses the identical term
("IPC table") for a task's port namespace, though per-task rather than whole-machine as here.

## Scope note

Pure rename, no behaviour change, milestone 69's proof obligation applies, and the lock rank keeps
its position in the ordering whatever it is called. The note filename that started this
(`notes/sched-lock-inventory.md`) follows the code and is renamed last, not first. **Do not do half
of it**: a tree where the type is renamed and the static still says `SCHED` is worse than either
consistent answer. The abbreviation question is answered by this milestone's premise and does not
need its own entry: `sched` stays, because the module really does schedule.

## Follow-on

- **Refused.** The dozen-plus `notes/*.md` files and the other `design/roadmap/` and
  `design/decisions/` entries that still spell `SCHED` or `Scheduler` were left as written. The
  design entries cite the old name as a historical fact about the tree at the time they were
  written. A developer lane does not edit `design/` beyond its own entry. Only
  `notes/sched-lock-inventory.md` was in the milestone's measured scope, and it carries the new name with a
  provenance line for a reader who remembers the old name.
- **Refused.** Renaming the `sched` module, which is where the abbreviation question started.
  `schedule()`, the preemption and the round-robin policy all live there and all genuinely schedule,
  and `sched` is a word every kernel reader arrives knowing (POSIX ships `sched.h`, Linux keeps
  `kernel/sched/`). Renaming it would also have been 915 call sites across 70 files against this
  milestone's hundred in one.

## Index row

Decided (DECISIONS §118): `Scheduler`/`SCHED` renamed to `IpcTables`/`IPC_TABLES` throughout `kernel/src/sched.rs`, the lock rank constant, and every doc comment across `kernel/src`/`crates`
naming the lock. The module keeps `sched` (it really does schedule). `notes/sched-lock-inventory.md` renamed to `notes/ipc-tables-lock-inventory.md` last, per the
scope note. `script/fastpath-footprint` measured 0% delta on both ISAs
