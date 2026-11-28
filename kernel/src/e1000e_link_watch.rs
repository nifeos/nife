//! **The bench boot's link watch on a PCH part**: when `STATUS.LU` has not risen within
//! [`crate::e1000e::LINK_WAIT_MILLISECONDS`], keep reading both sides of the link for
//! [`WATCH_MILLISECONDS`] more and say which side never saw it. Only the `network_bench` build runs
//! it ([`crate::e1000e::bring_up`]).
//!
//! Each poll reads the MAC's `STATUS` and the PHY's status register twice (its link bit latches
//! low, so the first read reports a drop since the last poll and the second the link now), and
//! prints a line only when what it read changed. Times are milliseconds since
//! `initialize_hardware` returned. The verdict line names which explanation of a missed link
//! wait the observations fit: (a) the wait was too short, (b) the PHY has link and the MAC never
//! hears of it, (c) the PHY never has link (cable, switch port, or the PHY's own setup).
//!
//! The watch writes no configuration. Its only writes are the ones a PHY read is made of: the
//! `MDIC` command and taking and releasing `EXTCNF_CTRL.SWFLAG`, which the crate's
//! [`physical_layer_device::read`] requires so the Management Engine is not on the bus at the same
//! time. Every PHY register it reads is on page 0 below register 16, so no page-select write
//! happens either. See nifeos/record:notes/e1000e.md.
//!
//! Name: provisional.
//!
//! # BUGS
//! - The PHY and MAC bit positions below are IEEE 802.3 clause 22's and FreeBSD's, spelled here
//!   because `partsfoundry/e1000e` keeps them crate-private or does not name them. They belong in
//!   the crate; until it exports them, this module is the exception to "every offset is the
//!   crate's" in [`super`].
//! - The PHY-specific status register of the 82579/I217-family PHY (FreeBSD's `HV_M_STATUS`, page
//!   0 register 26) is not read: the crate does not name it, and a register above 15 takes a
//!   page-select write.

use ::e1000e::platform_controller_hub::Hardware;
use ::e1000e::platform_controller_hub::physical_layer_device::{self, Error, SoftwareFlag};
use ::e1000e::registers;

use crate::e1000e::LINK_WAIT_MILLISECONDS;

/// How long the watch runs after the link wait gives up.
pub(crate) const WATCH_MILLISECONDS: u64 = 30_000;
/// The gap between two polls.
const POLL_MILLISECONDS: u64 = 100;

/// MII register 1, the basic mode status register (OpenBSD's `PHY_STATUS`, `if_em_hw.h`).
#[doc(alias = "PHY_STATUS")]
#[doc(alias = "BMSR")]
const STATUS_REGISTER: u32 = 1;
/// MII register 4, our autonegotiation advertisement (OpenBSD's `PHY_AUTONEG_ADV`).
#[doc(alias = "PHY_AUTONEG_ADV")]
const ADVERTISEMENT_REGISTER: u32 = 4;
/// MII register 5, the link partner's base page ability (OpenBSD's `PHY_LP_ABILITY`).
#[doc(alias = "PHY_LP_ABILITY")]
const LINK_PARTNER_ABILITY_REGISTER: u32 = 5;
/// MII register 9, our 1000BASE-T advertisement (OpenBSD's `PHY_1000T_CTRL`).
#[doc(alias = "PHY_1000T_CTRL")]
const GIGABIT_CONTROL_REGISTER: u32 = 9;
/// MII register 10, the 1000BASE-T status (OpenBSD's `PHY_1000T_STATUS`).
#[doc(alias = "PHY_1000T_STATUS")]
const GIGABIT_STATUS_REGISTER: u32 = 10;

/// Link status in register 1 (OpenBSD's `MII_SR_LINK_STATUS`). Latches low.
#[doc(alias = "MII_SR_LINK_STATUS")]
const STATUS_LINK: u16 = 1 << 2;
/// Autonegotiation complete in register 1 (OpenBSD's `MII_SR_AUTONEG_COMPLETE`).
#[doc(alias = "MII_SR_AUTONEG_COMPLETE")]
const STATUS_AUTONEGOTIATION_COMPLETE: u16 = 1 << 5;

