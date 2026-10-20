---
status: PROPOSED
raised: 2026-10-20
milestone_dependencies: 890
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# `serial_driver` ships with nothing to start it

*(Title and slug provisional.)*

Reuse: not applicable; this removes or rewires an existing program and writes none.

Raised 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`. `components/src/serial_driver.rs`
is the interrupt-driven NS16550 input driver from milestone 20 (a portable HAL, proven on a second architecture), and `packages/drivers.package.toml`
ships it. Its only starter was riscv64's tour, whose UART-driver step the self-test replaced
(milestone 890 (riscv64 and `x86_64` join the shared boot path)). The hand-over's input driver now
serves riscv64's UART, so nothing in the kernel or the progenitor starts this program.

## The choice

- **Retire it**: remove the binary from `components/Cargo.toml` and the drivers package. A
  package's program list is published through basalt, so this is a change two repositories see.
- **Give it a starter**: make it the input driver the progenitor builds on riscv64, if it is the
  better driver for the NS16550 than what runs there now.

Measure first: compare it with the hand-over's riscv64 input driver before choosing.
