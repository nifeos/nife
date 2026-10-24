# The scheduler's two diagnostic instruments: the event rings and the canary

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-syscall-selftest.*

`kernel/src/sched.rs` declares two instruments built for first silicon, when a hang on a board had
to be diagnosed from a serial log alone. Both module names are provisional. Both are compiled out of
`--features bench` builds, which get no-op twins with the same functions
(`sched/trace_disabled.rs`, `sched/canary_disabled.rs`). So the call sites are the same in every
configuration, and the numbers the benchmark tripwire watches never measure an instrument. The board
tour build carries no features and keeps both.

## The per-core event rings (`sched::trace`)

A dump of the end state of a wedge cannot say which transition wrote it. A boot-7 bench dump on the
VisionFive 2 showed a state no legal sequence produces: a thread `Blocked` at a pc that was not a
syscall, and a receiver `Running` as another core's current for ten seconds. So each core keeps its
last `trace::DEPTH` events, and `dump_threads` prints them, which puts the path into the wedge on the
log.

- An event costs one relaxed `fetch_add` and one relaxed store. It is recorded on a path that
  already holds `IPC_TABLES`, or in interrupt context with interrupts masked, so each ring has one
  writer, its own core. An entry is one `u64` and cannot tear.
- A dump reads other cores' rings racily, and drain and steal events are recorded outside
  `IPC_TABLES`. Because an entry is atomic, the worst case is an event missing from the tail, never
  a torn one.

## The corruption canary (`sched::canary`)

Armed, the canary re-reads its watched ranges on the timer tick and prints every byte that changed
since the last look, with its address, the tick, and the value before and after. A legal change
prints as a recognizable delta at a table offset: a spawn writing a fresh `ThreadControlBlockPointer`,
or a reap bumping a generation. A stray write prints as bytes nothing in the choreography explains.
It shows and does not judge, because boots 7 through 9 showed that the judging is the part that goes
wrong.

It watches the thread table and the rendezvous registry, the `slots` arrays and their generations,
which are quiescent between spawns and creates. The per-core blocks are not watched: `ticks`,
`runnable`, `current` and the queues change on every scheduler entry, so a checksum there would
measure the scheduler working.

- Unarmed, which is every build's steady state, the tick path pays one relaxed load.
- Armed, the owner core re-reads about 13 KiB per tick.
- The watched memory changes under `IPC_TABLES` while the check reads it without the lock. A torn
  read of a legal write in flight can print as a divergence, and the printed delta shows it.
- The canary's own state, its watch table and shadow copy, is serialized by
  `memory_corruption_canary_gate::Gate`, a one-word state machine whose guards loom searches. The
  crate holds the two races the gate closes and the harnesses that falsify the spelling it replaced.

### Why it is kept with nothing arming it

Nothing arms the canary today: its one consumer was the hang watcher around the riscv64 initial
ramdisk demo, and that demo is gone. It is kept as a marked exception. It is how a board hang gets
diagnosed, it was the evidence that overturned the VisionFive 2 "hang" (notes/visionfive2.md, fifth
stop), and its serialization is loom-checked. Rebuilding it at a bench at two in the morning is the
cost this avoids, and unarmed it costs one load per tick. Point it at a window that can hang, and
then the exception can go.

## BUGS

- The canary has no caller, so nothing exercises it outside its own kernel test. A change that
  breaks arming would go unnoticed until a board needs it.