/// The `STATUS` bits a change line reports: full duplex (`E1000_STATUS_FD`), link up and the
/// speed field (`E1000_STATUS_SPEED_MASK`, bits 7:6).
#[doc(alias = "E1000_STATUS_FD")]
const MAC_FULL_DUPLEX: u32 = 1 << 0;
const MAC_SPEED_SHIFT: u32 = 6;
const MAC_WATCHED: u32 =
    MAC_FULL_DUPLEX | ::e1000e::DEVICE_STATUS_LINK_UP | 0b11 << MAC_SPEED_SHIFT;

/// `CTRL.FRCSPD` and `CTRL.FRCDPLX` (FreeBSD's `E1000_CTRL_FRCSPD`, `E1000_CTRL_FRCDPLX`).
#[doc(alias = "E1000_CTRL_FRCSPD")]
const CONTROL_FORCE_SPEED: u32 = 1 << 11;
#[doc(alias = "E1000_CTRL_FRCDPLX")]
const CONTROL_FORCE_DUPLEX: u32 = 1 << 12;

/// Watch until `STATUS.LU` rises or [`WATCH_MILLISECONDS`] pass, printing each change, then the
/// verdict. `since` is the timer count when `initialize_hardware` returned. True when `STATUS.LU`
/// rose, so the bench carries on to its lease.
pub(crate) fn watch(hw: &mut impl Hardware, since: u64) -> bool {
    let at = || ticks_to_milliseconds(crate::arch::timer::now().saturating_sub(since));
    crate::println!(
        "  e1000e: link watch: STATUS.LU not set after {LINK_WAIT_MILLISECONDS} ms; watching up to \
         {WATCH_MILLISECONDS} ms more, every {POLL_MILLISECONDS} ms, times from the end of \
         initialize_hardware"
    );
    let control = hw.read(registers::DEVICE_CONTROL_REGISTER);
    let phy_control = read_phy(
        hw,
        [physical_layer_device::registers::PHYSICAL_LAYER_DEVICE_CONTROL_REGISTER],
    )
    .map(|[v]| v);
    crate::println!(
        "{}",
        Start {
            at: at(),
            control,
            phy_control
        }
    );
    let mut w = Watch::default();
    let end = at() + WATCH_MILLISECONDS;
    loop {
        let now = at();
        let sample = Sample {
            status: hw.read(registers::DEVICE_STATUS_REGISTER),
            phy_status: read_phy(hw, [STATUS_REGISTER, STATUS_REGISTER]),
        };
        let seen = w.observe(now, &sample);
        if seen.changed {
            crate::println!(
                "{}",
                Change {
                    at: now,
                    sample: &sample
                }
            );
        }
        if seen.autonegotiation_completed {
            let abilities = read_phy(
                hw,
                [
                    ADVERTISEMENT_REGISTER,
                    LINK_PARTNER_ABILITY_REGISTER,
                    GIGABIT_CONTROL_REGISTER,
                    GIGABIT_STATUS_REGISTER,
                ],
            )
            .map(Abilities::from);
            if let Ok(a) = abilities {
                w.resolved = a.resolved();
            }
            crate::println!("{}", Negotiated { at: now, abilities });
        }
        if w.mac_link_at.is_some() || now >= end {
            break;
        }
        hw.delay_microseconds(POLL_MILLISECONDS * 1000);
    }
    crate::println!("{}", w.verdict());
    w.mac_link_at.is_some()
}

fn ticks_to_milliseconds(ticks: u64) -> u64 {
    ticks.saturating_mul(1000) / crate::arch::timer::frequency().max(1)
}

