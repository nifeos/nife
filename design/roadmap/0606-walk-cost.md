---
status: BUILT
raised: 2026-09-26
built: 2026-10-19
milestone_dependencies: 121
decision_dependencies: 230, 236, 237
machine_requirements: none
specific_machine: none
needs_person: no
---
# 606. A directory walk costs what it does on Linux

calef, 2026-09-26 (UTC), on the walk figures of milestone 121 (`ripgrep` on nife: enumeration as
a capability): *"20x slower is unacceptable."* Raised
the same day for lane `milestone/606-walk-cost`. *(Number and title provisional: the integrator
mints the number at merge, and the title is a draft until an architect names it.)*

## Index row

A recursive `std::fs` walk on nife was about 68x slower than Linux on the same machine. The FS
server now keeps what it read, so a warm walk never reaches the device, and a subtree grant the
server enforces itself brings it to 0.85x Linux.

## The goal

Parity with Linux on the same hardware tier for the walk `ripgrep`'s `ignore` crate performs: list
every directory, read every file. The instrument is milestone 121's (`walk_pricing` over
`file_system_contract::fixture::walk`), through the grant a confined `rg pattern src/` will hold.

## What is built

- The measurement. `fs_walk` in `script/bench --real --release --smp` (the release kernel, HVF)
  runs `std_exerciser`'s priced walk and prints it split by `std::fs` call.
  `bench/host/run_linux_walk.sh` runs the same walker as PID 1 of an Alpine kernel on the same
  `virt,accel=hvf` machine. `redoxfs_server/examples/walk_replay.rs` replays the walk's requests
  against the server core and counts device reads and server time.
- The fix inside the contract. The FS server remembers listings, names and small files until a
  mutating verb forgets them (`redoxfs_server/src/memo.rs`). Its block cache is 256 slots,
  allocated as they fill, without the header ring. `OPEN` and `CLOSE` skip the engine's
  transaction when the memo answered, and the `std` PAL copies its shared page with `memcpy`.
- The gates. `redoxfs_server/tests/walk_cost.rs` asserts a warm walk reads nothing from the
  device. Host tests in `redoxfs_server` prove every namespace change, write and truncate is seen
  through the memo. The kernel suite's walk test asserts the new `walk split` line.

Warm walk, release kernel, HVF, 2026-09-26: 27.0 to 29.5 ms before, 0.76 to 0.82 ms after;
Linux 0.41 to 0.42 ms. [notes/walk-cost.md](../../notes/walk-cost.md) has the split, the method
and the caveats.

## The wire changes, ruled 2026-09-27

calef ruled on #1387. Each option was prototyped and measured first; the note has the table.

- A (07:04Z), built on `milestone/606-open-path`: `OPEN` and `OPENDIR` take a relative path,
  resolved per step on the server with the rights the hop-by-hop walk would get. 0.58 to 0.61 ms.
- B (07:06Z, second form), built on the same branch: `OPEN`'s reply carries the size, and the
  std overlay sizes whole-file reads from it. With A, 0.515 to 0.53 ms, about 1.26x Linux.
- D (07:17Z, option 2), built for RedoxFS on `milestone/606-scope`: an eligible server narrows a
  subtree grant itself, keyed on the badge milestone 599 (a frame per filesystem client channel)
  added. Every path and handle goes through `crates/subtree_scope`, whose Kani harnesses prove it.
  0.347 to 0.355 ms, about 0.85x Linux. Caretakers stay the default.
- T1 (15:22Z), built on `milestone/606-progenitor`: the server says it is eligible in a second ELF
  note, and `script/lint` checks it against a package field. The progenitor gives a directory grant
  a bound badge when the note is there and a caretaker otherwise.
- E, not built and not asked: a 64 KiB channel for `std`, which `std_runtime_contract` fixes at
  one page.

## Follow-on

- **Done.** `UNBIND` at reap. The progenitor takes a bound window's grant back when
  `job_undertaker`'s "reaped" message arrives (milestone 685 (a job is finished when its memory
  is back)). A window whose job never started is unbound at once. Only then does the window go
  back to the pool: `Windows::take` in `grant_plan::job_windows` skips a window still bound, and
  `take_never_hands_out_a_bound_window` fails if the unbind moves back to reuse.
- **Recorded.** A cold walk on both sides, with Linux on ext4 over virtio rather than tmpfs, is a
  limitation in `notes/walk-cost.md`.

## BUGS

- `fs_walk` waits forever on a `std_exerciser` that aborts, because an aborting program never
  sends its end-of-stream marker (the kernel suite's `drain_sink` records the same gap).
- Under load the host sometimes runs the guest on an efficiency core, and every figure in the run
  moves about 1.55x at once. The ranges quoted are the fast mode, from three boots.
