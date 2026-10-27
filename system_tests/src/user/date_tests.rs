use super::*;
use crate::cap::{Rights, page_frame_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where `date` expects the clock page, read-only. Must match components/src/date.rs's `CLOCK_VA`.
const CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);

// The `a0` format selector, and the `a1`/`a2` conventions. Must match components/src/date.rs.
const FMT_HUMAN: u64 = 0;
const FMT_RFC3339: u64 = 1;
const FMT_UNIX: u64 = 4;
const PROVENANCE: u64 = 1;

/// Spawn `date` and return `(the endpoint its output arrives on, the region it was retyped
/// from)`. The caller reads its output, then reclaims the region: a `date` run is one process for
/// one test, and milestone 608 (kernel tests give back their rendezvous points) is what makes that
/// endpoint go with it instead of spending the kernel's rendezvous registry for the rest of the
/// boot.
///
/// `page` is the whole of its clock authority: `Some(phys)` grants the frame with **`READ`**
/// and maps it **read-only**, which is the read rung of §43's ladder and is what makes a
/// `date -s` unbuildable rather than merely absent. `None` grants no clock at all, which is the
/// other unknown-clock cause and a different message.
fn spawn_date(
    page: Option<u64>,
    fmt: u64,
    offset_minutes: i64,
    provenance: u64,
) -> (RendezvousId, u64) {
    let image = program("date").expect("no date program in the initial ramdisk");
    let region = crate::memory_region::create(1).expect("no region for date's stdout");
    let out = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
    crate::sched::spawn(move || match page {
        Some(phys) => run(
            image,
            Spawn {
                arg0: fmt,
                arg1: offset_minutes as u64,
                arg2: provenance,
                grants: &[
                    rendezvous_cap(out, Rights::WRITE), // slot 0: stdout
                    page_frame_cap(phys, Rights::READ), // slot 1: a READER, and nothing more
                ],
                maps: &[Mapping {
                    va: CLOCK_VA,
                    phys,
                    flags: Flags::user_rodata(),
                }],
            },
        ),
        None => run(
            image,
            Spawn {
                arg0: fmt,
                arg1: offset_minutes as u64,
                arg2: provenance,
                grants: &[rendezvous_cap(out, Rights::WRITE)],
                maps: &[],
            },
        ),
    })
    .expect("could not spawn date");
    (out, region)
}

/// **Spawn a `date` that holds a second stream**, and hand back both endpoints (output,
/// diagnostic) plus the region they were retyped from, for the caller to reclaim once it is done
/// reading (see [`spawn_date`]).
///
/// The diagnostic endpoint goes in at the slot `date`'s manifest declares, through `grant_at`, as
/// the real progenitor does with `abi::thread_control_block::CAP_INSERT`'s explicit target: the
/// named slot of DECISIONS §67 (a program's second stream is a declaration, not a number), which
/// does not move with whatever else the child was granted. No clock, deliberately: a `date` granted
/// no clock is one of the two real unknown-clock causes rather than an injected fault, and it
/// reaches the sentence §67 was taken for.
fn spawn_date_with_diagnostics() -> (RendezvousId, RendezvousId, u64) {
    let image = program("date").expect("no date program in the initial ramdisk");
    let region = crate::memory_region::create(2).expect("no region for date's two streams");
    let out = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
    let diag = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
    crate::sched::spawn(move || {
        crate::sched::grant_at(
            grant_plan::DIAGNOSTICS_SLOT,
            rendezvous_cap(diag, Rights::WRITE),
        )
        .expect("date's declared diagnostic slot was already occupied");
        run(
            image,
            Spawn {
                arg0: FMT_HUMAN,
                arg1: 0,
                arg2: 0,
                grants: &[rendezvous_cap(out, Rights::WRITE)],
                maps: &[],
            },
        )
    })
    .expect("could not spawn date");
    (out, diag, region)
}

