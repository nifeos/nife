---
status: PARTIAL
raised: 2026-08-03
milestone_dependencies: 75, 127
decision_dependencies: none
machine_requirements: aarch64 and riscv64 silicon; PMU cycle counter
specific_machine: none
needs_person: yes
---
# 74. Cycle counters: SBI PMU on RISC-V, `PMCCNTR_EL0` on aarch64

Raised 2026-08-03, from an audit of what milestone 16a actually needs. Its
deliverable includes "the benches on real cycles via the SBI PMU extension", and until 2026-09-03
nothing in the tree implemented it: `PMU` appeared only in device-tree test fixtures and in this
file. Both ISA halves are now built (riscv64 2026-09-03, aarch64 2026-09-19), and it stays
`PARTIAL` because argon has not run the aarch64 half and radon's procedure is still owed. What
aarch64's counter counts and what the user-mode read is called were ruled by calef on 2026-10-07
(UTC) and built by milestone 353 (`design/roadmap/0353-the-aarch64-half-of-74.md`).

Both halves of this line are under correction, and neither says
what it used to.

Milestone 75's index row reads `NOT-STARTED` and that is false: its mechanism is built.
`design/decisions/0139-cycle-counter-authority.md` was DECIDED by calef on 2026-09-02,
`kernel/src/sched.rs`'s `install_cycle_counter_grant` applies the per-thread grant at every context
switch behind the `cycle_counter_grant` feature, and
`system_tests::user::cycle_counter_tests::a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults`
passes on all three architectures, negative case included. Whether 75's row is flipped, and what
that unblocks, is an architect's rather than a lane's; the mechanism cited here is what a reader can
check today. The aarch64 half of 74 was out of the riscv64 lane's scope, not blocked by an
unanswered question. Until 2026-09-19 `PMCR_EL0.E` and `PMCNTENSET_EL0.C` were never written by
this kernel, so `PMCCNTR_EL0` was a stopped counter that a granted thread read the same value from
every time. That is built now; see "What the aarch64 half built" below.

The `HARDWARE` half is now the second sense of that gate rather than the first: the riscv64 code is
written and gated. What remains is a person at radon following notes/riscv-cycle-counters.md's
procedure, because QEMU-TCG models an instruction counter that has
nothing to do with cycles.

The riscv64 half is built (2026-09-03). The SBI PMU extension is probed as an optional fifth row of
`SBI_TABLE`. `kernel/src/arch/riscv64/pmu.rs` asks firmware to find and start a counter for
`SBI_PMU_HW_CPU_CYCLES` and checks it is actually counting. It remembers which CSR reads it and
records why when there is none. The boot prints all of it, and `cargo xtask bench --riscv` prints one
`cycles_per_tick` probe. The aarch64 half is built too (2026-09-19), in the same shape. The two
decisions it left were ruled and built on 2026-10-07,
`design/roadmap/0353-the-aarch64-half-of-74.md`.

## What we read today, and why it is not cycles

Both ISAs read a fixed-rate reference counter, not a cycle counter:

| | aarch64 | riscv64 |
|---|---|---|
| today | `CNTVCT_EL0` + `CNTFRQ_EL0` | the `time` CSR (`rdtime`) |
| counts | a fixed tick, 62.5 MHz under QEMU | a fixed tick |
| resolution | ~41 ns on real silicon | comparable |
| the cycle counter we lack | `PMCCNTR_EL0` | SBI PMU, or the `cycle` CSR when `mcounteren` permits |

notes/pmu.md already sets this out for aarch64 and calls confusing the two a category error. The
generic timer is the OS's clock; the PMU counts CPU cycles at ~0.25 ns resolution and its rate moves
with frequency scaling. Two ways to measure a fast operation: one shot at high resolution (PMU,
which is what sel4bench does) or a long loop at low resolution (the generic timer, which is what we
do). Both are valid and they fail under different conditions.

## Why it matters more than "another counter"

The thesis claim is a cross-OS comparison, and the literature it is compared against is denominated
in cycles, not nanoseconds. notes/benchmarks.md does the conversion by hand, across silicon:

