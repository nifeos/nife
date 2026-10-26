# Stubbing an instruction

An appendix to [notes/kernel-proofs.md](../kernel-proofs.md), which links here from its stub-list
item 2: how a harness reaches logic whose call graph ends at an `asm!`, and what that costs.

*Name provisional: notes are an interface and their names are calef's call.*

Added 2026-09-25 by the lane `lane/asm-behind-stubbable-wrappers`. It is the stub half of the
containment priced in
[milestone 589 (Kani can prove riscv64 from the hosts we already have)](../../design/roadmap/0589-kani-can-target-riscv64-from-the-hosts-we-have.md).

## The pattern

Each architecture keeps the `asm!` its logic calls in `kernel/src/arch/<isa>/instructions.rs`, one
function per instruction. riscv64's SBI `ecall` has its own file, `arch/riscv64/sbi.rs`. Every
wrapper is `#[inline(always)]` and keeps its call site's `options`, so the machine code does not
change. Logic calls the wrappers and holds no `asm!` of its own. A harness then replaces a wrapper
with a model:

```rust
#[kani::proof]
#[kani::stub(super::super::instructions::read_satp, read_satp_model)]
fn the_live_root_is_the_root_that_was_installed() { ... }
```

`script/verify` and `script/falsifications` pass `-Z stubbing` for the `kernel` row. A model
usually keeps the register in a `static` atomic, so the harness can set it and read back what the
code wrote. rustc cannot see `#[kani::stub]`, so each model carries `#[allow(dead_code)]`.

## The caveat: a stub is an assumption about the hardware

The model says what the silicon does. Somebody wrote it by reading a specification, and the proof
holds only if that reading is right. The `mod proofs` doc comment beside each harness lists what
its model assumes: WARL fields, which bits exist, what a barrier is taken to do. Never quote one of
these proofs without that list. The QEMU and board suites test the model; the harness tests only
the code against it. This is the parent note's item 6, "stub the boundary, do not pretend", with
the stub now written down in code.

## What it reached

Five harnesses whose call graphs used to end at an `asm!`:

| harness | file | proves |
|---|---|---|
| `the_live_root_is_the_root_that_was_installed` | `arch/riscv64/mmu.rs` | `current_root_pa` and `asid_of` invert `ttbr0_value` |
| `the_asid_probe_counts_the_implemented_bits_and_puts_satp_back` | same | every WARL pattern, zero bits included |
| `a_switch_sweeps_the_tlb_exactly_when_the_asid_cannot_be_trusted` | same | the skipped flush of milestone 58 (RISC-V TLB shootdown) |
| `every_cpacr_edit_changes_exactly_the_field_it_names` | `arch/aarch64/fp.rs` | `init`, `enable`, `disable`, `is_enabled` |
| `every_fp_edit_changes_exactly_the_bits_it_names` | `arch/x86_64/fp.rs` | the same four over `CR0` and `CR4` |

What stays `asm!` is listed in each `instructions.rs` header. It is fixed sequences with no Rust
logic inside (TLB maintenance, the aarch64 `at` probes, PSCI, GIC bring-up), tests of registers or
traps themselves, and the two icount calibration loops, whose instruction count is the point.

## EXAMPLES

The aarch64 harness, on an aarch64 host:

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm -Z stubbing \
      --harness arch::aarch64::fp::proofs
Complete - 1 successfully verified harnesses, 0 failures, 1 total.
```

The riscv64 three need the patched Kani that pull request #1287 builds (`script/verify-riscv64`).

## BUGS

- The riscv64 harnesses run nowhere in CI until pull request #1287 lands. On the aarch64 and x86_64
  hosts their file is not compiled, which is the parent note's item 3 and is silent.
- The x86_64 harness is `unfalsified`: this lane had no x86_64 host with Kani. Its doc comment
  names the three mutations to attest it with.
- aarch64's system registers are accessors in `arch/aarch64/instructions.rs` (`TTBR0_EL1`, the
  timer), and no harness stubs one yet. The timer re-arm is the obvious first.
- `sbi::call` has no harness yet. The logic around it (`flush_asid`'s remote half) also reads
  `cpu::id()` and the online-hart mask, which need models of their own.
