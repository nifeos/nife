---
status: BUILT
raised: 2026-09-21
built: 2026-10-22
---
# 524. The three x86_64 boot gates: NX, SYSCALL, and the invariant TSC

All three gates refuse, are tested on the host, and have been watched refusing a real boot. The
third refuses only on bare metal. Under a hypervisor it warns, because a guest's clock is the
hypervisor's to keep and QEMU's TCG never reports the bit (calef, 2026-10-22 00:54 UTC, on #2534:
"524: refuse a CPU without invariant TSC unless the hypervisor bit is set; under a hypervisor,
warn.").

The defect was one defect. `kernel/src/arch/x86_64/isa.rs`'s own `BUGS` said it plainly:
*"Nothing is recorded or checked yet. The other two implementations gate the boot on the features
the kernel actually uses (RISC-V refuses a firmware without the SBI extensions it calls). The x86
equivalents worth gating on are NX, SYSCALL, and the invariant TSC; this reports and does not
refuse."* x86_64 booted on three assumptions it never confirmed, and the file that would confirm
them already existed and already executed `CPUID`.

## What each gate checks, and where it sits in the boot

A check belongs at the last moment before the thing it protects. Two of these could obey that and
one could not, and the exception is the interesting one.

| Feature | `CPUID` | Where | Why there |
|---|---|---|---|
| NX | `80000001H:EDX[20]` | `boot.s`, in the 32-bit trampoline, before `EFER.NXE` is written | that `wrmsr` is itself the hazard |
| `syscall` | `80000001H:EDX[11]` | `isa::init`, six lines before `arch::init` programs the four MSRs | the console is up by then |
| invariant TSC | `80000007H:EDX[8]` | `isa::init`, before `timer::init_frequency` measures a rate | same, and see below |

**NX has to be checked in assembly, and that is not a stylistic preference.** `boot.s` sets
`EFER.NXE` before it builds a page table, because a table built while `NXE` is clear and walked
while it is set changes meaning under the walker. On a part without NX that bit is reserved, so the
`wrmsr` raises `#GP` with no IDT installed, which escalates to a double fault with no IDT and then
triple-faults. On QEMU that is a machine reset with no output at all. So the check goes before the
write, and the report is bytes out of the legacy COM1 transmit register with a five-write
initialization, because that is the only medium in existence at that point in the boot.

**That report is a marked exception rather than a design.** On a machine with no COM1 the `out`
instructions go nowhere and the halt is the whole of the message. It is still strictly better than
the reset it replaces, because a halted machine can be attached to and a reset one cannot, but a
reader at a blank screen learns nothing. `boot.s` says so where a reader meets it.

`isa::init` re-checks NX in Rust rather than trusting that the assembly ran, because the assembly
gate can only halt and this one can say what is wrong.

## The third one is a different kind of check

NX and `syscall` announce their own absence the moment the kernel uses them: a `#GP` on the `EFER`
write, a `#UD` on the first system call. **The invariant TSC does not, and no care at boot would
find it.** `arch::timer::init_frequency` measures the TSC's rate once, against the PIT, and stores
it. On a part without `CPUID.80000007H:EDX[8]` the counter's tick rate moves with the core's
frequency and idle state, so there is no single rate for that measurement to be the answer to.
Every check available at boot passes, because the measured value is correct at the instant it is
measured. The error appears later, when the CPU changes power state and the stored rate keeps
saying what it always said, and it appears as time itself being wrong.

**A wrong assumption cannot be caught by measuring harder.** That is the whole argument for
treating it as a gate rather than as something a better calibration loop would catch.

## The finding: under an emulator it cannot be a plain gate, and the machine is why

A plain refusal refuses the machine this project's suite runs on.

**QEMU's TCG does not advertise the invariant TSC and cannot be persuaded to.** `-cpu max,invtsc=on`
answers:

```
qemu-system-x86_64: warning: TCG doesn't support requested feature: CPUID[eax=80000007h].EDX.invtsc [bit 8]
```

and clears the bit (QEMU 11, measured 2026-09-21). It is a KVM-only feature there, because QEMU
will not promise rate constancy across a migration it does not control. The x86_64 suite runs
entirely under TCG, on an Apple Silicon host where KVM is not available at all, so every x86
machine this project can run on today reports zero, and a refusal would refuse every boot.

That is worth stating as a finding rather than as an obstacle: **the rate `arch::x86_64::timer`
measures under QEMU is one the machine underneath has never promised.**

Real silicon does promise it. Every x86 part since about 2008 has the bit, and xenon, milestone 87 (the
x86_64 bare-metal machine), an Intel Core i5-7500T, prints `features : nx syscall invariant-tsc
rdseed` (`bench/xenon-2026-10-21/README-524.md`). So the rule splits on who keeps the clock, and
`CPUID.1:ECX[31]`, the hypervisor bit, says which. It reads zero on silicon. QEMU sets it under TCG,
measured on the boot below.

The other two architectures' feature tables carry `required: bool`, and two values were enough for
them. This one has three:

| `Gate` | Means |
|---|---|
| `Refuse` | the boot stops and says what is missing (NX, `syscall`) |
| `RefuseUnlessHypervisor` | on bare metal, as `Refuse`; under a hypervisor the boot says so on its own line every time and continues (invariant TSC) |
| `Report` | useful to know, never fatal (`RDSEED`) |

Collapsing the middle value into `required: false` would file a load-bearing assumption beside an
optional convenience, and collapsing it into `Refuse` would refuse every boot the suite runs.
`REQUIRED` and `REQUIRED_UNLESS_HYPERVISOR` are both derived from the table at compile time, so a
row's `Gate` is the only thing anyone can get wrong, and a host test pins today's values. The
decision reads the hypervisor bit beside the feature bits, so an absent row of either gate lands in
exactly one of `missing_requirements` and `unpromised`, and the Kani proof over the decode holds
that. Both new names are provisional.

## How the refusal was tested, given that it is unreachable

**A gate that has never refused is a gate nobody has tested**, and every machine available here has
NX and `syscall`. Both halves of the answer are in the tree.

On the host, `crates/machine_discovery/tests/x86_64_cpu_features.rs`: the decision function
takes the `CPUID` words as an argument, so a part that does not exist can be handed to it and the
refusal watched happening, in milliseconds and without an emulator. One test per required feature
cleared on its own (so a decode reading the wrong bit cannot pass by clearing everything at once).
The invariant TSC gets four, present or absent on bare metal or under a hypervisor, from two
fixtures: QEMU's `-cpu max` and xenon. Three more are maximum-leaf cases. Those are the rule x86
actually needs: `CPUID` has no way to report "I do not implement that leaf" other than answering
with some other leaf's bits, and that includes leaf 1, so the hypervisor bit is believed only from
a part that answers it.

And on a real boot, which is the stronger test and turned out to be available: QEMU can hide a
feature from the guest.

```
NIFE_CPU="max,nx=off"         helpers/qemu-bounded.sh 60 cargo xtask boot-check --arch x86_64
NIFE_CPU="max,syscall=off"    helpers/qemu-bounded.sh 60 cargo xtask boot-check --arch x86_64
NIFE_CPU="max,hypervisor=off" helpers/qemu-bounded.sh 60 cargo xtask boot-check --arch x86_64
```

The first produces the assembly refusal, over a UART nothing had configured, before paging:

```
nife cannot run on this machine:
  cpu feature : nx is absent (the hardware bit W^X is made of; without it the page tables' no-execute is a comment)
                CPUID.80000001H:EDX[20] reads 0
halted before enabling paging, because EFER.NXE would #GP here.
```

The second produces the Rust refusal, after the console banner, naming the feature and the leaf.
The third hides the hypervisor bit, so TCG's guest looks like silicon without an invariant TSC, and
the boot refuses it (2026-10-22):

```
nife cannot run on this machine:
  cpu feature : invariant-tsc is absent (the boot measures the TSC's rate ONCE; without this there is no single rate to measure)
                CPUID.80000007H:EDX[8] reads 0
                refused on bare metal: CPUID.1:ECX[31] reads 0, so the part's promise is all there is
```

That is the only machine on which the bare-metal refusal can fire: xenon reports the bit, so a
boot there takes the clean branch, and the host tests are the proof for the refusing one. None of
these is a substitute for the host tests: QEMU can only hide a feature from a whole boot, so each
case costs a minute of emulator and cannot assert on the decision in isolation.

## What the part reports, which was the other half of the old BUGS entry

`CPUID`'s 48-character brand string was sitting unread. The boot now prints it, and both leaf
spaces' maxima, and every row of the table present or absent (`no-invariant-tsc` rather than
silence, because a missing word in a list of present ones is not something a reader notices).

