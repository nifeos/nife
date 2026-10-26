---
status: AMENDED
raised: 2026-07-13
decided: 2026-07-13
ratified_by: calef
---

# 3. Use the crate ecosystem

(§46 (thin primitives or whole subsystems; we write everything in between) replaces this decision
as it applies to register access.)

Ruled 2026-10-26 (UTC): register access is written in the tree, not taken from a crate. aarch64's
system registers are one accessor each in `kernel/src/arch/aarch64/instructions.rs`, and its MMIO
registers are named offsets and bit constants in `kernel/src/drivers/`, the way riscv64 and x86_64
already spell theirs. Register access is on the verification path, and under §46's rule 1 we write
what a proof must reach: a Kani harness stubs an accessor in `instructions.rs`, and it cannot stub
another crate's trait method. Milestone 707 (aarch64 registers spelled the way riscv64 and x86_64
spell them) holds the work.

A wrong bit is the cost this accepts. Each field constant cites the Arm document that defines it,
by the register's own heading, so a reader checks a bit against its source rather than against a
crate's spelling of it.
