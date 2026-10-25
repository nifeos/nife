#![no_std]
//! **The Intel `e1000e` family, as pure logic**: the 82574L and the Sunrise Point I219.
//!
//! Everything both halves of the driver compute, and nothing either touches. A kernel half
//! resets the controller, reads its MAC address and programs the ring bases; a network-stack half
//! fills descriptors and moves the tails.
//! A ring's base and tail share a page ([`regs`]), so the IOMMU is the whole DMA confinement; the
//! process holds only [`RX_QUEUE_PAGE`] and [`TX_QUEUE_PAGE`]. The register facts come from Linux's
//! `regs.h` and `hw.h` for the 82574L and from FreeBSD's Intel shared code for the I219 ([`pch`]).

//! # BUGS
//!
//! - **The I219's bring-up is ported in [`pch`] but not its link-up half**, which [`pch`]'s own
//!   BUGS records.
//! - **Legacy descriptors on the I219 are an assumption**, believed from the 8254x lineage and not
//!   read anywhere.
//! - **One receive and one transmit queue, polled.** No interrupt, no checksum offload, no
//!   segmentation offload, no jumbo frames. Each is a later decision rather than an omission.

#[cfg(test)]
extern crate std;

pub mod pch;

/// Register offsets in BAR0. Read from Linux's `regs.h`; queue 0 only.
pub mod regs;

/// Offsets inside the DMA region, which the kernel allocates contiguous and confines whole.
pub mod layout;

/// Where the process finds what it was mapped, which the kernel's spawn and `net_stack` must
/// agree on (rule 7: a crate, not a pair of "must match" comments).
pub mod process;

#[cfg(test)]
mod tests;

mod control;
mod data_plane;
mod descriptor;
mod device;
mod handoff;

pub use control::{
    CTRL_ASDE, CTRL_EXT_DRV_LOAD, CTRL_RST, CTRL_SLU, RAH_AV, RFCTL_EXTEN, STATUS_LU,
    mac_from_receive_address, rctl, tctl, tipg,
};
pub use data_plane::{DataPlane, Region, Tails, TxError};
use descriptor::*;
pub use descriptor::{
    BUFFER_SIZE, MAX_FRAME, RX_ENTRIES, RX_ERRORS_FRAME, RX_QUEUE_PAGE, RX_STATUS_DD,
    RX_STATUS_EOP, RxVerdict, TAIL_IN_PAGE, TX_COMMAND, TX_ENTRIES, TX_QUEUE_PAGE, TX_STATUS_DD,
    rx_verdict, tx_word,
};
pub use device::{DEVICE_IDS, VENDOR_INTEL, is_supported, model};
pub use handoff::{Handoff, ROLE_TAG};

#[cfg(kani)]
mod proofs;
