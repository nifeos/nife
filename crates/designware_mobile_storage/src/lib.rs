#![cfg_attr(not(test), no_std)]
//! The Synopsys `DesignWare` Mobile Storage Host Controller, as pure logic: radon's microSD slot
//! and eMMC socket. The whole driver sits behind [`host::Registers`], a window and a clock; a
//! kernel implements it over a mapped window and nothing more, so every sequence is tested
//! against `sim` on a host.
//! [`jh7110`] has the evidence for the part and the board's constants. Modules: [`regs`],
//! [`command`], [`host`], [`idmac`], [`card`], [`sd`], [`partition`], [`bench`](mod@bench).
//!
//! Name: ratified 2026-10-07 (calef, #1806): the databook's name for the part, spelled out as §154
//! (the acronym test is whether the phrase is spoken) asks; permanent on first publication. The
//! argument the minting lane gave: PROVISIONAL (the storage lane of milestone 53 (the board's own
//! peripherals: network and storage on real silicon), 2026-10-06 UTC). The Synopsys databook's own
//! name for the part, spelled out as `designware_ethernet` (the same lane's network half) spells
//! its own. An architect may prefer `designware_mmc` (Linux's `dw_mmc`) or a name for what it does
//! rather than whose IP it is.

//! Adapted from OpenBSD's `sys/dev/fdt/dwmmc.c` revision 1.33 (register map, reset, clock, bus
//! width, command flags and the polled FIFO transfer; identification after its `sdmmc_mem.c`),
//! under the ISC license, whose notice must be kept. GPL sources (Linux, U-Boot, the device trees)
//! were read only for hardware facts; no GPL code was copied.
//!
//! ```text
//! Copyright (c) 2017 Mark Kettenis
//!
//! Permission to use, copy, modify, and distribute this software for any
//! purpose with or without fee is hereby granted, provided that the above
//! copyright notice and this permission notice appear in all copies.
//!
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
//! WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
//! ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
//! WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
//! ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
//! OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//! ```

//! # BUGS
//! - The polled path has read radon's card; the IDMAC path has run only against `sim`, until a
//!   bench step on silicon measures it.
//! - The IDMAC moves at most its region's data pages per command and copies once each way
//!   through them; the EL0 server moves its data through the FIFO.
//! - Default speed only (25 MHz, 4-bit SD, 1-bit eMMC); no tuning, no 8-bit bus. Polled, with
//!   `INTMASK` zero. A 64-bit FIFO is refused. No card-detect or write-protect handling.
//! - Which part of radon's card the EL0 server serves is an architect's call; the window and its
//!   spawn words are nife's, outside this crate.

pub mod bench;
pub mod card;
pub mod command;
pub mod host;
pub mod idmac;
pub mod jh7110;
pub mod partition;
pub mod regs;
pub mod sd;

#[cfg(test)]
mod sim;

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod proofs;
