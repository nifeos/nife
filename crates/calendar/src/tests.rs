use super::*;

/// **A full buffer drops, it does not panic**, which is what `Writer`'s doc promises. No format
/// reaches the cap (the longest is two bytes short), so only a direct write can test the
/// promise; an off-by-one in the bound turns the drop into an index past the array.
#[test]
fn a_full_writer_drops_the_next_byte_instead_of_panicking() {
    let mut w = Writer::new();
    for _ in 0..FMT_CAP + 1 {
        w.byte(b'9');
    }
    assert_eq!(w.len, FMT_CAP);
}

/// A whole civil reading as a tuple, which is how the tables below state their expectations.
type Fields = (i32, u8, u8, u8, u8, u8);

/// A date without a time, for the rollover table.
type Ymd = (i32, u8, u8);

fn fields(c: &Civil) -> Fields {
    (
        c.year(),
        c.month(),
        c.day(),
        c.hour(),
        c.minute(),
        c.second(),
    )
}

/// The dates a reader can check against a wall calendar, plus the two the range is defined by.
/// Every timestamp here was cross-checked against Python's `datetime` rather than derived from
/// this crate, so the table is an independent witness rather than a snapshot of our own output.
const KNOWN: &[(i64, i32, u8, u8, u8, u8, u8)] = &[
    (0, 1970, 1, 1, 0, 0, 0),                      // the epoch itself
    (-1, 1969, 12, 31, 23, 59, 59),                // one second before it
    (1_000_000_000, 2001, 9, 9, 1, 46, 40),        // the billennium
    (951_782_400, 2000, 2, 29, 0, 0, 0),           // the leap day the 400 rule creates
    (-2_203_891_200, 1900, 3, 1, 0, 0, 0),         // the day after the 1900 leap day that isn't
    (1_709_208_000, 2024, 2, 29, 12, 0, 0),        // an ordinary leap day
    (1_785_414_896, 2026, 7, 30, 12, 34, 56),      // the day this was written
    (MINIMUM_TIMESTAMP, 0, 1, 1, 0, 0, 0),         // 0000-01-01T00:00:00Z
    (MAXIMUM_TIMESTAMP, 9999, 12, 31, 23, 59, 59), // 9999-12-31T23:59:59Z
    (-62_135_596_800, 1, 1, 1, 0, 0, 0),           // 0001-01-01, where Python's calendar starts
];

#[test]
fn known_timestamps_convert_both_ways() {
    for &(ts, y, mo, d, h, mi, s) in KNOWN {
        let c = Civil::from_unix(ts).unwrap();
        assert_eq!(fields(&c), (y, mo, d, h, mi, s), "from_unix({ts})");
        assert_eq!(Civil::new(y, mo, d, h, mi, s).unwrap().to_unix(), ts);
    }
}

/// 1900 is not a leap year and 2000 is. This is the pair that breaks date code written from
/// memory: the "divisible by 4" rule gets both wrong, and "except centuries" gets 2000 wrong.
#[test]
fn the_century_rules() {
    assert!(!is_leap_year(1900));
    assert!(is_leap_year(2000));
    assert!(!is_leap_year(2100));
    assert!(is_leap_year(2024));
    assert!(!is_leap_year(2023));
    assert!(is_leap_year(0)); // year 0 is divisible by 400

    assert_eq!(days_in_month(1900, 2), Some(28));
    assert_eq!(days_in_month(2000, 2), Some(29));
    assert_eq!(days_in_month(2024, 2), Some(29));
    assert_eq!(days_in_month(2023, 2), Some(28));
    assert_eq!(days_in_month(2024, 13), None);
    assert_eq!(days_in_month(2024, 0), None);

    // The dates themselves, not just the predicate.
    assert_eq!(Civil::new(1900, 2, 29, 0, 0, 0), Err(Error::BadDay));
    assert!(Civil::new(2000, 2, 29, 0, 0, 0).is_ok());
    assert_eq!(Civil::new(2023, 2, 29, 0, 0, 0), Err(Error::BadDay));
    assert!(Civil::new(2024, 2, 29, 0, 0, 0).is_ok());
}

