---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: 812
decision_dependencies: 266, 281
machine_requirements: none
specific_machine: none
needs_person: no
---
# 872. The kernel's user-mode file is split along its seams

*(The title and slug are provisional.)*

`kernel/src/user.rs` is 3,866 lines at `396187b0b`. Services leave it for `kernel/src/user/` while new work
arrives. §266 (a Rust source file stays under 2,000 lines) sets the ceiling and a 2026-12-31
goal of no file over 4,000. Milestone 840 (the scheduler file is split along its seams) is the
model.

The cut was calef's call, and he ruled it on 2026-10-11 (UTC) in §281 (the kernel's user-mode file
splits into child modules of `user`). On the first fork: *"Agreed, make the parent an index."* That
set a precedent for every split, so `AddressSpace` left the parent too. On the second, *"B"*: the
device starters got a module of their own. Every module name still ships provisional.

Reuse: not applicable; this moves code and adds none. The precedent is `kernel/src/user/`, which
already holds 24 child modules (`fs_service.rs` and its siblings), each a service that grew its own
file.

## Pull request #1892 moves part of this first

Pull request #1892 (milestone 812 (`std::thread::spawn` runs real threads in one address space)) was
in flight when this was written, at head `43d8c5df3`. It changes this file by +41 -388. It cuts
lines 489 to 870, the address-space registry, into a new child module,
`kernel/src/user/address_space_registry.rs` (487 lines there), behind `pub use
address_space_registry::*`. It also adds 22 lines to `AddressSpace` (lines 95 to 257) and six to its
`Drop` (near line 906). The file would end near 3,519 lines.

