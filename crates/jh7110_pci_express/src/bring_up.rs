//! **The bring-up sequence** over an injected register bus, in Linux's order. See
//! notes/jh7110-pci-express.md.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

use crate::{Port, PortNumber, Window, general_purpose_input_output, glue, physical_layer, bridge};

/// **The five windows a bring-up writes.**
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {
    /// The port's bridge window, [`Port::bridge`].
    Bridge,
    /// [`SYSTEM_TOP_GROUP_CONTROLLER`].
    SystemTopGroupController,
    /// The port's PHY window, [`PortNumber::physical_layer_physical_address`].
    PhysicalLayer,
    /// [`SYSTEM_CONTROLLER`].
    SystemController,
    /// [`SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT`].
    GeneralPurposeInputOutput,
}

/// **What a bring-up needs from its caller**: 32-bit register access in each [`Block`], a delay,
/// and the port's clocks and resets.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub trait Registers {
    /// Read the word at `offset` bytes into `block`.
    fn read(&mut self, block: Block, offset: u32) -> u32;
    /// Write the word at `offset` bytes into `block`.
    fn write(&mut self, block: Block, offset: u32, value: u32);
    /// Wait at least `milliseconds`.
    fn wait_milliseconds(&mut self, milliseconds: u32);
    /// Turn on the port's clocks and release its resets (see
    /// `jh7110_clock_and_reset::pci_express_bring_up`), answering whether every reset released.
    fn enable_clocks_and_resets(&mut self, port: PortNumber) -> bool;
}

/// How long `PERST#` is held after power and clocks are stable (`T_PVPERL`, PCI Express card
/// electromechanical specification 2.0, table 2-4), and how long software waits after releasing
/// it before the first configuration request (`PCIE_RESET_CONFIG_WAIT_MS`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "T_PVPERL", alias = "PCIE_RESET_CONFIG_WAIT_MS")]
pub const RESET_HOLD_MILLISECONDS: u32 = 100;
/// How many times the bring-up reads the link status (`PCIE_LINK_WAIT_MAX_RETRIES`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const LINK_POLLS: u32 = 10;
/// How long between link status reads (`PCIE_LINK_WAIT_SLEEP_MS`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const LINK_POLL_INTERVAL_MILLISECONDS: u32 = 90;

/// What [`bring_up`] did.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroughtUp {
    /// Whether the data link came up within [`LINK_POLLS`] reads.
    pub link_up: bool,
    /// How many reads it took, or [`LINK_POLLS`] when it never came up.
    pub link_polls: u32,
    /// The last link status word read.
    pub link_status: u32,
    /// How many outbound translation entries were programmed: configuration, 32-bit memory, and
    /// prefetchable memory when the port has it.
    pub windows: u32,
}

/// Why [`bring_up`] stopped.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// A reset did not release.
    ClocksAndResets,
    /// A window cannot be expressed as a translation entry: the entry's index.
    Window(u32),
}

fn update(registers: &mut dyn Registers, block: Block, offset: u32, mask: u32, value: u32) {
    let before = registers.read(block, offset);
    registers.write(block, offset, (before & !mask) | (value & mask));
}

fn drive_reset_pin(registers: &mut dyn Registers, pin: u32, asserted: bool) {
    use general_purpose_input_output as gpio;
    let (word, shift) = gpio::word_and_shift(pin);
    let level = if asserted { gpio::LOW } else { gpio::HIGH };
    update(
        registers,
        Block::GeneralPurposeInputOutput,
        gpio::OUTPUT + word,
        gpio::OUTPUT_MASK << shift,
        level << shift,
    );
    update(
        registers,
        Block::GeneralPurposeInputOutput,
        gpio::OUTPUT_ENABLE + word,
        gpio::OUTPUT_ENABLE_MASK << shift,
        gpio::DRIVE << shift,
    );
}

