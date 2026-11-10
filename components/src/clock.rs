//! The clock service, sole owner of the RTC and the wall clock's offset (DECISIONS §43 (reading
//! the clock is a page, setting it is a page you may write, proposing is an endpoint)).
//!
//! Its whole authority is the propose endpoint (slot 0, RECEIVE), a report endpoint (slot 1,
//! WRITE), the clock page read/write and the RTC's registers. A compromised clock service is a
//! machine that believes the wrong time, no more. It stamps the page `UNKNOWN`, reads the RTC once
//! (the driver `a0` names: a mapped PL031 or Goldfish, or on `x86_64` the CMOS through a `PortRange`
//! over its two ports, §121 (x86 port I/O), §130 (the CMOS RTC and the
//! userspace clock service)), publishes only a
//! plausible reading (§42 (a filesystem must be truthful)'s no-silent-degradation rule), then
//! serves proposals judged by `clock_contract::policy`. See notes/clock.md, "Three authorities".

//! Name: ratified 2026-09-14 (calef, working the unratified worklist), confirming milestone 63's
//! name table (design/roadmap/0063-name-spellings.md). Introduced 2026-07-30. Milestone 63 argues
//! this name from the other side, while settling `credentialer`: it treats `clock` and `entropy`
//! as the established resource-name pattern and departs from that pattern only where the departure
//! is earned, because the credential service will never hand you a credential and naming it for
//! the resource would imply the one thing it exists to refuse. A clock service does hand you the
//! time, so the pattern holds and 63's exception does not reach it. That is a real argument for
//! this name, made in the tree, by a lane that was arguing about a different one.
//!
//! **The pair was ratified together**, `entropy` the same day, because 63 cites the two in one
//! sentence as the pattern; signing one and not the other would have split an argument the tree
//! makes as a unit. The strong form of the pattern is visible here: the name says what this hands
//! you, and what it conspicuously does not hand you is the ability to set it, which is three
//! objects apart (read the page, write the page, propose) rather than one service's options.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use clock_contract::{ClockPage, policy, propose, rtc, state, status};
use user_mode_runtime::{cntfrq, now, receive_request, reply, send};

/// The propose endpoint (slot 0): the service receives proposals on it. Everything that arrives here
/// is a request, never a command.
const PROPOSE_EP: u64 = 0;
/// The report endpoint (slot 1): one message at startup saying what the machine knows. Optional;
/// a wiring that grants no such endpoint gets a refused `SEND` the service ignores.
const REPORT: u64 = 1;

/// The clock page, mapped **read/write**: this process is a setter.
const CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);
/// The RTC's registers, device-typed. Only mapped when the machine has an RTC we can drive; `a0`
/// says which, and [`rtc::NONE`] means this address is not mapped and must not be touched.
const RTC_VA: u64 = address_space_map::pair_page(0x00d0_0000);

/// The startup report's first word, so a reader of the endpoint knows this is the clock speaking.
const RPT_READY: u64 = 0x_c10c_c0de;

user_mode_runtime::program_entry!(start);

fn start(rtc_kind: u64, _a1: u64, _a2: u64) -> ! {
    // SAFETY: the wiring maps the clock page read/write at CLOCK_VA before this program runs, and
    // nothing unmaps it. We are the first to touch it, so stamping it here cannot race a reader
    // that has already seen the magic.
    let page = unsafe { ClockPage::new(CLOCK_VA) };
    page.initialize();

    // The RTC, read exactly once. A second reading would not be more correct: from here on the
    // monotonic counter is the thing that advances, and re-reading the RTC would only import its
    // drift and its coarse resolution into a clock that already has better.
    if let Some(unix_nanoseconds) = read_rtc(rtc_kind)
        && policy::is_plausible(unix_nanoseconds)
    {
        page.publish(
            state::RTC,
            clock_contract::offset_for(unix_nanoseconds, monotonic_nanoseconds()),
        );
    }

    let r = page.read();
    send(REPORT, RPT_READY, r.state, wall_now(&page));

    serve(page)
}

/// The serve loop: one endpoint, one wait point, forever.
fn serve(page: ClockPage) -> ! {
    loop {
        let req = receive_request(PROPOSE_EP);
        let (w0, w1) = (req.w0, req.w1);
        let Some(capability) = req.delivered.into_reply() else {
            // A plain SEND, or a SEND_CAPABILITY, on a CALL-only contract. Nobody is waiting for an
            // answer, so there is nothing to do and nothing to report; drop it (a delegation's slot
            // is deleted by `into_reply`, milestone 706 (a `CALL` server can tell a Reply from a
            // delegation)) rather than replying into a slot that is not a Reply.
            continue;
        };
        match propose::operation(w0) {
            propose::PROPOSE => {
                let r = page.read();
                let current = wall_now(&page);
                let verdict = policy::decide(r.state, current, w1);
                if verdict == status::ACCEPTED {
                    // SYNCED rather than SET: this came from a source the service bounded, and the
                    // provenance is what a caller weighing a certificate expiry actually wants.
                    page.publish(
                        state::SYNCED,
                        clock_contract::offset_for(w1, monotonic_nanoseconds()),
                    );
                }
                reply(capability, verdict, wall_now(&page));
            }
            propose::STATE => {
                reply(capability, page.read().state, wall_now(&page));
            }
            _ => {
                reply(capability, status::BAD_REQUEST, 0);
            }
        }
    }
}

