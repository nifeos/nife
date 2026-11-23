#![cfg_attr(not(test), no_std)]
//! The `StarFive` JH7110's Peripheral Component Interconnect (PCI) Express root complex, as pure
//! logic: the `PLDA` `XpressRICH` bridge's registers ([`bridge`]), the `StarFive` glue in the
//! system top group (STG) controller and the physical (PHY) layer, the bring-up sequence
//! ([`bring_up`]) and the root port's bus and window assignment ([`root_port`]).
//!
//! The JH7110 has two ports. On a `VisionFive` 2, port 1 is the M.2 M-key slot and port 0 carries
//! the board's Universal Serial Bus (USB) 3 controller. Once [`bring_up`] has run, a port's `cfg`
//! window answers configuration requests in the Enhanced Configuration Access Mechanism (ECAM)
//! layout, so `pci_configuration_space` walks it as it walks QEMU's generic host. Nothing here
//! reads a device tree or touches memory: the caller supplies a [`Registers`] implementation over
//! the five windows a bring-up writes.
//!
//! Every offset, bit and step is transcribed from mainline Linux, `torvalds/linux` at
//! `cbbddb6b08cc`: `drivers/pci/controller/plda/pcie-starfive.c`, `pcie-plda.h`,
//! `pcie-plda-host.c`, `drivers/phy/starfive/phy-jh7110-pcie.c`,
//! `drivers/pinctrl/starfive/pinctrl-starfive-jh7110.c` and `-sys.c`, and
//! `arch/riscv/boot/dts/starfive/jh7110.dtsi` and `jh7110-common.dtsi`. No BSD drives this
//! controller, so each spelled-out name carries the Linux spelling as its alias. See
//! `nifeos/record:notes/jh7110-pci-express.md`.
//!
//! # BUGS
//!
//! - The fundamental reset pin's (`PERST#`) pad configuration (input enable, Schmitt trigger,
//!   bias) is left as reset leaves it. Linux clears those three when it turns the pin into an output; the output itself
//!   does not depend on them.
//! - The root port forwards one bus. A PCI Express switch behind it would need a subordinate bus
//!   number above 1.
//! - No `vpcie3v3` regulator is driven: the `VisionFive` 2's tree names none.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex,
//! after the chip and the bus as `pci_configuration_space` spells it. calef has not named it.

pub mod bridge;
mod bring_up;
pub mod general_purpose_input_output;
pub mod glue;
pub mod physical_layer;
mod port;
pub mod root_port;

pub use bring_up::{
    Block, BroughtUp, Failure, LINK_POLL_INTERVAL_MILLISECONDS, LINK_POLLS,
    RESET_HOLD_MILLISECONDS, Registers, bring_up,
};
pub use port::{
    PHYSICAL_LAYER_SIZE_BYTES, Port, PortNumber, SYSTEM_CONTROLLER,
    SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT, SYSTEM_TOP_GROUP_CONTROLLER, Window,
};

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod simulation;
#[cfg(test)]
mod tests;