/// One line of `date`'s output, without its newline.
///
/// The framing is the std PAL's stdout framing (`w0` = the byte count, `w1`|`w2` = the bytes,
/// little-endian), deliberately, so there is one convention for "a program printed something"
/// rather than two. `SEND` blocks until a receiver takes it, so stopping at the newline
/// consumes exactly the messages that line was made of and leaves any following line queued.
fn line(out: RendezvousId, buf: &mut [u8; 128]) -> usize {
    let mut len = 0usize;
    loop {
        let words = crate::sched::ipc_receive(out);
        let count = words[0] as usize;
        assert!(
            (1..=16).contains(&count),
            "date: a stdout message with a bad byte count: {count}"
        );
        let mut chunk = [0u8; 16];
        chunk[..8].copy_from_slice(&words[1].to_le_bytes());
        chunk[8..].copy_from_slice(&words[2].to_le_bytes());
        for &b in &chunk[..count] {
            if b == b'\n' {
                return len;
            }
            assert!(len < buf.len(), "date printed a line longer than a line");
            buf[len] = b;
            len += 1;
        }
    }
}

/// Start the clock service and take its startup report, so the page has been published to
/// before anything reads it.
fn clock() -> clock_service::Wiring {
    let image = program("clock").expect("no clock program in the initial ramdisk");
    let w = clock_service::start(image);
    let _ = crate::sched::ipc_receive(w.report);
    w
}

