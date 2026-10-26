#![cfg_attr(not(test), no_std)]
//! **The Synopsys `DesignWare` Ethernet `QoS` controller, as pure logic**: radon's network ports.
//!
//! Everything the driver's two halves compute and nothing either touches. A kernel half brings the
//! controller and PHY up, measures DMA coherence and starts the DMA; a network-stack half fills
//! descriptors and moves the tail pointers. Adapted from OpenBSD's `dwqe` (`sys/dev/ic/dwqe.c`,
//! `dwqereg.h`, `sys/dev/fdt/if_dwqe_fdt.c`) and `ytphy` (`sys/dev/mii/ytphy.c`), whose JH7110 path
//! reads `starfive,tx-use-rgmii-clk`; FreeBSD's `eqos` was a cross-check, and GPL sources gave
//! hardware facts only, no code.
//!
//! Name: ratified 2026-10-07 (calef, #1806): Synopsys's name, spelled out per §154 (the acronym
//! test). The argument its minting lane gave: PROVISIONAL (milestone 53 (the board's own
//! peripherals: network and storage on real silicon)'s lane, 2026-10-06 UTC); the manifest pointed
//! to this paragraph for the argument and the alternative an architect may prefer.

//! The ISC license requires this notice:
//!
//! ```text
//! Copyright (c) 2008, 2019 Mark Kettenis <kettenis@openbsd.org>
//! Copyright (c) 2017, 2022 Patrick Wildt <patrick@blueri.se>
//! Copyright (c) 2001 Theo de Raadt
//! Copyright (c) 2023 Mark Kettenis <kettenis@openbsd.org>
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
//!
//! - Nothing here has touched the device; every test runs against `sim`. One receive and one
//!   transmit queue, polled: no interrupt, offloads, jumbo frames or multicast.
//! - Only `gmac0` (`16030000`) is wired; `gmac1`'s SYS-domain clocks have no plan.
#[cfg(test)]
extern crate std;

pub mod coherence;
pub mod controller;
pub mod jh7110;
#[doc(alias = "mdio")]
pub mod management_data_input_output;
pub mod motorcomm;
#[doc(alias = "regs")]
pub mod registers;
#[cfg(test)]
mod sim;

/// Offsets inside the DMA region, which the kernel allocates contiguous: `e1000e`'s layout,
/// because the two NICs' data planes have the same shape.
pub mod layout;

#[cfg(test)]
mod tests;

mod data_plane;
mod descriptor;
mod hardware;

pub use data_plane::{DataPlane, Region, Tails, TransmitError};
use descriptor::*;
pub use descriptor::{
    BUFFER_SIZE, CRC_BYTES, DESCRIPTOR_BYTES, MAXIMUM_FRAME, RDES3_BUF1V, RDES3_CTXT, RDES3_ES,
    RDES3_FD, RDES3_IOC, RDES3_LD, RDES3_LENGTH, RDES3_OWN, RECEIVE_ENTRIES, ReceiveVerdict,
    TDES2_BUF1_LEN, TDES2_IC, TDES3_ES, TDES3_FRAME_LEN, TDES3_FS, TDES3_LS, TDES3_OWN,
    TRANSMIT_ENTRIES, receive_descriptor_words, receive_verdict, transmit_descriptor_words,
};
pub use hardware::Hardware;

#[cfg(kani)]
mod proofs;
