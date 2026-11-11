//! The device table and its walk, driven against arrays that behave as the silicon does: a clock
//! word keeps what is stored, and a reset's status bit sets once its assert bit is clear.
//!
//! Name: provisional.

use jh7110_clock_and_reset::CLOCK_PARENT_MASK;

use super::*;

/// `gmac0_tx`, the AON clock whose parent the GMAC plan selects, and that parent, `gmac0_rmii_rtx`:
/// `jh7110_clock_and_reset`'s numbers, which it keeps to itself.
const AONCLK_GMAC0_TX: u32 = 5;
const AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX: u32 = 1;

/// What one store or read touched, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read(WindowName, u64),
    Write(WindowName, u64, u32),
}

/// Four windows of one page each, and every access in order.
struct Fake {
    words: [[u32; 1024]; 4],
    held: [bool; 4],
    /// Reset status bits that never set, by window, for a reset that stays held.
    stuck: [u32; 4],
    /// A window whose stores are dropped and which reads zero: a gated or absent block.
    dead: [bool; 4],
    log: Vec<Access>,
}

fn slot(w: WindowName) -> usize {
    match w {
        WindowName::SystemTopGroup => 0,
        WindowName::System => 1,
        WindowName::AlwaysOn => 2,
        WindowName::SystemController => 3,
    }
}

impl Fake {
    /// Every window held, every clock gated, every reset asserted.
    fn new() -> Fake {
        let mut f = Fake {
            words: [[0; 1024]; 4],
            held: [true; 4],
            stuck: [0; 4],
            dead: [false; 4],
            log: Vec::new(),
        };
        for w in [
            WindowName::SystemTopGroup,
            WindowName::System,
            WindowName::AlwaysOn,
        ] {
            let d = w.domain().unwrap();
            for id in (0..d.resets).step_by(32) {
                f.words[slot(w)][(d.reset_bit(id).unwrap().assert_offset / 4) as usize] = !0;
            }
        }
        f
    }

    fn word(&self, w: WindowName, offset: u64) -> u32 {
        self.words[slot(w)][(offset / 4) as usize]
    }

    /// Every write, without the reads.
    fn writes(&self) -> Vec<(WindowName, u64, u32)> {
        self.log
            .iter()
            .filter_map(|a| match *a {
                Access::Write(w, o, v) => Some((w, o, v)),
                Access::Read(..) => None,
            })
            .collect()
    }
}

impl Windows for Fake {
    fn is_held(&self, window: WindowName) -> bool {
        self.held[slot(window)]
    }

    fn read(&mut self, window: WindowName, offset: u64) -> u32 {
        assert!(self.is_held(window), "read of a window not held");
        assert!(offset < USED_WINDOW_BYTES && offset % 4 == 0);
        self.log.push(Access::Read(window, offset));
        if self.dead[slot(window)] {
            return 0;
        }
        // A status word reads as the complement of its assert word, less the stuck bits.
        if let Some(d) = window.domain() {
            let words = 4 * d.resets.div_ceil(32) as u64;
            if (d.reset_status..d.reset_status + words).contains(&offset) {
                let assert = self.word(window, offset - d.reset_status + d.reset_assert);
                return !assert & !self.stuck[slot(window)];
            }
        }
        self.word(window, offset)
    }

    fn write(&mut self, window: WindowName, offset: u64, value: u32) {
        assert!(self.is_held(window), "write to a window not held");
        assert_ne!(
            window,
            WindowName::SystemController,
            "the syscon is never written"
        );
        assert!(offset < USED_WINDOW_BYTES && offset % 4 == 0);
        self.log.push(Access::Write(window, offset, value));
        if !self.dead[slot(window)] {
            self.words[slot(window)][(offset / 4) as usize] = value;
        }
    }
}

fn gmac() -> &'static Device {
    Device::at(0x1603_0000).unwrap()
}

fn trng() -> &'static Device {
    Device::at(0x1600_c000).unwrap()
}

#[test]
fn every_badge_names_one_device_and_zero_names_none() {
    assert_eq!(Device::at(0), None);
    assert_eq!(Device::at(0x1603_0001), None);
    for (n, d) in DEVICES.iter().enumerate() {
        assert_ne!(d.base_physical_address, 0);
        assert!(
            DEVICES[..n]
                .iter()
                .all(|e| e.base_physical_address != d.base_physical_address)
        );
        assert_eq!(Device::at(d.base_physical_address), Some(d));
    }
}

#[test]
fn every_plan_names_only_identifiers_its_domain_has() {
    for d in &DEVICES {
        let counts = d.bring_up(&mut Fake::new(), 1);
        assert_eq!(counts.refused, 0, "{d:?}");
    }
}