On QEMU `q35`, `-cpu max`, TCG, on an Apple Silicon host, 2026-10-22:

```
  unpromised  : invariant-tsc (CPUID.80000007H:EDX[8] reads 0); the boot measures the TSC's rate ONCE; without this there is no single rate to measure
                warned, not refused: CPUID.1:ECX[31] reads 1; a guest's clock is its hypervisor's
  cpu         : x86_64, vendor AuthenticAMD, cpuid leaves 0..0xd and 0x80000000..0x80000021, under a hypervisor
              : QEMU TCG CPU version 2.5+
  features    : nx syscall no-invariant-tsc rdseed
```

The vendor is `AuthenticAMD`, which is not what anyone guesses about QEMU's `max` model, and
the maximum extended leaf is `0x80000021`, well past Intel's. Both are now in the host fixture
rather than in a guess.

## What is not done

- The gates are checked on the boot CPU only. Every secondary replays `boot.s`'s `EFER` write
  and `arch::init`'s `syscall` MSRs with nothing re-reading `CPUID`. Right on a uniform part;
  wrong on a hybrid one, where Intel's P-cores and E-cores differ in what leaf 7 reports.
  `machine_discovery::riscv64` already solves the twin problem (the intersection over every hart),
  so the shape to copy exists. Recorded in `kernel/src/arch/x86_64/isa.rs`'s `BUGS`.