> seL4 publishes, for the same-core different-address-space path, 413 cycles for the IPC call and
> 426 for the IPC reply, one-way each ... So the corrected figure is roughly 1.1x to 1.7x an
> L4-lineage round trip, not 4 to 7 times.
>
> -- notes/benchmarks.md

This file has quoted that paragraph wrongly twice, which is why the quote above carries an
attribution line a gate can check. Both wrong versions, and why no gate caught either, are in
[`corrections.md`](0074-cycle-counters/corrections.md).

That ratio is not a measurement of the two kernels. It sets seL4's PMU cycles, read on bare metal
on a 2015 Cortex-A57 (the Jetson TX1), against nife's nanoseconds under HVF on an Apple M3,
multiplied by a clock rate nothing pinned. The ratio in force is the same-silicon one in
[arm-plan-under-hvf.md](../../notes/arm-plan-under-hvf.md), with seL4 booted as a guest on the same
Apple core. Its `Call` plus `ReplyRecv` between two address spaces takes 96 ns and nife's
`ipc_rtt_el0` 442 ns, about 4.6 times (2026-10-21, eight boots each, nife at `f87c9b5c4`). Both are guests, both
averaged over 10,000-pass loops against the generic timer rather than read per call through a PMU,
and seL4 carries one patched line for the guest's 8-bit ASIDs. The operation still differs: seL4's
two fastpath syscalls against `ipc_rtt_el0`'s four.

What that cannot give is a cycle count, and cycles are the unit the L4 literature publishes in.
Measuring them directly turns the cross-OS figure from a derived one into a read one, and it is the
number a reader from the L4 world will look for first.

## Two things block on it

- 16a cannot deliver "benches on real cycles" without it.
- Milestone 25's `sel4bench` is built and booting but was deferred to real hardware precisely
  because it times single operations through `PMCCNTR_EL0`, which neither QEMU-TCG nor Apple HVF
  provides. notes/pmu.md's last section explains why virtualization keeps the PMU out of reach: the
  generic timer is architected state a hypervisor must present, and the PMU is not.

## Parity: the capability half is two ISAs, the measurement half is three

This heading read differently until 2026-09-16, and the old wording asserted completeness over a
gap. What it said, and why it was wrong, is in [`corrections.md`](0074-cycle-counters/corrections.md).

The two halves of this milestone have different parity answers, and conflating them is what hid
the gap.

The capability half is legitimately two ISAs, by a recorded exception rather than an omission.
[DECISIONS §139 part 3](../decisions/0139-cycle-counter-authority.md) rules that x86_64 keeps its
ambient counter: `CR4.TSD` is clear at reset, this kernel never writes it, so ring 3 may execute
`rdtsc` and an ungranted read is not an error. There is therefore no x86_64 counterpart to
"grant the counter, fault without it", and
`system_tests::user::cycle_counter_tests::a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults`
skips its negative half there, saying so in the comment beside the `cfg`. That is §19 (architectural parity is a tenet) working: an
exception with a reason, where a reader meets it.

The measurement half is three ISAs.
`bench::cycles_per_tick`, the probe that converts every tick-denominated board row to cycles, was
`#[cfg(target_arch = "riscv64")]`; milestone 309 added the x86_64 arm. The ratio is the thing
cross-machine comparison needs, which is this milestone's whole purpose and milestone 25's and
§96's, so having it on fewer than three architectures is the parity gap that matters here rather
than a cosmetic one.

An earlier version of the paragraph above named the wrong mechanism for x86_64;
[`corrections.md`](0074-cycle-counters/corrections.md) keeps it, because it is the reasoning most
likely to be re-derived.

Scope note, per §19. The measurement half is built for riscv64 and aarch64 (this milestone)
and x86_64 (milestone 309, `design/roadmap/0309-x86-64-core-cycles.md`), so `bench::cycles_per_tick`
now has no `cfg` at all. All three count user and kernel: riscv64 every mode including M-mode
firmware, x86_64 ring 0 and 3, and aarch64 EL0 and EL1, by calef's ruling A1 of 2026-10-07 (UTC)
that `PMCCFILTR_EL0` is `0` on every board (`design/roadmap/0353-the-aarch64-half-of-74.md`).

