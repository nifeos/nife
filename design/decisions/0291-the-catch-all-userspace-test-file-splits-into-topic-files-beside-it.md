---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 291. The catch-all userspace test file splits into topic files beside it

Raised 2026-10-12 (UTC) as the cut that
[milestone 873 (the catch-all userspace test file is split by topic)](../roadmap/0873-the-catch-all-userspace-test-file-is-split-by-topic.md)
says is calef's call before a line moves.

It blocked the move and nothing else.

## The ruling

calef asked nothing separate on this section. The block named three forks, and rulings and
conventions already in place answer each, as the section below sets out. The topic files land as
siblings, the shape of the 77 existing `*_tests.rs` files and of
[#1892](https://github.com/nifeos/nife/pull/1892). `user.rs` stays an index under §282 (when a file
is split, the parent becomes an index), which calef set on
[#1959](https://github.com/nifeos/nife/pull/1959): *"Agreed, make the parent an index."* Under it the
shared helpers move to a child whichever layout is chosen. The split moves code only, so folding
topics into existing files waits for a later lane. Names stay provisional for his naming pass. The
maintainer recorded this as settled by precedent on
[PR #1966](https://github.com/nifeos/nife/pull/1966#issuecomment-6117068748) at 03:12 (UTC) on 2026-10-12.

## What the file was

`system_tests/src/user/tests.rs` was 3,533 lines on main at `17b23264e`, holding 69 `#[test_case]`
functions. The block counted 71; two of its grep hits were comments that mention the attribute. Of
the 69, 69 run on aarch64, 37 on riscv64 and 40 on `x86_64`.

## The layout

Ten topic files and one helper file, each a sibling module of `user`, declared in
`system_tests/src/user.rs` under `#[cfg(all(test, initrd))]` where `mod tests;` was.

| file | holds | lines | tests (a64 / rv64 / x86) |
|---|---|---|---|
| `test_programs.rs` | the seven helpers more than one topic uses, `pub(super)`; the suite's doc | 136 | none |
| `el0_boundary_tests.rs` | reaching EL0, a kernel address refused, preemption, the trap frame | 246 | 6 / 4 / 4 |
| `elf_loader_tests.rs` | forged images refused, the initrd's own image runs | 225 | 8 / 8 / 8 |
| `address_space_tests.rs` | ASID tagging and flush, a read-only segment, a space freed, a user-built space | 550 | 5 / 5 / 5 |
| `capability_tests.rs` | shared memory, `map_physical`, rendezvous, delegation, call and reply, frames | 361 | 9 / 9 / 9 |
| `filesystem_tests.rs` | the virtio disk, `std::fs`, the redoxfs server, per-file grants | 414 | 8 / 0 / 1 |
| `network_tests.rs` | virtio-net, DHCP, sockets, names, TCP, HTTP, `std::net` | 630 | 14 / 0 / 0 |
| `block_driver_tests.rs` | DMA descriptor escapes, block writes, the PCIe transport | 233 | 6 / 0 / 2 |
| `init_tests.rs` | userspace init, the least-authority demo, spending a memory region | 291 | 7 / 5 / 5 |
| `cycle_counter_tests.rs` | the cycle-counter grant, CoreMark | 223 | 2 / 2 / 2 |
| `child_thread_tests.rs` | child threads, reclaim, spawn-to-reap, a composed process | 390 | 4 / 4 / 4 |

`tests.rs` is gone. No new file is near 1,500 lines. `user.rs` stays an index under §282 (when a
file is split, the parent becomes an index), since `mod` lines and docs are free there.

## The block's three forks, and what answers each

1. Siblings or a `tests/` directory. The tree answers it. `system_tests/src/user/` holds 77
   `*_tests.rs` files and no subdirectory. New topics land as siblings, as #1892's
   `futex_tests.rs` and `process_tests.rs` did. A directory would make these ten the
   only topic files in the suite with a second path shape. Under §282, `tests.rs` would also have to
   become an index, so the helpers would move to a child either way.
2. Whether some topics fold into existing siblings. The maintainer's brief for every split lane says
   moves only, with no function body changed. A fold mixes moved lines with edited ones, so this
   milestone does not fold. A later lane can.
3. Every module name. Names ship provisional, each with a `//! Name:` line, for calef's naming
   pass. He does not rule on them here.

## One fact the block did not weigh: the leak police sorts by name

Tests run in link order, which is alphabetical by module path. `thread_leak_police` asserts that
no earlier test left a runnable thread, so it only polices modules that sort before it. Its own
doc says so. Under the old layout all 69 tests sat in `tests`, which sorts before it. The block's
`userspace_init_tests` would sort after it, so its seven tests would run unpoliced. The lane named
that file `init_tests` instead. Every new name sorts before `thread_leak_police`.

This is the one way the split changes behavior: the 69 tests now run interleaved with the other
modules by name, not as one block at `tests`. The three architectures pass with the new order.
A directory would have kept them contiguous, but the police's weakness is general. Seven modules
already sort after it, `thread_pointer_tests` and `time_tests` among them, which its BUGS entry
still calls a future risk. That belongs to a milestone
of its own, not to the layout of this one.

## What the move needed beyond moving lines

- The seven shared helpers are `pub(super)` in `test_programs`. Siblings import them by name. Both
  are `cfg(test)`, so no build's API changes.
- Each file imports only what it uses. An import used only by gated tests carries the same gate,
  as the old file's header already did for the `std_tests` imports. `network_tests` gates its
  `use super::*` to aarch64, since every item in it is aarch64-only.
- The doc above `mod tests;` became `test_programs`'s inner doc, unchanged apart from `///` to
  `//!`.
- Seven falsification patches are renamed from `user.tests.<test>.patch` to
  `user.<module>.<test>.patch`, with each test's `Falsification:` line. No hunk changed, because
  none of them patches this file. Each was replayed red on aarch64 under its new name, failing in its own test.
- `design/comment-block-baseline.tsv`'s two rows for `tests.rs` move with their blocks, to
  `address_space_tests.rs` and `cycle_counter_tests.rs`, under the split amendment of §267 (a
  comment states the constraint as it is now). The row for `user.rs` moves down with its block.
- `design/file-length-baseline.tsv` loses the file's row.

No test body changed and no test's gate widened or narrowed.

## Found while moving

Each is left alone, because this change moves and does not edit.

- The doc of `init_runs_the_coremark_workload_and_it_checks_out`, now in `cycle_counter_tests.rs`,
  opens with its first sentence twice, run together on one line.
- `kernel/src/user.rs`'s doc of `spawn_hello` says every caller is in `tests.rs`. They are in
  `cycle_counter_tests.rs` and `init_tests.rs`. Milestone 872 (the kernel's user-mode file is split
  along its seams) is moving that doc, so the fix belongs after it lands.
- `kernel/src/memory.rs` names `user/tests.rs` beside `free_page_frames`. No test there called it
  after `testing::RegionRun` replaced the direct reads, so the comment was stale before this move.

## What it does not decide

No public crate API, wire format or syscall changes. The suite is a test image. The module and file
names are provisional.
