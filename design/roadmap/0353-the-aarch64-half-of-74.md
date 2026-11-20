---
status: BUILT
raised: 2026-09-03
built: 2026-10-07
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 353. The aarch64 half of milestone 74: two decisions left, now that the counter runs

Filed 2026-09-03 as an unnumbered proposal by the
`milestone/74-cycle-counters-riscv` lane, numbered 2026-09-19 by milestone 433 (drain), and the work it
described was built the same day by the `milestone/74-cycle-counters-aarch64` lane. `PMCR_EL0.E`
and `PMCNTENSET_EL0.C` are written, so `PMCCNTR_EL0` is no longer a stopped counter. What was left
was two rulings. calef made both on 2026-10-07 (UTC), and the `milestone/353-cycle-read` lane built
what they require the same day. The options they chose between follow the rulings,
unchanged.

## The rulings, 2026-10-07 (UTC)

calef answered "Yes" to both as stated below. Recorded as
[§261 (PMCCFILTR_EL0 is zero on every aarch64 board, and a cycle read carries its meaning)](../decisions/0261-pmccfiltr-el0-is-zero-on-every-aarch64-board-and-a-cycle-read-carries-its-meaning.md)
(number provisional).

Decision A: A1, zero on every aarch64 board, not argon's firmware value. The reason: `PMCCFILTR_EL0`'s reset value is architecturally UNKNOWN and
firmware-dependent, so argon's value says what argon's firmware does and what seL4's TX1 figures
were measured under. It cannot set nife's policy. The kernel writes `0` (EL0 and EL1 counted, EL2
not), consistent with riscv64 and x86_64, which both count user and kernel. argon's firmware value is still read at first boot, for milestone 25
(cross-OS comparison) only: if firmware left `P` set, milestone 25 runs nife a second time with
the filter matched to seL4's and labels that run as such.

Decision B: B4, in two steps. Step 1: one public function in `crates/user_mode_runtime` that
returns the count paired with what it counts. On riscv64 that is core cycles, flagged if the kernel
probe was handed `hpmcounter3`. On x86_64 it is the TSC, labeled as constant-rate reference cycles
and not core cycles; `rdpmc` and `CR4.PCE` are not reopened. On aarch64 it is core cycles counting
user and kernel, per A1. A program knows whether it may read from its own manifest's grant, so
step 1 needs no system-call-surface change. Step 2, a kernel-provided "may I read" page or method, is a
later system-call-surface fork and is not built.

### What was built

- A. The constant is `PMCCFILTR_COUNT_EL0_AND_EL1` (name provisional), and the boot and bench
  lines no longer say `PROVISIONAL`. `FIRMWARE_FILTER` stays, as milestone 25's evidence only.
- B, step 1. `user_mode_runtime::cycle_reading()` returns `CycleReading { count, meaning }`, where
  `meaning` is an `abi::cycle_counter::CycleMeaning`; all three names are provisional. It is in
  `abi` so the kernel's test decodes it without a second copy (AGENTS.md rule 7), and that test
  checks the meaning on all three architectures. `cycle_counter_reader` reads through it.
- Not built: the riscv64 flag. A process cannot learn which counter the kernel's probe got without
  a page or a call, which is step 2's territory, so it is proposed with step 2 (Follow-on).

## Decision A: what `PMCCFILTR_EL0` counts

The ruling is A1, above. This section holds the options as they were laid out.

### What is being decided

`PMCCFILTR_EL0` says in which exception levels `PMCCNTR_EL0` increments. Its reset value is
architecturally UNKNOWN on every field (Arm's `AArch64-pmccfiltr_el0` page, read 2026-09-19 at
`arm.jonpalmisc.com/latest_sysreg/AArch64-pmccfiltr_el0`). The fields that matter here:

| bit | field | meaning when set |
|---|---|---|
| 31 | `P` | do not count EL1 |
| 30 | `U` | do not count EL0 |
| 29, 28 | `NSK`, `NSU` | Non-secure EL1, EL0: counted as `P`, `U` say when equal to them, not counted when different |
| 27 | `NSH` | do count EL2 (the one field whose sense is inverted) |
| 26 | `M` | EL3: counted as `P` says when equal to it, not counted when different |

The kernel writes `0` (`PMCCFILTR_COUNT_EL0_AND_EL1`, provisional until A1): EL0 and EL1, not
EL2, and EL3 wherever `MDCR_EL3` permits.

### The options

