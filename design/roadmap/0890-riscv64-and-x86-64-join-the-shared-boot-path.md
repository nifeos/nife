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

calef, 2026-10-19 (UTC), on the three per-port tours: "3". No per-port tours. Each claim the tours
make becomes a check in `self_test::run` on every port where it applies, as milestone 268 (every
architecture boots the same way) did for `mapping` and `scheduler`, and the tours and their port
files go. The `TOUR` line and boot stage 11, which `serial_console` and the hang watcher read, get a
new source in the same change.

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
every transcript line `serial_console` reads, or moves its claim into the shared path first.

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
- **`serial_console` reads only the ladder.** It matches `boot_ladder`'s markers (banner, machine,
  self-test, tour, prompt) and failure markers. No live gate reads riscv64's first-phase lines;
  they appear in captured logs under `bench/` and `crates/serial_console/tests/fixtures/`, which
  are records of what a kernel printed and stay as they are.
- **The tail's "different order" is not one in effect.** riscv64's boot printed the machine
  description and ran the self-test before `icount`, `bench` and the test suite, and `kernel_main`
  runs them after. Both are compiled out of every test, `bench` and `icount` build (`icount`
  implies `bench`), so no build ran both, and the shared order changes no transcript.

- **`x86_64`'s BAR window never avoided the APICs.** `mmu::memory_mapped_io_window` keeps the
  window clear of the local and IO APICs by reading their addresses from `arch::irq`, which
  recorded them only when the APICs came up, after PCI had already run. Neither was ever avoided.
  On QEMU both sit above the hole, so no transcript changed.
- **`x86_64`'s test build ran the tour before the suite.** Its kernel thread, ring-3 processes and
  initial ramdisk report ran in every build, the suite last. The shared tail runs the suite right
  after the bring-up, as on the other two, and the suite passes without them.

## What was built: riscv64 runs the shared bring-up

The first slice, chosen because riscv64 shares the device tree and every portable step with
aarch64. Which port goes first is reversible and nobody has acted on it.

- `kernel_main` calls `bring_up_machine` on riscv64. `riscv64_first_light` is gone.
- `bring_up_machine` gains riscv64's three calls into `arch/`, each `cfg`-gated beside the
  aarch64-only `check_frequency_against_device_tree` that was already there:
  `timer::init_frequency` and `irq::init_contexts` in the window where the tree is reachable,
  `isa::print_summary` and `pmu::init` after the MMU, and `pci::init_iommu` beside the SMMUv3.
- riscv64's `arch::irq::init` calls `init_this_cpu`, the unmasking its boot did by name before
  starting the other harts. `interrupts_init` reaches it at the same point.
- Four first-phase lines are retired: `traps`, `timer`, `memory` and `paging`. Milestone 268 put each
  claim in `self_test::run` on all three architectures (`exceptions`, `timer`, `frames`, `mapping`)
  and retired the tour's own copies of two of them the same way, so this follows the tree's
  precedent rather than deciding anew. The `memory` line also allocated a frame only to print its
  address and never freed it.

The order on riscv64 changes in one way that matters: interrupts come on after the scheduler, as on
aarch64, rather than before memory. The old order needed interrupts masked around `sched::init` for
exactly that reason (riscv64_boot.rs, before this milestone), and the mask is gone with it.

## What was built: the PLIC, and riscv64's tail

- riscv64's `arch::irq::init` brings the PLIC up: it stores the base from `memory::plic_region`,
  opens the boot hart's S context, then unmasks the hart. The three `drivers::plic::init` sites in
  `riscv64_boot.rs` are gone. The boot context's threshold opens before the other harts start; that
  delivers nothing on its own, because every source stays disabled until `irq::enable` routes it,
  and it means the base is stored before `sie.SEIE` is set.
- `kernel_main` runs riscv64's tail: the test suite, `icount`, `bench`, the machine description,
  the self-test, the tour, the measurement boots and the hand-over, on the same lines as aarch64.
  `riscv64_boot` and `riscv_hand_over` are gone. The hand-over prints a line when there is no
  initial ramdisk on both ports, as riscv64's did.
- riscv64's tour is its `milestone_tour`, in `riscv64_boot.rs`, reached from the line that reaches
  aarch64's. It keeps every line it printed, `boot_ladder::TOUR` and boot stage 11 included, so
  `serial_console` and the hang watcher read what they read before.
- riscv64's banner is `riscv64_banner` in `boot.rs`, beside aarch64's.

## What was built: `x86_64` runs the shared bring-up and tail

- `memory::init` has a front end per description. On `x86_64` it is `arch::x86_64::discovery::init`:
  the PVH handoff, the ACPI walk, COM1's interrupt line and the IOMMU windows, the PCIe window and
  topology while the boot map still reaches every bus, then the frame allocator. The walk is kept,
  because the tables are reachable only through the boot map `mmu::init` replaces.
- The steps after it read the kept walk. `smp::read_cpu_list` seats the cores from the MADT.
  `arch::irq::init_contexts` brings up the boot CPU's local APIC in the window riscv64 fills with
  its PLIC contexts. The fine map and the BAR window take both APICs' addresses from it.
