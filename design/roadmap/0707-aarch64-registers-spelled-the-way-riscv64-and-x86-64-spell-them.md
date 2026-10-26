---
status: BUILT
raised: 2026-10-03
built: 2026-10-26
promoted_from: aarch64-registers-spelled-the-way-riscv64-and-x86-64-spell-them
milestone_dependencies: none
decision_dependencies: 3, 46
machine_requirements: aarch64 silicon
specific_machine: none
needs_person: yes
---
# 707. aarch64 registers spelled the way riscv64 and x86_64 spell them

*Title and slug are drafts.*
Ruled 2026-10-26 (UTC): built in full. aarch64 spells its system registers and MMIO registers
the in-tree way, and the kernel links neither `aarch64-cpu` nor `tock-registers`. The ruling
replaces §3 (use the crate ecosystem) for register access under §46 (thin primitives or whole
subsystems; we write everything in between) rule 1, and both sections record it. A boot on argon,
the aarch64 Jetson board, remains (Follow-on).

## What was built

| Where | What |
|---|---|
| `arch/aarch64/instructions.rs` | one accessor per system register the arch code reads or writes whole: the generic timer, `MAIR`, `SCTLR`, `TCR`, `TTBR0`, `TTBR1`, `ESR`, `FAR`, `VBAR`, the ID registers, `TPIDR_EL0`, `TPIDR_EL1` and `CurrentEL`, plus `wfi`, `dsb sy` and `isb` |
| `arch/aarch64/mmu.rs` | field constants for `SCTLR_EL1.M`, every `TCR_EL1` field the kernel writes, and `TTBRn_EL1.BADDR` |
| `timer.rs`, `exceptions.rs`, `pmu.rs`, `isa.rs`, `percpu.rs`, `thread_pointer.rs`, `primitives.rs`, `semihosting.rs` | call the accessors |
| `arch::current_el()` | `CurrentEL` behind an `arch::` function, beside `arch::entry_el()`; `boot.rs`, `machine_description.rs` and `tests.rs` call it |
| `drivers/pl011.rs`, `gic.rs`, `gicv3.rs` | named offsets and bit constants with volatile access, as `ns16550.rs` and `plic.rs` do |
| `kernel/Cargo.toml` | no aarch64 dependency table |

Each accessor carries `options(nomem, nostack)`, so a write whose order against memory matters
sits between `dsb_sy` or `isb` at its call site. A write that changes reachable memory or the trap
target is an `unsafe fn`, as riscv64's `write_satp` is. Every bit constant cites the Arm document
that defines it by the register's heading: Arm DDI 0487 for system registers, DDI 0183 for the
PL011, IHI 0048B for the GICv2 and IHI 0069 for the GICv3. Each also agrees with Linux v6.16's
headers (`arch/arm64/tools/sysreg`, `pgtable-hwdef.h`, `arm_arch_timer.h`, `amba/serial.h`,
`arm-gic.h`, `arm-gic-v3.h`).

New names, all provisional: `arch::current_el`, the `instructions::read_*`/`write_*` accessors,
`instructions::{wfi, dsb_sy, isb}`, `gicv3`'s `Frame` and `Width`, and the register constants.

## Evidence

Measured 2026-10-26 (UTC) on this branch merged with `2a3cc0dbd`, against `2a3cc0dbd` itself.

- `cargo tree -e normal -p kernel`: aarch64 none, riscv64 none, x86_64 `uuid` 1.28.0.
- `script/test` passed on all three architectures, both images each.
- `script/fastpath-footprint`: identical to the byte on all three. aarch64 `ipc_send_receive`
  4916 B, `ipc_call_reply` 6316 B, `syscall_entry` 1684 B, `.text.hot` 11992 B, on both trees.
- `script/bench --check` passed on all three. riscv64 and x86_64 compile no changed code. On
  aarch64 every row that crosses a register access fell 1 to 3 percent and `coremark` did not move:

| benchmark | before | after |
|---|---|---|
| `yield_switch` | 547 | 534 |
| `ipc_rtt` | 1023 | 1005 |
| `null_syscall` | 18 | 17 |
| `ctx_switch` | 592 | 576 |
| `ipc_rtt_el0` | 2085 | 2039 |
| `coremark` | 81660 | 81660 |

  The bench image is a `dev` build. There the crates' trait methods (`Readable::get`, a field's
  `read`) stayed calls, and the accessors here are `#[inline(always)]`. The release footprint
  above shows optimized code did not change, so the gain belongs to the debug instrument only and
  the baseline is left as it is.
- `script/lint` passed, including the unsafe density ceiling outside `arch/`, which now counts the
  drivers' volatile accesses.

## Refused: typed wrappers for riscv64 and x86_64 instead

Converging the other way would add third-party crates (a `riscv` crate, an `x86_64` crate) to the
shipping graph, against §46, and on the verification path, which is the case §46 says to write.
The argument for typed wrappers is that they catch a wrong bit at compile time. No such bug is
recorded for riscv64 or x86_64. The search was `git log --grep` over the 289 commits that touch
`arch/riscv64` or `arch/x86_64`, for "wrong bit", "bit position", "wrong field" and "misencoded",
plus a grep of `notes/` and `design/roadmap/`. It found only mutation-test descriptions and a VT-d
commit saying its bit positions were checked against QEMU's source. That search is shallow: it finds
bugs someone wrote down in those words, not bugs fixed silently.

## BUGS

- No Kani harness stubs an aarch64 register accessor yet. They are stubbable now; the timer re-arm
  is the obvious first harness, and it still needs its arithmetic lifted out of the accesses
  (notes/kernel-proofs.md, "Why these two and not the timer").
- Nothing has booted this on silicon. QEMU and the suite exercise every accessor and driver path,
  and a wrong `SCTLR` or `TCR` bit on a real part is a boot that never prints.

## Follow-on

- **Recorded.** This block owes an argon boot, which needs a person at the bench. One `script/test` boot on argon, or a
  console boot to the shell, closes it; this block keeps `needs_person: yes` until then.
- **Recorded.** `design/tenets/codebase-rules.md` still names `aarch64-cpu` and `tock-registers` as examples of
  thin primitives the kernel takes; design text outside `design/decisions/` is not this lane's to
  edit.

## Index row

aarch64 reaches its system registers through accessors in `arch/aarch64/instructions.rs` and its device registers through named offsets, as riscv64 and x86_64 do, and the kernel links no register crate. Built; an argon boot is owed.