### The capability half, per ISA

§19 is a gate, and it bites here in an unobvious direction. The milestone reads as RISC-V work because
16a is the RISC-V board, but `PMCCNTR_EL0` is equally unimplemented, so a RISC-V-only cycle
counter would create a parity gap in the one subsystem whose entire purpose is cross-machine
comparison. Both sides are small and they are not symmetrical in shape:

- aarch64: enable the counter (`PMCR_EL0`), open it to EL0 (`PMUSERENR_EL0`), read `PMCCNTR_EL0`.
  Register writes, no firmware call. Whether that EL0 opening happens at all was milestone 75's
  decision, and it was answered by DECISIONS §139 (who may read the cycle counter, and by what
  authority): a per-thread grant, which milestone 229 built. The counter is ~160x finer than the one §10 already excepted, so it does not
  inherit that exception, and it does not: starting the counter opens nothing to EL0 by itself.
  Built 2026-09-19, below.
- riscv64: the SBI PMU extension (EID `0x504D55`), which discovers counters, configures an event,
  and starts and stops it. The tree already makes SBI calls (`SBI_HSM_EID`, `SBI_IPI_EID`,
  `SBI_RFENCE_EID`, SBI TIME), so the plumbing exists and this is a fifth extension rather than new
  machinery. Built 2026-09-03; the one change it forced is in
  [`riscv64-half.md`](0074-cycle-counters/riscv64-half.md).

## What can be done before the board, and what cannot

Buildable now: both drivers, the `Isa`-style capability probe, the benchmark harness change, and
the aarch64 path end to end (Apple Silicon has a real PMU; whether macOS lets a guest reach it is the
open question notes/pmu.md raises).

Not verifiable until silicon: the RISC-V numbers. QEMU-TCG models an instruction counter that has
nothing to do with cycles, so a green test under emulation proves the plumbing and says nothing about
the measurement. Say so in the note rather than publishing an emulated cycle count.

The emulator was more misleading than that prediction allowed for. The finding, and the printed
inputs that defend against it, are in [`riscv64-half.md`](0074-cycle-counters/riscv64-half.md).

## Scope note

Do not turn this into a profiling framework. One counter, read before and after, on two ISAs. The
PMU can count dozens of events and the temptation to expose them generically should wait for a second
consumer, which is CLAUDE.md's rule against speculative trait-ification. `sel4bench` comparability is
the requirement; anything beyond it is scope.

## What the riscv64 half built

Built 2026-09-03, lane `milestone/74-cycle-counters-riscv`: the optional SBI PMU row, the
`CounterInfo` decode, `kernel/src/arch/riscv64/pmu.rs` with its boot line, one
`bench-probe: cycles_per_tick` line, and notes/riscv-cycle-counters.md. What each one is, and what
was deliberately not built, is in [`riscv64-half.md`](0074-cycle-counters/riscv64-half.md).

## The riscv64 number, measured on radon 2026-09-16

`bench-probe: cycles_per_tick 250.00 (25000940 cycles over 100002 ticks at cntfrq 4000000)`,
transcript `bench/radon-2026-09-16/bench-134300.log`. That is this milestone's `HARDWARE` half in
its second sense: a person at the board, following `notes/riscv-cycle-counters.md`. The counter is
the one the riscv64 half built, and the boot reported it before the sweep:
`cycles : SBI PMU counter 0, CSR 0xc00, 64 bits`.

Why a round ratio is real here when it was an artifact under QEMU, and the bench rows it converts
to cycles and wall time, are in [`riscv64-half.md`](0074-cycle-counters/riscv64-half.md).

## What the aarch64 half built (2026-09-19)

Lane `milestone/74-cycle-counters-aarch64`, in the riscv64 half's shape: start one counter, check
it moves, refuse it and say why when it does not, print it, and add the probe line. Nothing here is
on the context-switch path. [`aarch64-half.md`](0074-cycle-counters/aarch64-half.md) holds what was
built, the `HPMN` defect in the EL2 drop of milestone 127 (the seL4 machine: a Jetson TX1) that it found and fixed, and which path each
QEMU configuration took.

