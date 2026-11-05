---
status: BUILT
raised: 2026-09-17
built: 2026-09-17
---
# 316. Which core booted: making `NIFE_SMP=2` mean something on x86_64

Built by a lane on `milestone/316-x86-smp-two-cores`.
*(Number provisional until the merge queue lands it.)*

`arch::x86_64::ap_boot`'s `BUGS` #3 is fixed at its root, both of its symptoms are gone, and the
two-core suite was run repeatedly rather than once. The `NIFE_SMP` default stays at 1, and the
reason it stays is not the reason it stayed before, which is the finding worth carrying out of this
milestone.

## The bug, and why it looked correct for a month

`arch::x86_64::boot_cpu_id` read `CPUID` leaf 1's initial local APIC id on every call:

```rust
let leaf1 = isa::cpuid(1);
((leaf1.ebx >> 24) & 0xff) as usize
```

That is a correct answer to "which core am I". Every caller in this tree wants "which core
booted", and those are the same number only while there is one core. Milestone 161 (kernel) changed this
function from a hardcoded `0` to the `CPUID` read for a real reason. The roster seats the boot core
at the slot its own local APIC id names, so the two have to agree. It got the mechanism wrong while
getting the requirement right.

The other two architectures never had the option of being wrong this way, and the asymmetry is
instructive:

| Architecture | `boot_cpu_id` | Why that shape |
|---|---|---|
| aarch64 | the constant `0` | the boot core is architecturally affinity 0 on `virt`; nothing to record |
| riscv64 | reads `BOOT_HARTID` | OpenSBI hands the id in `a0` and it is gone after `_start`, so boot.s must catch it |
| x86_64 (before) | recomputes from `CPUID` | `CPUID` stays readable forever, so nothing ever forced the record |

RISC-V got this right by accident of its boot protocol. Its id would have been lost if boot.s
had not stashed it, so the record existed for a reason unrelated to correctness-under-SMP, and then
turned out to be the correct shape anyway. x86 had no such forcing function: the value is always
available, so recomputing it read as the simpler option right up until a second core existed.

## Both symptoms, one cause

DECISIONS §28's placement can migrate a test body onto a secondary. When it did, that body asked
`boot_cpu_id` and got the secondary's id:

- `smp::tests::every_secondary_runs_scheduled_work` builds its "is a secondary" predicate as
  `|c| c != boot`. With `boot` naming core 1, core 1 is excluded and core 0 is included, so the test
  waited for a `RAN_ON[0]` mark that the real boot core, which runs no probe, never sets. It then
  failed on its bounded wait: *"secondary cores did not run scheduled work in time"*.
- `stack::report_high_water` skips the boot core's slot for a good reason (the boot core runs on
  the linker-script stack, so its `.bss` slot was never painted). Choosing that slot the same way
  meant it skipped core 1's painted slot and scanned core 0's unpainted one, which contains
  no paint to find a high-water mark in, so it reported a secondary stack at `65536/65536` bytes,
  100%.

One was a hang-to-timeout and the other a false alarm about stack exhaustion, and they looked like
two bugs in two subsystems. They are one line.

## The fix

`boot.s`'s `_start_high` stamps the id once, after it zeroes `.bss` and before it calls
`kernel_main`:

```asm
    mov eax, 1
    cpuid
    shr ebx, 24
    movzx ebx, bl
    movabs rax, offset BOOT_CPU_ID
    mov [rax], rbx
```

and `boot_cpu_id` reads the record. That is riscv64's `BOOT_HARTID` move, deliberately, rather than
a third shape: code that runs exactly once, on exactly the boot processor, cannot tell a later
caller that the later caller is it. aarch64 is untouched and keeps its constant, because a record
of a value the architecture fixes would be machinery with nothing to hold.

The ordering that makes this safe is worth stating, because the function is called from the kernel's
very first Rust statement (`cpu::init_this_cpu(arch::boot_cpu_id())`, before the console, the GDT or
ACPI). The stamp precedes `call kernel_main`, so there is no window in which the accessor can read
the `AtomicUsize`'s zero default. The secondary entry path (`ap_long_mode_entry`) never writes it.