| | value | counts | the question it answers |
|---|---|---|---|
| A1 | `0x0000_0000` | EL0, EL1 (EL3 if permitted) | "what did this operation cost the machine, kernel included, with nothing below us" |
| A2 | `0x0800_0000` (`NSH`) | EL0, EL1, EL2 (EL3 if permitted) | the same, plus any time a hypervisor below us spends on our behalf |
| A3 | `0x8000_0000` (`P`) | EL0 only | "what did userspace spend", a profile |
| A4 | not written | whatever firmware left | "the same as whatever else ran on this board's firmware" |

### Question 1: What else was considered, and why each is a live option rather than a loser

None loses on the facts; they answer different questions, which is why this is an architect's.

- A3 cannot referee an IPC comparison, and that is the one thing to know about it. An IPC round
  trip timed from EL0 spends almost all of its cycles in the kernel, and `P` = 1 stops the counter
  there. The number would be the user-mode instructions around `svc`. It is the right instrument for
  a userspace profile (milestone 147's profiler) and the wrong one for milestone 25.
- A4 is what seL4 does, and it is the one option this tree's own rule argues against. It inherits
  an UNKNOWN value, which is the exact defect milestone 228 fixed for `PMUSERENR_EL0`. It is listed
  because it is literally the seL4 configuration (below), and a reader deserves to know that the
  comparison target made this choice.
- A1 versus A2 changes nothing on this kernel today. `boot.s` drops to EL1 and installs no EL2
  vector table, so no cycle is ever spent at EL2 on argon or under QEMU TCG. They diverge only under
  a hypervisor that lets a guest program the PMU, or if this kernel ever ran at EL2 (VHE). Choosing
  between them is choosing what a future number would mean, not what today's means.

### Question 2: What this tree already does in the analogous case

Both other architectures count every privilege level, including the kernel's, and neither was
decided as a published position; each is the mechanism's default:

- riscv64 (`arch::riscv64::pmu`): `sbi_pmu_counter_config_matching` is called with
  `CLEAR_VALUE | AUTO_START` and none of the specification's inhibit flags (`SINH`, `UINH`, `MINH`,
  `VSINH`, `VUINH`), so the counter runs in every mode, M-mode firmware included. radon's
  published `cycles_per_tick 250.00` was measured that way.
- x86_64 (`arch::x86_64::pmu`, milestone 309 (unhalted)): `FIXED_CTR1_BOTH_RINGS`, ring 0 and ring 3, and
  its doc comment says why in one line: "a counter that stopped at the ring boundary would make
  `x86_64`'s number mean something different from theirs".

So A1 (or A2) is the option consistent with the two numbers in this tree, and A3 would make
aarch64 the odd one out. That is precedent, not a ruling: neither of those was put to calef either.

### Question 3: The prior art, read

- seL4 never writes `PMCCFILTR_EL0` in the configuration it publishes. The kernel's
  `arm_init_ccnt` (`src/arch/arm/benchmark/benchmark.c`, read 2026-09-19) writes
  `PMCR = E | C | P` and `PMCNTENSET` bit 31, and nothing else. libsel4bench's `sel4bench_init`
  (`libsel4bench/arch_include/arm/armv/armv8-a/sel4bench/armv/sel4bench.h`, read 2026-09-19) writes
  the filter only `#ifdef CONFIG_ARM_HYPERVISOR_SUPPORT`, and then to `BIT(27)`, which is `NSH`:
  with the kernel at EL2, it counts EL2. sel4bench's `settings.cmake` (read 2026-09-19) turns
  hypervisor support on only when `VCPU` is set, and the build line on
  `sel4.systems/performance.html` for the TX1 (read 2026-09-19: `init-build.sh` with `FASTPATH`,
  `HARDWARE`, `FAULT` and `AARCH64` set to `TRUE`, `ITERATIONS=5` and `PLATFORM=tx1`) sets no
  `VCPU`. So the 413 and
  426 were counted under whatever filter the TX1's firmware left, with seL4's kernel at EL1. The
  kernel is counted unless that firmware set `P`.
- What that firmware left is now something argon prints. Since this lane, the boot line reads
  `firmware left PMCCFILTR_EL0 0x... on this core before it was overwritten`. QEMU says `0x0`. argon
  runs the same family of TF-A and U-Boot the Foundation's board did, so its first boot is the best
  evidence available for what seL4's figures included. It is not proof: nobody here knows the
  firmware revision on the Foundation's bench.
