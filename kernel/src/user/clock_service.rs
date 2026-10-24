use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where the service expects its two mappings. Must match components/src/clock.rs.
///
/// `CLOCK_VA` is public because the set authority lives there, and the NTP tests aim a write at it
/// from a process that holds no such mapping.
pub const CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);
const RTC_VA: u64 = address_space_map::pair_page(0x00d0_0000);

/// What the clock service was wired with, so a test (or a real progenitor) can play its clients.
pub struct Wiring {
    /// The service's startup report.
    pub report: RendezvousId,
    /// The propose endpoint. A holder of `WRITE` here may ask; it may not tell.
    pub propose: RendezvousId,
    /// The clock page's physical frame, so a reader can be given a read-only mapping of it
    /// (and so the kernel's own tests can read it through the direct map).
    pub page_phys: u64,
    /// Which RTC the machine turned out to have, one of `clock_contract::rtc`.
    pub kind: u64,
}

/// Does this machine have an RTC at all?
///
/// Elsewhere the device tree answers (`memory::rtc_region()`); on `x86_64` the kernel's own CMOS
/// reader does, since that RTC is I/O ports the kernel keeps (§121 (what a device capability is
/// when the device has no page)), and no answer means a reading it could not make into a date. A
/// test asks before [`start`], so a skip leaves no service's frames charged to it.
pub fn machine_has_no_rtc() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::rtc::read_unix_nanos().is_none()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        crate::memory::rtc_region().is_none()
    }
}

/// The reason every RTC-dependent test gives when it skips. One string, because four files share
/// one cause and a reader comparing two runs should not have to decide whether two wordings mean
/// the same thing.
pub const NO_RTC: &str = "this machine has no working real-time clock (no RTC binding in its \
                          device tree, or on x86_64 a CMOS reading this kernel could not make \
                          into a real calendar date)";

/// Wire and spawn the clock service.
pub fn start(image: &'static [u8]) -> Wiring {
    // Zeroed: a page nobody has published to reads as `state::UNKNOWN`, not as 1970.
    let page_phys = crate::memory::alloc_zeroed()
        .expect("no frame for the clock page")
        .addr();

    let report = crate::sched::create_rendezvous();
    let propose = crate::sched::create_rendezvous();
    spawn_with(image, page_phys, report, propose).0
}

/// The pages [`start_in`] retypes from its region: the clock page and the two endpoints.
pub const REGION_PAGES: u64 = 3;

/// [`start`], with everything it builds retyped from `region`, and the service's thread beside
/// the wiring, so a caller can hand it all back.
///
/// The service blocks in `RECEIVE` on `propose`, which lives in `region`, so reclaiming the region
/// is what wakes it to spend an armed kill: `Holding`'s ordinary case. Reclaim it only once
/// nothing else reads the clock page, which is in the region too.
pub fn start_in(image: &'static [u8], region: u64) -> (Wiring, crate::thread::ThreadId) {
    let page_phys = crate::memory_region::retype_page(region).expect("no clock page in the region");
    let report = crate::sched::create_rendezvous_from(region).expect("no report endpoint");
    let propose = crate::sched::create_rendezvous_from(region).expect("no propose endpoint");
    spawn_with(image, page_phys, report, propose)
}

fn spawn_with(
    image: &'static [u8],
    page_phys: u64,
    report: RendezvousId,
    propose: RendezvousId,
) -> (Wiring, crate::thread::ThreadId) {
    let rtc = crate::memory::rtc_region();

    // On x86_64 there is no RTC register to map, so the kernel reads the CMOS once and hands the
    // wall clock over as `seed` (§130 (how the kernel-resident CMOS RTC reaches the userspace clock
    // service), option 3); elsewhere the driver polls its own.
    #[cfg(target_arch = "x86_64")]
    let (kind, seed) = match crate::arch::rtc::read_unix_nanos() {
        Some(nanos) => (clock_contract::rtc::CMOS, nanos),
        None => (clock_contract::rtc::NONE, 0u64),
    };
    #[cfg(not(target_arch = "x86_64"))]
    let (kind, seed) = (rtc.map_or(clock_contract::rtc::NONE, |(_, _, k)| k), 0u64);

    // The RTC page is mapped with no `DeviceFrame` capability, as the console's UART is: the
    // service never delegates it, and §41 (a device is revoked by taking it back)'s revocation
    // reaches the mapping either way.
    let mut maps = [
        Mapping {
            va: CLOCK_VA,
            phys: page_phys,
            flags: Flags::user_data(), // read/WRITE: the service is a setter
        },
        Mapping {
            va: RTC_VA,
            phys: 0,
            flags: Flags::user_device(),
        },
    ];
    let n_maps = match rtc {
        Some((phys, _, _)) => {
            maps[1].phys = phys;
            2
        }
        None => 1,
    };

    let tid = crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: kind, // which register layout the MACHINE has, not which ISA we are
                arg1: seed, // x86_64/CMOS only: the wall clock the kernel already read; 0 elsewhere
                arg2: 0,
                grants: &[
                    rendezvous_cap(propose, Rights::READ), // slot 0: serve proposals
                    rendezvous_cap(report, Rights::WRITE), // slot 1: the startup verdict
                ],
                maps: &maps[..n_maps],
            },
        )
    })
    .expect("could not spawn the clock service");

    (
        Wiring {
            report,
            propose,
            page_phys,
            kind,
        },
        tid,
    )
}

impl Wiring {
    /// The clock page as the kernel sees it, through the direct map. A reader process would
    /// hold a read-only mapping instead; the seqlock and the layout are the same either way,
    /// because they come from the one contract crate.
    pub fn page(&self) -> clock_contract::ClockPage {
        // SAFETY: a frame this module allocated and still owns, named through the direct map.
        unsafe { clock_contract::ClockPage::new(mmu::phys_to_virt(self.page_phys)) }
    }

    /// Wall-clock nanoseconds as a reader would compute them: the page's offset plus the
    /// monotonic counter. 0 when the machine does not know.
    pub fn wall_nanos(&self) -> u64 {
        let r = self.page().read();
        if clock_contract::state::is_known(r.state) {
            clock_contract::wall_nanos(r.offset_nanos, monotonic_nanos())
        } else {
            0
        }
    }

    /// Play a proposer: `CALL` the propose endpoint. Returns `(status, wall_nanos_after)`.
    pub fn propose_nanos(&self, unix_nanos: u64) -> (u64, u64) {
        let r = crate::sched::ipc_call(
            self.propose,
            [
                clock_contract::propose::req(clock_contract::propose::PROPOSE),
                unix_nanos,
            ],
        );
        (r[0], r[1])
    }
}

/// Monotonic nanoseconds since boot, the kernel's copy of the reader's arithmetic. Split into
/// seconds and a remainder for the same reason the service's version is: the naive
/// `ticks * 1_000_000_000` overflows a `u64` a few minutes into a boot.
pub fn monotonic_nanos() -> u64 {
    let freq = crate::arch::timer::frequency();
    let ticks = crate::arch::timer::now();
    let secs = ticks / freq;
    let rem = ticks % freq;
    secs * clock_contract::NANOS_PER_SEC + rem * clock_contract::NANOS_PER_SEC / freq
}