### BUGS

- No aarch64 cycle figure is a result yet: every number above is an emulator's.
- `Stuck` has never fired; see [`aarch64-half.md`](0074-cycle-counters/aarch64-half.md).
- The first PMU access on a board is still milestone 228's `PMUSERENR_EL0` write in `timer::init`,
  before the banner. If TF-A leaves `MDCR_EL3.TPM` set it traps there, not here; 127's procedure
  now says what that looks like.
- Per-core counters are unrelated numbers. `C` zeroes each at its own init; `arch::pmu::cycles`
  reads the current core's, and only a same-core difference means anything.
- The user-mode read is `user_mode_runtime::cycle_reading()` (decision B, step 1).

### argon's first evening, for this half

After 127's steps 1 to 5, read these lines, in this order:

   1.9 GHz over 19.2 MHz, about 99, and not a round integer. A round number means something is
   1.9 GHz over 19.2 MHz, about 99, and **not** a round integer. A round number means something is
   feeding both counters from one clock.
2. `firmware left PMCCFILTR_EL0 0x...`. Write this down whatever it says: it is the best
   evidence of what seL4's 413 and 426 counted. `0x0` means kernel included; bit 31 set means EL1
   excluded. It does not change what nife writes: calef ruled on 2026-10-07 (UTC) that the
   filter is `0` on every board (design/roadmap/0353-the-aarch64-half-of-74.md, decision A). It is
   the input to milestone 25 (cross-OS comparison): if bit 31 is set, 25 runs nife a second time with the filter matched to
   seL4's and labels that run as such.
3. `6 event counters visible`. The A57 implements six; a different number means `HPMN` or firmware.
4. If the line says `refused` or `disagrees`, the secure world is prohibiting counting on at least
   one core; record which.
5. Then `script/bench` on the board: `cycles_per_tick` with its inputs.

## Follow-on

- **Milestone 353.** The aarch64 half, rewritten 2026-09-19 now that the counter runs: decision A
  (what `PMCCFILTR_EL0` counts) and decision B (the user-mode read's name and promise). calef ruled
  A1 and B4 on 2026-10-07 (UTC) and 353 built both, except B4's step 2, which is a proposal.
- **Recorded.** `crates/machine_discovery/src/aarch64.rs`'s BUGS: `TGran4 = 0b0001` (FEAT_LPA2) is
  read as "no 4 KiB granule", so the kernel refuses `-cpu max` and any LPA2 part. Found by this
  half's CPU sweep; a two-line fix outside this milestone.
- **Recorded.** `notes/riscv-cycle-counters.md`: three limits the riscv64 half ships with, each
  beside the feature. It configures the boot hart only, because SBI PMU counters are per-hart
  and the one consumer is a single-hart probe. It refuses a firmware counter rather than reading
  one at an `ecall` per read. And nothing has been measured on silicon, which is the whole
  reason that note is mostly a bench procedure.
- **Recorded.** `kernel/src/arch/riscv64/pmu.rs`: QEMU's `rva23s64` model hands back an
  `hpmcounter` that TCG does not drive, so a counter firmware describes as working can read zero
  forever. `init` now refuses such a counter, and the module's `BUGS` and the stop test's own doc
  comment carry the observation.

## Index row

16a's deliverable names "benches on real cycles via the SBI PMU extension". Both halves are
built. riscv64 (2026-09-03): SBI PMU is probed as the first *optional* row of `SBI_TABLE`,
`arch::riscv64::pmu` starts a cycle counter and remembers which CSR reads it, and `bench --riscv`
prints one `cycles_per_tick` probe; radon measured `250.00` on 2026-09-16. aarch64 (2026-09-19):
every core starts and checks `PMCCNTR_EL0`, the boot prints the answer and what firmware left in
`PMCCFILTR_EL0`, and `bench` prints the probe under the filter calef ruled on 2026-10-07 (`0`, user
and kernel); argon has not run it. The user-mode read is `user_mode_runtime::cycle_reading()`, which
returns the count with what it counts (design/roadmap/0353-the-aarch64-half-of-74.md)
