# The x86_64 CPU state the kernel sets up

*Name: provisional, like every stem in this directory.*

This page holds the reasons behind the per-core state `kernel/src/arch/x86_64/` establishes: the
control-register bits, the port I/O gate, the per-CPU and thread pointers, the FP and SSE unit,
and the `CPUID` gates. Each doc comment there states its constraint and links here for the
argument.

## The control registers each core sets

`cpu_init::init` runs on every core, the boot core and each secondary, because every bit it sets
is per-CPU state.

### Write-combining for the framebuffer

`IA32_PAT` is programmed so that `PWT` alone selects write-combining, the type
`paging::Flags::write_combining()` encodes and the framebuffer is mapped with. Mapped like a
register (`PCD | PWT`, strong uncacheable), every four-byte store is its own bus transaction and
the MTRRs cannot relax it. xenon's 1920x1080 aperture is 8 MB, so a full redraw is two million
transactions, and the boot console's scrolling swept visibly down the screen. Write-combining
gathers a run of stores into one burst. A PAT type of WC also wins over whatever the firmware's
MTRRs say about the range, so it does not depend on firmware.

The SDM asks for a cache flush around a PAT change, because a mapping that already uses the
changed entry would hold lines of the old type. No mapping uses entry 1 when this runs. `boot.s`'s
coarse map sets neither `PWT` nor `PCD`, and every fine-map device page is `PCD | PWT` (entry 3).
On the boot core it runs before `mmu::init` builds the fine map, and on a secondary before
`mmu::init_secondary` adopts it. Every `x86_64` processor this tree knows of offers the PAT (from
memory, not a specification citation). Without it, `PWT` alone is write-through, which is merely
slow for a screen nothing reads back.

### SMEP: ring 0 does not execute ring-3 pages

`crates/paging`'s x86 decoder reports a user page as not kernel-executable, and every confinement
test that reads a mapping through `Flags::is_kernel_executable` believes it. The hardware agrees
only with `CR4.SMEP` set. x86 has one execute permission, `XD`, and it applies at every ring, so a
user code page with `XD` clear is executable at ring 0 too. RISC-V refuses a supervisor fetch from a
`U` page unconditionally, and aarch64 has `PXN`. Milestone 313 (userspace confinement, read adversarially) found that the decoder's
claim, and the sentence in [confinement-claims.md](../confinement-claims.md) that the hardware makes
a user page non-executable in supervisor mode, were true only with this bit on.

SMEP costs nothing on any path. The fetch unit checks it against the leaf's `U/S` bit, with no
per-access software toggle. That is what separates it from SMAP, which needs `stac` and `clac`
around every kernel access to user memory and stays off (`mmu::permit_kernel_access_to_user_pages`
has the BUGS). SMEP forbids nothing this kernel does: every kernel page is mapped `U/S` clear, and
user code is only entered through `iretq` or `sysret` at ring 3.

The feature bit is `CPUID.(EAX=7,ECX=0):EBX[7]`: Ivy Bridge and every Intel part since, AMD from
Excavator, and QEMU's `-cpu max`, HVF and the `q35` defaults. A machine without it gets a boot line
saying ring 0 can execute ring-3 pages there, because the decoder's answer is wrong on that machine.

### RDPMC stays closed to ring 3, and RDTSC stays open

`CR4.PCE` (bit 8) makes `RDPMC` legal at every ring. Fixed counter 2 runs at the TSC rate, so an
open `PCE` is a second path to a cycle-rate instrument the kernel never granted. `boot.s` writes
only `PAE` into `CR4`, so the bit is whatever firmware left. It is read back, not assumed
(milestone 228 (the cycle counters are closed by assumption)). QEMU leaves it clear: PVH hands over
`CR4 = 0x20` and OVMF `0x668`, five bits firmware set that this kernel never wrote. Real firmware is
unknown.

`CR4.TSD` (bit 2, gating `RDTSC`) stays open on purpose. `user_mode_runtime::now()` is `rdtsc` on
this architecture, and closing it would take `Instant`, `thread::sleep` and the random seed away.
[user-mode-runtime.md](user-mode-runtime.md) has the rest.

