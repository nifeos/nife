//! Runs the `jh7110_entropy` crate's board checks on a VisionFive 2 and prints the results.
//!
//! U-Boot's `booti` enters it in S-mode with `a0` the hart id and `a1` the device tree, exactly as
//! it enters the kernel, so the board's network-first boot script serves it as `nife-vf2.img` with
//! no typing at the prompt. It ungates the TRNG's clocks and releases its reset with
//! `jh7110_clock_and_reset::ENTROPY_BRING_UP`, then runs each check in [`checks`] in order and
//! prints one line per result on UART0. Every line is `board-check: <record> *<crc32>`, so the
//! checker (`results.py`) can drop a line the serial link damaged rather than misread it. When the
//! run ends it parks the hart; the board needs a power cycle before the next run.
//!
//! See notes/jh7110-entropy-board-check.md for the runbook, the record format and what each check
//! decides.
//!
//! Name: provisional.

#![no_std]
#![no_main]

mod checks;
mod console;
mod device;
mod machine;
#[cfg(feature = "model")]
mod model;