/// Every rollover a month can have: 28 to 29, 29 to 1, 30 to 1, 31 to 1, and December 31 to
/// January 1 (which is also a year rollover).
#[test]
fn month_and_year_ends_roll_over() {
    let cases: &[(Ymd, Ymd)] = &[
        ((2023, 2, 28), (2023, 3, 1)),  // non-leap February ends at 28
        ((2024, 2, 28), (2024, 2, 29)), // leap February does not
        ((2024, 2, 29), (2024, 3, 1)),
        ((1900, 2, 28), (1900, 3, 1)), // the century that is not a leap year
        ((2000, 2, 28), (2000, 2, 29)), // the century that is
        ((2026, 4, 30), (2026, 5, 1)), // a 30-day month
        ((2026, 7, 31), (2026, 8, 1)), // a 31-day month
        ((2026, 8, 31), (2026, 9, 1)), // the two 31s in a row
        ((2026, 12, 31), (2027, 1, 1)), // the year end
    ];
    for &((y1, m1, d1), (y2, m2, d2)) in cases {
        let a = Civil::new(y1, m1, d1, 23, 59, 59).unwrap();
        let b = Civil::from_unix(a.to_unix() + 1).unwrap();
        assert_eq!(
            fields(&b),
            (y2, m2, d2, 0, 0, 0),
            "one second after {y1}-{m1}-{d1}T23:59:59Z"
        );
    }
}

/// The year-2038 problem, proved to be a non-event here rather than assumed. `i32::MAX` seconds
/// is 2038-01-19T03:14:07Z, the last instant a 32-bit `time_t` can name; the next second is
/// ordinary, and so is a timestamp a century past it.
#[test]
fn the_2038_boundary_is_not_a_boundary() {
    let last32 = i32::MAX as i64;
    let c = Civil::from_unix(last32).unwrap();
    assert_eq!(c.format(Format::Rfc3339).as_str(), "2038-01-19T03:14:07Z");

    let next = Civil::from_unix(last32 + 1).unwrap();
    assert_eq!(
        next.format(Format::Rfc3339).as_str(),
        "2038-01-19T03:14:08Z"
    );
    assert_eq!(next.to_unix() - c.to_unix(), 1);

    // And a long way past it, where a 32-bit counter would have wrapped several times.
    let far = Civil::new(2138, 1, 19, 3, 14, 8).unwrap();
    assert_eq!(Civil::from_unix(far.to_unix()).unwrap(), far);
}

/// Times before the epoch are ordinary. The trap is truncating division: in Rust `-1 / 86400`
/// is 0, so a naive implementation reports 1970-01-01 for every instant in the last day of 1969.
#[test]
fn negative_timestamps_are_ordinary() {
    for seconds in [-1i64, -86_399, -86_400, -86_401, -1_000_000_000] {
        let c = Civil::from_unix(seconds).unwrap();
        assert_eq!(c.to_unix(), seconds);
        assert!(c.year() < 1970);
    }
    assert_eq!(
        Civil::from_unix(-86_400)
            .unwrap()
            .format(Format::Rfc3339)
            .as_str(),
        "1969-12-31T00:00:00Z"
    );
    assert_eq!(
        Civil::from_unix(-86_401)
            .unwrap()
            .format(Format::Rfc3339)
            .as_str(),
        "1969-12-30T23:59:59Z"
    );
}

#[test]
fn out_of_range_timestamps_are_refused_not_wrapped() {
    assert_eq!(
        Civil::from_unix(MINIMUM_TIMESTAMP - 1),
        Err(Error::OutOfRange)
    );
    assert_eq!(
        Civil::from_unix(MAXIMUM_TIMESTAMP + 1),
        Err(Error::OutOfRange)
    );
    assert_eq!(Civil::from_unix(i64::MIN), Err(Error::OutOfRange));
    assert_eq!(Civil::from_unix(i64::MAX), Err(Error::OutOfRange));
    assert_eq!(Civil::new(10_000, 1, 1, 0, 0, 0), Err(Error::OutOfRange));
    assert_eq!(Civil::new(-1, 1, 1, 0, 0, 0), Err(Error::OutOfRange));
}

