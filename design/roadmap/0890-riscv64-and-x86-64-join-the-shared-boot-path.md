---
status: PARTIAL
raised: 2026-10-12
promoted_from: riscv64-and-x86-64-join-the-shared-boot-path
milestone_dependencies: 876
decision_dependencies: 19, 284
machine_requirements: none
specific_machine: none
needs_person: no
---
# 890. riscv64 and x86_64 join the shared boot path

*(The title and slug are provisional.)*

calef, 2026-10-12 (UTC), ruling on §284 (the kernel crate root splits into child modules, one per
boot): "We should treat them as temporary."

Reuse: not applicable; this moves and deletes kernel code and adds none.

## The problem

`kernel_main` in `kernel/src/boot.rs` hands `x86_64` and riscv64 to their own boots,
`kernel/src/x86_64_boot.rs` and `kernel/src/riscv64_boot.rs`, and only aarch64 continues through
`bring_up_machine` and the shared path after it. Each port's boot is a chain of phase functions
that brings up its own traps, memory, scheduler and interrupts, runs its own demonstrations, and
ends at its own hand-over (`x86_hand_over`, `riscv_hand_over`). That is two copies of the boot's
portable middle, written per port because the arch layer beneath the shared path was not built for
either port when its boot was written.

calef's ruling makes both files temporary: each is its port's route until it joins the shared
path, and the parity work shrinks it toward the CPU bring-up that belongs under `kernel/src/arch/`.
Nothing yet tracks that work.

## What it would take

Measured at the head of milestone 876 (the kernel crate root is split along its seams):
`x86_64_boot.rs` holds six phase functions and `x86_hand_over`; `riscv64_boot.rs` holds nine, four
timing helpers and `riscv_hand_over`. A lane would, per port:

1. Sort each phase into what `bring_up_machine` already does portably, what is CPU bring-up and
   goes under `arch/<port>/`, and what is a demonstration the milestone tour already carries.
2. Make the shared path take the port, so `kernel_main` stops branching on the architecture.
3. Delete the port's boot file once it is empty, and the two hand-overs once the shared path's
   hand-over serves all three.

DECISIONS §19 (architectural parity is a tenet) is the reason. The riscv64 boot's middle stages
are the only proof this tree has of them on real silicon (notes/visionfive2.md), so a lane keeps
every transcript line `board_console` reads, or moves its claim into the shared path first.

## What the premises turned out to be

Checked against `main` at `ee1f946ad` before anything moved.

- **Both ports already reach a prompt.** Milestone 268 (every architecture boots the same way) ended
  both boots at the progenitor and `swish`. The comment above `kernel_main`'s riscv64 arm still
  said that boot "halts" before an "aarch64-shaped" full boot, and so did an orphaned comment in
  `kernel/src/lib.rs`. Both were stale. What still differs is the route, not the destination.
- **riscv64's bring-up was already `bring_up_machine`'s, in another order.** Its first phase called
  the same portable functions (`arch::init`, `stack::init`, `memory::init`, `arch::isa::init`,
  `smp::read_cpu_list`, `arch::mmu::init`, `interrupt_stack::init`, `sched::init`,
  `smp::bring_up_secondaries`), plus five calls into its own `arch/` and `pci::init_iommu`. Its
  `arch::irq::init` was an empty function documented as "a no-op until the two paths converge".
- **`x86_64` is not close.** `memory::init` is a device-tree parser, and this machine hands over a
  PVH `hvm_start_info` and ACPI, so its frame allocator comes up through
  `arch::machine::bring_up_memory` instead. PCI has to be surveyed before `mmu::init` maps the
  buses. Its interrupt controller and timer need the MADT. That is the discovery seam
  `notes/x86-port/acpi-and-pci.md` names, and it is a slice of its own.
- **`board_console` reads only the ladder.** It matches `boot_ladder`'s markers (banner, machine,
  self-test, tour, prompt) and failure markers. No live gate reads riscv64's first-phase lines;
  they appear in captured logs under `bench/` and `crates/board_console/tests/fixtures/`, which
  are records of what a kernel printed and stay as they are.

## What was built: riscv64 runs the shared bring-up

The first slice, chosen because riscv64 shares the device tree and every portable step with
aarch64. Which port goes first is reversible and nobody has acted on it.

