//! **The I219's own bring-up, ported from FreeBSD's `e1000` shared code.**
//!
//! The I219 is a MAC inside the platform controller hub (PCH) talking to a separate PHY that the
//! Management Engine shares. FreeBSD's Intel shared code (BSD-3-Clause, notice below) is the only
//! source read for it: [`ulp`], [`flush`], [`reset`], [`phy`], [`nvm`] and [`sequence`] each name
//! the FreeBSD functions they follow. Every sequence is generic over [`Hw`], so it runs on the host
//! against a simulated I219 (`sim`) as well as in the kernel. The constants are facts about the
//! hardware; the sequences are FreeBSD's, adapted, and carry Intel's license below as its first
//! condition requires.

//! ```text
//! SPDX-License-Identifier: BSD-3-Clause
//!
//! Copyright (c) 2001-2020, Intel Corporation
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//!  1. Redistributions of source code must retain the above copyright notice,
//!     this list of conditions and the following disclaimer.
//!
//!  2. Redistributions in binary form must reproduce the above copyright
//!     notice, this list of conditions and the following disclaimer in the
//!     documentation and/or other materials provided with the distribution.
//!
//!  3. Neither the name of the Intel Corporation nor the names of its
//!     contributors may be used to endorse or promote products derived from
//!     this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
//! LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//! ```

//! # BUGS
//!
//! - **It has run on silicon once, with no cable.** Otherwise it is proven only against `sim`, which
//!   is this crate's reading of FreeBSD and not the device.
//! - **The link-up half is not ported** (`e1000_check_for_copper_link_ich8lan`). A link that comes
//!   up and then loses frames points here first.
//! - **Flow control is not configured, so it is not advertised** ([`sequence::setup_copper_link`]).
//! - **The transmit flush diverges from FreeBSD in one place**, said at [`flush::tail_after`].
//! - **A failed `CV_SMB_CTRL` read is not written back**, where FreeBSD writes a stale value.
//! - **The license's second condition is not met by any binary this tree ships yet**.

pub mod nvm;
pub mod phy;
pub mod sequence;
#[cfg(test)]
mod sim;

/// MAC registers the I219 adds. Offsets from FreeBSD's `e1000_regs.h` and `e1000_ich8lan.h`.
pub mod regs;

/// Leaving ultra-low-power (ULP) mode through the Management Engine.
pub mod ulp;

/// The SPT descriptor-ring flush. FreeBSD's comment: "In I219, the descriptor rings must be
/// emptied before resetting the HW ... Failure to do this will cause the HW to enter a unit hang
/// state which can only be released by PCI reset on the device."
pub mod flush;

/// The MAC-register steps around the global reset.
pub mod reset;

#[cfg(test)]
mod tests;

mod hardware;

use hardware::*;
pub use hardware::{Hw, is_pch};
