---
status: BUILT
raised: 2026-10-09
built: 2026-10-12
promoted_from: a-mutable-static-becomes-an-atomic-a-lock-or-a-recorded-cell
milestone_dependencies: none
decision_dependencies: 19
machine_requirements: none
specific_machine: none
needs_person: no
---
# 891. A mutable static becomes an atomic, a lock or a recorded cell, and `static mut` leaves the kernel

*(The title and slug are provisional.)*

## What is built

Built in #1997, from base `0e72ca92a`. Every site in scope is
converted. `SERVER_MAPS` was done last, after #1988 moved it into
`kernel/src/user/fs_service/wiring.rs`.

| site | became | why that one |
|---|---|---|
| `IDT`, `TSS`, `GDT`, `INSTALLED_PORT_GRANT`, `BENCH_IOMAP` | `SyncUnsafeCell` | the ruling above; each comment names the per-core slot or the single writer before `lidt` |
| `SERVER_MAPS` (`fs_service/wiring.rs`) | `SyncUnsafeCell` | a server keeps its list for the boot, so a lock guard would never drop; the existing take-once claim bit is the ownership |
| `PIXELS`, `SCRATCH` (`screen.rs`) | `SyncUnsafeCell` | the emulator reads both by physical address, so they stay fixed and written in place |
| `PRIMARY`, `BLOCK`, `OPTIONS` (`uefi_loader`'s chooser) | one firmware page allocation `choose` owns and lends down | safe functions each handed out a fresh `&'static mut`; the borrow checker now holds what a comment claimed |
| `DATA_MARKER`, `BSS_MARKER` (`loaded_image_check`) | `AtomicU64` | still `.data` and `.bss`, checked in the built fixtures on aarch64 and riscv64 |
| `IMAGE` (`second_mount`) | `Mutex` | one thread, never contended; still 16 MiB of `.bss` |
| `POOL` (`second_mount`) | taken from `System` on first growth | an address handed to an allocator, page-aligned |

`#![feature(sync_unsafe_cell)]` is in `kernel/src/lib.rs`, with the dependency's way out written
beside it. The kernel's own `IrqSafeMutex` fitted nothing in scope: no site had fields changing
together under an interrupt, and the CPU-read tables cannot sit behind a lock.

The census counts `static mut` (`helpers/rust_source.py`, outside the system tests and fixtures,
`&'static mut` excluded). `static-mut-declarations` (provisional) is the first member of
`RATCHET_CEILINGS` since `agents-md-lines` left, so it fails in both directions. Its marker is in
`notes/unsafe-obligations/ceilings.md`.

| measure | before | after |
|---|---|---|
| `static mut`, census scope | 42 | 27, all in `components/` |
| `static mut`, kernel | 8 (5 in `arch/`) | 0 |
| `unsafe {}` outside `arch/` | 1,103 (density 61.09) | 1,099 (60.83, merged tree) |
| `unsafe {}` inside `arch/` | 347 | 347 |
| density ceiling | 61 (main had just caught it up to the tree) | 60 |

The arch count does not move because a cell access is still a raw-pointer write. The four blocks
outside `arch/` are real reductions in milestone 139 (drive the unsafe count down)'s sense: two
volatile reads became atomics, and three accessor blocks in the chooser became one allocation.

Proven by `script/test` on all three architectures and the host crates, `cargo xtask
rollback-boot` (the chooser spends a try, dies at the handoff and comes back on slot 0),
`cargo test -p loaded_image_check`, and the second-mount repro's unchanged output at the 8 MiB cap.

Open question 2 is answered by measurement rather than ruled: the atomics land in `.data` and
`.bss` as the `u64`s did. calef may still prefer the volatile form.

One of five lint milestones. Its numbers are in [notes/lint-census-2026-10-09.md](../../notes/lint-census-2026-10-09.md).

Reuse: the tree's own `IrqSafeMutex`, `core::sync::atomic`, and `core::cell::SyncUnsafeCell`
(nightly feature `sync_unsafe_cell`, compiled on the pinned nightly-2026-10-09 for this proposal).
The count is one more population in `helpers/rust_source.py`'s census. Nothing is written beyond
that.

## The ruling, 2026-10-09 (UTC)

calef ruled on open question 1 on 2026-10-09 (UTC). The five x86_64 tables the CPU reads (`IDT`,
`TSS`, `GDT`, `INSTALLED_PORT_GRANT` and `BENCH_IOMAP`) move to nightly `SyncUnsafeCell`. The
hand-written wrapper is refused.

### The feature gate, recorded as the dependency it is

`#![feature(sync_unsafe_cell)]` is a dependency on an unstable `core` API, so it is recorded the
way §46 (thin primitives or whole subsystems) asks of a dependency:

- What is taken: one type, `core::cell::SyncUnsafeCell`, behind one feature gate in the kernel
  crate. Nothing is vendored and no crate is added.
- Why it wins: it carries the thread-safety claim once, in `core`, where five wrappers would each
  add a claim to the unsafe census's `unsafe-thread-safety-claims` ceiling.
- What it costs: a toolchain bump can rename or remove an unstable API. The tree is pinned
  (`rust-toolchain.toml`), so the break lands in the bump's own pull request, where the gates run.
- The way out: if the feature changes shape, the fallback is the refused wrapper, with a written
  raise of that ceiling per site. If it stabilizes, the gate line is deleted.

## The problem

A `static mut` is shared mutable state the compiler does not reason about. Every access is
`unsafe`, and the argument that only one writer exists lives in a comment. Rust 2024 closed half of
the problem: `static_mut_refs` is deny-by-default in that edition, and it fires nowhere in this
tree. No reference to a mutable static exists. The other half is ordering. A plain store to a
`static mut`, read by another core with no release and acquire between them, is a data race. The
tree's fourth codebase rule says to assume weak memory ordering, and on aarch64 the race is real.

This tree has had that race. `kernel/src/arch/x86_64/exceptions.rs` records that the `syscall`
stack slot started "as one flat `static mut` every CPU raced", until milestone 161 (the x86_64
kernel port) made it per-CPU. `arch/x86_64/isa.rs` records the other conversion on file: its CPU
record "stopped being a `static mut` with a zeroed initializer" and is now an
`IrqSafeMutex<Option<Isa>>`.

Measured as declarations, with comments, strings and test code stripped:

- 52 in all tracked Rust. The request's grep said about 90; it matched `&'static mut` as well.
- 42 in the note's non-test scope: 27 in `components/`, 8 in the kernel, 3 in `uefi_loader`, 2 in
  `redoxfs_server` (a host-side repro binary) and 2 in `crates/loaded_image_check`.

The kernel's 8:

| site | where | what it is |
|---|---|---|
| `IDT` | `arch/x86_64/exceptions.rs` | the interrupt table the CPU loads by address |
| `TSS`, `GDT` | `arch/x86_64/segments.rs` | per-CPU tables the CPU loads by address |
| `INSTALLED_PORT_GRANT` | `arch/x86_64/segments.rs` | per-CPU record of the I/O-port grant loaded |
| `BENCH_IOMAP` | `arch/x86_64/segments.rs` | the bench build's I/O bitmap |
| `PIXELS`, `SCRATCH` | `screen.rs` | boot-time buffers, touched by one core before others start |
| `SERVER_MAPS` | `user/fs_service.rs` | per-server mapping lists, 17 KiB kept off the stack |

## Parity

aarch64 and riscv64 declare none. That is not a gap in those ports. Five of the eight are x86_64
structures the CPU reads from memory by address (`lidt`, `lgdt`, `ltr`), and aarch64 has no
equivalent: its vector table is code, in `arch/aarch64/vectors.s`, and riscv64's `stvec` points at
code too. The x86_64 layer is the only one that needs a statically placed, mutable, hardware-read
table. The milestone keeps the three ports'
answers the same where the structure is the same: `SERVER_MAPS` and the screen buffers are portable
code and convert once for all three. That is §19 (architectural parity is a tenet) applied to a
difference in hardware rather than a gap in a port.

## The mechanism, and its rung

Rung one where it fits. A counter becomes an atomic, and a record becomes a lock, as `isa.rs`
already did. Either removes the `unsafe` at every access. A table the CPU loads by address cannot
move behind a lock. For those the recommendation is `SyncUnsafeCell`. It keeps the fixed address
and the in-place write, and it makes the sharing a type rather than a comment.

The alternative for those tables is a hand-written wrapper with `unsafe impl Sync`. It is refused
here: each wrapper is a new thread-safety claim. The unsafe census holds those at the tree's exact
value (`unsafe-thread-safety-claims`, milestone 134 (the register of measures)), so each would need
a written raise. `SyncUnsafeCell` carries the claim once, in `core`.

Then rung two for what is left. The declaration count becomes a census population and a
counted-claims ceiling written at the tree's exact value, failing in both directions. That is the
ratchet-ceiling shape `script/lint` keeps for a small, consequential population. It fires when a
`static mut` is added, and it fires when one is removed and the number is not lowered. A grep
gate alone would not bank the fall.

## Scope

The kernel's 8, `uefi_loader`'s 3, `loaded_image_check`'s 2 and `redoxfs_server`'s 2. The 27 in
`components/` are counted from the start, and they convert in milestone 812 (real threads in one
address space). Its item 10 already names ten of them. Each argues soundness from "one thread per
process", which 812 ends, and converting them first would do 812's work without its reason.

## Done when

1. The census counts `static mut` declarations, and a ceiling at the tree's exact value runs in
   `script/lint`.
2. The kernel declares no `static mut`. The five x86_64 tables the CPU reads are each a
   `SyncUnsafeCell`, under `#![feature(sync_unsafe_cell)]` (the ruling above). The other three are an
   atomic, a lock or a `SyncUnsafeCell`, whichever fits. Each cell's comment names the hardware or boot-order fact that makes plain
   access sound.
3. `uefi_loader`, `loaded_image_check` and `redoxfs_server` declare none, or carry a recorded
   exception at the site.
4. The ceiling then reads 27, all in `components/`, and milestone 812's block cites it.

## Open questions for an architect

1. Ruled 2026-10-09 (UTC): yes, `#![feature(sync_unsafe_cell)]` for the five CPU-read tables. The
   question was whether one more feature gate a toolchain bump can break is worth it.
2. `loaded_image_check`'s two markers test that `.data` and `.bss` were loaded. An `AtomicU64`
   lands in the same sections, so the test still means what it says. Is that the reading wanted?
3. Should the components' 27 wait for 812, or convert now where the change is local?

## Where it sits in the ranking

The customer path is vacant, so the tie breaks toward the fatal risks. This serves risk 5 (it
cannot be made reliable on multicore), whose red result is a defect curve that keeps rising. The
one recorded `static mut` race was a multicore defect. It ranks second of the five: eight kernel
sites, each a small change.

## Follow-on

- **Milestone 812.** The 27 in `components/` convert in milestone 812 (real threads in one address
  space), each one lowering the ceiling. Open question 3 stands.
- **Done.** `SERVER_MAPS` is converted, in #1997: its ownership is `SERVER_MAPS_TAKEN` in
  `kernel/src/user/fs_service/wiring.rs`.

## Index row

`static mut` leaves the kernel. Every mutable static becomes an atomic, a lock or a recorded
`SyncUnsafeCell`, and the census counts what is left so the number can only fall.
