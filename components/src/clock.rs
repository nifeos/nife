//! The clock service, sole owner of the RTC and the wall clock's offset (DECISIONS §43 (reading
//! the clock is a page, setting it is a page you may write, proposing is an endpoint)).
//!
//! Its whole authority is the propose endpoint (slot 0, RECEIVE), a report endpoint (slot 1,
//! WRITE), the clock page read/write and the RTC's registers. A compromised clock service is a
//! machine that believes the wrong time, no more. It stamps the page `UNKNOWN`, reads the RTC once
//! (the driver the device tree names in `a0`, or on `x86_64` the kernel's CMOS reading in `a1`,
//! §121 (x86 port I/O), §130 (the CMOS RTC reaches the userspace clock service)), publishes only a
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

fn start(rtc_kind: u64, rtc_seed: u64, _a2: u64) -> ! {
    // SAFETY: the wiring maps the clock page read/write at CLOCK_VA before this program runs, and
    // nothing unmaps it. We are the first to touch it, so stamping it here cannot race a reader
    // that has already seen the magic.
    let page = unsafe { ClockPage::new(CLOCK_VA) };
    page.init();

    // The RTC, read exactly once. A second reading would not be more correct: from here on the
    // monotonic counter is the thing that advances, and re-reading the RTC would only import its
    // drift and its coarse resolution into a clock that already has better. `rtc_seed` is only
    // meaningful for `rtc::CMOS`, where it is the kernel's own reading rather than a register to
    // poll; every other kind ignores it.
    if let Some(unix_nanos) = read_rtc(rtc_kind, rtc_seed)
        && policy::is_plausible(unix_nanos)
    {
        page.publish(
            state::RTC,
            clock_contract::offset_for(unix_nanos, monotonic_nanos()),
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
        let Some(cap) = req.delivered.into_reply() else {
            // A plain SEND, or a SEND_CAP, on a CALL-only contract. Nobody is waiting for an
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
                        clock_contract::offset_for(w1, monotonic_nanos()),
                    );
                }
                reply(cap, verdict, wall_now(&page));
            }
            propose::STATE => {
                reply(cap, page.read().state, wall_now(&page));
            }
            _ => {
                reply(cap, status::BAD_REQUEST, 0);
            }
        }
    }
}

/// Wall-clock nanoseconds now, or 0 when the machine does not know. **Zero is not a time here**, it
/// is the absence of one, and the state word next to it is what says which.
fn wall_now(page: &ClockPage) -> u64 {
    let r = page.read();
    if state::is_known(r.state) {
        clock_contract::wall_nanos(r.offset_nanos, monotonic_nanos())
    } else {
        0
    }
}

/// Monotonic nanoseconds since boot, from the ambient counter.
///
/// Split into whole seconds and a remainder rather than multiplying the raw tick count by a
/// billion: at 62.5 MHz the naive product overflows a `u64` after about five minutes of uptime, and
/// the resulting wall clock would jump backwards by six centuries at the wrap.
fn monotonic_nanos() -> u64 {
    let freq = cntfrq();
    let ticks = now();
    let secs = ticks / freq;
    let rem = ticks % freq;
    secs * clock_contract::NANOS_PER_SEC + rem * clock_contract::NANOS_PER_SEC / freq
}

/// Read the machine's RTC, whichever one it has. `None` when it has none.
///
/// **Both mapped-register drivers are compiled on every ISA.** Neither `pl031_unix_nanos` nor
/// `goldfish_unix_nanos` is behind a `cfg(target_arch)`, because the device is a fact about the
/// board and not about the instruction set: the VisionFive 2 is riscv64 and has neither of these.
/// Keying the layout off the ISA would compile clean and read garbage on the first real board,
/// which is precisely the failure rule 5 exists to catch. `rtc::CMOS` needs no such driver at all:
/// there is no register at `RTC_VA` on `x86_64` (no mapping is ever made for it), and `seed` is
/// already the converted answer the kernel read from CMOS before this program ran.
fn read_rtc(kind: u64, seed: u64) -> Option<u64> {
    match kind {
        rtc::PL031 => Some(pl031_unix_nanos(RTC_VA)),
        rtc::GOLDFISH => Some(goldfish_unix_nanos(RTC_VA)),
        rtc::CMOS => Some(seed),
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
fn pl031_unix_nanos(base: u64) -> u64 {
    const DR: u64 = 0x00;
    // SAFETY: `base` is this process's device mapping of a PL031, placed by whoever built us.
    let secs = unsafe { core::ptr::read_volatile((base + DR) as *const u32) } as u64;
    secs * clock_contract::NANOS_PER_SEC
}

/// **The Goldfish RTC driver** (`google,goldfish-rtc`, QEMU `virt` on riscv64). Same rule 2 shape:
/// a base address and nothing else.
///
/// Two 32-bit registers holding **nanoseconds** since the Unix epoch, and the order of the two
/// reads is part of the device's contract rather than a preference: reading `TIME_LOW` **latches**
/// `TIME_HIGH`, so `TIME_HIGH` must be read second. Reading them the other way round is correct
/// except across the low word's wrap, which is once every four seconds and presents as a
/// four-second jump in a clock that is otherwise fine, i.e. the worst kind of bug to have.
fn goldfish_unix_nanos(base: u64) -> u64 {
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

user_mode_runtime::panic_handler!();