#[test]
fn the_gmac_comes_up_as_the_ruling_example_says() {
    let mut f = Fake::new();
    let counts = gmac().bring_up(&mut f, 4);
    assert_eq!(
        counts,
        Counts {
            clocks_in_plan: 6,
            clocks_running: 6,
            resets_in_plan: 2,
            resets_released: 2,
            already_up: false,
            refused: 0,
        }
    );
    // SYS 108, 111, 109, then AON 3, 2, the parent of 5, 5, then AON resets 0 and 1.
    let clocks: Vec<_> = f.writes().into_iter().map(|(w, o, _)| (w, o)).collect();
    use WindowName::{AlwaysOn, System};
    let r0 = ALWAYS_ON.reset_bit(0).unwrap().assert_offset;
    assert_eq!(
        clocks,
        [
            (System, 108 * 4),
            (System, 111 * 4),
            (System, 109 * 4),
            (AlwaysOn, 3 * 4),
            (AlwaysOn, 2 * 4),
            (AlwaysOn, 5 * 4),
            (AlwaysOn, 5 * 4),
            (AlwaysOn, r0),
            (AlwaysOn, r0),
        ]
    );
    let tx = f.word(AlwaysOn, u64::from(AONCLK_GMAC0_TX) * 4);
    assert_eq!(
        (tx & CLOCK_PARENT_MASK) >> 24,
        AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX
    );
    assert!(is_clock_enabled(tx));
}

#[test]
fn no_reset_is_released_before_every_clock_is_enabled() {
    for d in &DEVICES {
        let mut f = Fake::new();
        d.bring_up(&mut f, 1);
        let writes = f.writes();
        let first_reset = writes.iter().position(|&(w, o, _)| {
            let r = w.domain().unwrap();
            o >= r.reset_assert && o < r.reset_assert + 16
        });
        let last_clock = writes
            .iter()
            .rposition(|&(w, o, _)| o < 4 * u64::from(w.domain().unwrap().clocks));
        if let (Some(reset), Some(clock)) = (first_reset, last_clock) {
            assert!(clock < reset, "{d:?}: a reset before a clock");
        }
    }
}

#[test]
fn a_parent_is_chosen_while_its_clock_is_still_gated() {
    let mut f = Fake::new();
    gmac().bring_up(&mut f, 1);
    let tx = u64::from(AONCLK_GMAC0_TX) * 4;
    let stores: Vec<u32> = f
        .writes()
        .into_iter()
        .filter(|&(w, o, _)| w == WindowName::AlwaysOn && o == tx)
        .map(|(_, _, v)| v)
        .collect();
    assert_eq!(stores.len(), 2);
    assert!(
        !is_clock_enabled(stores[0]),
        "the parent store ran a live clock"
    );
    assert!(is_clock_enabled(stores[1]));
}

#[test]
fn a_reset_store_keeps_the_other_31_lines_of_its_word() {
    let mut f = Fake::new();
    trng().bring_up(&mut f, 1);
    let bit = SYSTEM_TOP_GROUP.reset_bit(3).unwrap();
    assert_eq!(
        f.word(WindowName::SystemTopGroup, bit.assert_offset),
        !bit.mask
    );
}

#[test]
fn a_second_bring_up_says_already_up_and_changes_nothing() {
    let mut f = Fake::new();
    let first = trng().bring_up(&mut f, 1);
    assert!(!first.already_up);
    let words = f.words;
    let second = trng().bring_up(&mut f, 1);
    assert!(second.already_up);
    assert_eq!(f.words, words);
    assert_eq!(
        Counts {
            already_up: false,
            ..second
        },
        first
    );
}

#[test]
fn one_device_never_touches_another_devices_bits() {
    for d in &DEVICES {
        let mut f = Fake::new();
        d.bring_up(&mut f, 1);
        let mine: Vec<(WindowName, u32)> = (0..).map_while(|i| d.clock(i)).collect();
        for (w, o, v) in f.writes() {
            let domain = w.domain().unwrap();
            if o < 4 * u64::from(domain.clocks) {
                assert!(
                    mine.contains(&(w, (o / 4) as u32)),
                    "{d:?} wrote clock word {o:#x}"
                );
            } else {
                // A reset word: only this device's own bits may change.
                let mask: u32 = (0..)
                    .map_while(|i| d.reset(i))
                    .filter(|&(rw, _)| rw == w)
                    .filter_map(|(_, r)| domain.reset_bit(r))
                    .filter(|b| b.assert_offset == o)
                    .map(|b| b.mask)
                    .fold(0, |a, m| a | m);
                assert_eq!(
                    !v & !mask,
                    0,
                    "{d:?} changed another device's reset at {o:#x}"
                );
            }
        }
    }
}

