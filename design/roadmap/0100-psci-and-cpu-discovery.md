---
status: BUILT
raised: 2026-08-04
built: 2026-08-04
---
# 100. Read the machine's PSCI and its CPU list, not QEMU `virt`'s

Built 2026-08-04 (PR #107). Raised 2026-08-04 from two limitations recorded in the code the same day,
in `psci_cpu_on` in `kernel/src/arch/aarch64/mod.rs` and in `bring_up_secondaries` in
`kernel/src/smp.rs`. Both were written by a lane that
found them while working on something else, and both name the other.

The finding. SMP bring-up is the one subsystem that assumes a board instead of asking it. Three
facts are compiled in:

| Fact | Where it is hardcoded | Where the machine states it |
|---|---|---|
| The PSCI conduit is `hvc` | `psci_cpu_on`'s `hvc #0` | `/psci`'s `method` property |
| The function id is `0xC400_0003` | `const PSCI_CPU_ON` | `/psci`'s `compatible` (the id set moved between PSCI 0.1 and 0.2) |
| The core list is `0..MAX_CPUS`, and `MAX_CPUS` is 4 | `MAXIMUM_CPUS` in `kernel/src/cpu.rs`, iterated in `bring_up_secondaries` | `/cpus` |

Everywhere else the machine describes itself. `crates/device_tree_blob` already parses the tree, and
milestone 60 (ISA discovery) reads the ISA out of it on both architectures. `crates/pci` holds its
own hardcodes against the tree in a host test, and the console's hardcoded UART base is checked against
`dtb`'s answer in `crates/device_tree_blob/tests/qemu_aarch64_virt.rs`. **The parser is not what is missing.**
`Fdt::node_prop` and `Fdt::node_reg` already answer both questions; what is missing is the call.

Two failure shapes, and only one of them is certain. The core-list half is a guaranteed silent
no-op: a board with more than four cores has cores 4 and up never started, with no error to return
because nothing asks about them. The conduit half is board-specific and untested. `smp.rs` does have
a degradation path, and it is written for one case, a core that is absent (`PSCI {ret}; not
present?`). A machine whose firmware answers on `smc` is under no obligation to produce a PSCI error
code from an `hvc` it never agreed to serve. Calling both "silent" is close enough for a warning and
too loose for a plan, so the plan should treat them separately.

A third site of the same class, which corrects the code comment that raised this.
`TIMEBASE_HZ` in `kernel/src/arch/riscv64/timer.rs` hardcodes it at 10 MHz with the comment "hardcoded
until the DTB parse lands", and the DTB parse landed. aarch64's twin computes the same interval from
`CNTFRQ_EL0` and asserts the value is nonzero (`initialize` in `kernel/src/arch/aarch64/timer.rs`), so the two ISAs
currently disagree about whether the machine gets to say how fast its clock runs. That is a parity
gap under rule 5, it is in this milestone's family, and it means the `psci_cpu_on` comment's claim to
be "the one place the kernel assumes a board" is one site short. Fix the claim here rather than
carry it forward.

What it costs, and who needs it. Milestone 24 (a second aarch64 board, Virtualization.framework)
and milestone 88 (nife on rented silicon) both boot a machine that is not QEMU `virt`. A
bring-up that reports success while starting nothing is the worst way to learn that, because the
symptom arrives later as a scheduler that never balances.

## Scope note

**Reading the device tree is necessary for milestone 24 and not sufficient for milestone 88.** An
ACPI machine has no device tree at all and states PSCI in the FADT, which is why milestone 88's row
already budgets "a UEFI boot path and an ACPI front door". This milestone gets the DTB path right
and leaves a clean seam for the second source; it does not build ACPI.

**Do not raise `MAX_CPUS` here.** It sizes static per-CPU arrays (the secondary stacks, `TICKS`,
`MISSED_TICKS`, `RAN_ON`), and milestone 90 is moving the stacks over a guard page. Reading `/cpus`
means starting the cores the machine reports up to the compiled ceiling and saying so when it
reports more; changing the ceiling is a separate decision with a memory cost attached.

## Follow-on

- **Milestone 88.** The ACPI front door. An ACPI machine has no device tree at all and states PSCI
  in the FADT. So this milestone got the DTB path right and left a seam for the second source.
  Milestone 88 (nife on rented silicon) already budgets the UEFI boot path and the ACPI reader.
- **Recorded.** `kernel/src/smp.rs` carries the compiled ceiling where the code does the seating.
  `MAX_CPUS` sizes the secondary stacks and the per-CPU arrays, so raising it costs memory and is a
  decision of its own. What shipped instead is starting the cores the machine reports up to the
  ceiling and printing how many were left unseated.
- **Recorded.** `kernel/src/arch/aarch64/mod.rs` records beside `psci_cpu_on` that the `smc` conduit
  path has never executed. The reading is exercised against a real QEMU dump that states `smc`, but
  no machine here runs the kernel below an EL2, so the call itself is untested.
- **Recorded.** `kernel/src/arch/riscv64/timer.rs` names the per-hart limitation at
  `init_frequency`: the RISC-V binding permits a per-hart `timebase-frequency` and this reads the
  boot hart's once for the whole machine, so a board whose harts genuinely differ is misread. That
  is also where the parity gap this block found, a hardcoded 10 MHz against aarch64's `CNTFRQ_EL0`
  read, was closed.

## Index row

The conduit (`hvc`), the function id, and the core list (`0..MAX_CPUS`, and `MAX_CPUS` is 4) are
compiled in, where `/psci` and `/cpus` state all three and `crates/device_tree_blob` can already read them. The
core-list half is a guaranteed silent no-op on a bigger board; the conduit half's failure is
board-specific and untested. Milestones 24 and 88 both boot a machine that is not `virt`