/// Read page-0 PHY registers in order under one hold of the software flag.
fn read_phy<const N: usize>(hw: &mut impl Hardware, which: [u32; N]) -> Result<[u16; N], Error> {
    let flag = SoftwareFlag::acquire(hw)?;
    let mut out = [0; N];
    let mut result = Ok(());
    for (o, &register) in out.iter_mut().zip(&which) {
        match physical_layer_device::read(hw, &flag, register) {
            Ok(v) => *o = v,
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    flag.release(hw);
    result.map(|()| out)
}

/// The bits a change line is about: the watched `STATUS` bits, then register 1's first read's link
/// bit and its second read's link and autonegotiation-complete bits.
type Key = (u32, Result<(u16, u16), Error>);

/// One poll: `STATUS`, and register 1 read twice (first, then second), or why the PHY read failed.
#[derive(Debug, Clone, Copy)]
struct Sample {
    status: u32,
    phy_status: Result<[u16; 2], Error>,
}

impl Sample {
    fn key(&self) -> Key {
        (
            self.status & MAC_WATCHED,
            self.phy_status.map(|[a, b]| {
                (
                    a & STATUS_LINK,
                    b & (STATUS_LINK | STATUS_AUTONEGOTIATION_COMPLETE),
                )
            }),
        )
    }
}

/// The MAC's speed field as Mbit/s.
fn mac_speed(status: u32) -> u32 {
    match (status >> MAC_SPEED_SHIFT) & 0b11 {
        0 => 10,
        1 => 100,
        _ => 1000,
    }
}

/// What one poll changed.
#[derive(Debug, Default, PartialEq, Eq)]
struct Seen {
    /// The watched bits differ from the last poll's, or this is the first poll.
    changed: bool,
    /// Register 1's autonegotiation-complete bit is set for the first time in the watch.
    autonegotiation_completed: bool,
}

/// What the watch has seen so far.
#[derive(Debug, Default)]
struct Watch {
    last: Option<Key>,
    last_status: u32,
    phy_link_at: Option<u64>,
    mac_link_at: Option<u64>,
    autonegotiation_at: Option<u64>,
    /// The speed and duplex the PHY's registers resolve to, once autonegotiation completed.
    resolved: Option<(u32, bool)>,
    polls: u32,
    phy_failures: u32,
    last_failure: Option<Error>,
    at: u64,
}

impl Watch {
    fn observe(&mut self, at: u64, s: &Sample) -> Seen {
        self.polls += 1;
        self.at = at;
        self.last_status = s.status;
        if s.status & ::e1000e::DEVICE_STATUS_LINK_UP != 0 {
            self.mac_link_at.get_or_insert(at);
        }
        let mut autonegotiation_completed = false;
        match s.phy_status {
            Ok([_, now]) => {
                if now & STATUS_LINK != 0 {
                    self.phy_link_at.get_or_insert(at);
                }
                if now & STATUS_AUTONEGOTIATION_COMPLETE != 0 && self.autonegotiation_at.is_none() {
                    self.autonegotiation_at = Some(at);
                    autonegotiation_completed = true;
                }
            }
            Err(e) => {
                self.phy_failures += 1;
                self.last_failure = Some(e);
            }
        }
        let key = s.key();
        let changed = self.last != Some(key);
        self.last = Some(key);
        Seen {
            changed,
            autonegotiation_completed,
        }
    }

    fn verdict(&self) -> Verdict<'_> {
        Verdict(self)
    }
}

/// The opening line: `CTRL` and the PHY's control register, once.
struct Start {
    at: u64,
    control: u32,
    phy_control: Result<u16, Error>,
}

impl core::fmt::Display for Start {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let c = self.control;
        let bit = |m: u32| u32::from(c & m != 0);
        write!(
            f,
            "  e1000e: link watch +{} ms: CTRL {c:#010x} (SLU {}, ASDE {}, FRCSPD {}, FRCDPLX {}); ",
            self.at,
            bit(::e1000e::DEVICE_CONTROL_SET_LINK_UP),
            bit(::e1000e::DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE),
            bit(CONTROL_FORCE_SPEED),
            bit(CONTROL_FORCE_DUPLEX),
        )?;
        match self.phy_control {
            Ok(v) => {
                let bit = |n: u32| (v >> n) & 1;
                write!(
                    f,
                    "PHY control {v:#06x} (reset {}, autonegotiation {}, power down {}, isolate {}, \
                     restart {})",
                    bit(15),
                    bit(12),
                    bit(11),
                    bit(10),
                    bit(9),
                )
            }
            Err(e) => write!(f, "PHY control unread: {e}"),
        }
    }
}

/// A change line: both sides as one poll read them.
struct Change<'a> {
    at: u64,
    sample: &'a Sample,
}