- `kernel_main` calls `bring_up_machine` on riscv64, then `riscv64_boot` for everything after it.
  `riscv64_first_light` is gone. Its banner lines are `riscv64_banner`, called where aarch64 prints
  its own.
- `bring_up_machine` gains riscv64's three calls into `arch/`, each `cfg`-gated beside the
  aarch64-only `check_frequency_against_device_tree` that was already there:
  `timer::init_frequency` and `irq::init_contexts` in the window where the tree is reachable,
  `isa::print_summary` and `pmu::init` after the MMU, and `pci::init_iommu` beside the SMMUv3.
- riscv64's `arch::irq::init` calls `init_this_cpu`, the unmasking its boot used to do by name
  before starting the other harts. `interrupts_init` reaches it at the same point.
- Four first-phase lines are retired: `traps`, `timer`, `memory` and `paging`. Milestone 268 put each
  claim in `self_test::run` on all three architectures (`exceptions`, `timer`, `frames`, `mapping`)
  and retired the tour's own copies of two of them the same way, so this follows the tree's
  precedent rather than deciding anew. The `memory` line also allocated a frame only to print its
  address and never freed it.

The order on riscv64 changes in one way that matters: interrupts come on after the scheduler, as on
aarch64, rather than before memory. The old order needed interrupts masked around `sched::init` for
exactly that reason (riscv64_boot.rs, before this milestone), and the mask is gone with it.

## Done when

1. `kernel/src/x86_64_boot.rs` and `kernel/src/riscv64_boot.rs` are gone, or hold only what
   `arch/<port>/` cannot.
2. `kernel_main` has no `cfg(target_arch)` branch to a per-port boot.
3. `script/test`, `script/swish-check` and the board's `board_console` stages pass on all three.

## Exit criteria, as built

| criterion | state |
|---|---|
| 1 | Not yet. `riscv64_boot.rs` lost its bring-up and keeps the ladder, the test and measurement boots, its tour and its hand-over. `x86_64_boot.rs` is unchanged. |
| 2 | Not yet. Both branches remain; riscv64's now comes after `bring_up_machine` rather than before it. |
| 3 | QEMU half for this slice: see the pull request's gate table. The board half needs radon and is not run by a lane. |

## BUGS

- The PLIC is still brought up by `drivers::plic::init` in three places in `riscv64_boot.rs`, not
  by `arch::irq::init`. Moving it opens the boot context's threshold before the other harts start,
  which is an order change on silicon this lane could not run.
- `bring_up_machine` carries five `cfg(target_arch)` sites, three of them riscv64's. Each calls
  into `arch/` or `pci`, so rule 1 holds, but a port that needs a step aarch64 does not is still a
  `cfg` here rather than a function every port implements.

## Follow-on

- **Outstanding.** The PLIC moves into riscv64's `arch::irq::init`, so its three
  `drivers::plic::init` sites in `kernel/src/riscv64_boot.rs` go (checked: lines in
  `riscv64_device_interrupt`, the test boot and `riscv_hand_over`). Needs a radon boot.
- **Outstanding.** riscv64's tail joins `kernel_main`'s: the ladder, the `icount`, `bench`,
  `shell` and test boots, which run in a different order there than on aarch64 (checked:
  `riscv64_boot` runs the machine description before `icount`; `kernel_main` runs the test suite
  first). The open question for that slice is what happens to the riscv64 tour, which has its own
  `boot_ladder::TOUR` line and stages `board_console` and the hang watcher read, beside aarch64's
  `milestone_tour`.
- **Outstanding.** `x86_64` reaches `bring_up_machine`, which needs `memory::init` to take a
  PVH and ACPI description as well as a device tree (checked: `x86_64_first_light` and
  `x86_64_memory` in `kernel/src/x86_64_boot.rs`).
- **Outstanding.** One hand-over for all three, retiring `riscv_hand_over` and `x86_hand_over`
  (checked: both still in their boot files; `xtask/src/swish_check/verdict.rs` reads
  `x86_hand_over`'s lines).

## Index row

Two of the three architectures boot through code that only they run, so a fix to the boot's middle
lands three times or drifts. This brings riscv64 and `x86_64` onto the path aarch64 takes, one slice
at a time, until the two per-port boot files hold only what `arch/` cannot. The first slice puts
riscv64's bring-up through `bring_up_machine`.
