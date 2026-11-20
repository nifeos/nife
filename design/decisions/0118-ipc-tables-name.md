---
status: DECIDED
raised: 2026-08-23
decided: 2026-08-23
ratified_by: calef
---

# 118. `Scheduler`/`SCHED` rename to `IpcTables`/`IPC_TABLES`

calef, 2026-08-23, on milestone 98 (name what `SCHED` actually guards)'s naming fork: *"Agreed, go with
IpcTables."*

## The question

`Scheduler` (the type) and `SCHED` (the static) hold a thread table and an endpoint registry.
Neither the run queue nor `current` live there any more (both moved to per-CPU storage, §11 steps
3a/3b). What's left is genuinely whole-machine but is not scheduling state. The module `sched` keeps
its name correctly (`schedule()`, preemption, and round-robin policy all live there and all
genuinely schedule). The type and the static do not. Milestone 98 named the mismatch without picking
a replacement, deliberately reserving the call for calef.

## The decision

`Scheduler` becomes `IpcTables`; `SCHED` becomes `IPC_TABLES`. The module keeps `sched`.

## Why, and why not the alternatives considered first

Milestone 98's own doc had already ruled out two names. `ObjectRegistry` claims more generality than
the type has: there are two kinds held, not any kind. `Objects` is too vague, exactly what DECISIONS
§39 (a component is named for what it is) warns a generic word does. `IpcState` was proposed and
considered next but rejected for the same underlying weakness one level down: "state" says *that*
IPC-relevant data lives here without saying *what*. So a reader still has to open the file to learn
it's a thread table and an endpoint registry. Milestone 98's own instruction is explicit: "Propose
with what it holds, and wait". That is a naming rule, not a suggestion: name the type for its
concrete contents, the pattern already used this session for `AddressSpace`, `MemoryRegion`,
`ThreadControlBlock`. `IpcTables` passes that test where `IpcState` does not. It says, correctly,
that there are tables (plural) inside, and a reader expects to find exactly what is there.

The `Ipc` half is not invented for this decision; it is already this tree's own finding.
`notes/sched-lock-inventory.md` classified every `SCHED.lock()` call site by reading, not guessing,
and its conclusion is on record: "The hot set is IPC." The oddity milestone 98 names
(`grant`/`current_cap`/`delete_current_cap` taking the scheduler lock for no reason connected to
scheduling) stops being a puzzle under this name: cspaces live inside thread-table entries, and
thread-table lookups are IPC's business. So of course IPC-shaped operations take this lock.

Checked for collisions: `crates/ipc` already exists and holds `Endpoint`, `Send`, `Recv` -- nothing
named `Tables` or `State`, so no clash with the new name.

## Prior art, checked rather than recalled

Mach uses this exact term for the analogous structure: each task's port namespace is called its
**"IPC table"**. It is backed by a per-task `ipc_space`, mapping port names to the ports and rights
a task holds. Apple's Kernel Programming Guide and the GNU Mach reference manual both describe the
mechanism. A process's port lookup table is documented plainly as its "IPC table."

**The match is on the word and the concept, not on scope, and that difference is worth keeping
rather than glossing over.** Mach's IPC table is *per-task*. `IpcTables` here is deliberately
*whole-machine*, which is the exact property milestone 98's own text calls out ("the thread table
and the endpoints... genuinely whole-machine"). The precedent supports the vocabulary, not a claim
that this tree copied Mach's per-task design. A kernel table resolving names into the objects an
IPC-capable system call needs is a real, named thing in the field.

## What this does not decide

The exact rename mechanics (order of edits, whether `notes/sched-lock-inventory.md` gets renamed in
the same commit or a following one) are milestone 98's own scope note's to follow, not this entry's.
That note already specifies pure rename and no behaviour change. It says milestone 69 (split
`kernel/src/user.rs` by service)'s proof obligation applies and the lock rank keeps its position. It
also specifies that the type/static must be renamed together in one commit ("do not do half of it").

## What it unblocks

Milestone 98 can now be built. `kernel/src/sched.rs`'s `Scheduler` type and `SCHED` static become
`IpcTables`/`IPC_TABLES`. The module path is unchanged, and `notes/sched-lock-inventory.md` follows
the code once the rename lands.
