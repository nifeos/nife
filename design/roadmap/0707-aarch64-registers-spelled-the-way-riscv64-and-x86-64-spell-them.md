---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: aarch64-registers-spelled-the-way-riscv64-and-x86-64-spell-them
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: aarch64 silicon
specific_machine: none
needs_person: yes
---
# 707. aarch64 registers spelled the way riscv64 and x86_64 spell them

*Title and slug are drafts.*
Raised 2026-10-03 (UTC) by the maintainer at calef's request, while PR #1477 (the kernel declares
each dependency where it is used) moved `tock-registers` into the aarch64 target table. calef's call
was to file this, not launch it. Title and slug are drafts.

## Today

Measured on `d6a902a9b` with `grep` under `kernel/src`.

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `asm!` sites in `arch/<isa>/` | 61, in 11 files | 49, in 9 files | 43, in 12 files |
| Control-register access | `aarch64-cpu` (20 registers named) plus `asm!` | 80 lines naming a `csr*` instruction | 53 `rdmsr`/`wrmsr`/`cr*` sites, behind `read_cr0`-style functions in `instructions.rs` |
| MMIO register blocks | `tock-registers` (`pl011`, `gic`, `gicv3`) | named offsets and volatile access | named offsets and volatile access |

So aarch64 alone reaches registers two ways, and alone carries two third-party crates for it.
DECISIONS §3 (use the crate ecosystem) chose both on 2026-07-13, before the in-tree pattern existed.
`kernel/Cargo.toml`'s comment on the aarch64 table calls the asymmetry history rather than a
decision. It has a visible cost: an unconditional `aarch64-cpu` pins Kani, and so CI, to an aarch64 runner,
which is why it is target-gated (milestone 193 (put `kernel/src` within reach of the prover,
because today the proofs cannot see it)).

**A second cost is current.** `arch/aarch64/instructions.rs` exists so a proof can stub each
instruction. Its own header says registers reached through `aarch64-cpu` are not moved there, so
"a harness that reaches one still fails".

## Proposal

Spell aarch64's system registers and MMIO the in-tree way, and remove both `aarch64-cpu` and
`tock-registers`. That is DECISIONS §46 (thin primitives or whole subsystems; we write everything in
between) applied as written: register access is on the verification path, and we cannot restructure
another crate's trait methods so a prover can stub them.

What moves, from `grep -rn 'aarch64_cpu\|tock_registers' kernel/src`:

| File | Uses today | Change |
|---|---|---|
| `arch/aarch64/timer.rs` | 5 registers, 14 typed accesses | accessor functions in `instructions.rs` |
| `arch/aarch64/mmu.rs` | 5 registers (`MAIR`, `SCTLR`, `TCR`, `TTBR0`, `TTBR1`), `barrier`, 18 accesses | same, plus field constants for `SCTLR` and `TCR` |
| `arch/aarch64/exceptions.rs` | `ESR`, `FAR`, `VBAR`, `barrier`, 6 accesses | same |
| `arch/aarch64/pmu.rs`, `isa.rs`, `mod.rs` | 6 more registers, `wfi`, `dsb`, 13 accesses | same |
| `lib.rs` | `CurrentEL`, read in three `cfg(target_arch = "aarch64")` blocks | an `arch::` function, which also takes a system register out of `lib.rs` (codebase rule 1) |
| `drivers/pl011.rs`, `gic.rs`, `gicv3.rs` | 189 lines of `register_structs!`/`register_bitfields!`, 51 accesses | named offsets and bit constants, the way `ns16550.rs` already does it |
| `kernel/Cargo.toml` | two dependencies | both removed |

The estimate is 11 source files and roughly 600 to 900 changed lines: about 40 small accessor
functions, a few dozen field constants, and the three drivers rewritten. That is an estimate from
the counts above, not a diff.

## Refused: typed wrappers for riscv64 and x86_64 instead

Converging the other way would add third-party crates (a `riscv` crate, an `x86_64` crate) to the
shipping graph, against §46, and on the verification path, which is the case §46 says to write.
The argument for typed wrappers is that they catch a wrong bit at compile time. No such bug is
recorded for riscv64 or x86_64. The search was `git log --grep` over the 289 commits that touch
`arch/riscv64` or `arch/x86_64`, for "wrong bit", "bit position", "wrong field" and "misencoded",
plus a grep of `notes/` and `design/roadmap/`. It found only mutation-test descriptions and a VT-d
commit saying its bit positions were checked against QEMU's source. That search is shallow: it finds
bugs someone wrote down in those words, not bugs fixed silently.

## Costs and risks

- Churn in 11 working files whose behavior is proven under QEMU and HVF (nothing has run on argon,
  the aarch64 Jetson board). A wrong
  `SCTLR` or `TCR` bit is a boot that never prints, so the failure is loud but slow to diagnose.
- It reverses DECISIONS §3, so it needs a written decision before it is built; that is the
  `unwritten` dependency above.
- Proof plan: `script/test` on all three architectures; `script/bench` floors unchanged or each
  difference explained; `script/fastpath-footprint` unchanged; a boot on argon. If argon is not
  available, the boot is recorded as owed in the block's follow-on, which is why `needs_person` is
  `yes`.

## Priority

Off the customer path and behind the swish-check work; calef's call on 2026-10-03 was to file it,
not launch it. At equal cost the elegance case holds: one pattern on three architectures, two fewer
dependencies, and every register access stubbable by a proof. The only argument against is timing.

## Blocked by

A decision that replaces DECISIONS §3. `tock-registers` is already in the aarch64 target table
(PR #1477).

## Index row

aarch64 alone reaches its control and MMIO registers through `aarch64-cpu` and `tock-registers`, while riscv64 and x86_64 spell theirs in the tree. Proposed: spell aarch64's the same way, filed rather than launched at calef's request.
