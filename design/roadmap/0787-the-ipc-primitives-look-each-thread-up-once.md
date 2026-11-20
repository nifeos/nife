---
status: NOT-STARTED
promoted_from: the-ipc-primitives-look-each-thread-up-once
raised: 2026-10-04
milestone_dependencies: 758
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 787. The IPC primitives look each thread up once

Raised by the lane for milestone 758 (the IPC fast paths shrink back inside their band) on
2026-10-04 (UTC), from the same per-line attribution that chose its levers. It is the next lever
by size. That lane left it alone on purpose. #1611 (plain `RECEIVE`), #1617 (64 capability
slots) and #1614 (null_system_call under load) were all rewriting the bodies it would touch.

## What is owed

The bodies of `ipc_send_badged`, `ipc_receive`, `ipc_receive_cap`, `ipc_call_badged` and
`ipc_reply` in `kernel/src/sched.rs` resolve the same thread name several times in one critical
section. `ipc_receive`'s collect arm calls `sched.threads.get(sender).unwrap()` three times and
`get_mut(sender).unwrap()` twice. The running thread is resolved by `thread_control_block_ptr` and
then again by `get_mut(current).unwrap()`. Each resolution is a generation compare, a slot load and
an `unwrap` landing pad. Each `IPC_TABLES.lock()` site adds an `expect("no scheduler")` pad.

Measured on the lane's tip, summed over every function in both closures, `core::option` lines
were 1,560 bytes on x86_64, 1,286 on riscv64 and 1,264 on aarch64. `generational_table`'s
lookup was 648, 536 and 696. Not all of it can go. Resolving each name once per critical section,
and sharing one cold exit for "no scheduler", should take a few hundred bytes per ISA.

The constraint that makes it a design question and not a sed: the wait-queue pointers already name
live TCBs. The code revalidates them by id on purpose ("the id revalidates it"). Keep that
check, once, rather than trading it for the raw pointer.

## Done when

`script/fastpath-footprint` shows the saving on all three ISAs, and the system tests that exercise
each IPC primitive pass unchanged.

## Index row

The IPC primitives in `kernel/src/sched.rs` resolve the same thread name several times per critical section, and each lookup and its unwrap pad costs bytes on the fast path. Resolving each name once and sharing one cold exit should save a few hundred bytes per ISA, subject to a design constraint on the wait-queue pointers.