- Linux `perf` counts everything by default and only userspace when it is not allowed more.
  `armv8pmu_set_event_filter` (`drivers/perf/arm_pmuv3.c`, read 2026-09-19) sets
  `ARMV8_PMU_EXCLUDE_EL1` (`P`) only for `exclude_kernel`, `EXCLUDE_EL0` only for `exclude_user`,
  and `ARMV8_PMU_INCLUDE_EL2` (`NSH`) unless `exclude_hv`. A plain `perf stat -e cycles` as root is
  therefore A2. Unprivileged, with the default `perf_event_paranoid` of 2, the tool retries with
  `exclude_kernel` and `exclude_hv` set and says so (`tools/perf/util/evsel.c`, read 2026-09-19:
  "kernel.perf_event_paranoid=%d, trying to fall back to excluding kernel and hypervisor samples"),
  which is A3. So the Linux answer to "what does `cycles` count" depends on who is asking.

### Question 4: Is the premise true?

Mostly, with one correction worth making before calef rules. The brief framed this as
"seL4-comparable versus userspace-only". **seL4-comparable is itself not a single value**. It is A4
on the published board, which is whatever that board's firmware left (argon's boot line is the best
guess at it, and QEMU's `0x0` would make it A1), and A2 in seL4's own hypervisor builds. So the decision is between "count the kernel" (A1 or A2) and "count userspace" (A3),
with A1 against A2 a second, smaller question about a configuration nothing runs today.

### Question 5: What each costs, measured rather than asserted

**The same, to build: one constant.** `PMCCFILTR_PROVISIONAL` is written once per core at init and
never on the switch path, so no option moves `script/fastpath-footprint` or the icount tripwire
(both were run with A1 in place; the block has the exit codes). **Not measured**: whether the filter
changes what a read costs, because QEMU does not model that and there is no silicon here. One real
cost difference exists and it is not performance: **A3 breaks `bench::cycles_per_tick`**, which
reads the counter at EL1 and would see it stopped. Choosing A3 means moving the probe to EL0, which
is milestone 237's grant build and a measurement build rather than the plain `--features bench`
boot.

### Question 6: How reversible, and who has already acted on it

The code is reversible in a line. **The number is not**, once published, and nothing has been: this
lane printed only QEMU's `16.00`, which is an instruction count and says so. radon's `250.00` and
xenon's future figure are the precedent a reader will compare an aarch64 number against, and both
count the kernel.

### Question 7: Would the choice be the same if every option cost the same?

They do cost the same, so nothing here is an effort argument: it is a question about what nife
wants its published numbers to mean.

### What happened when calef ruled

He chose to wait for argon's firmware value on 2026-09-19, then A1 on 2026-10-07 (UTC): the rulings
at the top of this block.

## Decision B: the portable user-mode cycle read, its name and its promise

### What is being decided

`fixtures/src/cycle_counter_reader.rs` carries the only user-mode read in the tree, one `mrs` (or
`csrr cycle`, or `rdtsc`) per architecture, and its doc comment says a portable function is milestone
74's to design. **This lane did not add it**, by instruction. The questions are what
`crates/user_mode_runtime` should call it, what it should promise, and whether it should exist yet.

### The fact that decides most of it: the three user-readable counters are not the same quantity

| | what EL0/U-mode/ring 3 reads | is it core cycles? | granted by |
|---|---|---|---|
| aarch64 | `PMCCNTR_EL0` | yes, filtered by decision A | milestone 229's grant (`PMUSERENR_EL0.CR`) |
| riscv64 | the `cycle` CSR | yes, **but** the kernel's own probe may have been handed a different counter (`hpmcounter3` on `rva23s64`) | 229's grant (`scounteren.CY`), and `mcounteren.CY` in firmware |
| x86_64 | `rdtsc` | **no**: constant-rate, the same counter as `user_mode_runtime::now()` | ambient (DECISIONS §139 part 3) |

The kernel's `cycles_per_tick` probe on x86_64 deliberately does not read the TSC (milestone
309's whole argument: it would divide one counter by itself). A user-mode function called
`cycles()` that read `rdtsc` on x86_64 would therefore be the exact mistake 309 was written to
prevent, in the one place a stranger would call it. Reading core cycles from ring 3 needs `rdpmc`
with `CR4.PCE` set, which milestone 228 deliberately closed and which is a DECISIONS §139 question,
not a naming one.

### The second fact: EL0 cannot ask whether it may read

An ungranted read is not an error return, it is a fault that ends the thread (that is the negative
half of `a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults`). So a user-mode
function cannot probe and fall back. Whatever it promises, it has to know from somewhere other than
trying.

### The options

- B1. No shared function yet. Programs that need the counter read it raw, as the one fixture
  does. The name waits for a second consumer, which is this tree's usual rule against building the
  abstraction first. Cost: each consumer re-derives the per-architecture caveats above.
- B2. A raw read whose name says what it reads, for example a `cycle_counter()` that exists on
  aarch64 and riscv64 only and is documented as "fatal unless the program's manifest carries the
  cycle-counter grant". Honest and small; the cost is that it is not portable, which on a project
  with §19 as a gate needs a scope note for x86_64.