/// Wall-clock nanoseconds now, or 0 when the machine does not know. **Zero is not a time here**, it
/// is the absence of one, and the state word next to it is what says which.
fn wall_now(page: &ClockPage) -> u64 {
    let r = page.read();
    if state::is_known(r.state) {
        clock_contract::wall_nanoseconds(r.offset_nanos, monotonic_nanoseconds())
    } else {
        0
    }
}

/// Monotonic nanoseconds since boot, from the ambient counter.
///
/// Split into whole seconds and a remainder rather than multiplying the raw tick count by a
/// billion: at 62.5 MHz the naive product overflows a `u64` after about five minutes of uptime, and
/// the resulting wall clock would jump backwards by six centuries at the wrap.
fn monotonic_nanoseconds() -> u64 {
    let freq = cntfrq();
    let ticks = now();
    let seconds = ticks / freq;
    let rem = ticks % freq;
    seconds * clock_contract::NS_PER_SECOND + rem * clock_contract::NS_PER_SECOND / freq
}

/// Read the machine's RTC, whichever one it has. `None` when it has none, or when its reading is
/// not a calendar date.
///
/// **Both mapped-register drivers are compiled on every ISA.** Neither `pl031_unix_ns` nor
/// `goldfish_unix_ns` is behind a `cfg(target_arch)`, because the device is a fact about the
/// board and not about the instruction set: the VisionFive 2 is riscv64 and has neither of these.
/// Keying the layout off the ISA would compile clean and read garbage on the first real board,
/// which is precisely the failure rule 5 exists to catch. `rtc::CMOS` is the exception the ISA
/// does force: port I/O exists only on `x86_64`, so the `cmos` module compiles only there.
fn read_rtc(kind: u64) -> Option<u64> {
    match kind {
        rtc::PL031 => Some(pl031_unix_ns(RTC_VA)),
        rtc::GOLDFISH => Some(goldfish_unix_ns(RTC_VA)),
        #[cfg(target_arch = "x86_64")]
        rtc::CMOS => cmos::read_unix_ns(),
        _ => None,
    }
}

/// **The PL031 driver** (`arm,pl031`, QEMU `virt` on aarch64). It takes a base address and knows
/// nothing else, which is the project's rule 2 in its smallest possible form.
///
/// One 32-bit register at offset 0, `DR`, holding **seconds** since the Unix epoch. The other three
/// registers (match, load, control) are for the alarm interrupt, which nothing here wants: this
/// service reads the clock once at startup and lets the monotonic counter carry the time after
/// that. `DR` is 32 bits wide, so this device's own epoch runs out in 2106; the wire format here is
/// nanoseconds in a `u64`, which runs out in 2554, and neither is a horizon worth engineering for.
fn pl031_unix_ns(base: u64) -> u64 {
    const DR: u64 = 0x00;
    // SAFETY: `base` is this process's device mapping of a PL031, placed by whoever built us.
    let seconds = unsafe { core::ptr::read_volatile((base + DR) as *const u32) } as u64;
    seconds * clock_contract::NS_PER_SECOND
}

/// **The Goldfish RTC driver** (`google,goldfish-rtc`, QEMU `virt` on riscv64). Same rule 2 shape:
/// a base address and nothing else.
///
/// Two 32-bit registers holding **nanoseconds** since the Unix epoch, and the order of the two
/// reads is part of the device's contract rather than a preference: reading `TIME_LOW` **latches**
/// `TIME_HIGH`, so `TIME_HIGH` must be read second. Reading them the other way round is correct
/// except across the low word's wrap, which is once every four seconds and presents as a
/// four-second jump in a clock that is otherwise fine, i.e. the worst kind of bug to have.
fn goldfish_unix_ns(base: u64) -> u64 {
    const TIME_LOW: u64 = 0x00;
    const TIME_HIGH: u64 = 0x04;
    // SAFETY: `base` is this process's device mapping of a Goldfish RTC, placed by whoever built
    // us. The read order is the device's latching contract, not an optimisation.
    unsafe {
        let lo = core::ptr::read_volatile((base + TIME_LOW) as *const u32) as u64;
        let hi = core::ptr::read_volatile((base + TIME_HIGH) as *const u32) as u64;
        (hi << 32) | lo
    }
}