## Port I/O is a confinement question

aarch64 and RISC-V reach every device through memory, so a device capability is a mapping. x86 also
has a separate 16-bit port space, reached only by `in` and `out`, with no page tables in front of
it. x86 gates ring 3's access to it two ways. `RFLAGS.IOPL` is all or nothing per privilege level.
The TSS I/O permission bitmap has one bit per port, which is the granularity a capability needs.
This kernel uses the bitmap, one per core inside that core's TSS, and installs a thread's grant
when it is switched in (`segments.rs`, §121 (a device capability when the device has no page: x86
port I/O)).

## The boot CPU's id, and the per-CPU pointer

`BOOT_CPU_ID` is stamped once by `boot.s` from `CPUID` leaf 1's initial APIC id, before
`kernel_main`. It is recorded rather than recomputed because `CPUID` answers "which core am I". A
caller that asked it after migrating to a secondary would get the secondary's id and call it the
boot core's. riscv64's `BOOT_HARTID` is the same shape for a different reason: OpenSBI hands the
hart id over in `a0`, and it is lost unless `boot.s` keeps it. aarch64 needs no record, because its
boot core is architecturally 0.

The number has to match the roster, because `smp::seat_cpus_from_acpi` seats each core at the slot
its local APIC id names. That is the same logical-id-equals-hardware-id rule the other two
architectures keep. Nothing guarantees the boot CPU's APIC id is 0, although on QEMU it usually is.

`percpu()` is one `gs`-relative load. `rdmsr IA32_GS_BASE` answers the same question, but it is
microcoded and serializing, and inlined it was the largest single line in the x86_64 IPC fast-path
closures on 2026-10-04 (UTC). The load runs on every lock, unlock and current-thread read.

## The thread pointer

The `FS` base is each thread's thread pointer. `CR4.FSGSBASE` stays off (§269 (how threads share a
process) fork 4), so `wrfsbase` faults and there is no `arch_prctl`. §269 fork 4 keeps it off.

The write is lazy. `wrmsr` to `IA32_FS_BASE` is serializing and far dearer than a compare, and most
switches are between threads that both hold zero, or between a thread and a kernel thread. Every
switch in installs the incoming value, and so does a thread that sets its own, and nothing else
writes the register. So the outgoing thread's value is what the register holds, and comparing it
with the incoming one decides whether to write.

## The FP and SSE unit

`fp.rs` holds a thread's FP and SSE register file and the control-register bits that decide who
may touch it. The policy is `kernel/src/fp.rs`'s. Four bits matter.

- `CR4.OSFXSR` says the operating system knows about `fxsave`, `fxrstor` and the `xmm` registers.
  While it is clear, every SSE instruction raises `#UD`.
- `CR4.OSXMMEXCPT` makes an unmasked SIMD floating-point exception arrive as `#XM` (vector 19)
  rather than `#UD`. It is set together with `OSFXSR`. A kernel that sets one without the other
  reports arithmetic as a bad opcode.
- `CR0.EM` and `CR0.MP` are the 80387 emulation pair. `EM` must be clear, or SSE raises `#UD`
  instead of `#NM`. `MP` is set so that `wait` and `fwait` respect `TS` like everything else.
- `CR0.TS` is the one that matters. While it is set, the next FP or SSE instruction raises `#NM`
  (vector 7, "device not available"). It is the enable bit, written per thread.

`TS` is also the mechanism behind `LazyFP`, CVE-2018-3665. The vulnerability was not `TS` itself.
It was using `TS` to defer the restore, so that one thread ran with another thread's `xmm`
registers still in the file and the `#NM` as the only barrier. Speculative execution reads them
before the fault is delivered. Here `TS` answers "has this thread ever wanted the unit", which is a
performance question. The registers always hold either the running thread's values or
`FpState::INITIAL`, so there is nothing behind the trap to speculate at. That is why
`fp::disable`'s caller owes the scrub. `clts` exists for exactly the lazy scheme, which this kernel
does not use.