That cut is this milestone's first seam, taken early, and it is why this block depends on 812.
Every line range below is measured on `main` at `396187b0b` and must be re-measured after #1892
merges. Ranges past line 870 move up by about 350.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges,
where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 63 | 63 | module docs, imports, the user stack constants |
| 64 to 488 | 425 | `AddressSpace`, `Backing`, the overhead constants, `impl AddressSpace`, `ASIDS` |
| 489 to 870 | 382 | the address-space registry: `USER_SPACES`, `BoundSpace`, the `user_address_space_*` functions (#1892 moves these) |
| 871 to 916 | 46 | `impl Drop for AddressSpace` |
| 917 to 1217 | 301 | `LoadError`, `load`, the timebase page, `map_segments`, `initrd`, `program` |
| 1218 to 1252 | 35 | `Mapping` and `Spawn` |
| 1253 to 1459 | 207 | per-architecture console constants, `PROGENITOR_ENTRY`, `INIT_STACK_PAGES`, `PROGENITOR_ROLE` |
| 1460 to 1897 | 438 | `spawn_hello`, `DeviceRun`, the `run` family, `enter_frame`, the `outlaw` roles |
| 1898 to 2284 | 387 | the `x86_64` userspace demo and the two riscv64 demos |
| 2285 to 2963 | 679 | `boot_progenitor`, one function (its body is 611 lines) |
| 2964 to 3001 | 38 | `boot_usb_keyboard` |
| 3002 to 3866 | 865 | 25 `mod` declarations with their docs (333 lines), interleaved with `term_print`, `wait_for` and the device starters |

The device starters (3220 to 3687, 468 lines) are `boot_clock_page`, `VirtioBootGrant`,
`write_dma_phys`, the virtio, e1000e and DesignWare network starters, `boot_config_page` and
`boot_screen_terminal`. `boot_progenitor` calls each one.

The file has no tests: `grep` finds no `#[test_case]` and no `mod tests`. Its tests live in
`system_tests/src/user/`. It has no Kani harness.

## The verdict: split by feature

The tests-only axis does not apply, because there are no tests here to move. The question is only
where the seams fall.

`AddressSpace` is the one type that should not be cut. Its four fields are private, and the loader,
the registry and the run path all reach them. Kept in the parent, every child module reaches those
fields as before, and nothing widens. #1892 makes the same choice for the registry and says so in
its comment. Moving `AddressSpace` out to a child instead would make each field `pub(super)`.

Everything else is a self-contained part with its own callers: loading an image, entering user
mode, the per-architecture demos, booting the progenitor, and the device starters.

## A proposed cut

Every name here is provisional. The modules are private, and `user.rs` re-exports their `pub` items
the way #1892 does, so every `user::` path callers use today still resolves.

| module, provisional | from the table | about |
|---|---|---|
| `user` (`user.rs`, the parent) | 1 to 488, 871 to 916, 1253 to 1459, the `mod` lines, `term_print`, `wait_for` | 1,140 |
| `user::address_space_registry` | 489 to 870, already cut by #1892 | 490 |
| `user::loader` | 917 to 1217 | 300 |
| `user::entry` | 1218 to 1252, 1460 to 1897 | 470 |
| `user::boot_demos` | 1918 to 2284 | 370 |
| `user::progenitor_boot` | 2285 to 2963 | 680 |
| `user::device_starters` | 2964 to 3001, 3220 to 3687 | 510 |

The parent ends near 1,170 lines once #1892's additions to `AddressSpace` are counted. The
`x86_programs` declaration (line 1916) stays in the parent so its file does not move.
`progenitor_boot` and `entry` avoid the names of the functions they hold, `boot_progenitor` and
`run`, for the reason milestone 842 (the grant-plan crate root is split along its seams) gives.

## What an architect had to rule

Answered 2026-10-11 (UTC) in §281: 1 was ruled the other way (the parent became an index, and
`AddressSpace` moved to a child), 2 as recommended, 3 was not put to calef (§281 says why), and 4
ships provisional. The questions stay here as they were asked.

1. Whether `AddressSpace` stays in the parent. The recommendation is yes, for the reason above. It
   holds at equal cost, so it is not about effort.
2. Whether the device starters get a module of their own. Milestone 811 (the boot services leave
   the kernel: the kernel starts only the progenitor) is NOT-STARTED and would move each starter
   out of the kernel. The recommendation is yes: in one file, 811's change is a deletion of that
   file. The functions `boot_progenitor` calls there become `pub(super)`, which is visible to
   `user` and nothing wider.
3. Whether the `cfg(target_arch)` demos and console constants move under `kernel/src/arch/`. The
   recommendation is no. AGENTS.md rule 1 (all architecture-specific code lives under
   `kernel/src/arch/`) names assembly, `asm!` and system registers. These ranges hold none of those;
   `grep` finds `asm!` only in a comment at line 1861. They call into `sched` and the loader, so
   under `arch/` they would point the layering upward. `helpers/lint_arch_parity.py` also skips
   every file under `arch/`, so they would leave its check. These reasons hold at equal cost.
4. Every module name in the table.

No public crate API, wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `396187b0b`. A lane has to carry each one.

- Four patches under `system_tests/falsifications/` diff this file. Their names carry the test's
  module path (`user.running_space_tests.*`, `user.spawn_mapping_revocation_tests.*`), not this
  file's, so no name changes. #1892 retargets three of them to `address_space_registry.rs`. The
  fourth keeps a hunk in `impl AddressSpace`, which stays in the parent. Each must still apply and
  fail its test.
- No patch under `kernel/falsifications/` names this file.
- `design/comment-block-baseline.tsv` keys two rows on this path. The 46-line doc of
  `AddressSpace::map_physical` stays in the parent and is unaffected. The 52-line doc of
  `boot_progenitor` lands in `user/progenitor_boot.rs`. The ratchet admits no row on a
  path the merge base gave none, and a block in a new file meets the 40-line cap outright. So it
  fails this block unless the doc is cut to 40 lines in the same change. The alternative is a fix
  to the ratchet, letting a row move with its block, ruled separately.
- `design/file-length-baseline.tsv` lists this file at 3,867. #1892 does not bank it. The split
  takes the file under 2,000, so the row is removed in the same change.
- 26 citations of the form `user.rs:NNNN` name this file. Milestone 811's block holds 11, dated to
  one commit, and `notes/naming-a-running-address-space.md` holds 6. Several are already stale,
  such as the one `notes/repo-hardening.md` held for a test that left the file long ago. A function name is cheaper to
  keep true.
- 95 tracked files name the path, 63 of them Markdown. `xtask/src/swish_check.rs`'s progenitor-stack failure
  tells a reader to raise `INIT_STACK_PAGES` in this file.
- `script/fastpath-footprint` keys on `kernel5sched` and `kernel7syscall` symbols, and its cold
  list names `12AddressSpace` by type, so no regex follows this file's module path.
  `script/stack-frame-check` names `kernel::user::disk_service::*`, which does not move.
  `script/icount` names no symbol here. `helpers/cap_abbreviation.py` has no row for this path.
- No `#[path]`, `include!` or `include_str!` points at this file, and no `xtask` code reads it.
- Codegen units follow modules, so the split may change inlining across `load`, `run_with` and
  `enter_frame`. That is from memory, not read; neither `Cargo.toml` sets `codegen-units` or `lto`.
  Measure it with `script/fastpath-footprint`, `script/icount` and `script/bench`.
- 29 first-parent merges in the 14 days to 2026-10-11 touched this file. Take it when few lanes
  are open in it.

## Done when

1. `kernel/src/user.rs` is under 2,000 lines, no new file is over 1,500, and the file-length row
   is gone.
2. Every `user::` path callers use today still resolves, so no Rust file outside
   `kernel/src/user.rs` and `kernel/src/user/` changes, and no field of `AddressSpace` widens.
3. `script/test` passes on aarch64, riscv64 and x86_64, and so does `script/swish-check`.
4. `script/fastpath-footprint`, `script/stack-frame-check` and `script/icount` are unchanged, or
   each difference is explained. `script/bench` floors hold.
5. The four falsification patches apply and fail their tests.
6. Each new module carries a `//! Name:` block marked provisional.

## What was built

Built 2026-10-12 (UTC) on `97650277b`. `kernel/src/user.rs` went from 3,519 lines to 457 and is
now an index, so its file-length row is gone. Every name below is provisional.

| file | lines | holds |
|---|---|---|
| `user.rs` | 457 | module docs, imports, `USER_STACK_VA` and `USER_STACK_TOP`, every `mod` line with its doc |
| `user/progenitor_boot.rs` | 735 | `PROGENITOR_ENTRY`, `PROGENITOR_ROLE`, `INIT_STACK_PAGES`, `boot_progenitor` |
| `user/entry.rs` | 532 | `Mapping`, `Spawn`, the test roles, `spawn_hello`, the `run` family, `enter_frame` |
| `user/address_space.rs` | 509 | `AddressSpace`, `Backing`, the overhead constants, `ASIDS`, `Drop` |
| `user/device_starters.rs` | 488 | the eleven starters and `VirtioBootGrant` |
| `user/boot_demos.rs` | 377 | the `x86_64` and riscv64 demos, compiled only there |
| `user/loader.rs` | 316 | `LoadError` through `program`, and `INITRD_VA` |
| `user/console_device.rs` | 109 | the console UART's interrupt, page and port-range constants |
| `user/terminal_print.rs`, `user/virtio_dma.rs`, `user/test_support.rs` | 46, 38, 34 | `term_print`; `write_dma_phys`; `wait_for` |

Each child takes `use super::*`, and the index re-exports each at the visibility its items had, so
no `user::` path changed. A private item a sibling reaches became `pub(super)`, as §281 rules,
including `AddressSpace`'s four fields: done-when 2's "no field widens" was ruled the other way.
A line-multiset check against the old file finds nothing changed but those keywords.

`script/test` passed on aarch64 (273 s), riscv64 (311 s) and x86_64 (368 s, with its root-port,
AMD-Vi and OVMF legs), with the same counts milestone 840 recorded. They ran on `42e0d1db6`; main changed no
kernel, system-test, crate or xtask code between it and `97650277b`. `script/icount` reads
identically, and `script/bench --check` and `script/stack-frame-check` pass on all three. On
riscv64, `riscv_trap_dispatch` stopped inlining `preempt_if_needed`, undoing what 840 recorded, so
`syscall_entry` fell 2,136 to 1,918 bytes and `bench/fastpath-riscv64.txt` was re-recorded with
the reason. The one falsification record that patched this file was regenerated at its hunk's new
home and replayed red at its headline assertion on aarch64.

## Follow-on

- **Decision.** Every module name is provisional and is calef's to rule, per
  `design/decisions/0281-the-kernels-user-mode-file-splits-into-child-modules-of-user.md`.
- **Done.** The riscv64 `syscall_entry` change is recorded in `bench/fastpath-riscv64.txt`.
- **Recorded.** The `user.rs:NNNN` citations in other milestones' blocks go to a maintainer follow-up,
  listed on pull request #1959.

## Index row

`kernel/src/user.rs` was 3,519 lines with no tests in it. It held the address space, the loader,
the path into user mode, per-architecture demos, the progenitor's boot and the device starters.
It is now an index of child modules under `kernel/src/user/`, each named for what it holds, as
§281 rules. Every module name is provisional.