- `arch::irq::init` brings up the IO APIC and proves the PIT's line through it before the tick is
  armed, so the `device irq` line `uefi-boot` reads is unchanged. `discovery::bring_up_iommus`
  brings up VT-d or AMD-Vi beside the SMMUv3.
- `kernel_main` runs `x86_64`'s tail on the same lines as the other two. `x86_64_boot.rs` is gone;
  `x86_64_banner` is in `boot.rs` beside riscv64's.
- One hand-over, `boot.rs`'s `hand_over`, serves all three, and `x86_64` adds its install offer
  before it. After it the boot thread leaves the scheduler on every port. Every gate that types at
  the shell waits for its `$ `: `script/swish-check`'s three legs and `x86_64`'s install, rollback
  and confirm boots.
- The bring-up's `traps` and `timer` lines retire, as riscv64's did: `self_test::run` makes both
  claims on all three. The `frames` line no longer allocates a frame it never frees.

The order on `x86_64` changes the way riscv64's did: interrupts come on after the scheduler, the IO
APIC after the fine map, and the PIT's window runs with the scheduler up and the tick not yet armed.

## What was built: the tours are the self-test's checks

calef, 2026-10-20 (UTC), on how a check reports a machine without its device: "Agreed", to a
third outcome, `absent`. The condition: each `serial_console` board profile names the checks that
may not be absent on that board, and reads such an absence as a failure.

Every claim the three tours made, and where each one is now:

| Claim | Tours that made it | Now |
|---|---|---|
| Never-yield threads are preempted | aarch64, riscv64 | `preemption`, all three: one spinner more than cores online, none starved |
| A user-level program reaching a kernel address is killed; the kernel survives | aarch64 (outlaw), riscv64 (outlaw, address space), `x86_64` (a hand-built child) | `userspace`, all three |
| A process spends its untyped and the kernel allocates nothing | aarch64 (memory region) | `budget`, all three; the process is killed and its region destroyed |
| A spawned kernel thread runs | `x86_64` (kernel task) | `scheduler` |
| A user-level driver reads a file off a virtio disk, woken by its interrupt | aarch64 (MMIO and PCIe); riscv64 (discovery) | `blockdev`, all three; absent without a disk |
| A device's interrupt line reaches the CPU | `x86_64` (the PIT, in `arch::irq::init`); riscv64 (the UART, given a keystroke) | `deviceirq`; absent on aarch64 and riscv64, where the hand-over's input driver proves the line |
| The machine's entropy source serves a client | `x86_64` (RDSEED), riscv64 (JH7110 TRNG) | `entropy`, all three; absent with no TRNG or seed instruction |
| The JH7110's clock controller ungates the TRNG | riscv64 | `clock`; absent off a JH7110 |
| The JH7110 TRNG's draws, timed | riscv64 | `trng`; absent off a JH7110 |
| The boot's middle stages finished (`boot_ladder::TOUR`) | riscv64 | `kernel_main`, all three, after the verdict; the checks note stages 3 to 14 and 15 is finished |
| A faulting ring-3 thread's death reaches its supervisor; two rounds of children leak no frames | `x86_64` | Not a check (BUGS) |
| A bare riscv64 ELF initrd runs | riscv64 | Gone: every boot carries an archive |
| Reports: image bounds, initial ramdisk, boot file | aarch64, `x86_64` | The machine description's |
| An idle console server | aarch64 | Gone with `console_service` |

Absences board profiles forbid: radon `entropy`, `trng` and `clock`; xenon `entropy` and
`deviceirq`. The emulator profile, which `boot-check` and the soak use, forbids none.
`SELF_TEST_CHECKS_EARLIER` (provisional) records the five-check list's length, so a captured log
or an older image still reads green. `milestone_tour.rs`, `riscv64_boot.rs`, `boot_demos.rs` and
`console_service.rs` are gone.

## The xenon boot, and the screen half that remains