`FpState::INITIAL` is not all zeros. The x87 control word is `0x037f`, as `finit` installs it, and
`MXCSR` is `0x1f80`. A zeroed `MXCSR` would unmask every SIMD exception, so the first underflow in a
scrubbed thread would be a `#XM` nobody asked for.

The save is `fxsave`, the 512-byte legacy area: x87 state, `MXCSR` and `xmm0` to `xmm15`. A thread
that used AVX would have `ymm` upper halves this does not move, and AVX-512's `zmm` likewise. That
is safe only because `CR4.OSXSAVE` is clear, so `xgetbv` and every VEX-encoded instruction raise
`#UD` and no thread can reach that state. Enabling `XCR0` needs an `xsave` path in the same change,
and nothing enforces that coupling. `xsave` would also bring its init optimization: `XSTATE_BV`
records which components are in use, so a thread that touched one `xmm` register would save far
less than 512 bytes. That is worth doing with the `XCR0` work, not before it.

The Kani proof `every_fp_edit_changes_exactly_the_bits_it_names` checks the `CR0` and `CR4` edits
from every starting value. `CR0` carries `PG` and `WP` beside the four FP bits, and `CR4` carries
`PAE`, `SMEP` and the paging controls beside the two SSE enables. On QEMU most wrong masks land on a
bit that happens to be clear already, which is why the proof exists. `fp::init` is the only thing
that sets `TS` before the first thread runs.

## CPUID and the three boot gates

`isa.rs` answers "what CPU is this, and can it run us" with `CPUID`, the third implementation of the
contract aarch64 answers from `MIDR_EL1` and RISC-V from the device tree and `mvendorid`. The
decoding is in `machine_discovery::x86_64`, host-tested. What can only happen on the machine stays
in the kernel: the `cpuid` instructions, the record and the refusal.

Three features are gates (milestone 524 (the three `x86_64` boot gates)). A check belongs at the
last moment before the thing it protects, unless that moment is before there is any way to say what
went wrong.

| Feature | Checked | Why there |
|---|---|---|
| NX | `boot.s`, before `EFER.NXE` is written | that write is itself the hazard: setting a reserved `EFER` bit on a part without NX is `#GP` with no IDT, which QEMU shows as a silent reset |
| `syscall` | `isa::init`, just before `arch::init` programs the four MSRs | the console is up, so the refusal can be read |
| invariant TSC | `isa::init`, before `timer::init_frequency` measures a rate | the same, and under a hypervisor it warns rather than refuses |

`isa::init` re-checks NX because the assembly gate can only halt, and this one can say what is
wrong.

The invariant TSC is a different kind of check. NX and `syscall` announce their own absence the
moment the kernel uses them. The invariant TSC does not. `timer::init_frequency` measures the TSC's
rate once, against the PIT. On a part without `CPUID.80000007H:EDX[8]` the rate moves with the
core's frequency and idle state, so there is no single rate for that measurement to find. The
error appears later, as time itself being wrong, and measuring harder cannot catch a wrong
assumption.

On bare metal it refuses, and under a hypervisor it warns (`Gate::RefuseUnlessHypervisor`). A
guest's clock is the hypervisor's to keep. QEMU's TCG reports the bit as zero and will not be
persuaded (`-cpu max,invtsc=on` clears it; the bit is KVM-only), so a refusal there would refuse
every boot the x86_64 suite runs. The warning prints on its own line each boot and names the
hypervisor bit as the reason it is not a refusal. It is not a token in the feature list, because a
missing word in a list of present ones is not something a reader notices.

`isa::tsc_crystal_hz` reads leaf `0x15`, the TSC to crystal ratio, which some parts populate.
Under the project's QEMU invocation (`-cpu max`, TCG) the maximum leaf is `0xd`, so it returns
`None` and every boot's rate comes from PIT calibration. When the leaf exists but `ECX` is zero,
Linux's `native_calibrate_tsc` falls back on a model-to-crystal table, which this port does not
carry. Whichever path answers, the rate is a constant only because of the invariant-TSC gate.

