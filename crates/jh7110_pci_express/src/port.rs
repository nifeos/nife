//! **A port's windows**: what the tree describes for each of the JH7110's two root ports, and
//! the shared system controller windows a bring-up also writes.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

/// **A window of the processor's physical address space.**
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The first byte.
    pub physical_address: u64,
    /// The length.
    pub size_bytes: u64,
}

/// **Which of the JH7110's two root ports**, as the tree's `linux,pci-domain` numbers them.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortNumber {
    /// `pcie@940000000`: the VL805 USB 3 controller on a `VisionFive` 2.
    Zero,
    /// `pcie@9c0000000`: the M.2 M-key slot on a `VisionFive` 2.
    One,
}

impl PortNumber {
    /// The port from its PCI domain number, or `None` for any other.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    #[must_use]
    pub const fn from_domain(domain: u32) -> Option<Self> {
        match domain {
            0 => Some(Self::Zero),
            1 => Some(Self::One),
            _ => None,
        }
    }

    /// Where this port's words start in the STG system controller.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    #[doc(alias = "STG_SYSCON_PCIE0_BASE")]
    #[doc(alias = "STG_SYSCON_PCIE1_BASE")]
    #[must_use]
    pub const fn system_top_group_controller_base(self) -> u32 {
        match self {
            Self::Zero => 0x48,
            Self::One => 0x1f8,
        }
    }

    /// The port's PHY window, `pciephy0` or `pciephy1`.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    #[must_use]
    pub const fn phy_physical_address(self) -> u64 {
        match self {
            Self::Zero => 0x1021_0000,
            Self::One => 0x1022_0000,
        }
    }

    /// Mainline's description of the port, for when the tree has no node.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
    #[must_use]
    pub const fn mainline(self) -> Port {
        match self {
            Self::Zero => Port {
                number: self,
                bridge: Window {
                    physical_address: 0x2b00_0000,
                    size_bytes: 0x10_0000,
                },
                configuration: Window {
                    physical_address: 0x9_4000_0000,
                    size_bytes: 0x100_0000,
                },
                memory_32_bit: Window {
                    physical_address: 0x3000_0000,
                    size_bytes: 0x800_0000,
                },
                prefetchable_memory: Some(Window {
                    physical_address: 0x9_0000_0000,
                    size_bytes: 0x4000_0000,
                }),
                reset_pin: Some(26),
            },
            Self::One => Port {
                number: self,
                bridge: Window {
                    physical_address: 0x2c00_0000,
                    size_bytes: 0x10_0000,
                },
                configuration: Window {
                    physical_address: 0x9_c000_0000,
                    size_bytes: 0x100_0000,
                },
                memory_32_bit: Window {
                    physical_address: 0x3800_0000,
                    size_bytes: 0x800_0000,
                },
                prefetchable_memory: Some(Window {
                    physical_address: 0x9_8000_0000,
                    size_bytes: 0x4000_0000,
                }),
                reset_pin: Some(28),
            },
        }
    }
}

/// **One root port, as its tree node describes it.** The PCI side of each memory window equals
/// its processor side on the JH7110, as both trees' `ranges` give it.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    /// Which port.
    pub number: PortNumber,
    /// The bridge's register window, `reg-names = "apb"`.
    pub bridge: Window,
    /// The configuration window, `reg-names = "cfg"`, ECAM-shaped once [`bring_up`] has run.
    pub configuration: Window,
    /// The 32-bit non-prefetchable memory range the kernel places BARs in.
    pub memory_32_bit: Window,
    /// The 64-bit prefetchable memory range, if the tree names one.
    pub prefetchable_memory: Option<Window>,
    /// The PERST# pin in the SYS GPIO controller, active low, if the tree names one.
    pub reset_pin: Option<u32>,
}

/// **The STG system controller's window**, which holds both ports' glue words and port 0's PHY
/// mode bits.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const SYSTEM_TOP_GROUP_CONTROLLER: Window = Window {
    physical_address: 0x1024_0000,
    size_bytes: 0x1000,
};
/// **The SYS system controller's window**, which holds port 0's PHY split-reset bit.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const SYSTEM_CONTROLLER: Window = Window {
    physical_address: 0x1303_0000,
    size_bytes: 0x1000,
};
/// **The SYS GPIO controller's window**, `sysgpio`, which drives each port's PERST#.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const SYSTEM_GENERAL_PURPOSE_INPUT_OUTPUT: Window = Window {
    physical_address: 0x1304_0000,
    size_bytes: 0x1000,
};
/// Each PHY's window size. Only the first page is written.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const PHY_SIZE_BYTES: u64 = 0x1_0000;