/// **`date` prints the time the machine actually knows, in a form that reads back.**
///
/// Three formats over the one clock. The kernel computes the wall clock from the page's offset plus
/// the ambient counter and requires `date`'s output to name the same instant, so a wrong epoch, a
/// nanoseconds/seconds confusion, a timezone applied twice or a wrong calendar fails where a regex
/// would not. The window is ten seconds because a process spawn sits between the two readings and
/// the clock has one-second resolution; the failures above miss it by decades (notes/date.md).
#[test_case]
fn date_prints_the_wall_clock_it_was_granted() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    let w = clock();
    let mut buf = [0u8; 128];

    // `Unix`, the format with nothing between the clock and the text: the kernel's own reading
    // of the same page, in seconds, must be within a few seconds of what date printed.
    let (ep, region) = spawn_date(Some(w.page_phys), FMT_UNIX, 0, 0);
    let n = line(ep, &mut buf);
    let printed: i64 = core::str::from_utf8(&buf[..n])
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("date printed {:?}, which is not a number", &buf[..n]));
    let ours = (w.wall_nanos() / clock_contract::NANOS_PER_SEC) as i64;
    assert!(
        (printed - ours).abs() < 10,
        "date printed {printed} seconds; the kernel reads {ours} from the same page",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");

    // `Rfc3339`, the interchange form, parsed back by the same crate that printed it. The
    // round trip is the weak half of this; the strong half is that the instant it names must
    // still be the one above, so the text carries the time rather than merely being well-formed.
    let (ep, region) = spawn_date(Some(w.page_phys), FMT_RFC3339, 0, 0);
    let n = line(ep, &mut buf);
    let s = core::str::from_utf8(&buf[..n]).expect("date printed non-UTF-8");
    let dt = calendar::DateTime::parse_rfc3339(s)
        .unwrap_or_else(|e| panic!("date printed {s:?}, which is not RFC 3339: {}", e.as_str()));
    assert_eq!(dt.offset().minutes(), 0, "no offset was asked for");
    assert!(
        (dt.to_unix() - ours).abs() < 10,
        "date printed {s}, which is not the {ours} the kernel reads",
    );
    assert!(
        s.ends_with('Z') && s.len() == 20,
        "{s:?} is not the 20-byte Z-terminated form RFC 3339 asks for at zero offset",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");

    // `Human`, the default a person gets, at a non-zero offset so the offset is not merely
    // accepted and dropped. `Thu 2026-07-30 18:04:56 +05:30`: the weekday is the crate's whole
    // natural-language surface, and the trailing field is what says which clock this is.
    let (ep, region) = spawn_date(Some(w.page_phys), FMT_HUMAN, 330, 0);
    let n = line(ep, &mut buf);
    let s = core::str::from_utf8(&buf[..n]).expect("date printed non-UTF-8");
    assert!(
        s.ends_with(" +05:30"),
        "{s:?} should carry the +05:30 offset it was asked for",
    );
    let day = &s[..3];
    assert!(
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&day),
        "{s:?} should start with a weekday abbreviation, not {day:?}",
    );
    // Same instant again, seen through a different rendering. `Human` is
    // `Fri 2026-07-31 14:59:43 +05:30`: drop the weekday and close the space before the offset,
    // and what is left is RFC 3339 (which permits the space in place of `T`, but not the one
    // before the offset). Doing it this way rather than eyeballing the digits is what makes the
    // check an assertion about the *instant* rather than about the layout.
    assert_eq!(s.len(), 30, "{s:?} is not the Human rendering at an offset");
    let mut iso = [0u8; 32];
    iso[..19].copy_from_slice(&s.as_bytes()[4..23]); // the date and time
    iso[19..25].copy_from_slice(&s.as_bytes()[24..]); // the offset, its space removed
    let iso = calendar::DateTime::parse_rfc3339_bytes(&iso[..25])
        .unwrap_or_else(|e| panic!("{s:?} does not reduce to RFC 3339: {}", e.as_str()));
    assert_eq!(iso.offset().minutes(), 330);
    assert!(
        (iso.to_unix() - ours).abs() < 10,
        "{s:?} names a different instant from the {ours} the kernel reads",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");
}

/// **An unknown clock is a sentence, not 1970 and not a panic.**
///
/// The no-silent-degradation rule of DECISIONS §42 (a filesystem declares what it offers and must
/// be truthful) where a person can see it, and why `date` reads `clock_contract::state` before the
/// offset. `std`'s `SystemTime::now()` would have to panic; `date` names the cause. Either a frame
/// nobody published to (what a machine with no RTC, or one whose RTC reading was refused, leaves);
/// the page is the contract, so a blank page is that machine, which proves in the guest what §43
/// (reading the clock is a page, setting it is a page you may write) left proven by construction.
/// Or no clock capability at all, a different fix and sentence. Then there is no mapping either,
/// and `date` must answer without touching `CLOCK_VA` (notes/date.md).
#[test_case]
fn an_unknown_clock_is_said_plainly_rather_than_printed_as_1970() {
    let mut buf = [0u8; 128];

    // A clock page nobody has published to. Zeroed, which is what the frame allocator hands
    // out and what `clock_contract`'s `a_zeroed_page_reads_as_unknown` pins as UNKNOWN.
    let blank = crate::memory::alloc_zeroed()
        .expect("no frame for a blank clock page")
        .addr();

    let (out, region) = spawn_date(Some(blank), FMT_HUMAN, 0, PROVENANCE);
    let n = line(out, &mut buf);
    let s = core::str::from_utf8(&buf[..n]).expect("date printed non-UTF-8");
    assert_eq!(
        s, "date: the time is unknown: the machine has no clock it believes",
        "an unknown clock must be reported, not guessed at",
    );
    // And the provenance line agrees rather than inventing a source for a time it does not have.
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: clock source: unknown, generation 0",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");

    // The other cause: no capability in the slot, so no mapping either.
    let (out, region) = spawn_date(None, FMT_HUMAN, 0, 0);
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: the time is unknown: this process holds no clock capability",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");
}

