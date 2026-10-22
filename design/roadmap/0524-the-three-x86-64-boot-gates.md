---
status: PARTIAL
raised: 2026-09-21
milestone_dependencies: none
decision_dependencies: none
machine_requirements: an x86_64 CPU reporting invariant TSC
specific_machine: xenon (milestone 87's machine; the boot that reports the invariant-TSC bit)
needs_person: yes
---
# 524. The three x86_64 boot gates: NX, SYSCALL, and the invariant TSC

Two of the three gates refuse, are tested on the host, and have been watched refusing a real boot.
The third cannot refuse on any machine this project can run on, for a reason that was measured
rather than assumed, and it warns instead; the trigger that promotes it is written down below and in
the code.

What is left is one token in one table row, and what gates it is a boot on a
part that reports `CPUID.80000007H:EDX[8]`. QEMU's TCG will not, on any invocation, so no amount of
work here can clear it: milestone 87 (the x86_64 bare-metal machine) is the machine, and the trigger is
a boot there reporting the bit.

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

## The finding: it cannot be a gate here, and the machine is why

The first run of the check refused the boot, which is what it is for, and what it refused was the
machine this project runs on.

**QEMU's TCG does not advertise the invariant TSC and cannot be persuaded to.** `-cpu max,invtsc=on`
answers:

```
qemu-system-x86_64: warning: TCG doesn't support requested feature: CPUID[eax=80000007h].EDX.invtsc [bit 8]
```

and clears the bit (QEMU 11, measured 2026-09-21). It is a KVM-only feature there, because QEMU
will not promise rate constancy across a migration it does not control. The x86_64 suite runs
entirely under TCG, on an Apple Silicon host where KVM is not available at all, so every x86
machine this project can run on today reports zero, and a refusal would refuse every boot.

That is worth stating as a finding rather than as an obstacle: **the assumption
`arch::x86_64::timer` had been making since the port landed is one the machine underneath has never
promised.** The old `BUGS` entry in `timer.rs` said "QEMU's TSC is invariant". It is not, or at
least the machine declines to say so, and nothing had ever asked.

What was built instead of a two-valued flag. The other two architectures' feature tables carry
`required: bool`, and two values were enough for them. This one has three:

| `Gate` | Means |
|---|---|
| `Refuse` | the boot stops and says what is missing (NX, `syscall`) |
| `Warn` | the kernel is built on it, no machine here reports it, and the boot says so on its own line every time (invariant TSC) |
| `Report` | useful to know, never fatal (`RDSEED`) |

Collapsing the middle value into `required: false` would file a load-bearing assumption beside an
optional convenience, which is the shape of record that goes stale without anyone noticing.
`REQUIRED` and `WARNED` are both derived from the table at compile time, so a row's `Gate` is the
only thing anyone can get wrong, and a host test pins today's three values.

The promotion trigger (the `BUGS`-to-roadmap convention): a boot on real silicon that reports
the bit. Milestone 87 (the x86_64 bare-metal machine) is that machine, every x86 part since about 2008
has had it, and the change is one token in one table row.

## How the refusal was tested, given that it is unreachable

**A gate that has never refused is a gate nobody has tested**, and every machine available here has
NX and `syscall`. Both halves of the answer are in the tree.

On the host, `crates/machine_discovery/tests/x86_64_cpu_features.rs`: the decision function
takes the `CPUID` words as an argument, so a part that does not exist can be handed to it and the
refusal watched happening, in milliseconds and without an emulator. Nine tests, one per required
feature cleared on its own (so a decode reading the wrong bit cannot pass by clearing everything at
once), plus the two maximum-leaf cases. Those are the rule x86 actually needs: `CPUID` has no way
to report "I do not implement that leaf" other than answering with some other leaf's bits.

And on a real boot, which is the stronger test and turned out to be available: QEMU can hide a
feature from the guest.

```
NIFE_CPU="max,nx=off"      helpers/qemu-bounded.sh 60 cargo xtask boot-check --arch x86_64
NIFE_CPU="max,syscall=off" helpers/qemu-bounded.sh 60 cargo xtask boot-check --arch x86_64
```

The first produces the assembly refusal, over a UART nothing had configured, before paging:

```
nife cannot run on this machine:
  cpu feature : nx is absent (the hardware bit W^X is made of; without it the page tables' no-execute is a comment)
                CPUID.80000001H:EDX[20] reads 0
halted before enabling paging, because EFER.NXE would #GP here.
```

The second produces the Rust refusal, after the console banner, naming the feature and the leaf.
Neither is a substitute for the host tests: QEMU can only hide a feature from a whole boot, so each
case costs a minute of emulator and cannot assert on the decision in isolation.

## What the part reports, which was the other half of the old BUGS entry

`CPUID`'s 48-character brand string was sitting unread. The boot now prints it, and both leaf
spaces' maxima, and every row of the table present or absent (`no-invariant-tsc` rather than
silence, because a missing word in a list of present ones is not something a reader notices).

On QEMU `q35`, `-cpu max`, TCG, on an Apple Silicon host, 2026-09-21:

```
  unpromised  : invariant-tsc (CPUID.80000007H:EDX[8] reads 0); the boot measures the TSC's rate ONCE; without this there is no single rate to measure
  cpu         : x86_64, vendor AuthenticAMD, cpuid leaves 0..0xd and 0x80000000..0x80000021
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
- The invariant TSC is read on silicon: xenon, an Intel Core i5-7500T, prints `features : nx
  syscall invariant-tsc rdseed` and no `unpromised` line (`bench/xenon-2026-10-21/README-524.md`).
  The row is still `Gate::Warn`, which is why this milestone is PARTIAL rather than BUILT.

## Follow-on

- **Outstanding.** Promote the invariant-TSC row from `Gate::Warn` to `Gate::Refuse`. The
  silicon trigger is met (above). The token alone cannot move: QEMU's TCG never reports the bit,
  so `Refuse` would refuse every `x86_64` boot the gates run. The promotion needs a rule for an
  emulated CPU first (for example, refuse only where `CPUID.1:ECX[31]`, the hypervisor bit, is
  clear), and that rule is an architect's call. The table row is in
  `crates/machine_discovery/src/x86_64.rs`'s `TABLE`, and the host test that pins today's value
  fails on purpose when it moves.
- **Recorded.** Per-core feature checks, for a hybrid part where the boot CPU does not speak for
  the others. In `kernel/src/arch/x86_64/isa.rs`'s `BUGS` section, beside the gates it qualifies.

## Index row

x86_64 booted on three assumptions it never confirmed, where the other two architectures refuse a
machine that cannot run them. NX is checked in the 32-bit trampoline before `EFER.NXE` is written,
because that write is itself the hazard and there is no console yet to explain a `#GP`; `syscall`
and the invariant TSC are checked in `isa::init` before their consumers. The decision takes the
`CPUID` words as an argument, so the refusal path is exercised on the host in milliseconds although
no machine here lacks any of the three, and it was also watched refusing a real boot with
`NIFE_CPU=max,nx=off`. The invariant TSC turned out not to be gateable. QEMU's TCG declines to
advertise the bit under any invocation, so the rate `arch::x86_64::timer` measures once at boot is
a constant the machine has never promised. The gate that cannot fire yet says so on its own
line every boot.

## Where this leaves milestone 161

Milestone 161 (the x86_64 kernel port) is BUILT, so `isa.rs`'s `BUGS` was pointing at finished
work. It now points here.
