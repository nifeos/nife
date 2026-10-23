# Bringing up the other cores

`kernel/src/smp.rs` reads which cores the machine has, starts the ones it can, and turns each into
a scheduler participant. DECISIONS §11 is the design; §28 is the load balancing that runs on top of
it, and [the scheduler note](scheduler.md) covers placement and stealing.

## How a secondary starts

The boot core reads the roster ([below](#the-roster-is-seated-by-hardware-id)), then calls the
architecture's start mechanism (`arch::cpu_start`: PSCI `CPU_ON` on aarch64, SBI HSM on riscv64)
once per startable core, handing it a physical entry point (`secondary_boot` in boot.s) and the top
of its own stack. The secondary starts with its MMU off, replays the MMU-enable, and lands in
`secondary_main`, which in order:

1. sets its per-CPU pointer, before anything can take a lock (the lock path reads the held rank
   out of the per-CPU block);
2. installs its exception vectors, which are per core;
3. adopts the kernel's fine map, without which it cannot reach the thread-stack area;
4. adopts its boot context as its idle thread and reserves its run queue;
5. brings up its own interrupt controller interface and its banked timer;
6. in a test build, spawns a probe onto its own queue;
7. sets its bit in the online mask, then increments the online count (Release), and unmasks
   interrupts;
8. yields once, then runs the shared idle body, which steals work from a loaded core.

The boot core waits for the count with Acquire, so once it sees a core counted it sees everything
that core set up. The mask is set before the count so that a TLB shootdown issued the instant a
core is counted also targets it.

The entry point is computed only after `arch::can_start_secondaries` says the machine can be asked
at all. On `x86_64` it is not converted from virtual to physical: `secondary_boot` is linked at the
fixed low address it executes from (`AP_TRAMPOLINE_PHYS`, below 1 MiB because a STARTUP IPI can
only name such a page), so its virtual address already is its physical one, and `virt_to_phys`
would underflow subtracting the direct-map base. `arch::secondary_boot_entry` is the identity
function for that reason, kept as a function so the reasoning sits beside the one call site.

The `single_hart` build (kernel/Cargo.toml) marks the boot core online and starts nobody. It
returns after setting the mask, not before, because everything that broadcasts reads the mask, and
an empty online set while `online_count` says one is a disagreement. It exists because a board has
no `-smp 1` to pass, and measurements that need threads contending for one core's cache need it;
see [footprint-perturbation.md](footprint-perturbation.md).

## The secondary stacks

Each core's slot is a 4 KiB guard page and then a 64 KiB stack that grows down into it. The guard
has no physical frame: `arch::mmu::map_everything` skips it, so the first byte written past the
bottom of the stack faults. It is the same mechanism as the boot stack's `__stack_guard` and a
kernel thread's guard. `verify` refuses to install a map in which a guard translates.

The stacks live in their own `.secondary_stacks` section for two reasons.

- **Writable.** An immutable `static` of plain arrays lands in `.rodata`, which the fine kernel map
  makes read-only (W^X). The `UnsafeCell` keeps it out. The failure is invisible on the coarse boot
  map, where `.rodata` is writable, and fires the instant a secondary adopts the fine map.
- **Holes.** `map_everything` maps `.data` through `__bss_end` as one range, which leaves nowhere
  for a hole. In their own region the mapper maps each stack separately and skips each guard. The
  region is `(NOLOAD)`, so nothing zeroes it; a stack does not need zeroing, and the high-water
  instrument paints every usable byte in a test build anyway.

`MAX_CPUS` is stated once, in Rust. The linker scripts only anchor `__secondary_stacks_start` and
`__secondary_stacks_end` around whatever the section holds, and
`the_secondary_stack_region_is_the_size_the_linker_reserved` holds the two together. On the coarse
boot map the guards sit inside a 2 MiB block and are mapped; a secondary is on that map only
between `secondary_boot` and `mmu::init_secondary`, the same window the boot stack's guard has (see
[stack-high-water.md](stack-high-water.md)).

## The roster is seated by hardware id

`read_cpu_list` reads `/cpus` out of the device tree once, on the boot core, before
`bring_up_secondaries`. It has to run while the tree is reachable, which is through the coarse boot
map that `mmu::init` replaces. On `x86_64` the same step reads the ACPI MADT's `LocalApic` entries
from the walk `memory::init` kept (`seat_cpus_from_acpi`).

A core's "id" is whatever the start call names it by: an `MPIDR_EL1[39:0]` affinity value on
aarch64, a hart id on riscv64, a local APIC id on `x86_64`. Each described core is seated **at the
slot its own hardware id names**. Everywhere the kernel indexes hardware it uses the logical id:
`cpu::PERCPU`, the secondary stacks and the RISC-V trap stashes are arrays indexed by it; the GICv2
targets an SPI by CPU-interface number and `send_sgi` uses the same number; the PLIC's S-mode
context table is indexed by hart id; the SBI IPI mask is a bitmap of hart ids. Seating by id makes
logical id equal hardware id by construction, so none of those has to translate.

Two consequences follow.

- An excluded core costs no other core its seat. On the VisionFive 2 the unusable S7 occupies slot
  0, its own, and U74 hart 4 sits at slot 4.
- A machine with sparse but small ids (harts {0, 2}) seats {0, 2} and works; nothing between them is
  started or counted. A core whose id is at or past `MAX_CPUS` cannot be seated. That includes every
  id on a clustered aarch64 board, where `MPIDR_EL1.Aff1` is in use. It gets a named line at roster
  time and stays parked rather than being remapped.

`u64::MAX` marks an empty slot. It is safe as well as convenient: an aarch64 affinity value is 40
bits by architecture and cannot reach it, and a hart id that could is past `MAX_CPUS` and is never
seated. Two nodes claiming one id (a malformed tree, or `cpu_list`'s missing-`reg` fallback of zero
colliding with the real hart 0) keep the first.

A core is **startable** unless firmware marked it `status = "disabled"`, its own `riscv,isa`
disclaims supervisor mode, or its `enable-method` is one this kernel does not speak (`spin-table`, a
per-SoC method). The supervisor rule is the one that matters on the VisionFive 2's vendor tree,
whose S7 node claims a schedulable core in both `status` and `mmu-type`; handing that hart to SBI
`hart_start` crashes vendor OpenSBI, and the roster line naming it is the only place its exclusion
is visible. The predicate is `machine_discovery::cpu_list::Cpu::is_startable`, host-tested against
both JH7110 fixtures. On `x86_64` the MADT's `enabled` flag is the whole answer: a present but
disabled entry already means a socket that firmware will not start. `arch::boot_cpu_id` reads the
boot core's own local APIC id through `CPUID` rather than assuming 0, so the equality holds for the
boot core too.

`DESCRIBED` keeps how many `cpu@` nodes the tree had, which may exceed the seats, so the boot line
can say a machine has more cores than the build was sized for instead of quietly using fewer.

## The online set is not contiguous

`online_cpus` is the online mask as an iterator, and every per-cpu loop, both IRQ-affinity
round-robins and the spawn and wake placement go through it or through `nth_online`. The RISC-V
RFENCE calls mask with `online_harts_mask` directly; aarch64's `tlbi ..., is` broadcasts in
hardware and needs no mask. A loop over `0..online_count()` is the bug: on the VisionFive 2 the
online set is {1,2,3,4}, and an index of 0 places work into a parked core's inbox that nothing will
ever drain. The mask arithmetic is in `crates/cpu_set`, where the non-contiguous shape QEMU cannot
boot is host-tested.

## How the SMP tests wait

**Bounded by the clock, at 60 seconds.** A yield count is not a duration: on a CI runner emulating
four cores under TCG, this core can burn millions of cheap yields before a secondary is scheduled
at all. The bound is 60 seconds rather than 10 because on a shared host the guest's vCPU threads
are starved of host CPU, so guest wall clock advances while the secondaries do not run. Ten seconds
failed on unmodified `main` about one run in six under synthetic load, on a different RISC-V model
each time, which is a statement about the host rather than the ISA. Widening is honest here because
the condition waited on is exactly the property under test, not a proxy for it. The suite's own
watchdog (`DEFAULT_BUDGET_SECS`, 90 s) still catches a dead secondary; staying under it means the
failure names the subsystem rather than reading as a generic livelock. See
[load-sensitive-assertions.md](load-sensitive-assertions.md).

**The probes are placed with `spawn_on`, not `spawn`.** `spawn` places by DECISIONS §28's two
random choices, deliberately not on the spawner's core, so each secondary's probe would land on a
random core and the cores nobody sampled would never set their slot. That wait is for a condition
that cannot become true, and a longer deadline does not fix an unreachable condition.

**The placement test asserts arrival, not execution.** `work_can_be_placed_on_every_cpu` reads the
target's own adoption counter (`PerCpu::adopted`) and does not require the target to run the thread.
Stealing is pull-based from an idle core (§28.3), and a core that is not idle neither steals nor
pushes, so a wait for every core to run a probe can wedge permanently:

1. the probe for core A is placed on A's queue while the test thread runs on A;
2. an idle core B steals it;
3. the remaining probes are placed and every core is busy, so nothing is idle;
4. A holds only the test thread, which never yields into idleness, so A never steals one back.

Execution is covered elsewhere: `every_secondary_runs_scheduled_work` proves each core runs what is
on its own queue, and `a_batch_of_cpu_bound_work_reaches_every_cpu` proves placement plus stealing
fills the machine. To test this core as a target, the placement has to come from another core, or it
is not the cross-core path (`place_on` puts a local target straight onto the local queue).

**The migration tests.** RISC-V keeps the kernel per-CPU pointer in `tp`, a general register the
trap frame saves and restores. A kernel thread preempted on one hart carries that hart's `tp` in its
frame, and if an idle hart steals it and resumes it with the stale `tp`, it reads and corrupts the
origin hart's scheduler state. aarch64's pointer is `TPIDR_EL1`, which no frame carries, so there
`percpu_matches_hart` is a constant `true`. `a_migrated_kernel_thread_keeps_its_hart_pointer` runs
waves of cpu-bound workers, more than there are cores, each alive about 40 ms (several 100 Hz ticks,
so it is preempted and has a saved frame). Early finishers drain their harts to idle, those harts
steal still-running workers, and every worker re-checks `tp` on every spin.

Each wave's drain is budgeted in delivered ticks (`testing::TickBudget`), not counter time:
`timer::now()` advances while the emulator is descheduled, so a counter-time budget is host time of
which a loaded host gives the guest a fraction. A descheduled emulator misses tick deadlines, so 200
ticks is two seconds on a quiet machine and longer on a busy one. The workers' own `life` stays in
counter time on purpose: it bounds how long a worker holds a hart, and a worker cut short has done
less spinning, which costs coverage rather than correctness.

`a_migrated_kernel_thread_reads_its_own_name` uses the same motion against `PerCpu::current`. If
reading it takes two steps (the per-CPU register into a general register, then the field), a tick
between them that moves the thread leaves the load reading the first core's `current`, which by then
names another thread. `sched::adopt_address_space` asks from the kernel thread every `user::run`
spawn starts on, so a split read there gives the new space to another thread. Three choices in the
test are measured: staggered lives keep cores draining and stealing through every wave; the clock is
read once per 256 reads because TCG takes an interrupt only between translation blocks; and 24
waves, because the two-core `x86_64` leg moved a worker only four or five times per run at fewer.
With all three, every run on every ISA failed against a split read.

## BUGS

- **The migration tests lose coverage quietly under host load rather than going red.** A worker's
  `life` is counter time, so a worker descheduled through most of its 40 ms is preempted fewer times
  and drives less migration. Measured: one riscv64 wave took 1005 ms of wall clock and was charged
  ten delivered ticks, so the guest ran about a tenth of it. The direction is safe (a loaded run
  passes having proven less). Denominating `life` in ticks too would stretch the workers as well as
  the drain, and their product is what the 90 s per-test ceiling has to hold.
- **The PSCI `smc` conduit is proved by parsing and not by calling.** `crates/machine_discovery`'s
  host tests read a real QEMU dump that states `smc`, but no machine here boots that configuration,
  so the `smc` instruction path has never executed. See `arch::cpu_start`.
- **The roster test has no `x86_64` form.** `the_roster_is_the_machines_own_cpu_list` re-reads the
  device tree, and `crate::DTB` on `x86_64` holds PVH's `hvm_start_info` pointer, so it skips there.
  An independent re-walk of the MADT would give it the same power; nobody has written it.
