# aarch64

## What it is

An instruction set architecture (ISA) is the contract between software and silicon: which
instructions exist, what registers there are, how memory is addressed, how privilege
works. Compile for aarch64 and you get bytes an aarch64 CPU knows how to execute. Feed
those bytes to an x86 CPU and you get garbage.

aarch64 is the 64-bit mode of ARM's architecture, introduced with ARMv8-A in 2011.

Also called ARM64, arm64, and AArch64. Apple says arm64, Rust and GCC say aarch64, ARM's
own docs say AArch64. Same thing. The naming is just a mess.

Arm Ltd. designs the ISA and licenses it; Apple, Qualcomm, Broadcom, and Amazon build the
chips. Our MacBook is aarch64. So is every Raspberry Pi since the 3, and most of AWS.

## Why we chose it over x86_64

x86 grew organically from the Intel 8086 in 1978 and never threw anything away. It still
boots in a 16-bit mode from 1978, transitions to 32-bit, then to 64-bit. Instructions are
1 to 15 bytes long. Learning to boot it means learning a lot of Intel history.

aarch64 was designed clean, in one go, by people who had watched x86 accumulate scar
tissue for thirty years:

- Fixed 32-bit instruction width. Every instruction is exactly 4 bytes.
- 31 general-purpose 64-bit registers (`x0`–`x30`), plus a stack pointer. x86 has 16.
- Load/store architecture. Arithmetic happens only on registers. Touching memory
  requires an explicit load or store. x86 lets you add directly to a memory location;
  ARM makes you load, add, store. More instructions, far simpler model.

## Exception levels (the privilege model)

This is the thing an OS lives and dies on.

| Level | Who lives here |
|---|---|
| **EL0** | Userspace. Programs. Least privilege. |
| **EL1** | **The kernel. This is where nife lives.** |
| **EL2** | Hypervisor. Runs virtual machines. |
| **EL3** | Secure firmware. Below everything. |

Clean, numbered, orthogonal. Higher number, more power.

x86's equivalent is rings 0–3 (nobody uses 1 and 2), plus System Management Mode bolted
on the side, plus VMX root/non-root for virtualization, layered on over decades.

The EL0/EL1 boundary is the single most important line in the OS. Everything until
milestone 7 runs at EL1. Milestone 7 is the moment we construct a world at EL0, drop into
it, and catch it when it asks us for something. That transition *is* what an operating
system is.

But the kernel does not always *arrive* at EL1. QEMU's `virt` starts a payload at EL1; every real aarch64 bootloader starts one at EL2,
because that is where a hypervisor would go and firmware does not know you are not one. U-Boot
on the Jetson TX1 (milestone 127, the seL4 machine) does exactly this, with TF-A's tegra210 BL31
at EL3 below it providing PSCI.

So `boot.s` reads `CurrentEL` and drops itself to EL1 when it finds itself at EL2, and continues
unchanged when it is already there. Detection rather than a build-time switch, because a switch
would mean one binary for QEMU and a different one for the board, which is how you get a defect
nobody can reproduce on the machine they have.

The drop is `enter_el1` in `kernel/src/arch/aarch64/boot.s`, and its interesting half is not the
`eret` but the list of registers configured before it. Each one is there because its reset value
is UNKNOWN or because a bootloader may leave it however it liked, which means that at EL1 it is
either invisible or reads as something the kernel would then believe. Two are worth knowing
about away from the code:

- `CNTVOFF_EL2` is subtracted from the physical counter to produce the virtual one, and
  `arch/aarch64/timer.rs` uses the *virtual* timer deliberately (it is the one an EL1 kernel can
  always reach). Leaving `CNTVOFF_EL2` as found is leaving the system clock offset by an
  arbitrary 64-bit number.
- `VPIDR_EL2` and `VMPIDR_EL2` are what `MIDR_EL1` and `MPIDR_EL1` *return* once EL2 is
  implemented and you are reading them from EL1. Not copying the real values across would have
  the core-parking branch, `arch::isa`'s refusal check and the SMP bring-up all reading garbage,
  each failing somewhere far from the cause.

`arch::entry_el` records which level the entry was, and the boot banner prints both numbers,
because on a board's first boot they are the two different questions. See
[boot-protocol.md](boot-protocol.md) for how the entry level pairs with the PSCI conduit.

## Registers

