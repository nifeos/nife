//! **One device's clocks and resets, named by the device's register base**: what a server holding
//! the shared windows needs to bring one device up for a client and to touch nothing else.
//!
//! A [`Device`] is one block of the chip, keyed by the physical address its own registers start at,
//! with the plan that brings it up in each domain it draws on. [`Device::at`] finds it by that
//! address, the badge a client's capability carries. Every store and read goes through a
//! [`Windows`] the caller implements, so this module still dereferences no pointer and a host test
//! drives it against an array. See notes/jh7110-clock-and-reset.md.
//!
//! # BUGS
//!
//! - Each device's plan is this crate's constant, never the device tree's `clocks` and `resets`.
//!   A board whose tree wires a device differently gets the constant plan.
//!
//! Name: provisional.

use crate::{
    ALWAYS_ON, CLOCK_ENABLE, Domain, ENTROPY_BRING_UP, GMAC0_ALWAYS_ON_BRING_UP,
    GMAC0_SYSTEM_BRING_UP, InterIntegratedCircuitClockWords, MAXIMUM_SD_CARD_DIVIDER,
    POWER_MANAGEMENT_CHIP_BUS_BRING_UP, SD_INPUT_OUTPUT_BRING_UP, SYSTEM,
    SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0,
    SYSTEM_CLOCK_ADVANCED_PERIPHERAL_BUS_FUNCTION, SYSTEM_CLOCK_BUS_ROOT,
    SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD,
    SYSTEM_CLOCK_SYSTEM_TOP_GROUP_ADVANCED_EXTENSIBLE_INTERFACE_ADVANCED_HIGH_PERFORMANCE_BUS,
    SYSTEM_TOP_GROUP, Step, advanced_extensible_interface_configuration_0_hertz, clock_divider,
    is_clock_enabled, is_deasserted, sd_card_divider_for, with_divider, with_parent,
};

/// **Which window a word lives in**: one of the three clock and reset domains a device draws on,
/// or the SYS syscon, whose PLL words a rate needs.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowName {
    /// The STG domain, [`SYSTEM_TOP_GROUP`].
    SystemTopGroup,
    /// The SYS domain, [`SYSTEM`].
    System,
    /// The AON domain, [`ALWAYS_ON`].
    AlwaysOn,
    /// The SYS syscon, read for PLL2's rate and never written.
    SystemController,
}

impl WindowName {
    /// The clock and reset domain this window holds, or `None` for the syscon, which holds none.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn domain(self) -> Option<&'static Domain> {
        match self {
            WindowName::SystemTopGroup => Some(&SYSTEM_TOP_GROUP),
            WindowName::System => Some(&SYSTEM),
            WindowName::AlwaysOn => Some(&ALWAYS_ON),
            WindowName::SystemController => None,
        }
    }
}

/// **The bytes of each window a server touches**, counted from the window's base: every clock word
/// and every reset word of [`SYSTEM_TOP_GROUP`], [`SYSTEM`] and [`ALWAYS_ON`], and the syscon's
/// PLL2 words, lie in the window's first 4 KiB page, so a server needs one page of each.
///
/// Name: provisional.
pub const USED_WINDOW_BYTES: u64 = 0x1000;

/// The byte past a domain's last clock or reset word.
const fn domain_end(domain: &Domain) -> u64 {
    let clocks = 4 * domain.clocks as u64;
    let reset_words = 4 * domain.resets.div_ceil(32) as u64;
    let assert_end = domain.reset_assert + reset_words;
    let status_end = domain.reset_status + reset_words;
    let resets = if assert_end > status_end {
        assert_end
    } else {
        status_end
    };
    if clocks > resets { clocks } else { resets }
}

const _: () = assert!(domain_end(&SYSTEM_TOP_GROUP) <= USED_WINDOW_BYTES);
const _: () = assert!(domain_end(&SYSTEM) <= USED_WINDOW_BYTES);
const _: () = assert!(domain_end(&ALWAYS_ON) <= USED_WINDOW_BYTES);