impl core::fmt::Display for Change<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = self.sample.status;
        write!(
            f,
            "  e1000e: link watch +{} ms: MAC STATUS {s:#010x} (LU {}, {} Mbit/s, {} duplex); ",
            self.at,
            u32::from(s & ::e1000e::DEVICE_STATUS_LINK_UP != 0),
            mac_speed(s),
            if s & MAC_FULL_DUPLEX != 0 {
                "full"
            } else {
                "half"
            },
        )?;
        match self.sample.phy_status {
            Ok([first, now]) => write!(
                f,
                "PHY status {first:#06x} then {now:#06x} (link {}, latched {}, autonegotiation \
                 complete {})",
                u16::from(now & STATUS_LINK != 0),
                u16::from(first & STATUS_LINK != 0),
                u16::from(now & STATUS_AUTONEGOTIATION_COMPLETE != 0),
            ),
            Err(e) => write!(f, "PHY status unread: {e}"),
        }
    }
}

/// Registers 4, 5, 9 and 10, read once autonegotiation completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Abilities {
    advertisement: u16,
    partner: u16,
    gigabit_control: u16,
    gigabit_status: u16,
}

impl From<[u16; 4]> for Abilities {
    fn from([advertisement, partner, gigabit_control, gigabit_status]: [u16; 4]) -> Self {
        Abilities {
            advertisement,
            partner,
            gigabit_control,
            gigabit_status,
        }
    }
}

impl Abilities {
    /// The highest speed and duplex both ends advertise, IEEE 802.3 Annex 28B's priority order:
    /// 1000BASE-T full (register 9 bit 9 and register 10 bit 11), then half (bits 8 and 10), then
    /// 100BASE-TX full, 100BASE-TX half, 10BASE-T full, 10BASE-T half (registers 4 and 5, bits 8
    /// down to 5). `None` when they share nothing.
    fn resolved(&self) -> Option<(u32, bool)> {
        let gigabit = (self.gigabit_control >> 8) & (self.gigabit_status >> 10);
        if gigabit & 0b10 != 0 {
            return Some((1000, true));
        }
        if gigabit & 0b01 != 0 {
            return Some((1000, false));
        }
        let common = self.advertisement & self.partner;
        [
            (8, 100, true),
            (7, 100, false),
            (6, 10, true),
            (5, 10, false),
        ]
        .into_iter()
        .find(|&(bit, _, _)| common & 1 << bit != 0)
        .map(|(_, speed, full)| (speed, full))
    }
}

/// The line printed when autonegotiation first completes.
struct Negotiated {
    at: u64,
    abilities: Result<Abilities, Error>,
}

impl core::fmt::Display for Negotiated {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "  e1000e: link watch +{} ms: autonegotiation complete; ",
            self.at
        )?;
        let a = match self.abilities {
            Ok(a) => a,
            Err(e) => return write!(f, "registers 4, 5, 9 and 10 unread: {e}"),
        };
        let g = a.gigabit_status;
        write!(
            f,
            "advertised {:#06x}, partner {:#06x}, 1000BASE-T control {:#06x}, status {g:#06x} \
             (local receiver ok {}, remote receiver ok {}, master/slave fault {}); resolves to ",
            a.advertisement,
            a.partner,
            a.gigabit_control,
            (g >> 13) & 1,
            (g >> 12) & 1,
            (g >> 15) & 1,
        )?;
        match a.resolved() {
            Some((speed, full)) => {
                write!(
                    f,
                    "{speed} Mbit/s {} duplex",
                    if full { "full" } else { "half" }
                )
            }
            None => f.write_str("nothing both ends advertise"),
        }
    }
}

/// The closing line: which explanation the observations fit.
struct Verdict<'a>(&'a Watch);