#[test]
fn a_dead_window_reads_as_clocks_not_running_and_resets_held() {
    let mut f = Fake::new();
    f.dead[slot(WindowName::SystemTopGroup)] = true;
    let counts = trng().bring_up(&mut f, 3);
    assert_eq!((counts.clocks_in_plan, counts.clocks_running), (2, 0));
    assert_eq!((counts.resets_in_plan, counts.resets_released), (1, 0));
    // The poll bound held: three status reads for the one reset, after the one before the write.
    let bit = SYSTEM_TOP_GROUP.reset_bit(3).unwrap();
    let polls = f
        .log
        .iter()
        .filter(|a| **a == Access::Read(WindowName::SystemTopGroup, bit.status_offset))
        .count();
    assert_eq!(polls, 1 + 3);
}

#[test]
fn a_stuck_reset_is_counted_held() {
    let mut f = Fake::new();
    f.stuck[slot(WindowName::AlwaysOn)] = ALWAYS_ON.reset_bit(1).unwrap().mask;
    let counts = gmac().bring_up(&mut f, 2);
    assert_eq!((counts.resets_in_plan, counts.resets_released), (2, 1));
    assert_eq!(counts.clocks_running, 6);
}

#[test]
fn a_device_is_served_only_where_every_window_it_draws_on_is_held() {
    let mut f = Fake::new();
    f.held[slot(WindowName::AlwaysOn)] = false;
    assert!(!gmac().is_served_by(&f));
    assert!(trng().is_served_by(&f));
}

#[test]
fn a_read_reaches_the_devices_own_words_and_nothing_past_them() {
    let mut f = Fake::new();
    let d = gmac();
    d.bring_up(&mut f, 1);
    assert_eq!(d.clock(0), Some((WindowName::System, 108)));
    assert_eq!(d.clock(5), Some((WindowName::AlwaysOn, 5)));
    assert_eq!(d.clock(6), None);
    assert!(is_clock_enabled(d.read_clock(&mut f, 5).unwrap()));
    assert_eq!(d.read_clock(&mut f, 9), None);
    assert_eq!(d.reset(1), Some((WindowName::AlwaysOn, 1)));
    let status = d.read_reset(&mut f, 1).unwrap();
    assert!(is_deasserted(status, ALWAYS_ON.reset_bit(1).unwrap().mask));
    assert_eq!(d.read_reset(&mut f, 2), None);
}

/// The SYS words radon's firmware leaves, as the storage bench read them: `bus_root` on PLL2,
/// `axi_cfg0` divided by 2 and PLL2 in integer mode at 1188 MHz, so the card clock's parent runs at
/// 594 MHz, with the card clock's own divider left at 2.
fn radon_like() -> Fake {
    let mut f = Fake::new();
    let sys = slot(WindowName::System);
    f.words[sys][SYSTEM_CLOCK_BUS_ROOT as usize] = 1 << 24;
    f.words[sys][SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0 as usize] = 2;
    let syscon = slot(WindowName::SystemController);
    // DACPD and DSMPD set, FBDIV 99; POSTDIV1 0 (divide by 1); PREDIV 2.
    f.words[syscon][0x2c / 4] = 1 << 15 | 1 << 16 | 99 << 17;
    f.words[syscon][0x34 / 4] = 2;
    f.words[sys][SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD[1] as usize] = CLOCK_ENABLE | 2;
    f
}

#[test]
fn a_rate_is_the_fastest_at_or_below_the_ask_and_keeps_the_enable_bit() {
    let mut f = radon_like();
    let card = Device::at(0x1602_0000).unwrap();
    let rate = card.set_rate(&mut f, 50_000_000).unwrap();
    assert_eq!(rate.parent_hertz, 594_000_000);
    assert_eq!(clock_divider(rate.after), 12);
    assert_eq!(rate.hertz, 49_500_000);
    assert!(is_clock_enabled(rate.after));
    // Gated while the divider moved.
    let stores: Vec<u32> = f.writes().into_iter().map(|(_, _, v)| v).collect();
    assert_eq!(stores.len(), 2);
    assert!(!is_clock_enabled(stores[0]));
}

#[test]
fn a_rate_is_refused_where_no_divider_reaches_it_or_no_rated_clock_exists() {
    let mut f = radon_like();
    let card = Device::at(0x1602_0000).unwrap();
    // 594 MHz over 15 is 39.6 MHz, so 1 MHz cannot be reached.
    assert_eq!(
        card.set_rate(&mut f, 1_000_000),
        Err(RateRefusal::NoRateAtOrBelow)
    );
    assert_eq!(
        gmac().set_rate(&mut f, 50_000_000),
        Err(RateRefusal::NoRatedClock)
    );
    assert!(f.writes().is_empty());
    f.held[slot(WindowName::SystemController)] = false;
    assert_eq!(
        card.set_rate(&mut f, 50_000_000),
        Err(RateRefusal::NoRatedClock)
    );
}