- B3. A portable read that promises core cycles on all three, which on x86_64 means `rdpmc` of
  fixed counter 1 and therefore reopening `CR4.PCE` behind the grant. That is a §139 extension
  (a new door, granted) and a system-call-surface-adjacent change, so it is two decisions wearing one
  name.
- **B4. A read paired with its meaning**, a function returning the count together with what it
  counts (the filter, the core it ran on), in the shape the kernel's probe prints a meaning line.
  Linux's answer is closest to this: user-space self-monitoring reads the counter directly, and the
  `perf_event_mmap_page` it maps says whether it may (`cap_user_rdpmc`) and how wide the counter is
  (`pmc_width`). That is also what solves the second fact above. It is the most machinery and the
  only option that lets a program ask before it reads.

### The seven questions, briefly

1. Alternatives: the four above; each answers a different "what does a caller need".
2. This tree: `user_mode_runtime::now()` and `cntfrq()` are the analogous pair, and `now()` is
   documented per architecture with its caveats in a `BUGS` section. The kernel names the per-arch
   module `pmu` and the reading `cycles()` on all three, and on x86_64 that kernel `cycles()` is not
   the TSC. A user-mode `cycles()` that was the TSC would collide with the kernel's own meaning of
   the word.
3. Prior art: seL4's `sel4bench_get_cycle_count()` is a raw read with no promise beyond
   "whatever `PMCCNTR` says" (libsel4bench, read 2026-09-19); Linux has no libc function and uses the
   mmap page above; Rust's `core::arch::x86_64::_rdtsc` names the instruction rather than the
   quantity.
4. Premise: true that it is a naming decision; **not only** a naming decision, because B3 is a
   §139 extension on x86_64.
5. Cost: B1 zero; B2 one function and a scope note; B3 a `CR4.PCE` grant on the switch path
   (milestone 237 measured what the aarch64 and riscv64 version of that cost the fastpath); B4 a
   page or a call. None was built or measured here.
6. Reversibility: a public function in `user_mode_runtime` is a name programs compile against;
   renaming it later touches every caller, which is the expensive category.
7. **Equal cost**: B1 and B2 are not chosen for being cheap; B4 would still be the most honest if
   all four cost the same, and B3 is the only one that is more than a naming question.

### What happened when calef ruled

B4, in two steps, on 2026-10-07 (UTC): the rulings at the top of this block. Step 1 is built.

## Where it came from

The riscv64 lane's handoff (2026-09-03) and the aarch64 lane that built the counter
(2026-09-19). Everything cited was read on the date given, not recalled.

## Follow-on

- **Milestone 856.** Milestone 856 (a program asks whether it may read the cycle counter):
  *step 2 of decision B, and the riscv64 flag that needs it.* A kernel-provided way for
  a program to ask whether it may read the counter, and to learn which counter the kernel's own
  probe reads, is a system-call-surface fork calef deferred.
- **Recorded.** *No manifest field grants the cycle counter yet*, so no real aarch64 or riscv64
  program can call `cycle_reading` without being killed; only the kernel's test grants it, through
  a test-only door. DECISIONS §139 (who may read the cycle counter, and by what authority) put the
  grant in the spawn manifest, and carrying it there is milestone 75 (who may read the cycle
  counter, and by what authority). `cycle_reading`'s `BUGS` says so where a caller meets it.
- **Done.** *The rulings were recorded only in this block.* They are now §261 (PMCCFILTR_EL0 is zero on every aarch64 board), linked from "The rulings, 2026-10-07 (UTC)".
- **Done.** *`script/test --arch x86_64 --test <name>` failed on a test that lives only in the
  system-tests image*, because the OVMF kernel leg selected nothing and printed no time record.
  `xtask/src/time_record.rs`'s `whole` now accepts a leg that selected zero tests, in this branch.

## Index row

The two rulings the aarch64 half of milestone 74 (cycle counters) left, made by calef on 2026-10-07 (UTC) and built.
A1: `PMCCFILTR_EL0` is `0` on every aarch64 board, so aarch64 counts user and kernel like riscv64
and x86_64, and the boot and bench lines stopped saying `PROVISIONAL`; argon's firmware value is
read for milestone 25 (cross-OS comparison) only. B4 step 1: `user_mode_runtime::cycle_reading()`
returns the count with an `abi::cycle_counter::CycleMeaning`, core cycles on aarch64 and riscv64
and constant-rate reference cycles on x86_64, proved on all three by the granted-read test. Step 2,
a "may I read" page or method, is a deferred system-call-surface fork and is proposed, not built.