/// The SYS syscon's PLL2 words, as `InterIntegratedCircuitClockWords` names them.
const PHASE_LOCKED_LOOP_2_WORDS: [u64; 3] = [0x2c, 0x30, 0x34];
const _: () = assert!(PHASE_LOCKED_LOOP_2_WORDS[2] + 4 <= USED_WINDOW_BYTES);

/// **The windows a server holds**, one word at a time. A host test implements it over arrays; the
/// server over its mapped pages.
///
/// Name: provisional.
pub trait Windows {
    /// Whether the server holds `window`. A device drawing on a window it does not hold is a
    /// device it does not serve.
    fn is_held(&self, window: WindowName) -> bool;
    /// The word at byte `offset` of `window`, below [`USED_WINDOW_BYTES`] and 4-aligned.
    fn read(&mut self, window: WindowName, offset: u64) -> u32;
    /// Store `value` at byte `offset` of `window`. Never called for the syscon.
    fn write(&mut self, window: WindowName, offset: u64, value: u32);
}

/// **One domain's share of a device's bring-up**: the window, and the plan run against it.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    /// The domain's window.
    pub window: WindowName,
    /// The steps, in the order the silicon needs.
    pub plan: &'static [Step],
}

/// **A block of the chip that a server brings up**, keyed by the physical address its registers
/// start at.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// The device tree's `reg` base for the block: the badge a client's capability carries.
    pub base_physical_address: u64,
    /// What it needs, domain by domain.
    pub parts: &'static [Part],
    /// The one clock a client may set the rate of, a SYS clock index, or `None`.
    pub rated_clock: Option<u32>,
}

/// **Every device a server serves.** The TRNG and the first GMAC port, both SD/MMC controllers, and
/// I2C5, the bus to the PMIC. The bases are the published trees' `reg`, which both agree on.
///
/// Name: provisional.
pub const DEVICES: [Device; 5] = [
    Device {
        base_physical_address: 0x1600_c000,
        parts: &[Part {
            window: WindowName::SystemTopGroup,
            plan: ENTROPY_BRING_UP,
        }],
        rated_clock: None,
    },
    Device {
        base_physical_address: 0x1603_0000,
        parts: &[
            Part {
                window: WindowName::System,
                plan: GMAC0_SYSTEM_BRING_UP,
            },
            Part {
                window: WindowName::AlwaysOn,
                plan: GMAC0_ALWAYS_ON_BRING_UP,
            },
        ],
        rated_clock: None,
    },
    Device {
        base_physical_address: 0x1601_0000,
        parts: &[Part {
            window: WindowName::System,
            plan: SD_INPUT_OUTPUT_BRING_UP[0],
        }],
        rated_clock: Some(SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD[0]),
    },
    Device {
        base_physical_address: 0x1602_0000,
        parts: &[Part {
            window: WindowName::System,
            plan: SD_INPUT_OUTPUT_BRING_UP[1],
        }],
        rated_clock: Some(SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD[1]),
    },
    Device {
        base_physical_address: 0x1205_0000,
        parts: &[Part {
            window: WindowName::System,
            plan: POWER_MANAGEMENT_CHIP_BUS_BRING_UP,
        }],
        rated_clock: None,
    },
];

/// **What a device's bring-up did**, counted: the numbers the wire's summary carries.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// `EnableClock` steps in the plan.
    pub clocks_in_plan: u8,
    /// Of those, the clocks whose enable bit read back set.
    pub clocks_running: u8,
    /// `DeassertReset` steps in the plan.
    pub resets_in_plan: u8,
    /// Of those, the resets that read released within the poll bound.
    pub resets_released: u8,
    /// Every clock read enabled and every reset released before anything was written.
    pub already_up: bool,
    /// Steps whose identifier the domain rejected, and so never stored.
    pub refused: u8,
}

/// Why [`Device::set_rate`] changed nothing.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateRefusal {
    /// The device has no rated clock, or the server holds no syscon to compute one from.
    NoRatedClock,
    /// Even the largest divider runs the clock faster than asked.
    NoRateAtOrBelow,
}