xenon is the one machine whose firmware places the APICs, the framebuffer and the BAR window for
real. With no monitor attached it boots `main` to the prompt: the `io apic`, `device irq` and `smp`
lines, every tour line, and `echo hello` echoing (`bench/xenon-2026-10-21/README-890.md`). With its
monitor attached the hand-over panics before the prompt, `the framebuffer driver did not come up
(0xdead000000000002)` (pull request #2528). One boot with the monitor, after that lands, closes the
`x86_64` half.

## The radon boot: to the prompt, and every keystroke

radon netbooted twice on 2026-10-21 (UTC) from this milestone's branch, and both boots proved, on
the JH7110:

- The PLIC slice reads the S contexts from the device tree. The console is source 32. `smp: 4
  core(s) online`. The self-test reads `11 of 13 passed, 2 absent: blockdev deviceirq`, with
  `clock`, `trng` and `entropy` passing.
- The hand-over prints `uart shape: registers 4 byte(s) apart, 32-bit access (device tree)`, and
  the shell's prompt follows.
- With the fix, `echo hello` and a 50-byte line, each typed in one write, and `ls` typed a byte at
  a time, all echo and run (`bench/radon-2026-10-21/890-keystroke-fix.log`). `reboot` at the
  prompt reset the board through the AXP15060 and it netbooted the next image.

`bench/radon-2026-10-21/README-890.md` lists every byte typed and what each boot ran.

## Why radon took one keystroke per boot

The trap handler claimed source 32, cleared its enable bit on the claiming context, then wrote the
completion. The PLIC specification ignores a completion for a source its target does not enable,
and the JH7110's PLIC does, so its gateway waited for a completion it had already been sent and
forwarded nothing more. QEMU's PLIC completes whatever it is given, so every gate passed.

The bench probe that showed it printed the UART's and the PLIC's state every 5 s, then completed
source 32 on each context that enabled it (`bench/radon-2026-10-21/890-plic-probe.log`). After one
keystroke a second sat in the UART, `lsr=0x61` (data ready) and `iir=0xc4` (receive data
available), with the source enabled at priority 1 and `pending=false`. Each probe completion let
exactly one more byte through.

The fix makes the claim the mask: the gateway holds a claimed source until its completion, so the
handler leaves the source enabled, records the claiming context (`arch::irq::hold_claim`), and the
driver's `Irq` ACK completes it there. `drivers/plic.rs` has no `disable`. The masked-line test's
replayable falsification is an ACK that forgets its claim.

## How userspace learns the UART's shape

The JH7110's UART registers are four bytes apart with 32-bit access (`reg-shift = <2>`,
`reg-io-width = <4>`); QEMU's are one byte apart. The kernel passes the tree's shape as the
progenitor's first start word, and the progenitor passes it to `console` and `input`
(`abi::uart::RegisterShape`). The console's wait for THRE is bounded at one second, so a wrong shape
kills it with a load fault at the LSR offset it polled (`stval` `0x14` or `0x5`). The timer tick
tests for a stalled log drainer once a second (`kernel_log::stall_tick`), so that kill report
reaches the wire.

## Done when

1. `kernel/src/x86_64_boot.rs` and `kernel/src/riscv64_boot.rs` are gone, or hold only what
   `arch/<port>/` cannot.
2. `kernel_main` has no `cfg(target_arch)` branch to a per-port boot.
3. `script/test`, `script/swish-check` and the board's `serial_console` stages pass on all three.

## Exit criteria, as built

| criterion | state |
|---|---|
| 1 | Done. `riscv64_boot.rs` and `x86_64_boot.rs` are gone. |
| 2 | Done. `kernel_main`'s `cfg(target_arch)` lines are each port's banner and riscv64's console register shape; `hand_over`'s one is `x86_64`'s install offer. |
| 3 | Under QEMU, both ports: see each pull request's gate table. radon's half is the boot above; xenon's lacks only its boot with a monitor. |

## BUGS

- QEMU cannot show a dropped PLIC completion: its PLIC accepts a completion for a disabled source.
  A handler that disables before it completes passes every gate, and only radon's console shows
  it.
- `bring_up_machine` carries six `cfg(target_arch)` sites: aarch64's frequency check, two blocks
  riscv64 and `x86_64` share, the SMMUv3, riscv64's IOMMU and `x86_64`'s. Each calls into `arch/`
  or `pci`, so rule 1 holds, but a port that needs a step aarch64 does not is still a `cfg` here
  rather than a function every port implements.
- `x86_64`'s new order is proven on xenon without a screen only (above).
- `clock`, `trng` and `entropy` pass on radon, but `serial_console` has not read radon's profile
  against them.
- `x86_64`'s supervised death and two-round frame steady state are not checks. No portable spawn
  takes a supervisor, and one before-and-after over a spawn and its teardown counts whatever else
  allocated in the window, which is why that demo ran two rounds. `system_tests` proves both.
- Gates run `console` and `input` at QEMU's stride; a host test pins the JH7110's.
- `blockdev`'s driver is left running on disk 0, as the tour's was.

## Follow-on

- **Done.** radon's console register shape: the boot above reaches the prompt with the tree's
  shape (2026-10-21).
- **Done.** radon's keystroke path: the prompt takes every keystroke, and `echo hello` echoes and
  runs (above, "Why radon took one keystroke per boot";
  `bench/radon-2026-10-21/890-keystroke-fix.log`).
- **Outstanding.** xenon's boot of `main` with its monitor attached, after pull request #2528 (above).
- **Proposed.** A portable check for a supervised death that reaches its supervisor, and one for
  teardown returning every frame (BUGS):
  `design/roadmap/proposals/the-self-test-proves-supervision-and-teardown.md`.
- **Milestone 945.** `serial_driver`, which the self-test left with no starter, is retired:
  `design/roadmap/0945-serial-driver-has-no-starter.md`.

## Index row

Two of the three architectures boot through code that only they run, so a fix to the boot's middle
lands three times or drifts. This brings riscv64 and `x86_64` onto the path aarch64 takes, one slice
at a time, until neither per-port boot file is left. Both ports run the shared bring-up and tail
under QEMU, `x86_64` through `memory::init`'s PVH and ACPI front end, and the per-port tours are the
self-test's checks on all three. radon runs both slices through the hand-over, its console now uses
the UART's real stride, its prompt takes every keystroke, and xenon's boot with a monitor remains.