/// **Bring `port` up**, in the order Linux's `starfive_pcie_host_init` and `plda_pcie_host_init`
/// use: PHY mode, glue, clocks and resets, `PERST#` asserted, functions 1 to 3 disabled, the
/// bridge made a root port, `PERST#` released, the link polled, then the outbound windows.
///
/// A link that never comes up is reported in [`BroughtUp::link_up`] rather than as a failure, as
/// Linux does: the root port itself still answers at `00:00.0`. Refuses only when a reset does not
/// release (nothing after it would be reached) or a window cannot be translated.
///
/// # Errors
///
/// [`Failure::ClocksAndResets`] and [`Failure::Window`], above.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub fn bring_up(registers: &mut dyn Registers, port: &Port) -> Result<BroughtUp, Failure> {
    let glue_base = port.number.system_top_group_controller_base();

    // The PHY. Port 1's tree node names no system controller words, so only the tuning applies.
    registers.write(Block::PhysicalLayer, physical_layer::OSCILLATOR_GAIN_LEVEL, physical_layer::OSCILLATOR_GAIN_FINE_TUNE_LEVEL);
    registers.write(
        Block::PhysicalLayer,
        physical_layer::OSCILLATOR_GAIN_TUNE_SIGNAL,
        physical_layer::OSCILLATOR_GAIN_FINE_TUNE_SIGNALS,
    );
    if port.number == PortNumber::Zero {
        let top_group = Block::SystemTopGroupController;
        update(registers, top_group, physical_layer::MODE, physical_layer::MODE_MASK, 0);
        update(
            registers,
            top_group,
            physical_layer::USB_SELECT,
            physical_layer::USB3_BUS_WIDTH_MASK,
            physical_layer::USB3_BUS_WIDTH,
        );
        update(registers, top_group, physical_layer::USB_SELECT, physical_layer::USB3_PHYSICAL_LAYER_ENABLE, 0);
        update(
            registers,
            Block::SystemController,
            physical_layer::CONNECT,
            physical_layer::USB_POWER_DOWN_RESET_SPLIT,
            0,
        );
        update(
            registers,
            Block::PhysicalLayer,
            physical_layer::PHASE_LOCKED_LOOP_CONTROL,
            physical_layer::USB3_PHYSICAL_LAYER_ENABLE,
            0,
        );
    }

    let top_group = Block::SystemTopGroupController;
    update(
        registers,
        top_group,
        glue_base + glue::ROOT_PORT,
        glue::ROOT_PORT_NOT_ENDPOINT,
        glue::ROOT_PORT_NOT_ENDPOINT,
    );
    update(
        registers,
        top_group,
        glue_base + glue::WRITE_CHANNEL,
        glue::REFERENCE_CLOCK_SOURCE_MASK,
        glue::REFERENCE_CLOCK_SOURCE,
    );
    update(
        registers,
        top_group,
        glue_base + glue::WRITE_CHANNEL,
        glue::CLOCK_REQUEST,
        glue::CLOCK_REQUEST,
    );

    if !registers.enable_clocks_and_resets(port.number) {
        return Err(Failure::ClocksAndResets);
    }

    if let Some(pin) = port.reset_pin {
        drive_reset_pin(registers, pin, true);
    }

    for function in 1..glue::FUNCTIONS {
        update(
            registers,
            top_group,
            glue_base + glue::READ_CHANNEL,
            glue::READ_CHANNEL_MASK,
            function << glue::READ_CHANNEL_FUNCTION_SHIFT,
        );
        update(
            registers,
            top_group,
            glue_base + glue::WRITE_CHANNEL,
            glue::WRITE_CHANNEL_MASK,
            function << glue::WRITE_CHANNEL_FUNCTION_SHIFT,
        );
        update(
            registers,
            Block::Bridge,
            bridge::MISCELLANEOUS,
            bridge::PHYSICAL_FUNCTION_DISABLE,
            bridge::PHYSICAL_FUNCTION_DISABLE,
        );
    }
    update(
        registers,
        top_group,
        glue_base + glue::READ_CHANNEL,
        glue::READ_CHANNEL_MASK,
        0,
    );
    update(
        registers,
        top_group,
        glue_base + glue::WRITE_CHANNEL,
        glue::WRITE_CHANNEL_MASK,
        0,
    );

    let bridge = Block::Bridge;
    update(
        registers,
        bridge,
        bridge::GENERAL_SETTINGS,
        bridge::ROOT_PORT_ENABLE,
        bridge::ROOT_PORT_ENABLE,
    );
    registers.write(bridge, bridge::ROOT_PORT_BAR0, 0);
    registers.write(bridge, bridge::ROOT_PORT_BAR1, 0);
    update(
        registers,
        bridge,
        bridge::IDENTIFICATION_CLASS_AND_REVISION,
        !bridge::REVISION_MASK,
        bridge::CLASS_PCI_TO_PCI_BRIDGE,
    );
    update(
        registers,
        bridge,
        bridge::MESSAGE_RECEPTION,
        bridge::LATENCY_TOLERANCE_REPORTING_RECEPTION,
        0,
    );
    update(
        registers,
        bridge,
        bridge::WINDOW_CAPABILITIES,
        bridge::PREFETCH_WINDOW_64_BIT,
        bridge::PREFETCH_WINDOW_64_BIT,
    );

    registers.wait_milliseconds(RESET_HOLD_MILLISECONDS);
    if let Some(pin) = port.reset_pin {
        drive_reset_pin(registers, pin, false);
    }
    registers.wait_milliseconds(RESET_HOLD_MILLISECONDS);

    let mut link_polls = 0;
    let mut link_status = 0;
    let mut link_up = false;
    while link_polls < LINK_POLLS {
        link_status = registers.read(top_group, glue_base + glue::LINK_STATUS);
        link_polls += 1;
        if link_status & glue::DATA_LINK_ACTIVE != 0 {
            link_up = true;
            break;
        }
        registers.wait_milliseconds(LINK_POLL_INTERVAL_MILLISECONDS);
    }

    let mut windows = 0;
    let mut program = |registers: &mut dyn Registers, index: u32, window: Window, pci: u64| {
        let entry =
            bridge::outbound_window(index, window.physical_address, pci, window.size_bytes)
                .ok_or(Failure::Window(index))?;
        for (offset, value) in entry {
            registers.write(bridge, offset, value);
        }
        windows += 1;
        Ok(())
    };
    program(registers, 0, port.configuration, 0)?;
    program(
        registers,
        1,
        port.memory_32_bit,
        port.memory_32_bit.physical_address,
    )?;
    if let Some(window) = port.prefetch_memory {
        program(registers, 2, window, window.physical_address)?;
    }

    Ok(BroughtUp {
        link_up,
        link_polls,
        link_status,
        windows,
    })
}