`BOOT_CPU_ID` is a provisional name. It mirrors `BOOT_HARTID` on the architecture that already
had this shape, which is the answer to "what does this tree already do in the analogous case", but
names are an architect's.

## The number

Measured with `NIFE_SMP=2 script/repeat-under-load -n 12 -s 0 -- --arch x86_64` on patagonia
(Mac15,3, 8 cores), no induced load, logs under `target/acceptance/`. Each run is a full
`script/test --arch x86_64`, which is three boots: SeaBIOS plus two OVMF.

5 of 12 runs green, load average 2.0 to 8.9, one QEMU at every sample (no neighboring lane).
The interesting number is not that one. It is that all seven failures are the same assertion in
`a_revoked_holder_faults_on_its_next_port_write` in `system_tests/src/user/x86_port_tests.rs`, and none of them is #3 or #1:

| what was measured | across the 12 runs |
|---|---|
| `smp::tests::every_secondary_runs_scheduled_work` | 19 of 19 `ok` (it runs once per boot that reaches it) |
| `stack::report_high_water`'s secondary line | 12 of 12 read `core1 9760/65536 bytes (14%)` |
| `smp: N core(s) online` | 26 of 26 boots read `2 core(s) online` |
| failures | 7, all `a_revoked_holder_faults_on_its_next_port_write`, `left: 2, right: 1` |

