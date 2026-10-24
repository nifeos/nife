# The JH7110 watchdog: how the block counts, and what was read

`crates/jh7110_watchdog` holds the logic (the offsets, the sequences, the arithmetic and the
device-tree query), and `kernel/src/drivers/jh7110_watchdog.rs` walks its sequences. Milestone 593
(a wedged kernel resets itself, through a watchdog that does not need asking) is the plan and holds
the bench step.

## How the block counts

The block is an Arm SP805 with `StarFive`'s layout. Software loads a count of core-clock ticks and
the counter runs down. At zero it raises its interrupt and reloads. At zero again with the interrupt
still raised, it asserts reset. Clearing the interrupt reloads the counter, and that is the pet.

So the reset lands `2 x LOAD / f` after the last pet. `load_for` halves the time a caller asks for,
the way Linux's `double_timeout` variant does, so the number a caller writes is the time to the
reset.

Every register write is refused until `0x1ACCE551` is written to the lock word. Each sequence in the
crate unlocks first and locks last. A write before the unlock is dropped silently, and a sequence
that ends unlocked leaves the timer open to any stray store.

The core clock is `wdt_core`, a gate of the 24 MHz oscillator. So the longest reset the block can
arm is about 357 s (`max_reset_after_seconds`).

## Sources

All were read on 2026-10-20 (UTC).

- Linux `drivers/watchdog/starfive-wdt.c`, the JH7110 variant. It gave the offsets, `RST_EN` at bit 1
  of the control word, the unlock key, `double_timeout`, and its `start`, `keepalive` and `stop`.
- Linux `arch/riscv/boot/dts/starfive/jh7110.dtsi` and `drivers/clk/starfive/clk-starfive-jh7110-sys.c`.
  They gave the node, and `wdt_core`'s parent `osc`.
- `StarFive`'s U-Boot `JH7110_VisionFive2_devel` branch: `arch/riscv/dts/jh7110.dtsi` and its clock
  and reset headers. They gave the vendor binding `starfive,dskit-wdt`, clocks 123 and 122, and
  resets 109 and 110.

## Limits

- QEMU has no model of this block. CI exercises the arithmetic, the sequences and the tree query,
  and only a board can test the reset itself. Run on one board, netbooted only (milestone 593's
  bench step, 2026-10-22); a reset back into a card boot is unseen.
- No evidence of a watchdog reset survives it that this crate knows of. The x86_64 TCO has
  `SECOND_TO_STS`, but no JH7110 source read here names a reset-cause register. So a second boot
  cannot say the watchdog ended the first, only that it came back in the expected window.
- The core clock's rate is the oscillator's, taken from the clock driver and not from the tree. A
  gate's rate is its parent's, and the tree names only the gate. The kernel measures the counter
  against its own timer before it trusts this number (`rate_agrees`).