/// What [`Device::set_rate`] did: the clock's word before and after, and the rates.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// The clock's word before the store.
    pub before: u32,
    /// The word read back after it.
    pub after: u32,
    /// The parent's rate, from the silicon's own words.
    pub parent_hertz: u64,
    /// The rate the clock now runs at.
    pub hertz: u64,
}

impl Device {
    /// **The device whose registers start at `base_physical_address`**, or `None`, which a badge
    /// of 0 always is.
    ///
    /// Name: provisional.
    #[must_use]
    pub fn at(base_physical_address: u64) -> Option<&'static Device> {
        DEVICES
            .iter()
            .find(|d| d.base_physical_address == base_physical_address)
    }

    /// Whether `windows` holds every window this device's plan draws on. [`Device::set_rate`] also
    /// needs the SYS window and the syscon, and checks for them itself.
    ///
    /// Name: provisional.
    pub fn is_served_by(&self, windows: &impl Windows) -> bool {
        self.parts.iter().all(|p| windows.is_held(p.window))
    }

    /// The `index`th clock the plan enables, counted across the parts in order: its window and its
    /// domain's clock number.
    ///
    /// Name: provisional.
    #[must_use]
    pub fn clock(&self, index: u8) -> Option<(WindowName, u32)> {
        self.steps()
            .filter_map(|(window, step)| match step {
                Step::EnableClock(clock) => Some((window, clock)),
                _ => None,
            })
            .nth(usize::from(index))
    }

    /// The `index`th reset the plan releases, counted as [`Device::clock`] counts.
    ///
    /// Name: provisional.
    #[must_use]
    pub fn reset(&self, index: u8) -> Option<(WindowName, u32)> {
        self.steps()
            .filter_map(|(window, step)| match step {
                Step::DeassertReset(reset) => Some((window, reset)),
                _ => None,
            })
            .nth(usize::from(index))
    }

    /// Every step of every part, with the part's window, in plan order.
    fn steps(&self) -> impl Iterator<Item = (WindowName, Step)> + '_ {
        self.parts
            .iter()
            .flat_map(|p| p.plan.iter().map(move |s| (p.window, *s)))
    }

    /// **Bring the device up**: every clock step of every part first (each parent choice where the
    /// plan puts it, before its clock's enable), then every reset, each polled at most
    /// `poll_limit` times. Idempotent. The caller has checked [`Device::is_served_by`].
    ///
    /// Name: provisional.
    pub fn bring_up(&self, windows: &mut impl Windows, poll_limit: u32) -> Counts {
        let mut counts = Counts::default();
        let mut up_before = true;
        // Clocks before resets across the whole device, not part by part: a reset in one domain
        // may need a clock in another, and Linux's reset driver warns that a deassert against a
        // gated clock can hang.
        for (window, step) in self.steps() {
            let Some(domain) = window.domain() else {
                counts.refused += 1;
                continue;
            };
            match step {
                Step::EnableClock(clock) => {
                    let Some(offset) = domain.clock_offset(clock) else {
                        counts.refused += 1;
                        continue;
                    };
                    let before = windows.read(window, offset);
                    up_before &= is_clock_enabled(before);
                    windows.write(window, offset, before | CLOCK_ENABLE);
                    counts.clocks_in_plan += 1;
                    counts.clocks_running +=
                        u8::from(is_clock_enabled(windows.read(window, offset)));
                }
                Step::SelectParent { clock, parent } => {
                    let Some(offset) = domain.clock_offset(clock) else {
                        counts.refused += 1;
                        continue;
                    };
                    let before = windows.read(window, offset);
                    windows.write(window, offset, with_parent(before, parent));
                }
                Step::DeassertReset(_) => {}
            }
        }
        for (window, step) in self.steps() {
            let (Some(domain), Step::DeassertReset(reset)) = (window.domain(), step) else {
                continue;
            };
            let Some(bit) = domain.reset_bit(reset) else {
                counts.refused += 1;
                continue;
            };
            counts.resets_in_plan += 1;
            up_before &= is_deasserted(windows.read(window, bit.status_offset), bit.mask);
            // A read-modify-write: the word holds 31 other resets, and the server is the one
            // writer of every reset word it holds.
            let assert = windows.read(window, bit.assert_offset);
            windows.write(window, bit.assert_offset, assert & !bit.mask);
            let released = (0..poll_limit.max(1))
                .any(|_| is_deasserted(windows.read(window, bit.status_offset), bit.mask));
            counts.resets_released += u8::from(released);
        }
        counts.already_up = up_before && counts.clocks_in_plan + counts.resets_in_plan > 0;
        counts
    }

    /// **The `index`th clock's raw word**, or `None` beyond the device's own clocks.
    ///
    /// Name: provisional.
    pub fn read_clock(&self, windows: &mut impl Windows, index: u8) -> Option<u32> {
        let (window, clock) = self.clock(index)?;
        let offset = window.domain()?.clock_offset(clock)?;
        Some(windows.read(window, offset))
    }

    /// **The `index`th reset's raw status word**, or `None` beyond the device's own resets.
    ///
    /// Name: provisional.
    pub fn read_reset(&self, windows: &mut impl Windows, index: u8) -> Option<u32> {
        let (window, reset) = self.reset(index)?;
        let bit = window.domain()?.reset_bit(reset)?;
        Some(windows.read(window, bit.status_offset))
    }

    /// **Run the rated clock at no more than `most_hertz`**: the smallest divider that does, the
    /// clock gated while the divider moves so the device never sees a half-switched clock, and its
    /// enable bit as found. The parent's rate comes from the SYS window's bus words and the
    /// syscon's PLL2 words.
    ///
    /// # Errors
    ///
    /// [`RateRefusal`] says why nothing was stored.
    ///
    /// Name: provisional.
    pub fn set_rate(
        &self,
        windows: &mut impl Windows,
        most_hertz: u64,
    ) -> Result<Rate, RateRefusal> {
        let clock = self.rated_clock.ok_or(RateRefusal::NoRatedClock)?;
        if !windows.is_held(WindowName::System) || !windows.is_held(WindowName::SystemController) {
            return Err(RateRefusal::NoRatedClock);
        }
        let mut sys = |index: u32| windows.read(WindowName::System, u64::from(index) * 4);
        let bus_root = sys(SYSTEM_CLOCK_BUS_ROOT);
        let configuration_0 = sys(SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0);
        let top_group = sys(
            SYSTEM_CLOCK_SYSTEM_TOP_GROUP_ADVANCED_EXTENSIBLE_INTERFACE_ADVANCED_HIGH_PERFORMANCE_BUS,
        );
        let peripheral = sys(SYSTEM_CLOCK_ADVANCED_PERIPHERAL_BUS_FUNCTION);
        let [pll_0, pll_1, pll_2] =
            PHASE_LOCKED_LOOP_2_WORDS.map(|o| windows.read(WindowName::SystemController, o));
        let parent_hertz = advanced_extensible_interface_configuration_0_hertz(
            &InterIntegratedCircuitClockWords {
                bus_root,
                advanced_extensible_interface_configuration_0: configuration_0,
                system_top_group_advanced_extensible_interface_advanced_high_performance_bus:
                    top_group,
                advanced_peripheral_bus_function: peripheral,
                phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word: pll_0,
                phase_locked_loop_2_post_divider_1_word: pll_1,
                phase_locked_loop_2_pre_divider_word: pll_2,
            },
        );
        if parent_hertz / u64::from(MAXIMUM_SD_CARD_DIVIDER) > most_hertz {
            return Err(RateRefusal::NoRateAtOrBelow);
        }
        let offset = SYSTEM
            .clock_offset(clock)
            .ok_or(RateRefusal::NoRatedClock)?;
        let before = windows.read(WindowName::System, offset);
        let gated = with_divider(
            before & !CLOCK_ENABLE,
            sd_card_divider_for(parent_hertz, most_hertz),
        );
        windows.write(WindowName::System, offset, gated);
        windows.write(WindowName::System, offset, gated | (before & CLOCK_ENABLE));
        let after = windows.read(WindowName::System, offset);
        Ok(Rate {
            before,
            after,
            parent_hertz,
            hertz: parent_hertz / u64::from(clock_divider(after).max(1)),
        })
    }
}

#[cfg(test)]
mod tests;