/// **The CMOS RTC driver** (`x86_64`): an MC146818-compatible clock behind
/// [`rtc::INDEX_PORT`] and [`rtc::DATA_PORT`], reached through the `PortRange` the
/// wiring grants in slot 2. See notes/clock.md for the register map, tearing and the century.
///
/// # BUGS
/// - No century byte: the year is `2000 + value`, so dates outside 2000..=2099 cannot be read.
/// - Daylight saving and the alarm and periodic-interrupt registers are untouched.
/// - Not proven against real hardware, only QEMU's MC146818A model.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/cmos-read-in-the-clock-service for
/// milestone 1062 (the device control planes leave the kernel), step 6: the device's own name, as
/// `pl031_unix_ns` and `goldfish_unix_ns` carry theirs.
#[cfg(target_arch = "x86_64")]
mod cmos {
    use clock_contract::rtc::{DATA_PORT, INDEX_PORT};
    use user_mode_runtime::{inb, outb};

    const REGISTER_SECONDS: u8 = 0x00;
    const REGISTER_MINUTES: u8 = 0x02;
    const REGISTER_HOURS: u8 = 0x04;
    const REGISTER_DAY: u8 = 0x07;
    const REGISTER_MONTH: u8 = 0x08;
    const REGISTER_YEAR: u8 = 0x09;
    const REGISTER_STATUS_A: u8 = 0x0A;
    const REGISTER_STATUS_B: u8 = 0x0B;

    /// Status register A, bit 7: the RTC is mid-update and every other register is unreliable.
    const STATUS_A_UPDATE_IN_PROGRESS: u8 = 1 << 7;
    /// Status register B, bit 2: fields are binary (set) rather than BCD (clear).
    const STATUS_B_BINARY: u8 = 1 << 2;
    /// Status register B, bit 1: the hours field is 24-hour (set) rather than 12-hour with a PM
    /// flag.
    const STATUS_B_24_HOUR: u8 = 1 << 1;
    /// The hours register's bit 7 in 12-hour mode: set for PM. In 24-hour mode it reads clear.
    const HOUR_PM: u8 = 1 << 7;

    /// Read one CMOS register: name it at the index port, then read the data port.
    fn read(register: u8) -> u8 {
        outb(INDEX_PORT, register);
        inb(DATA_PORT)
    }

    fn is_update_in_progress() -> bool {
        read(REGISTER_STATUS_A) & STATUS_A_UPDATE_IN_PROGRESS != 0
    }

    /// One look at the six date and time registers, taken together so [`read_unix_ns`]'s tearing
    /// check can compare two of them for equality.
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Snapshot {
        second: u8,
        minute: u8,
        hour: u8,
        day: u8,
        month: u8,
        year: u8,
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            second: read(REGISTER_SECONDS),
            minute: read(REGISTER_MINUTES),
            hour: read(REGISTER_HOURS),
            day: read(REGISTER_DAY),
            month: read(REGISTER_MONTH),
            year: read(REGISTER_YEAR),
        }
    }

    /// One BCD byte (two packed decimal digits) to binary.
    fn bcd_to_binary(v: u8) -> u8 {
        (v & 0x0F) + 10 * (v >> 4)
    }

    /// Read the CMOS once and return the wall clock it reports, in nanoseconds since the Unix
    /// epoch.
    ///
    /// `None` when the reading is not a real calendar date. Every field zero, which is what a
    /// CMOS with a dead battery and no host to seed it reads as, is the case this exists to catch
    /// (`calendar::Civil::new` refuses month 0 and day 0). The caller still runs a date through
    /// `policy::is_plausible`, as it does every RTC's.
    pub(super) fn read_unix_ns() -> Option<u64> {
        let raw = loop {
            while is_update_in_progress() {
                core::hint::spin_loop();
            }
            let first = snapshot();
            while is_update_in_progress() {
                core::hint::spin_loop();
            }
            let second = snapshot();
            if first == second {
                break first;
            }
            // An update began between the snapshots; one UIP wait alone is not enough
            // (notes/clock.md, "Tearing").
        };

        let status_b = read(REGISTER_STATUS_B);
        let binary = status_b & STATUS_B_BINARY != 0;
        let twenty_four_hour = status_b & STATUS_B_24_HOUR != 0;

        // The PM flag shares the hours byte with the hour in every mode, so it comes off before
        // either decode.
        let pm = !twenty_four_hour && raw.hour & HOUR_PM != 0;
        let hour_field = raw.hour & !HOUR_PM;

        let decode = |v: u8| if binary { v } else { bcd_to_binary(v) };
        let (second, minute, hour, day, month, year) = (
            decode(raw.second),
            decode(raw.minute),
            decode(hour_field),
            decode(raw.day),
            decode(raw.month),
            decode(raw.year),
        );

        let hour = match (twenty_four_hour, pm, hour) {
            (true, _, h) => h,
            (false, true, 12) => 12, // 12 PM is noon, unchanged
            (false, true, h) => h + 12,
            (false, false, 12) => 0, // 12 AM is midnight
            (false, false, h) => h,
        };

        // No century byte; see notes/clock.md.
        let year = 2000 + i32::from(year);

        let civil = calendar::Civil::new(year, month, day, hour, minute, second).ok()?;
        let seconds = u64::try_from(civil.to_unix()).ok()?;
        Some(seconds * clock_contract::NS_PER_SECOND)
    }
}

user_mode_runtime::panic_handler!();