`RDSEED` is reported and never gated. It is an entropy source, and a machine without it boots
with a different backend (`kernel/src/user/entropy_service.rs`). `isa::draw_random_seed` retries 100
times, the high end of Intel's "between 1 and 100" for an asynchronous caller, because it is a
one-shot boot-tour probe rather than a service under load.

The record is `None` until `isa::init` writes it, so a read before discovery panics with this
file's name. A zeroed record would decode as a machine that should have been refused, which is the
worst thing a missed initialization could look like.

`isa::cpuid` wraps `core::arch::x86_64::__cpuid` in an `unsafe` block under
`allow(unused_unsafe)`. The function is safe on the toolchain this tree pins, and was an
`unsafe fn` until upstream changed it. Kani bundles its own rustc, about ten months behind
(`kani-0.67.0` pins `nightly-2025-11-21`, rustc 1.93.0). There the bare calls were four `E0133`
errors, and all of `arch/x86_64/` was out of the model checker's reach (milestone 304 (`cargo kani
-p kernel` only ever compiled one architecture)). The allow is the exception this file owes a
reader, and it goes once Kani's rustc is past 2026-02.

### BUGS

- Under a hypervisor the invariant-TSC gate warns and does not refuse, so a guest whose hypervisor
  promises no constant TSC rate boots and is trusted for timekeeping anyway. The gate believes the
  hypervisor bit, so a hypervisor that hides itself is treated as silicon and refused.
- The gates are checked on the boot CPU only. Secondaries replay the `EFER` write and the
  `syscall` MSRs without re-reading `CPUID`. On a hybrid part (Intel's P-cores and E-cores differ in
  leaf 7) a per-core check would earn its cost. `machine_discovery::riscv64` already solves the
  RISC-V twin by intersecting over every hart.

## The interrupt stack table

Milestone 915 (NMI, double fault and machine check run on their own stacks) gives each of those
three vectors (2, 8 and 18) a stack per core, named by the TSS's Interrupt Stack Table. None of the three can trust the `rsp` it interrupted. An
NMI ignores `cli`, arrives on every TLB shootdown, and can land in the two instructions
`x86_syscall_entry` runs on the user's `rsp`. A double fault usually means a stack could not be
pushed to. A machine check promises nothing. A gate with a nonzero IST field makes the CPU load
`rsp` from `TSS.ist[n - 1]` before it pushes anything, from either ring.

Each stack is 16 KiB over a 4 KiB guard page that the fine map leaves unmapped, and the fine map's
`verify` refuses a mapped guard. `TSS.ist` points `TOP_RESERVE` bytes below the top, and the top
word holds the core's per-CPU pointer, so `isr_paranoid` finds the kernel's GS base at a fixed
offset from its own `rsp` without trusting `gs` or the saved `CS`. `ist::install` runs once per core
from `segments::init`. On the boot core it runs on the coarse boot map, which covers the region
writable, and on a secondary on the fine map.

Each vector has its own stack, so one interrupting another corrupts nothing. A double fault inside a
double fault is a triple fault, and a machine check inside a machine check is a shutdown; neither
handler returns anyway. A second NMI is held off until an `iretq`, and any `iretq` counts, including
one from an exception taken inside the NMI handler. So the NMI handler takes no exception that
returns. It serves the shootdown (an `invlpg` or a `CR3` reload, and atomics) and touches nothing
else. Linux's nested-NMI frame copy exists because its NMI handlers can fault, and this kernel's
cannot, so it is not built. `exceptions::x86_paranoid_dispatch` checks the rule on every NMI and
panics if the interrupted `rsp` is on an NMI stack. [ring-3.md](ring-3.md) has the windows this
closes.

### BUGS

- `script/stack-depth-check` does not measure these stacks. They are the size of the per-CPU
  interrupt stack, which carries the same panic path and is measured.
- They cost 3 x 20 KiB x `MAX_CPUS` (480 KiB) of address space whether or not the cores exist.