/// **A declared second stream carries the complaint, and the output carries nothing** (DECISIONS
/// §67).
///
/// The claim `2>` is built on, checked by value. The same `date`, spawned with no clock and a
/// diagnostic endpoint at its manifest's slot: the complaint arrives on the diagnostic endpoint,
/// ending in `OPERATION_EOF`, and the output carries `OPERATION_EOF` and not one byte before it, so
/// `date > when.txt` leaves the file empty and the complaint readable. The diagnostic stream is
/// drained first and to completion: a `date` that interleaved the two would block this test, the
/// same deadlock a real prompt would hit (notes/date.md, "The second stream").
#[test_case]
fn a_declared_second_stream_carries_the_complaint_and_the_output_stays_empty() {
    let (out, diag, region) = spawn_date_with_diagnostics();

    let mut buf = [0u8; 128];
    let n = line(diag, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: the time is unknown: this process holds no clock capability",
        "the complaint should arrive on the declared second stream",
    );
    // And the stream ends, which is not decoration: its reader drains to end-of-stream before it
    // reads anything else, so a program that never said it was finished would hang a prompt.
    let m = crate::sched::ipc_receive(diag);
    assert!(
        matches!(
            byte_sink_contract::unpack(
                m[0],
                m[1],
                m[2],
                &mut [0u8; byte_sink_contract::INLINE_MAX]
            ),
            byte_sink_contract::Msg::Eof
        ),
        "the second stream did not end: {m:?}",
    );

    // **The output, which is where the complaint used to go.** One message, and it is the end of a
    // stream that carried nothing. This is the assertion `date > when.txt` cares about.
    let m = crate::sched::ipc_receive(out);
    assert!(
        matches!(
            byte_sink_contract::unpack(
                m[0],
                m[1],
                m[2],
                &mut [0u8; byte_sink_contract::INLINE_MAX]
            ),
            byte_sink_contract::Msg::Eof
        ),
        "a clockless date wrote {m:?} to its OUTPUT; that is the byte that used to land in the file",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");
}

/// **A `date` granted no second stream says it in-band, exactly as it always did.**
///
/// The fallback is the honest one and it is what keeps every other wiring in this tree working: a
/// program whose declared slot is empty has nothing to write to, and dropping the sentence would be
/// worse than putting it where it used to go. `spawn_date(None, ..)` above is that wiring, and
/// `an_unknown_clock_is_said_plainly_rather_than_printed_as_1970` already reads the complaint off
/// the output endpoint; this names why that is still correct after §67 rather than a leftover.
///
/// The pair is what makes the declaration meaningful: the same binary, two wirings, and the sentence
/// moves because the capability moved.
#[test_case]
fn without_a_second_stream_the_complaint_stays_in_band() {
    let mut buf = [0u8; 128];
    let (ep, region) = spawn_date(None, FMT_HUMAN, 0, 0);
    let n = line(ep, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: the time is unknown: this process holds no clock capability",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");
}

/// **The provenance is readable, and it is the four-state model rather than a boolean.**
///
/// "A human set it" and "an external source the service bounded accepted it" are different
/// claims, and the difference is exactly what a caller weighing a certificate expiry wants
/// (DECISIONS §43). Nothing else in the tree renders that distinction for a person, so if it is
/// not asserted here it is asserted nowhere.
///
/// The generation is the load-bearing half: it counts publishes, so a reader can see that the
/// clock was stepped under it. Proposing a small correction moves `rtc, generation 1` to
/// `synced, generation 2`, and a `date` that had cached or invented either would not follow.
#[test_case]
fn date_reports_where_the_time_came_from() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    let w = clock();
    let mut buf = [0u8; 128];

    let (out, region) = spawn_date(Some(w.page_phys), FMT_UNIX, 0, PROVENANCE);
    let _ = line(out, &mut buf); // the time itself, asserted elsewhere
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: clock source: rtc, generation 1",
        "the clock was read from the RTC once and published once",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");

    // Step it through the propose endpoint, which is an authority `date` does not hold, and the
    // provenance follows the page rather than the process.
    let (status, _) = w.propose_nanos(w.wall_nanos() + clock_contract::NANOS_PER_SEC / 2);
    assert_eq!(status, clock_contract::status::ACCEPTED);
    let (out, region) = spawn_date(Some(w.page_phys), FMT_UNIX, 0, PROVENANCE);
    let _ = line(out, &mut buf);
    let n = line(out, &mut buf);
    assert_eq!(
        core::str::from_utf8(&buf[..n]).unwrap(),
        "date: clock source: synced, generation 2",
        "an accepted proposal is a SYNCED clock one generation on, and date should say so",
    );
    crate::sched::reclaim_region(region).expect("date's region would not reclaim");
}