- Under a hypervisor the invariant-TSC gate warns, and the gate believes the hypervisor bit, so a
  hypervisor that hides itself is treated as silicon and refused. Recorded in the same `BUGS`.

## Follow-on

- **Done.** The invariant-TSC row is `Gate::RefuseUnlessHypervisor`, per calef's ruling of
  2026-10-22 00:54 UTC on #2534, in `crates/machine_discovery/src/x86_64.rs`'s `TABLE`, carried by
  branch `milestone/524-hypervisor-tsc-gate`.
- **Recorded.** Per-core feature checks, for a hybrid part where the boot CPU does not speak for
  the others. In `kernel/src/arch/x86_64/isa.rs`'s `BUGS` section, beside the gates it qualifies.

## Index row

x86_64 booted on three assumptions it never confirmed, where the other two architectures refuse a
machine that cannot run them. NX is checked in the 32-bit trampoline before `EFER.NXE` is written,
because that write is itself the hazard and there is no console yet to explain a `#GP`; `syscall`
and the invariant TSC are checked in `isa::init` before their consumers. The decision takes the
`CPUID` words as an argument, so the refusal path is exercised on the host in milliseconds although
no machine here lacks any of the three, and it was also watched refusing a real boot with
`NIFE_CPU=max,nx=off`. The invariant TSC refuses on bare metal and warns under a hypervisor,
because QEMU's TCG declines to advertise the bit under any invocation and a guest's clock is the
hypervisor's to keep. The hypervisor bit, `CPUID.1:ECX[31]`, decides which, and the warning says so
on its own line every boot.

## Where this leaves milestone 161

Milestone 161 (the x86_64 kernel port) is BUILT, so `isa.rs`'s `BUGS` was pointing at finished
work. It now points here.