#[test]
fn each_bad_field_names_itself() {
    assert_eq!(Civil::new(2026, 0, 1, 0, 0, 0), Err(Error::BadMonth));
    assert_eq!(Civil::new(2026, 13, 1, 0, 0, 0), Err(Error::BadMonth));
    assert_eq!(Civil::new(2026, 1, 0, 0, 0, 0), Err(Error::BadDay));
    assert_eq!(Civil::new(2026, 4, 31, 0, 0, 0), Err(Error::BadDay));
    assert_eq!(Civil::new(2026, 1, 1, 24, 0, 0), Err(Error::BadHour));
    assert_eq!(Civil::new(2026, 1, 1, 0, 60, 0), Err(Error::BadMinute));
    assert_eq!(Civil::new(2026, 1, 1, 0, 0, 60), Err(Error::BadSecond));
}

/// Weekdays, pinned against dates a reader can check. 1970-01-01 was a Thursday, which is the
/// anchor the arithmetic uses, so the other four are the ones that would catch an off-by-one.
#[test]
fn weekdays() {
    let cases: &[((i32, u8, u8), Weekday)] = &[
        ((1970, 1, 1), Weekday::Thursday),
        ((2000, 1, 1), Weekday::Saturday),
        ((2000, 2, 29), Weekday::Tuesday),
        ((1900, 1, 1), Weekday::Monday),
        ((2026, 7, 30), Weekday::Thursday),
        ((1969, 12, 31), Weekday::Wednesday), // before the epoch, where rem_euclid matters
        ((2038, 1, 19), Weekday::Tuesday),
    ];
    for &((y, m, d), want) in cases {
        assert_eq!(
            Civil::new(y, m, d, 0, 0, 0).unwrap().weekday(),
            want,
            "{y}-{m}-{d}"
        );
    }
    assert_eq!(Weekday::Monday.iso_number(), 1);
    assert_eq!(Weekday::Sunday.iso_number(), 7);
    assert_eq!(Weekday::Thursday.abbrev(), "Thu");
    assert_eq!(Weekday::Thursday.name(), "Thursday");
}

#[test]
fn day_of_year() {
    assert_eq!(Civil::new(2026, 1, 1, 0, 0, 0).unwrap().day_of_year(), 1);
    assert_eq!(
        Civil::new(2026, 12, 31, 0, 0, 0).unwrap().day_of_year(),
        365
    );
    assert_eq!(
        Civil::new(2024, 12, 31, 0, 0, 0).unwrap().day_of_year(),
        366
    );
    assert_eq!(Civil::new(2024, 3, 1, 0, 0, 0).unwrap().day_of_year(), 61);
    assert_eq!(Civil::new(2023, 3, 1, 0, 0, 0).unwrap().day_of_year(), 60);
    assert_eq!(
        Civil::new(1900, 12, 31, 0, 0, 0).unwrap().day_of_year(),
        365
    );
    assert_eq!(
        Civil::new(2000, 12, 31, 0, 0, 0).unwrap().day_of_year(),
        366
    );
}

#[test]
fn the_five_formats() {
    let dt = DateTime::from_unix(1_785_414_896, UtcOffset::UTC).unwrap();
    assert_eq!(dt.format(Format::Rfc3339).as_str(), "2026-07-30T12:34:56Z");
    assert_eq!(dt.format(Format::Date).as_str(), "2026-07-30");
    assert_eq!(dt.format(Format::Time).as_str(), "12:34:56");
    assert_eq!(
        dt.format(Format::Human).as_str(),
        "Thu 2026-07-30 12:34:56 UTC"
    );
    assert_eq!(dt.format(Format::Unix).as_str(), "1785414896");

    // Away from UTC the same instant reads differently and says so.
    let ist = DateTime::from_unix(1_785_414_896, UtcOffset::from_hm(5, 30).unwrap()).unwrap();
    assert_eq!(
        ist.format(Format::Rfc3339).as_str(),
        "2026-07-30T18:04:56+05:30"
    );
    assert_eq!(
        ist.format(Format::Human).as_str(),
        "Thu 2026-07-30 18:04:56 +05:30"
    );
    assert_eq!(ist.format(Format::Unix).as_str(), "1785414896"); // the instant is unchanged

    let pst = DateTime::from_unix(1_785_414_896, UtcOffset::from_hm(-8, 0).unwrap()).unwrap();
    assert_eq!(
        pst.format(Format::Rfc3339).as_str(),
        "2026-07-30T04:34:56-08:00"
    );

    // Zero padding everywhere, including a year that needs three leading zeros.
    let early = Civil::new(7, 1, 2, 3, 4, 5).unwrap();
    assert_eq!(
        early.format(Format::Rfc3339).as_str(),
        "0007-01-02T03:04:05Z"
    );

    // A negative Unix value prints its sign.
    assert_eq!(
        Civil::from_unix(-1).unwrap().format(Format::Unix).as_str(),
        "-1"
    );

    // The buffer is big enough for the longest output, which is Human at an extreme offset.
    let widest = DateTime::new(
        Civil::new(9999, 12, 31, 23, 59, 59).unwrap(),
        UtcOffset::from_hm(23, 59).unwrap(),
    );
    assert_eq!(
        widest.format(Format::Human).as_str(),
        "Fri 9999-12-31 23:59:59 +23:59"
    );
    assert!(widest.format(Format::Human).as_bytes().len() <= FMT_CAP);
}

