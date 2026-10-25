---
status: BUILT
promoted_from: serial-driver-has-no-starter
raised: 2026-10-20
built: 2026-10-24
---
# 945. `serial_driver` ships with nothing to start it

*Name: provisional. The number 945 is provisional until the merge queue lands it, and the title
and slug are too.*

Reuse: not applicable; this removes an existing program and writes none.

Raised 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`. `serial_driver` was the
interrupt-driven NS16550 receive driver from milestone 20 (a portable HAL, proven on a second
architecture), and the drivers package shipped it. Its only starter was riscv64's tour, whose
UART-driver step the self-test replaced in milestone 890 (riscv64 and `x86_64` join the shared boot
path). The input driver the progenitor starts at the hand-over, `components/src/input.rs`, serves
riscv64's UART, so nothing in the kernel or the progenitor started this program.

The choice was to retire it, or to make it the input driver the progenitor builds on riscv64 if it
was the better driver for the NS16550. The proposal said to compare the two first. The comparison
favored `input` on every axis, so `serial_driver` is retired.

## The comparison

Both programs were read at the commit this lane started from. `serial_driver` is quoted from its
last version in git, 85 lines; `input` is 310 lines covering all three architectures.

| | `serial_driver` | `input` on riscv64 |
|---|---|---|
| Register stride | fixed at one byte (QEMU's) | the shape the progenitor passes in its first start word (`abi::uart`), so one byte on QEMU and four on the JH7110 |
| Arms the receive interrupt | no; it relies on something else having set `IER` | sets `IER.ERBFI` itself, after a polled drain of what arrived before it ran |
| Bytes per interrupt | one, then `ACK`; the level-triggered line re-fires for the next byte in the FIFO | the whole FIFO, before the `ACK` |
| Calls per received byte | three: `WAIT`, `SEND`, `ACK` | per burst of n bytes: one `WAIT`, ⌈n/8⌉ `CALL`s, one `ACK`; for a 16-byte FIFO burst, 4 calls rather than 48 |
| Who reads its output | a report endpoint carrying one byte in a bare `SEND`; no program speaks that | the line discipline, over the terminal contract's `OPERATION_BYTES` (`notes/terminal-contract.md`), whose rendezvous is the flow control |
| Proof | none in CI since the tour step went | `script/swish-check` boots riscv64 to the prompt through it, and radon takes every keystroke through it (milestone 890) |

The stride row alone decides it. Started on radon, `serial_driver` would read the JH7110's
registers at the wrong offsets: its `LSR` read at byte 5 lands inside the second register, not
the fifth. So it was never a candidate for riscv64's real hardware without first acquiring
`input`'s shape handling. Adding that, `IER` arming and the terminal contract would rebuild the
riscv64 arm of `input` as a second program. That splits the one portable input driver into a
riscv64 exception for no gain.

No bench number applies. A timed run needs a starter, and writing one to measure a program that
loses on correctness first would build what this milestone exists to remove. The call count above is
read from the two loops and is exact for the paths they take.

## What was retired

`components/src/serial_driver.rs`, its `[[bin]]` in `components/Cargo.toml`, its row in
`design/module-block-baseline.txt`, and its place in `packages/drivers.package.toml` and the
`drivers` row of `notes/package-boundaries.md` (`python3 helpers/packages.py --table`). Every
archive packs every program, so each of the three archives is one entry smaller. A package's program
list reaches the distribution through basalt's pin of nife.

## Exit criteria

| # | Criterion | Evidence |
|---|---|---|
| 1 | The two drivers are compared on correctness and interrupt handling, and the comparison is recorded | The comparison, above |
| 2 | The losing side is retired, or the winning side gets a starter | `serial_driver` is out of `components/Cargo.toml`, `packages/drivers.package.toml` and the tree; `git grep serial_driver` outside history records and this block finds nothing; §241's floor table dropped it too |
| 3 | The tree builds and the gates pass without it | this pull request's `script/lint` and CI |

## Follow-on

- **Done.** §241 (a threadbare base)'s floor table no longer names the retired program: nifeos/nife#2695.

## Index row

riscv64's NS16550 receive driver from milestone 20 lost its only starter when the self-test
replaced the riscv64 tour, and the drivers package still shipped it. Compared with the input driver
the progenitor starts, it read the wrong register stride on the JH7110, never armed its own
interrupt, cost three calls per byte and spoke a protocol nothing reads, so it is retired.
