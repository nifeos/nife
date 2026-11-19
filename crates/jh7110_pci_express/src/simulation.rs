//! **A simulated JH7110 root complex**, for the host tests: the five register windows a bring-up
//! writes, a link that trains only after a correct sequence, and an ECAM view of the `cfg` window
//! that decodes only once translation entry 0 maps it as configuration space, with a Non-Volatile
//! Memory Express (NVMe) controller at `01:00.0` that answers only once the root port forwards bus 1.
//!
//! What it models is what Linux's sequence implies the hardware needs. It cannot say the silicon
//! needs nothing more; the board does that.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

use std::collections::HashMap;

use crate::{
    Block, PortNumber, Registers, bridge, general_purpose_input_output as gpio, glue,
    physical_layer,
};

/// One thing the bring-up did, in order, for the ordering tests.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Write(Block, u32, u32),
    Wait(u32),
    ClocksAndResets,
}

/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub struct Simulated {
    pub port: PortNumber,
    pub words: HashMap<(Block, u32), u32>,
    pub events: Vec<Event>,
    /// Whether a reset fails to release.
    pub resets_stick: bool,
    /// Whether a device is fitted behind the port at all.
    pub device_fitted: bool,
    /// Milliseconds since `PERST#` was last driven low, while it is low; `None` while it is high.
    reset_held: Option<u32>,
    reset_was_held_long_enough: bool,
    clocks_on: bool,
    pub link_trained: bool,
    /// The root port's type 1 header words this model stores (bus numbers, windows, command).
    root_port: HashMap<u64, u32>,
    /// The NVMe controller's BAR0 and BAR1 as written.
    endpoint_bars: [u32; 2],
    reset_pin: u32,
}

/// What BAR0 of the simulated NVMe controller decodes: 16 KiB, 64-bit, non-prefetchable.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const ENDPOINT_BAR_BYTES: u32 = 0x4000;

impl Simulated {
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    pub fn new(port: PortNumber) -> Self {
        let mut words = HashMap::new();
        // Reset values that matter: LTR forwarding on, as the comment in Linux says, and the
        // controller's revision in the identification word.
        words.insert(
            (Block::Bridge, bridge::MESSAGE_RECEPTION),
            bridge::LATENCY_TOLERANCE_REPORTING_RECEPTION,
        );
        words.insert(
            (Block::Bridge, bridge::IDENTIFICATION_CLASS_AND_REVISION),
            0x0000_0002,
        );
        // Port 0's PHY wakes in USB 3 mode, which is what the mode writes undo.
        words.insert(
            (Block::SystemTopGroupController, physical_layer::MODE),
            1 << 20,
        );
        words.insert(
            (Block::SystemTopGroupController, physical_layer::USB_SELECT),
            physical_layer::USB3_PHYSICAL_LAYER_ENABLE,
        );
        let reset_pin = port.mainline().reset_pin.unwrap_or(0);
        Self {
            port,
            words,
            events: Vec::new(),
            resets_stick: false,
            device_fitted: true,
            reset_held: None,
            reset_was_held_long_enough: false,
            clocks_on: false,
            link_trained: false,
            root_port: HashMap::new(),
            endpoint_bars: [0; 2],
            reset_pin,
        }
    }

    fn word(&self, block: Block, offset: u32) -> u32 {
        self.words.get(&(block, offset)).copied().unwrap_or(0)
    }

    fn glue_word(&self, offset: u32) -> u32 {
        self.word(
            Block::SystemTopGroupController,
            self.port.system_top_group_controller_base() + offset,
        )
    }

    fn reset_pin_driven_low(&self) -> bool {
        let (word, shift) = gpio::word_and_shift(self.reset_pin);
        let enable = (self.word(Block::GeneralPurposeInputOutput, gpio::OUTPUT_ENABLE + word)
            >> shift)
            & gpio::OUTPUT_ENABLE_MASK;
        let level = (self.word(Block::GeneralPurposeInputOutput, gpio::OUTPUT + word) >> shift)
            & gpio::OUTPUT_MASK;
        enable == gpio::DRIVE && level == gpio::LOW
    }

    fn physical_layer_in_pci_express_mode(&self) -> bool {
        let tuned = self.word(Block::PhysicalLayer, physical_layer::OSCILLATOR_GAIN_LEVEL)
            == physical_layer::OSCILLATOR_GAIN_FINE_TUNE_LEVEL
            && self.word(
                Block::PhysicalLayer,
                physical_layer::OSCILLATOR_GAIN_TUNE_SIGNAL,
            ) == physical_layer::OSCILLATOR_GAIN_FINE_TUNE_SIGNALS;
        let mode = match self.port {
            PortNumber::One => true,
            PortNumber::Zero => {
                self.word(Block::SystemTopGroupController, physical_layer::MODE)
                    & physical_layer::MODE_MASK
                    == 0
                    && self.word(Block::SystemTopGroupController, physical_layer::USB_SELECT)
                        & (physical_layer::USB3_BUS_WIDTH_MASK
                            | physical_layer::USB3_PHYSICAL_LAYER_ENABLE)
                        == physical_layer::USB3_BUS_WIDTH
            }
        };
        tuned && mode
    }

    /// What the hardware would need before training: the PHY in PCI Express mode, the glue saying
    /// root port with a reference clock, clocks on, the bridge a root port, `PERST#` held for
    /// [`crate::RESET_HOLD_MILLISECONDS`] and then released.
    fn may_train(&self) -> bool {
        self.device_fitted
            && self.physical_layer_in_pci_express_mode()
            && self.clocks_on
            && self.glue_word(glue::ROOT_PORT) & glue::ROOT_PORT_NOT_ENDPOINT != 0
            && self.glue_word(glue::WRITE_CHANNEL) & glue::REFERENCE_CLOCK_SOURCE_MASK
                == glue::REFERENCE_CLOCK_SOURCE
            && self.word(Block::Bridge, bridge::GENERAL_SETTINGS) & bridge::ROOT_PORT_ENABLE != 0
            && self.reset_was_held_long_enough
            && !self.reset_pin_driven_low()
    }