impl core::fmt::Display for Verdict<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let w = self.0;
        f.write_str("  e1000e: link watch verdict ")?;
        match (w.phy_link_at, w.mac_link_at) {
            (Some(p), Some(m)) => write!(
                f,
                "(a): PHY link first seen at +{p} ms and STATUS.LU at +{m} ms, so the \
                 {LINK_WAIT_MILLISECONDS} ms link wait is too short for this link"
            )?,
            (Some(p), None) => {
                write!(
                    f,
                    "(b): PHY link first seen at +{p} ms and STATUS.LU never rose by +{} ms, so the \
                     MAC is not told of the link (partsfoundry/e1000e issue 11); STATUS last said {} \
                     Mbit/s",
                    w.at,
                    mac_speed(w.last_status),
                )?;
                if let Some((speed, full)) = w.resolved {
                    let duplex = if full { "full" } else { "half" };
                    write!(f, ", the PHY resolved {speed} Mbit/s {duplex} duplex")?;
                }
            }
            (None, None) if w.phy_failures == w.polls => write!(
                f,
                "none of (a), (b), (c): every PHY read failed, so the PHY's side is unknown; \
                 STATUS.LU never rose by +{} ms",
                w.at
            )?,
            (None, None) => {
                write!(
                    f,
                    "(c) or PHY setup: the PHY never reported link by +{} ms (autonegotiation ",
                    w.at
                )?;
                match w.autonegotiation_at {
                    Some(t) => write!(f, "completed at +{t} ms)")?,
                    None => f.write_str("never completed)")?,
                }
            }
            (None, Some(m)) => write!(
                f,
                "none of (a), (b), (c): STATUS.LU rose at +{m} ms and the PHY never reported link"
            )?,
        }
        if let (Some(e), n) = (w.last_failure, w.phy_failures)
            && n != w.polls
        {
            write!(f, "; {n} of {} PHY reads failed, the last: {e}", w.polls)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed buffer to format one line into.
    struct Line {
        bytes: [u8; 512],
        len: usize,
    }

    impl core::fmt::Write for Line {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let end = (self.len + s.len()).min(self.bytes.len());
            self.bytes[self.len..end].copy_from_slice(&s.as_bytes()[..end - self.len]);
            self.len = end;
            Ok(())
        }
    }

    fn assert_line(shown: impl core::fmt::Display, want: &str) {
        let mut l = Line {
            bytes: [0; 512],
            len: 0,
        };
        let _ = core::fmt::write(&mut l, format_args!("{shown}"));
        let got = core::str::from_utf8(&l.bytes[..l.len]).unwrap_or("<not utf-8>");
        assert!(got == want, "got  {got}\nwant {want}");
    }

    const LINK_DOWN: u16 = 0x7949;
    const LINK_UP: u16 = 0x796d;
    /// `STATUS` with `LU` clear and the speed field reading 1000, as after a reset.
    const MAC_DOWN: u32 = 0x0008_0080;
    /// `STATUS` with `LU` set at 100 Mbit/s full duplex.
    const MAC_UP_100_FULL: u32 = 0x0008_0043;

    fn sample(status: u32, phy: [u16; 2]) -> Sample {
        Sample {
            status,
            phy_status: Ok(phy),
        }
    }

    /// The (b) case the cabled run's lights point at: the PHY gets link at 100 Mbit/s and `STATUS`
    /// never follows. Only the polls that change print, autonegotiation's completion is seen once,
    /// and the verdict names (b) with the PHY's time and the speeds the two sides hold.
    #[test_case]
    fn a_phy_link_the_mac_never_sees_is_case_b() {
        let mut w = Watch::default();
        let down = sample(MAC_DOWN, [LINK_DOWN, LINK_DOWN]);
        let up = sample(MAC_DOWN, [LINK_DOWN, LINK_UP]);
        let held = sample(MAC_DOWN, [LINK_UP, LINK_UP]);
        let seen = |changed, autonegotiation_completed| Seen {
            changed,
            autonegotiation_completed,
        };
        assert_eq!(w.observe(3001, &down), seen(true, false));
        assert_eq!(w.observe(3101, &down), seen(false, false));
        assert_eq!(w.observe(4201, &up), seen(true, true));
        assert_line(
            Change {
                at: 4201,
                sample: &up,
            },
            "  e1000e: link watch +4201 ms: MAC STATUS 0x00080080 (LU 0, 1000 Mbit/s, half \
             duplex); PHY status 0x7949 then 0x796d (link 1, latched 0, autonegotiation complete 1)",
        );
        let a = Abilities::from([0x01e1, 0x45e1, 0x0200, 0x0000]);
        w.resolved = a.resolved();
        assert_line(
            Negotiated {
                at: 4201,
                abilities: Ok(a),
            },
            "  e1000e: link watch +4201 ms: autonegotiation complete; advertised 0x01e1, partner \
             0x45e1, 1000BASE-T control 0x0200, status 0x0000 (local receiver ok 0, remote receiver \
             ok 0, master/slave fault 0); resolves to 100 Mbit/s full duplex",
        );
        assert_eq!(w.observe(4301, &held), seen(true, false));
        assert_eq!(w.observe(33001, &held), seen(false, false));
        assert_line(
            w.verdict(),
            "  e1000e: link watch verdict (b): PHY link first seen at +4201 ms and STATUS.LU never \
             rose by +33001 ms, so the MAC is not told of the link (partsfoundry/e1000e issue 11); \
             STATUS last said 1000 Mbit/s, the PHY resolved 100 Mbit/s full duplex",
        );
    }

    /// Both rising, late, is (a) with both times; neither rising is (c); `STATUS.LU` with no PHY
    /// link and a PHY that never answers are each said plainly rather than forced into a case.
    #[test_case]
    fn the_verdict_names_each_case_from_what_rose() {
        let mut a = Watch::default();
        a.observe(3001, &sample(MAC_DOWN, [LINK_DOWN, LINK_UP]));
        a.observe(4501, &sample(MAC_UP_100_FULL, [LINK_UP, LINK_UP]));
        assert_line(
            a.verdict(),
            "  e1000e: link watch verdict (a): PHY link first seen at +3001 ms and STATUS.LU at \
             +4501 ms, so the 3000 ms link wait is too short for this link",
        );
        let mut c = Watch::default();
        c.observe(3001, &sample(MAC_DOWN, [LINK_DOWN, LINK_DOWN]));
        c.observe(
            3101,
            &Sample {
                status: MAC_DOWN,
                phy_status: Err(Error::SoftwareFlagHeld),
            },
        );
        c.observe(33001, &sample(MAC_DOWN, [LINK_DOWN, LINK_DOWN]));
        assert_line(
            c.verdict(),
            "  e1000e: link watch verdict (c) or PHY setup: the PHY never reported link by +33001 ms \
             (autonegotiation never completed); 1 of 3 PHY reads failed, the last: another agent \
             holds the software flag",
        );
        let mut mac_only = Watch::default();
        mac_only.observe(3001, &sample(MAC_UP_100_FULL, [LINK_DOWN, LINK_DOWN]));
        assert_line(
            mac_only.verdict(),
            "  e1000e: link watch verdict none of (a), (b), (c): STATUS.LU rose at +3001 ms and the \
             PHY never reported link",
        );
        let mut unread = Watch::default();
        unread.observe(
            3001,
            &Sample {
                status: MAC_DOWN,
                phy_status: Err(Error::NotReady),
            },
        );
        assert_line(
            unread.verdict(),
            "  e1000e: link watch verdict none of (a), (b), (c): every PHY read failed, so the PHY's \
             side is unknown; STATUS.LU never rose by +3001 ms",
        );
    }

    /// The resolution prefers gigabit only when both ends offer it, and full duplex over half.
    #[test_case]
    fn the_resolution_takes_the_highest_common_ability() {
        let both_gigabit = Abilities::from([0x01e1, 0xc1e1, 0x0200, 0x0800]);
        assert_eq!(both_gigabit.resolved(), Some((1000, true)));
        let partner_ten_half = Abilities::from([0x01e1, 0x0021, 0x0200, 0x0000]);
        assert_eq!(partner_ten_half.resolved(), Some((10, false)));
        assert_eq!(Abilities::from([0x0001, 0x0001, 0, 0]).resolved(), None);
    }

    #[test_case]
    fn the_opening_line_decodes_control_and_the_phy_control() {
        assert_line(
            Start {
                at: 3001,
                control: 0x0018_0261,
                phy_control: Ok(0x1140),
            },
            "  e1000e: link watch +3001 ms: CTRL 0x00180261 (SLU 1, ASDE 1, FRCSPD 0, FRCDPLX 0); \
             PHY control 0x1140 (reset 0, autonegotiation 1, power down 0, isolate 0, restart 0)",
        );
    }
}