Before the fix, `every_secondary_runs_scheduled_work` failed about half of runs at two cores
(`ap_boot.rs`'s own measurement) and the high-water report printed `65536/65536`. Nineteen clean
runs of the one and twelve of the other is not a proof. A flake that fires one run in six needs
more than twelve to be called gone. It is enough to say the half-the-time failure is not
half-the-time any more.

Neither `BUGS` #1 nor the UEFI two-core start flake fired once. Milestone 412
(`design/roadmap/0412-the-uefi-boot-gate-asserts-two-cores-that-do-not-always-start.md`)
records `smp: cpu 1 did not start (firmware returned -1)` at one run in three from a three-run
sample; this campaign saw 26 two-core boots and zero such lines, 8 of them under OVMF. That is
incidental evidence rather than the dedicated `cargo xtask uefi-boot` measurement the proposal asks
for. It is a different tree than the one that proposal was written against. One-in-three and
zero-in-twenty-six do not sit together comfortably, and somebody should reconcile them.

## Why the default stays at 1, and what changed about the reason

The remaining blocker is not an SMP bring-up bug. It is a confinement defect that the second
core makes visible:

`user::x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` goes red intermittently at
two cores, with `left: 2, right: 1`. The revoked holder's `out` succeeded and the word arrived,
where the test demands a fault. That is exactly the window
`sched::delete_port_range_caps_impl` already documents at itself:

> This resets **the revoker's core only**. ... On a multi-core x86 a holder that is *running on
> another core* keeps its installed bitmap until that core's next context switch, so its `in`/`out`
> succeed for up to one tick after this returns.

Milestone 313's audit found that window, reasoned it to "at most one tick", and accepted it.
Milestone 315 is the lane that closes it by broadcasting the reset over the existing shootdown IPI.

**This is a better outcome than a green run**, and it bears directly on
`design/decisions/0153-two-core-x86-test-sequencing.md`, which is `PROPOSED` and was gated on this
milestone:

- §153's option 1 (order 315 behind the SMP bugs) worried that *"the substrate a two-core port test
  would run on is one where an existing SMP test is already unreliable"*. The SMP test named there
  is `every_secondary_runs_scheduled_work`, and it is now reliable.
- §153's option 2 worried about *"a test that may be red for reasons unrelated to what it tests"*.
  The red observed here is **for exactly the reason it tests**: a capability that was revoked was
  still usable. That is the ambiguity the decision was built around, and it did not materialize.
- §153 calls milestone 315's test *"the first port test to run on two cores and the first able to
  observe the window at all"*. **It already exists.** `a_revoked_holder_faults_on_its_next_port_write`
  became that observer the moment the substrate under it worked, without being written for it.

None of which is this lane's call to make: §153 (how a two-core x86_64 test earns its place) is an
architect's, and the above is evidence for it rather than an answer to it.

## BUGS

- `ap_boot`'s `BUGS` #1 is untouched and still open. A third or later secondary fails
  intermittently at `-smp 3` and above, two hypotheses tested and refuted, no root cause. It was out
  of this milestone's scope by the brief, and it did not fire in 26 two-core boots here, which is
  consistent with its recorded "three or later" bound and proves nothing about three.
- The two-core suite is not clean, per the section above, and `NIFE_SMP` therefore still
  defaults to 1 in `helpers/qemu-runner-x86_64.sh`. Nothing in CI gates two cores, and nothing here
  changes that. The two-core result above is a measurement on one host, not a promise.
- Milestone 412
  (`design/roadmap/0412-the-uefi-boot-gate-asserts-two-cores-that-do-not-always-start.md`)
  is a separate failure and was not addressed. Its signature is `smp: cpu 1 did not start
  (firmware returned -1)`, a core that never starts at all, which is a different thing from a core
  that starts and is then misidentified. This lane's campaign is incidental evidence about its rate
  and nothing more; the proposal's step 1 asks for a dedicated measurement of
  `cargo xtask uefi-boot`, which this is not.
- This lane measured on QEMU TCG only. Every claim above is about an emulated machine.
- **Milestone 161's block is `PARTIAL` and names this bug in its status text.** This lane did not
  edit 161 (a developer edits only its own block); the maintainer reconciles it at merge. 161's
  item 5 is where this PARTIAL comes from.

## Follow-on

- **Milestone 315.** The named next step, and better specified than it was: its test does not have
  to be invented, because the suite already contains a two-core observer of the window. What 315
  still owes is the broadcast.
- **Decision.** `design/decisions/0153-two-core-x86-test-sequencing.md` has its premise changed and
  wants re-reading with the section above beside it. Not this lane's to resolve.
- **Recorded.** A bench boot on xenon would settle `BUGS` #1, and it is cheap now that the stick and the
  procedure exist (notes/x86-uefi-boot.md). #1 was *"measured extensively on QEMU TCG"* and has
  never been tried on silicon. There the INIT-SIPI-SIPI timing, the `STARTUP` delays and the
  self-modifying-code behavior that two of its refuted hypotheses turned on are all real rather
  than emulated. xenon has four cores and printed `nife machine: x86_64, 4 processor(s)` on
  2026-09-17. **The exact line to look for is `smp: N core(s) online`** in the boot transcript at
  `-smp 4`. `smp: 4 core(s) online` says #1 is a TCG artifact, and anything less, with a
  `smp: cpu N did not start` line above it naming a core that varies between boots, says it is real.
  Boot it several times; one boot decides nothing, since the defect is intermittent under TCG too.

## Index row

`arch::x86_64::boot_cpu_id` recomputed `CPUID` leaf 1's initial local APIC id on every call, which
answers "which core am I" where every caller wants "which core booted"; the two agree only while
there is one core. So a test body DECISIONS §28 (SMP placement: two random choices at spawn)'s
placement had migrated onto a secondary took that
secondary for the boot core. Two symptoms in two subsystems turned out to be one line.
`every_secondary_runs_scheduled_work` inverted its own "is a secondary" predicate and waited out its
bound for a mark the real boot core never sets. `stack::report_high_water` scanned an unpainted
slot and reported a secondary stack at 65536/65536. `boot.s` now stamps the id into `BOOT_CPU_ID`
(provisional) before calling `kernel_main` and the accessor reads the record: riscv64's
`BOOT_HARTID` shape, because code that runs once on exactly the boot processor cannot misinform a
later caller. Twelve two-core runs came back 5 green, and the useful numbers are the others. 19 of
19 on the migrated-boot-core test, 12 of 12 correct high-water lines, 26 of 26 boots with both cores
online, and **all seven failures one assertion at one line**. **The `NIFE_SMP` default stays at 1,
for a new reason**: what keeps the two-core suite from being clean is
`a_revoked_holder_faults_on_its_next_port_write`, red because `PortRange::REVOKE` resets the TSS I/O
bitmap on the revoker's core only. That is milestone 313's accepted window and milestone 315's
target. It is a red for exactly the reason the test exists rather than substrate flakiness, which is
evidence DECISIONS §153 was written without. The two-core observer 315 planned to write already
exists in the suite. `BUGS` #1 remains open and did not fire at two cores.
