use pipeline_service::answer;

use super::*;

/// The last line `components/src/swish.rs`'s timing role prints. Must match `TIMING_DONE`.
const DONE: &[u8] = b"== timings done\n";

/// Run the timing script under one clock state and hand back what the shell printed.
///
/// Not cached, unlike [`pipeline_tests`]'s and [`redirection_tests`]'s transcripts, because the
/// whole point here is to run the **same script** under three different capability tables: caching would make
/// the second and third runs the first one's answer.
fn transcript(clock: Option<u64>, out: &mut [u8; 2048]) -> usize {
    let Some(w) = pipeline_service::start_timing(clock) else {
        panic!("no swish program in the initial ramdisk, or no memory to wire one");
    };
    pipeline_service::transcript(&w, DONE, out)
}

/// Start the clock service and take its startup report, so the page has been published to before
/// anything reads it. The same helper `date_tests` uses, and for the same reason.
fn clock() -> clock_service::Wiring {
    let image = program("clock").expect("no clock program in the initial ramdisk");
    let w = clock_service::start(image);
    let _ = crate::sched::ipc_receive(w.report);
    w
}

/// The nanoseconds in a `time:` line, as a number.
///
/// The line is `  time: real 4.213 ms`, and this parses it back into nanoseconds rather than
/// checking that it looks like a duration, because "it printed something with a decimal point in it"
/// is exactly the assertion that would survive every bug worth catching. A unit this does not know
/// is a failure: a renderer that grew a fourth unit and a reader that silently ignored it would let
/// a wrong number through.
fn nanos_of(said: &[u8]) -> u64 {
    let text = core::str::from_utf8(said).expect("the shell printed non-UTF-8");
    let line = text
        .lines()
        .find(|l| l.contains("time: real "))
        .unwrap_or_else(|| panic!("no timing line in {text:?}"));
    let rest = line.split("time: real ").nth(1).expect("just matched");
    let mut fields = rest.split_ascii_whitespace();
    let value = fields
        .next()
        .unwrap_or_else(|| panic!("no value in {line:?}"));
    let unit = fields
        .next()
        .unwrap_or_else(|| panic!("no unit in {line:?}"));
    let (whole, frac) = value
        .split_once('.')
        .unwrap_or_else(|| panic!("{value:?} is not a fixed-point duration"));
    assert_eq!(frac.len(), 3, "the fraction is thousandths of a unit");
    let thousandths: u64 = whole
        .parse::<u64>()
        .and_then(|w| frac.parse::<u64>().map(|f| w * 1_000 + f))
        .unwrap_or_else(|_| panic!("{value:?} is not a number"));
    match unit {
        "us" => thousandths,
        "ms" => thousandths * 1_000,
        "s" => thousandths * 1_000_000,
        other => panic!("the shell printed a unit this test does not know: {other:?}"),
    }
}

/// **A command is timed by the shell's clock, and the command is handed nothing.**
///
/// Three assertions make the claim. `least_authority_demo 3`, run bare and behind `time`, must
/// report `3*3 = 9` both times, so a prefix that re-tokenized its tail, dropped an option or
/// spawned something else fails. The duration must be positive, since a real spawn cannot take zero
/// time and a zero means two readings of one instant; the ten-second ceiling is loose because TCG
/// under load is slow, yet a nanoseconds-for-microseconds error misses it by four orders of
/// magnitude. `least_authority_demo` declares no clock and is still timed: measuring needs the
/// observer's authority, not the subject's. And `time echo hello` spawns no process at all and
/// still reports a duration, because what is measured is an interval.
#[test_case]
fn a_command_is_timed_by_the_shells_clock_and_holds_none_itself() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    let w = clock();
    let mut buf = [0u8; 2048];
    let n = transcript(Some(w.page_phys), &mut buf);
    let t = &buf[..n];

    let bare = answer(t, b"least_authority_demo 3");
    let timed = answer(t, b"time least_authority_demo 3");
    assert!(
        core::str::from_utf8(bare).unwrap().contains("3*3 = 9"),
        "the untimed control did not run: {:?}",
        core::str::from_utf8(bare).unwrap(),
    );
    assert!(
        core::str::from_utf8(timed).unwrap().contains("3*3 = 9"),
        "timing a command must not change what it answers: {:?}",
        core::str::from_utf8(timed).unwrap(),
    );

    // Ten seconds, because this is not a benchmark: `bench` measures spawn latency and this proves a
    // stopwatch exists. See notes/time-command.md.
    const SANE: u64 = 10_000_000_000;
    let spawned = nanos_of(timed);
    assert!(
        spawned > 0 && spawned < SANE,
        "a spawn took {spawned} ns, which is not a duration this machine could have spent",
    );

    // The builtin: no process, no grant, and a duration all the same.
    let builtin = answer(t, b"time echo hello");
    assert!(
        core::str::from_utf8(builtin).unwrap().contains("hello"),
        "the timed builtin did not run: {:?}",
        core::str::from_utf8(builtin).unwrap(),
    );
    let echoed = nanos_of(builtin);
    assert!(
        echoed > 0 && echoed < SANE,
        "a timed builtin took {echoed} ns",
    );

    // And the prefix with nothing after it is about the line rather than about a clock, which is
    // why it is a third sentence and not one of `date`'s two.
    let empty = answer(t, b"time");
    assert!(
        core::str::from_utf8(empty)
            .unwrap()
            .contains("name a command to time"),
        "{:?}",
        core::str::from_utf8(empty).unwrap(),
    );
}

/// **A shell with no usable clock times the command anyway** (§72 (`time` needs no clock: duration
/// is ambient, wall-clock identity is authority)).
///
/// A duration is two reads of the monotonic counter, which the kernel opens to EL0, so the offset a
/// clock page carries cancels and `time` can always measure. Both of `date`'s causes are exercised
/// as non-causes: a zeroed clock page (UNKNOWN to `clock_contract`) and no clock capability at
/// all. In each, the timed line must answer and carry a duration, and neither retired refusal
/// sentence may appear anywhere in the transcript.
#[test_case]
fn a_shell_with_no_usable_clock_times_the_command_anyway() {
    let mut buf = [0u8; 2048];

    // A frame nobody has published to, which is what a reader on a machine with no believable RTC
    // holds. Zeroed, which `clock_contract`'s `a_zeroed_page_reads_as_unknown` pins as UNKNOWN.
    let blank = crate::memory::alloc_zeroed()
        .expect("no frame for a blank clock page")
        .addr();

    // Ten seconds, the sibling test's bound and for its reason: this proves a stopwatch exists,
    // it does not benchmark one.
    const SANE: u64 = 10_000_000_000;

    for clock in [Some(blank), None] {
        let n = transcript(clock, &mut buf);
        let t = &buf[..n];
        let timed = core::str::from_utf8(answer(t, b"time least_authority_demo 3")).unwrap();
        assert!(timed.contains("3*3 = 9"), "the command must run: {timed:?}",);
        let nanos = nanos_of(answer(t, b"time least_authority_demo 3"));
        assert!(nanos > 0 && nanos < SANE, "duration out of range: {nanos}");
        // The two sentences §72 retired must be gone from the whole transcript, not merely from
        // this line: a shell that printed them anywhere would still be claiming the old boundary.
        let whole = core::str::from_utf8(t).unwrap();
        assert!(!whole.contains("holds no clock capability"), "{whole:?}");
        assert!(!whole.contains("no clock it believes"), "{whole:?}");
    }
}
