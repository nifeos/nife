# The thread dump, and what each of its fields can claim

*Name: provisional.*

`sched::dump_threads` (`kernel/src/sched/inspection.rs`) prints every thread's scheduler state, the
rendezvous and notifications with someone waiting, and one line per online core. The test watchdog
calls it when a run stops making progress ([notes/scheduler.md](../scheduler.md), "The two hang
watchdogs").
It takes `IPC_TABLES`, which is free when the hang is a blocked thread rather than a lock deadlock.

## Which fields are a consistent snapshot

- `state`, `on_cpu`, `wake_pending`, `wait` and the rendezvous counts are consistent: every writer
  holds `IPC_TABLES`, and so does the dump.
- `pc` is the trap frame at the top of the thread's kernel stack, which trap entry writes without
  `IPC_TABLES`. For a thread off its core it is trustworthy: the frame write happened before the
  state write on that core, and the dump's lock acquire synchronizes with that core's release. For a
  thread on a core it is a racing read of live state, and the dump marks it `pc*`.
- The per-core lines read other cores' atomics relaxed. `current` is written under `IPC_TABLES`, so
  it holds still while the dump holds the lock, except on a core mid-switch, between its release of
  `IPC_TABLES` and its `finish_switch`. There `current` names the incoming thread while the outgoing
  one still runs a few instructions.

## What each column is for

- `address_space` is the root of the thread's space, and it is what tells threads of different
  processes apart. Every user program is linked at `address_space_map::IMAGE_BASE`, so a bare `pc`
  resolves plausibly against several binaries. Threads sharing a root are one process; 0 is a
  kernel thread.
- `wait` is written by the same statement that writes `Blocked`. A `Blocked` thread with `wait=-`
  means a state byte was written outside the block paths (corruption, or a block applied to the
  wrong TCB), because every legal block records what it waits on. notes/visionfive2.md has the bench
  stop where this mattered.
- The rendezvous lines show only rendezvous with senders, receivers or a pending signal, so a
  deadlock shows as a sender with no receiver. A notification line with a waiter beside a nonzero
  word is the notification twin.
- `ticks` is the core's timer-interrupt count. A core whose count holds still across dumps takes no
  timer traps: it spins with interrupts masked, or sits inside an SBI call in M-mode (a remote fence
  that never completes looks exactly like this). It is the one field that tells a wedged core from a
  scheduler that chose not to run somebody.
- `steal_req` that stays claimed dump after dump means the victim never reached a scheduler entry to
  serve it: the same wedge, seen from the thief's side.
- A parked core with a non-empty inbox is a thread nothing will ever run, the shape of the
  VisionFive 2 placement hang. No path should produce it, so the line means something is picking
  cores by count again. Every loop over cores walks the online set, never `0..count`, because the
  VisionFive 2's online harts are {1, 2, 3}.
- The header repeats `BOOT_STAGE`, the boot tour's last-reached stage. A serial line printed once can
  go missing from a bench log; a dump that fires every couple of seconds restates it each time. The
  riscv tour bumps it at each step, and the number-to-step table is beside the tour in `main.rs`.
