# Login: the budgets, and giving things back

An appendix to [notes/login.md](../login.md). `components/src/login.rs` is a long-lived server that
builds a channel and a caretaker for every login and tears them down again. This is what it takes
for that to cost nothing over the life of the process, and the measurements behind its numbers.

## Four budgets, each with its own parent

`_start` splits three regions off `CONSTRUCTION_UT` before it serves anyone, and stops at `fail(1)`
or `fail(2)` if it cannot.

- `OWN_UT_PAGES` (128) pays for the page tables of `build_child`'s temporary mappings, never a
  child's. Its only spender is `mint`'s `build_child`. `supervision_protocol::fill_and_map`'s scratch
  cursor is one process-wide allocator, so every segment and blob page of every caretaker this
  process builds moves it on, successful build or not. Each 2 MiB it first reaches costs a page
  table. Since milestone 604 (the builder's scratch cursor is bounded) the cursor wraps inside `supervision_protocol::SCRATCH_WINDOW`, so the
  most it can cost is `SCRATCH_TABLE_PAGES` (32). Measured on 2026-08-26 over a whole aarch64 suite
  run, the twenty-six caretakers the login tests build spend `usage() == (1, 128)` of it. So 128 is
  128 times the observed high-water, not a tight count.
- `CHANNEL_UT_PAGES` (32) is spent by `connect` and nothing else. See the next section.
- `DURABLE_UT_PAGES` holds durable sessions (milestone 152 (durable delegation)), split only when a
  schedule can be opened at all. A durable session outlives the logins around it, so carving one
  from `CONSTRUCTION_UT` would leave a hole there each time one is reclaimed out of order. It has
  room for exactly one session. Its owners derive `LOGIN_CONSTRUCTION_PAGES` from
  `login_contract::durable::BUDGET_PAGES`, and `_start` stops at `fail(2)` when it is short.

`DURABLE_SESSIONS` is one even when the capability table and the budget would allow more. The
durable window is one channel, and two timetables staging in one page would overwrite each other
(Fork 8 D). Raising it takes a window and a budget per session from this process's spawners.

## Why a channel has a budget of its own

`MemoryRegion::DESTROY` returns a child's pages to its parent's watermark only when the child sits
at the top of it (`crates/regions`, LIFO). A child freed out of order leaves a hole that does not
come back until no child above it is live (`RegionTable::return_to_parent`). A channel is minted
before the login it carries and destroyed after it. Split from `CONSTRUCTION_UT`, it is never the
top when it is destroyed, because the caretaker region and the client budget that login minted sit
above it and outlive it while the client stays logged in. Every connect then stranded
`CHANNEL_REGION_PAGES` for the life of the process. The suite measured it as 368 pages of holes in
a `CONSTRUCTION_UT` whose real residents accounted for 1664 (`kernel::user::login_tests`'
`CONSTRUCTION_PAGES` account).

A budget with exactly one spender fixes that outright. The process has one thread and serves one
channel at a time: `connect` answers one caller, and the channel is destroyed before the next
`CONNECT` is received. So a channel region is always its budget's only live child, always the LIFO
top, and always fully returned. Nothing else may be split or retyped from that budget, or the
property quietly stops holding. That is also why it is not a share of `OWN_UT_PAGES`, whose spender
allocates during a login with the channel still live.

## Giving a channel back: the region, not the capabilities

A rendezvous retyped by `RETYPE_OBJ` lives in the kernel's global rendezvous registry
(`kernel::sched::MAX_RENDEZVOUS`, 512 slots shared by every process on the machine) until the
region it was retyped from is destroyed. `cap_delete` removes only this process's reference. So a
channel's request and result endpoints and its staging page are retyped from a small region of
their own, along with the page tables for the page's mapping, and the region is destroyed when the
login is done. With `cap_delete` alone, the suite's ~29 connects once spent 58 of the 512, and a
later unrelated test failed with "out of rendezvous points". No thread runs in a channel's region,
so its `DESTROY` has nothing to wait on and is expected to succeed first time. It still goes
through the bounded retry, so that expectation is not a correctness dependency.

## Giving a region back: the memory and the slot

`DESTROY` tears down the objects retyped from a region and returns its pages, and `revoke_region`
deletes every `PageFrame` capability naming a page it freed. Neither touches a `Rendezvous`
capability, and nothing deletes the `MemoryRegion` capability naming the region being destroyed.
Both stay in the caller's table, stale, until the caller deletes them. A long-lived server that
destroys a region per request therefore runs out of table slots while its budget still looks
healthy. The failure arrives as whatever the server says when it cannot serve, which for `login`
is `DENIED` on a correct password. So every site that stops wanting a region calls `discard`
(destroy, then delete the capability), and every abandoned step of `connect` and `mint` deletes
what it retyped before the step that failed.

## The bounded retry

`reclaim` retries a refused `DESTROY` up to `RECLAIM_ATTEMPTS` (1024) times, yielding between
tries. A region holding a live thread is refused with §16 (object revocation)'s kill armed, and
preemptions later the retry succeeds. `system_initializer` uses 64. `login` needs more because of
`Durable::retire`: a session region's two store caretakers are killed by the cascade of §40 (a supervisor's death is its subtree's death) as their
supervisor's supervisor exits, an asynchronous teardown this process can only wait out. Measured
on 2026-09-30 on the riscv64 TCG leg: at 64 the wait gave up inside the race in 4 boots of 4,
`discard` leaked the whole 784-page durable budget, and the next `SCHEDULE` login was `DENIED` for
want of a split. At 1024 the same boot passed 7 of 7. `user_timetable_keeper`'s `ATTEMPTS` (1024,
from `job_undertaker`) waits the same way.

## The capability table, in order

At a login's peak the process holds its resting slots, the channel's three, and up to four of
`mint`'s own (`region`, `narrow_ep`, `ready`, and briefly a TCB). So `authenticate` drops
`channel.request` as soon as its one message arrives and `channel.page` as soon as it is read and
wiped, before `mint` runs. A durable session's two store caretakers are built first, while the
process holds as little as it can, and the ordinary login's peak comes after.
`login_contract::durable` counts it against `abi::CAPABILITY_TABLE_SLOTS`.

## Two blobs rather than the archive

The spawner maps `fs_subtree_caretaker`'s ELF and the measurement table read-only at
`login_contract::CARETAKER_ELF_VA` and `PROGRAM_MEASUREMENTS_VA`, with their lengths in the first two
argument registers (milestone 233 (`login` dies on every boot)). The real boot could not hand over
the initial ramdisk at all, so the program died at `_start` on every interactive boot while it read
the archive. An unvouched, unparseable or absent caretaker is not a reason to refuse to start, as
`crates/system_initializer` treats this program as optional: every login is answered `DENIED`
instead. Both blobs are read-only for the life of the process, so the check runs once at start-up.
