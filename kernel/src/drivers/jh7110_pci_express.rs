//! The JH7110's PCI Express root complex, the kernel's own volatile half: the register bus
//! `crates/jh7110_pci_express` drives its bring-up through, and the configuration accessors the
//! root port is configured with before `kernel/src/pci.rs` knows the port exists.
//!
//! `crates/jh7110_pci_express` owns the offsets, bits and order, host-tested against a simulation.
//! This stores to the pages it is handed and refuses any offset outside them. It reaches into no
//! kernel global (§4 (kernel shape)): every base, the clock and reset windows and the delay come
//! from the caller. See partsfoundry/record:notes/jh7110-pci-express.md.
//!
//! # SAFETY
//!
//! Every store lands in a window [`regions`] names, which `mmu::map_everything` maps device-typed
//! from the same function. The caller establishes that the machine is a JH7110 with this port
//! first: `memory::jh7110_pci_express` records a port only when the tree names its node with
//! `status = "okay"`, and that discovery has no constant fallback, so QEMU's `virt` never reaches
//! this driver.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring, after
//! the crate it drives. calef has not named it.

use jh7110_clock_and_reset::{
    BringUpReport, PCI_EXPRESS_SYSTEM_BRING_UP, PCI_EXPRESS_SYSTEM_TOP_GROUP_BRING_UP, SYSTEM,
    SYSTEM_TOP_GROUP,
};
use jh7110_pci_express::{
    Block, Port, PortNumber, Registers, SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT,
    SYSTEM_TOP_GROUP_CONTROLLER,
};
use pci_configuration_space::BusDeviceFunction;

/// One bus of configuration space in the ECAM layout.
const BUS_BYTES: u64 = 0x10_0000;

/// One function's configuration space.
const FUNCTION_BYTES: u64 = 0x1000;

/// How much of the bridge window is mapped: two pages, because the root port's own header (whose
/// BARs the bring-up clears) sits at `bridge::ROOT_PORT_CONFIGURATION`, `0x1000`.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub const BRIDGE_MAPPED_BYTES: u64 = 0x2000;

/// How much of each other register window is mapped: one page holds every word the bring-up
/// writes in the STG system controller, the PHY and the GPIO controller.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub const PAGE_MAPPED_BYTES: u64 = 0x1000;

/// How many buses of the `cfg` window are mapped: the root port's bus and the one it forwards
/// (`jh7110_pci_express::root_port::SECONDARY_BUS`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub const CONFIGURATION_BUSES: u16 = 2;

/// **Every window this driver stores to or reads, as (physical address, bytes)**: the bridge, the
/// STG system controller, the port's PHY, the SYS GPIO controller and the mapped buses of the
/// `cfg` window, in that order. Each is clamped to what the tree describes. `mmu::map_everything`
/// maps exactly these, and `pci.rs` builds [`Windows`] from the same list, so the two cannot
/// disagree about what is mapped.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
#[must_use]
pub fn regions(port: &Port) -> [(u64, u64); 5] {
    [
        (
            port.bridge.physical_address,
            BRIDGE_MAPPED_BYTES.min(port.bridge.size_bytes),
        ),
        (
            SYSTEM_TOP_GROUP_CONTROLLER.physical_address,
            PAGE_MAPPED_BYTES.min(SYSTEM_TOP_GROUP_CONTROLLER.size_bytes),
        ),
        (
            port.number.physical_layer_physical_address(),
            PAGE_MAPPED_BYTES,
        ),
        (
            SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT.physical_address,
            PAGE_MAPPED_BYTES.min(SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT.size_bytes),
        ),
        (
            port.configuration.physical_address,
            (u64::from(CONFIGURATION_BUSES) * BUS_BYTES).min(port.configuration.size_bytes),
        ),
    ]
}

/// **A mapped register window**: its kernel virtual address and how many bytes of it are mapped.
/// Zero bytes is a window this kernel does not map, and every access to it is refused.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    /// The first mapped byte, in the direct map.
    pub virtual_address: usize,
    /// How many bytes from it are mapped.
    pub bytes: u64,
}

impl Page {
    /// A window nothing maps. Port 1's bring-up never writes the SYS system controller, which only
    /// port 0's PHY uses, so it stays unmapped.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
    pub const UNMAPPED: Page = Page {
        virtual_address: 0,
        bytes: 0,
    };
}

/// **What the caller hands the driver**: one [`Page`] per [`Block`], the two clock-and-reset
/// windows the port's plans walk, and the `cfg` window.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
#[derive(Debug, Clone, Copy)]
pub struct Windows {
    /// [`Block::Bridge`].
    pub bridge: Page,
    /// [`Block::SystemTopGroupController`].
    pub system_top_group_controller: Page,
    /// [`Block::PhysicalLayer`].
    pub physical_layer: Page,
    /// [`Block::SystemController`].
    pub system_controller: Page,
    /// [`Block::GeneralPurposeInputOutput`].
    pub general_purpose_input_output: Page,
    /// The SYS clock-and-reset window, mapped for its whole domain.
    pub system_clock_and_reset_virtual_address: usize,
    /// The STG clock-and-reset window, mapped for its whole domain.
    pub system_top_group_clock_and_reset_virtual_address: usize,
    /// The `cfg` window; [`Page::bytes`] is a whole number of buses.
    pub configuration: Page,
}

