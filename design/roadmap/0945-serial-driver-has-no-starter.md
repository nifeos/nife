---
status: NOT-STARTED
promoted_from: serial-driver-has-no-starter
raised: 2026-10-20
milestone_dependencies: 890
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 945. `serial_driver` ships with nothing to start it

*Name: provisional. The number 945 is provisional until the merge queue lands it, and the title
and slug are too.*

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

## Index row

riscv64's NS16550 receive driver from milestone 20 lost its only starter when the self-test
replaced the riscv64 tour, and the drivers package still ships it. Compare it with the input driver
the progenitor starts, then retire it or make it the one riscv64 runs.