**General purpose:** `x0`–`x30`, 64-bit. (`w0`–`w30` are the lower 32 bits of the same
registers.) `x30` is the link register: it holds the return address after a `bl` call.
The stack pointer `sp` is separate. The program counter `pc` is not directly writable.

System registers: a separate namespace only privileged code can touch. Read/written
with the special `MRS` and `MSR` instructions. A userspace program can never touch these.
A kernel does almost nothing else.

| Register | What it does | Milestone |
|---|---|---|
| `VBAR_EL1` | Address of the exception handler table | 2 |
| `TTBR0_EL1` / `TTBR1_EL1` | Physical address of the page tables | 4 |
| `SCTLR_EL1` | Master system control, including "is the MMU on?" | 4 |
| `MPIDR_EL1` | Which CPU core am I? (used to park cores 1-3 at boot) | 1 |
| `CurrentEL` | Which exception level am I running at? | 2 |
| `HCR_EL2`, `SPSR_EL2`, `ELR_EL2`, `CNTVOFF_EL2`, `VMPIDR_EL2` (and the rest of `enter_el1`'s list) | Only touched when firmware entered us at EL2, on the way down to EL1 | 127 |

`kernel/src/arch/aarch64/instructions.rs` holds one accessor for each of these the Rust side
touches, so a proof can stub it. The field constants sit beside their users (`SCTLR_M` and the
`TCR_*` fields in `mmu.rs`), each citing the Arm Architecture Reference Manual by the register's
heading, so a reader checks a bit position against its source.

## Control flow: `b`, `bl`, `ret`

`b label` is a plain unconditional jump. Sets `pc` to the target. A `goto`.

`bl label` (branch with link) does the same jump and stores the address of the
*next* instruction into `x30`. "Link" = "remember where to come back to."

`ret` is just: *jump to the address in `x30`*. That's the entire instruction.

So a function call on aarch64 is `bl foo` ... `ret`. No stack involved. Compare x86,
where `call` pushes the return address to the stack and `ret` pops it.

### The consequence

`x30` holds exactly one return address. If `foo` calls anything, that second `bl`
overwrites `x30` and `foo` has forgotten how to get home. Which is exactly why the
prologue in [the stack note](stack.md) exists:

```asm
stp  x29, x30, [sp, #-32]!   ; stash x30 before it gets clobbered
```

The stack is not where return addresses *go* on ARM. It's where they get parked when
a function needs `x30` for a call of its own.

Corollary: a leaf function (calls nothing) never has to save `x30`, so it skips the
prologue entirely and just `ret`s.

### The four variants

| Instruction | Jumps to | Saves return address? |
|---|---|---|
| `b label` | a fixed label | no |
| `bl label` | a fixed label | **yes**, into `x30` |
| `br xN` | the address in register `xN` | no |
| `blr xN` | the address in register `xN` | **yes**, into `x30` |

The register forms exist because `b`/`bl` encode the target as a signed offset *inside the
instruction*, and instructions are only 32 bits wide. 26 bits are available for the
offset, scaled by 4, giving a reach of ±128 MiB. To call farther, load the full 64-bit
address into a register and use `blr`. Function pointers and virtual dispatch use `blr`
for the same reason: the target isn't known at assembly time.

### `cbnz` / `cbz`

"Compare and branch if (not) zero." aarch64 folds the compare and the branch into one
instruction for the common test-against-zero case. We use `cbnz x0, park` in `boot.s` to
park every core but the boot core in a spin loop at reset. That was the whole SMP story until
milestone 41; it is now just the reset path, because a secondary is started deliberately by a PSCI
`CPU_ON` that lands it at `secondary_boot` rather than at `_start`.

## Decoding `aarch64-unknown-none-softfloat`

That's our Rust target triple, and every piece is meaningful:

- **`aarch64`**: the ISA.
- **`unknown`**: the vendor. Nobody in particular.
- **`none`**: the operating system: there isn't one. This is what "bare metal" means,
  spelled out in the target name. No syscalls, no libc, no `std`. It's why the kernel is
  `#![no_std]`.
- **`softfloat`**: do not use the hardware floating-point / SIMD registers.

That last one is a real kernel design decision, not a technicality. aarch64 has 32
FP/SIMD registers (`v0`–`v31`, 128 bits each). If kernel code used them, every interrupt
and every context switch would have to save and restore all of them, which is a lot of
bytes to shuffle on every timer tick. So kernels traditionally don't use floating point at
all, and the `softfloat` target enforces that at compile time.

---

*Add to this file as new aarch64 concepts come up.*