#[test]
fn parse_accepts_rfc3339() {
    let cases: &[(&str, i64)] = &[
        ("1970-01-01T00:00:00Z", 0),
        ("2026-07-30T12:34:56Z", 1_785_414_896),
        ("2026-07-30t12:34:56z", 1_785_414_896), // lowercase, allowed by 5.6
        ("2026-07-30 12:34:56Z", 1_785_414_896), // space separator, the 5.6 NOTE
        ("2026-07-30T12:34:56.789Z", 1_785_414_896), // fraction parsed and dropped
        ("2026-07-30T12:34:56.000000001Z", 1_785_414_896),
        ("2026-07-30T18:04:56+05:30", 1_785_414_896), // same instant, other offset
        ("2026-07-30T04:34:56-08:00", 1_785_414_896),
        ("2026-07-30T12:34:56+00:00", 1_785_414_896), // an explicit zero offset
        ("1969-12-31T23:59:59Z", -1),
        ("2038-01-19T03:14:08Z", 2_147_483_648),
        ("0000-01-01T00:00:00Z", MINIMUM_TIMESTAMP),
        ("9999-12-31T23:59:59Z", MAXIMUM_TIMESTAMP),
    ];
    for &(text, seconds) in cases {
        let dt = DateTime::parse_rfc3339(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        assert_eq!(dt.to_unix(), seconds, "{text}");
    }

    // The offset survives parsing, it is not normalised away.
    let dt = DateTime::parse_rfc3339("2026-07-30T18:04:56+05:30").unwrap();
    assert_eq!(dt.offset().minutes(), 330);
    assert_eq!(dt.civil().hour(), 18);
    assert_eq!(
        dt.format(Format::Rfc3339).as_str(),
        "2026-07-30T18:04:56+05:30"
    );
}

#[test]
fn parse_refuses_what_it_should() {
    let cases: &[(&str, Error)] = &[
        ("", Error::Syntax),
        ("2026-07-30", Error::Syntax),            // no time, no offset
        ("2026-07-30T12:34:56", Error::Syntax),   // no offset: this is the ISO/RFC difference
        ("2026-07-30T12:34:56Zx", Error::Syntax), // trailing bytes
        ("2026-07-30X12:34:56Z", Error::Syntax),  // wrong separator
        ("2026/07/30T12:34:56Z", Error::Syntax),
        ("26-07-30T12:34:56Z", Error::Syntax), // two-digit year
        ("2026-7-30T12:34:56Z", Error::Syntax), // unpadded month
        ("2026-07-30T12:34:56.Z", Error::Syntax), // a dot with no digits
        ("2026-07-30T12:34:56+0530", Error::Syntax), // offset needs its colon
        ("2026-07-30T12:34:56+05", Error::Syntax),
        ("2026-07-30T12:34:60Z", Error::LeapSecond), // the one that is not a syntax error
        ("2026-07-30T12:60:56Z", Error::BadMinute),
        ("2026-07-30T24:00:00Z", Error::BadHour),
        ("2026-02-30T12:34:56Z", Error::BadDay),
        ("2023-02-29T12:34:56Z", Error::BadDay), // valid-looking, wrong year
        ("2026-13-01T12:34:56Z", Error::BadMonth),
        ("2026-07-30T12:34:56+05:60", Error::BadOffset),
        ("2026-07-30T12:34:56+24:00", Error::BadOffset),
        // Malformed twice over, and the offset is judged before the trailing byte is. The
        // six-byte check ahead of the offset is a length check; a mutant that turns it into a
        // trailing-bytes check refuses the same inputs and renames this refusal Syntax.
        ("2026-07-30T12:34:56+05:60x", Error::BadOffset),
    ];
    for &(text, want) in cases {
        assert_eq!(DateTime::parse_rfc3339(text), Err(want), "{text:?}");
    }
}

/// Format and parse are inverses over the whole supported range, sampled densely. The Kani
/// harness proves this for every timestamp in a smaller window; this one walks the extremes and
/// a large stride, which is the coverage a bounded proof cannot reach.
#[test]
fn format_and_parse_round_trip_across_the_range() {
    let mut seconds = MINIMUM_TIMESTAMP;
    // Prime-ish, so it lands on every hour and weekday over time. Under Miri the walk is
    // sampled three orders of magnitude coarser: the native stride is ~315,000 format+parse
    // round trips, which an interpreter turns from a second into the better part of an hour.
    // ~300 samples keep the round trip on Miri's paths; the native run keeps the density.
    // "Miri-clean" means the sampled walk (notes/undefined-behavior.md).
    let stride = if cfg!(miri) { 1_000_036_991 } else { 1_000_037 };
    while seconds <= MAXIMUM_TIMESTAMP {
        let dt = DateTime::from_unix(seconds, UtcOffset::UTC).unwrap();
        let text = dt.format(Format::Rfc3339);
        let back = DateTime::parse_rfc3339(text.as_str()).unwrap();
        assert_eq!(back.to_unix(), seconds, "{}", text.as_str());
        seconds += stride;
    }
}

#[test]
fn utc_offsets() {
    assert_eq!(UtcOffset::UTC.minutes(), 0);
    assert_eq!(UtcOffset::from_hm(5, 30).unwrap().minutes(), 330);
    assert_eq!(UtcOffset::from_hm(-8, 0).unwrap().seconds(), -28_800);
    assert_eq!(UtcOffset::from_minutes(-1439).unwrap().minutes(), -1439);
    assert_eq!(UtcOffset::from_minutes(1440), Err(Error::BadOffset));
    assert_eq!(UtcOffset::from_minutes(-1440), Err(Error::BadOffset));
    assert_eq!(UtcOffset::from_hm(5, -30), Err(Error::BadOffset)); // mixed signs
    assert_eq!(UtcOffset::from_hm(-5, 30), Err(Error::BadOffset)); // and mixed the other way
    // Both halves negative is one sign written twice, not mixed signs, and it is how -05:30 is
    // written. Only the negative-hours row of the guard's truth table proves that; with 5, -30
    // alone the `hours < 0` test can rot into anything and every case still lands right.
    assert_eq!(UtcOffset::from_hm(-5, -30).unwrap().minutes(), -330);
    assert_eq!(UtcOffset::from_hm(0, 60), Err(Error::BadOffset));

    // A local reading can fall outside the range even when the instant is inside it. That edge
    // is documented on from_unix rather than hidden.
    assert_eq!(
        DateTime::from_unix(MAXIMUM_TIMESTAMP, UtcOffset::from_hm(1, 0).unwrap()),
        Err(Error::OutOfRange)
    );
}

/// `Civil`'s derived ordering is chronological, which the formatting relies on being true and
/// which a field reorder would silently break.
#[test]
fn civil_ordering_is_chronological() {
    let a = Civil::new(2026, 7, 30, 12, 0, 0).unwrap();
    let b = Civil::new(2026, 8, 1, 0, 0, 0).unwrap();
    let c = Civil::new(2027, 1, 1, 0, 0, 0).unwrap();
    assert!(a < b && b < c);
    assert!(a.to_unix() < b.to_unix() && b.to_unix() < c.to_unix());
}

// The tests from here down each pin something milestone 85's mutation run showed nothing
// pinned. See notes/mutation-testing.md.

/// The parser's edges the refusal table above did not reach: a fraction that runs off the end
/// of the input (the digit scan's bound could become `<=` and read one past), an offset whose
/// colon is elsewhere than checked (the `expect` index could become `i - 3`, which lands on the
/// seconds colon and is ':' in EVERY well-formed input, so garbage like `+05300` parsed), and
/// the last legal offset minute.
#[test]
fn parser_edges_the_mutation_run_found() {
    assert_eq!(
        DateTime::parse_rfc3339("2026-07-30T12:34:56.5"),
        Err(Error::Syntax),
        "input ends inside the fraction"
    );
    assert_eq!(
        DateTime::parse_rfc3339("2026-07-30T12:34:56+05300"),
        Err(Error::Syntax),
        "six bytes of offset, colon in the wrong place"
    );
    let ok = DateTime::parse_rfc3339("2026-07-30T12:34:56+05:59").unwrap();
    assert_eq!(
        ok.offset().minutes(),
        5 * 60 + 59,
        ":59 is the last legal offset minute"
    );
}

/// A known Sunday, absolutely, not relative to another weekday: the cycle test above proves
/// the weekdays rotate, and rotation is preserved by deleting one match arm and letting the
/// wildcard eat it.
#[test]
fn the_epoch_week_lands_on_the_right_days() {
    let sunday = Civil::new(1970, 1, 4, 0, 0, 0).unwrap();
    assert_eq!(sunday.weekday(), Weekday::Sunday);
    let epoch = Civil::new(1970, 1, 1, 0, 0, 0).unwrap();
    assert_eq!(epoch.weekday(), Weekday::Thursday);
}

/// Zero is "0": the sign branch in the Unix format is `value < 0`, and `<=` would print "-0",
/// which is not a timestamp anything reads back.
#[test]
fn unix_zero_formats_without_a_sign() {
    let epoch = Civil::new(1970, 1, 1, 0, 0, 0).unwrap();
    assert_eq!(epoch.format(Format::Unix).as_str(), "0");
}

/// `Formatted` speaks for itself through three impls, and each has to say the same thing: a
/// replaced body returning "" or Ok(()) would print nothing, silently, everywhere a timestamp
/// is shown.
#[test]
fn formatted_says_the_same_thing_three_ways() {
    use core::fmt::Write;

    /// This crate is `no_std` with no allocator even under test, so catching what `{}` and
    /// `{:?}` actually emit takes a fixed buffer that implements `fmt::Write`.
    struct Sink {
        buf: [u8; 64],
        len: usize,
    }
    impl Sink {
        fn new() -> Sink {
            Sink {
                buf: [0; 64],
                len: 0,
            }
        }
    }
    impl Write for Sink {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let end = self.len + s.len();
            self.buf[self.len..end].copy_from_slice(s.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    impl Sink {
        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buf[..self.len]).unwrap()
        }
    }

    let f = Civil::new(2026, 7, 30, 12, 34, 56)
        .unwrap()
        .format(Format::Rfc3339);
    assert_eq!(f.as_str(), "2026-07-30T12:34:56Z");
    assert_eq!(f.as_ref(), f.as_str());

    let mut shown = Sink::new();
    write!(shown, "{f}").unwrap();
    assert_eq!(shown.as_str(), "2026-07-30T12:34:56Z");

    let mut debugged = Sink::new();
    write!(debugged, "{f:?}").unwrap();
    assert_eq!(debugged.as_str(), "\"2026-07-30T12:34:56Z\"");
}

/// Every refusal has words, and its own words: two variants sharing a message would make a
/// refusal ambiguous at the one place it is read, a shell with no allocator.
#[test]
fn every_error_has_its_own_message() {
    const ALL: &[Error] = &[
        Error::OutOfRange,
        Error::BadMonth,
        Error::BadDay,
        Error::BadHour,
        Error::BadMinute,
        Error::BadSecond,
        Error::BadOffset,
        Error::LeapSecond,
        Error::Syntax,
    ];
    for (i, a) in ALL.iter().enumerate() {
        assert!(!a.as_str().is_empty());
        for b in &ALL[i + 1..] {
            assert_ne!(a.as_str(), b.as_str());
        }
    }
}