/// **One root port's registers, as `jh7110_pci_express::bring_up` reaches them.**
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub struct Controller<'a> {
    windows: Windows,
    wait_milliseconds: &'a mut dyn FnMut(u32),
    /// How many accesses fell outside a mapped page and were not made.
    pub refused: u32,
    /// The SYS plan's report, once the bring-up has turned the clocks on.
    pub system: BringUpReport,
    /// The port's STG plan's report.
    pub system_top_group: BringUpReport,
}

impl<'a> Controller<'a> {
    /// The driver over `windows`, waiting through `wait_milliseconds`.
    ///
    /// # Safety
    ///
    /// Each [`Page`] in `windows` must be mapped device-typed for its `bytes` and belong to the
    /// JH7110 block its field names, and both clock-and-reset addresses must be mapped for their
    /// whole domain (see the module's `SAFETY`).
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
    pub unsafe fn new(windows: Windows, wait_milliseconds: &'a mut dyn FnMut(u32)) -> Self {
        Self {
            windows,
            wait_milliseconds,
            refused: 0,
            system: BringUpReport::default(),
            system_top_group: BringUpReport::default(),
        }
    }

    fn word(&mut self, block: Block, offset: u32) -> Option<*mut u32> {
        let page = match block {
            Block::Bridge => self.windows.bridge,
            Block::SystemTopGroupController => self.windows.system_top_group_controller,
            Block::PhysicalLayer => self.windows.physical_layer,
            Block::SystemController => self.windows.system_controller,
            Block::GeneralPurposeInputOutput => self.windows.general_purpose_input_output,
        };
        if offset % 4 != 0 || u64::from(offset) + 4 > page.bytes {
            self.refused += 1;
            return None;
        }
        Some((page.virtual_address + offset as usize) as *mut u32)
    }

    fn configuration_word(
        &self,
        bus_device_function: BusDeviceFunction,
        offset: u64,
    ) -> Option<*mut u32> {
        let at = bus_device_function.ecam_offset() + (offset & !3);
        (bus_device_function.device < 32
            && bus_device_function.function < 8
            && offset < FUNCTION_BYTES
            && at < self.windows.configuration.bytes)
            .then(|| (self.windows.configuration.virtual_address + at as usize) as *mut u32)
    }

    /// **A configuration read** through the `cfg` window: the word at `offset` in `bus_device_function`'s space,
    /// or all ones (nothing there) for a bus that is not mapped.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
    #[must_use]
    pub fn configuration_read(&self, bus_device_function: BusDeviceFunction, offset: u64) -> u32 {
        match self.configuration_word(bus_device_function, offset) {
            // SAFETY: `configuration_word` bounded the address inside the mapped `cfg` buses,
            // which `new`'s contract makes a device mapping of this port's window.
            Some(word) => unsafe { core::ptr::read_volatile(word) },
            None => 0xffff_ffff,
        }
    }

    /// **A configuration write**, [`Self::configuration_read`]'s counterpart; dropped for a bus that
    /// is not mapped.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
    pub fn configuration_write(
        &self,
        bus_device_function: BusDeviceFunction,
        offset: u64,
        value: u32,
    ) {
        if let Some(word) = self.configuration_word(bus_device_function, offset) {
            // SAFETY: as `configuration_read`.
            unsafe { core::ptr::write_volatile(word, value) }
        }
    }
}

impl Registers for Controller<'_> {
    fn read(&mut self, block: Block, offset: u32) -> u32 {
        match self.word(block, offset) {
            // SAFETY: `word` bounded the offset inside a page `new`'s contract makes a device
            // mapping of this block.
            Some(word) => unsafe { core::ptr::read_volatile(word) },
            None => 0,
        }
    }

    fn write(&mut self, block: Block, offset: u32, value: u32) {
        if let Some(word) = self.word(block, offset) {
            // SAFETY: as `read`. The crate does its own read-modify-write where a word holds other
            // fields, and the boot thread is the only writer of these blocks this early.
            unsafe { core::ptr::write_volatile(word, value) }
        }
    }

    fn wait_milliseconds(&mut self, milliseconds: u32) {
        (self.wait_milliseconds)(milliseconds);
    }

    fn enable_clocks_and_resets(&mut self, port: PortNumber) -> bool {
        let plan = PCI_EXPRESS_SYSTEM_TOP_GROUP_BRING_UP[match port {
            PortNumber::Zero => 0,
            PortNumber::One => 1,
        }];
        // SAFETY: `new`'s contract maps both windows for their whole domain on a JH7110, and
        // `bring_up` bounds every identifier the plans name by the domain it is handed.
        self.system = unsafe {
            crate::drivers::jh7110_clock_and_reset::bring_up(
                self.windows.system_clock_and_reset_virtual_address,
                &SYSTEM,
                PCI_EXPRESS_SYSTEM_BRING_UP,
            )
        };
        // SAFETY: as above.
        self.system_top_group = unsafe {
            crate::drivers::jh7110_clock_and_reset::bring_up(
                self.windows
                    .system_top_group_clock_and_reset_virtual_address,
                &SYSTEM_TOP_GROUP,
                plan,
            )
        };
        [&self.system, &self.system_top_group]
            .iter()
            .all(|r| r.rejected == 0 && r.resets_released == r.resets)
    }
}