    fn configuration_decodes(&self) -> bool {
        let source = self.word(Block::Bridge, bridge::OUTBOUND_SOURCE_LOW);
        let configuration = self.port.mainline().configuration;
        source & bridge::TRANSLATION_ENABLE != 0
            && self.word(Block::Bridge, bridge::OUTBOUND_INTERFACE)
                == bridge::INTERFACE_CONFIGURATION
            && u64::from(source & !0xfff) == configuration.physical_address & 0xffff_f000
            && u64::from(self.word(Block::Bridge, bridge::OUTBOUND_SOURCE_HIGH))
                == configuration.physical_address >> 32
            && bridge::translated_size_bytes(source) == Some(configuration.size_bytes)
    }

    fn forwards_bus_one(&self) -> bool {
        let buses = self
            .root_port
            .get(&pci_configuration_space::BUS_NUMBERS)
            .copied()
            .unwrap_or(0);
        let command = self
            .root_port
            .get(&pci_configuration_space::COMMAND)
            .copied()
            .unwrap_or(0);
        (buses >> 8) & 0xff == 1
            && (buses >> 16) & 0xff >= 1
            && command & u32::from(pci_configuration_space::CMD_MEMORY_SPACE) != 0
    }

    /// A configuration read at `offset` bytes into the `cfg` window, as the processor sees it.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    pub fn configuration_read(&self, offset: u64) -> u32 {
        if !self.configuration_decodes() {
            // An untranslated window is a bus error on the silicon; the model answers what a
            // walk would treat as nothing there.
            return 0xffff_ffff;
        }
        let bus = (offset >> 20) as u8;
        let device = ((offset >> 15) & 0x1f) as u8;
        let function = ((offset >> 12) & 0x7) as u8;
        let register = offset & 0xffc;
        match (bus, device, function) {
            (0, 0, 0) => self.root_port_read(register),
            (1, 0, 0) if self.link_trained && self.forwards_bus_one() => {
                self.endpoint_read(register)
            }
            _ => 0xffff_ffff,
        }
    }

    /// A configuration write, the counterpart of [`Self::configuration_read`].
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    pub fn configuration_write(&mut self, offset: u64, value: u32) {
        if !self.configuration_decodes() {
            return;
        }
        let bus = (offset >> 20) as u8;
        let device = ((offset >> 15) & 0x1f) as u8;
        let register = offset & 0xffc;
        match (bus, device) {
            (0, 0) => {
                self.root_port.insert(register, value);
            }
            (1, 0) if self.link_trained && self.forwards_bus_one() => {
                if register == 0x10 {
                    self.endpoint_bars[0] = value;
                } else if register == 0x14 {
                    self.endpoint_bars[1] = value;
                }
            }
            _ => {}
        }
    }

    fn root_port_read(&self, register: u64) -> u32 {
        match register {
            // PLDA's vendor identifier and the JH7110's device identifier.
            0x00 => 0x0100_1556,
            0x08 => self.word(Block::Bridge, bridge::IDENTIFICATION_CLASS_AND_REVISION),
            0x0c => u32::from(pci_configuration_space::HEADER_TYPE_BRIDGE) << 16,
            // Hidden: the `StarFive` glue reads zero here, as Linux's config accessor makes it.
            0x10 | 0x14 => 0,
            r => self.root_port.get(&r).copied().unwrap_or(0),
        }
    }

    fn endpoint_read(&self, register: u64) -> u32 {
        match register {
            // A Samsung vendor identifier, as a typical M.2 drive reports.
            0x00 => 0xa808_144d,
            0x08 => pci_configuration_space::CLASS_NVME << 8,
            0x0c => 0,
            0x10 => {
                let written = self.endpoint_bars[0];
                // 64-bit memory BAR (type 2 in bits 2:1).
                (written & !(ENDPOINT_BAR_BYTES - 1)) | 0b100
            }
            0x14 => self.endpoint_bars[1],
            _ => 0,
        }
    }
}

impl Registers for Simulated {
    fn read(&mut self, block: Block, offset: u32) -> u32 {
        let base = self.port.system_top_group_controller_base();
        if block == Block::SystemTopGroupController && offset == base + glue::LINK_STATUS {
            return if self.link_trained {
                glue::DATA_LINK_ACTIVE
            } else {
                0
            };
        }
        self.word(block, offset)
    }

    fn write(&mut self, block: Block, offset: u32, value: u32) {
        self.events.push(Event::Write(block, offset, value));
        self.words.insert((block, offset), value);
        if block == Block::GeneralPurposeInputOutput {
            if self.reset_pin_driven_low() {
                if self.reset_held.is_none() {
                    self.reset_held = Some(0);
                    self.reset_was_held_long_enough = false;
                    self.link_trained = false;
                }
            } else if let Some(held) = self.reset_held.take() {
                self.reset_was_held_long_enough = held >= crate::RESET_HOLD_MILLISECONDS;
            }
        }
    }

    fn wait_milliseconds(&mut self, milliseconds: u32) {
        self.events.push(Event::Wait(milliseconds));
        if let Some(held) = self.reset_held.as_mut() {
            *held += milliseconds;
        }
        if self.may_train() {
            self.link_trained = true;
        }
    }

    fn enable_clocks_and_resets(&mut self, _port: PortNumber) -> bool {
        self.events.push(Event::ClocksAndResets);
        self.clocks_on = !self.resets_stick;
        self.clocks_on
    }
}
