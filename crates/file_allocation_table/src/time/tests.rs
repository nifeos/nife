//! Packing a time into the fields of a directory entry, and what clamps.
//!
//! Name: provisional.

use super::*;

fn at(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Timestamp {
    Timestamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

#[test]
fn a_time_packs_into_the_specifications_fields() {
    // 2026-11-01 12:34:57: year 46 from 1980, hour 12, minute 34, seconds in twos (28), and the
    // odd second in the creation tenths byte.
    let stamp = Stamp::pack(Some(at(2026, 11, 1, 12, 34, 57)));
    assert_eq!(stamp.creation_date, (46 << 9) + (11 << 5) + 1);
    assert_eq!(stamp.creation_time, (12 << 11) + (34 << 5) + 28);
    assert_eq!(stamp.creation_tenths, 100);
    assert_eq!(stamp.write_date, stamp.creation_date);
    assert_eq!(stamp.write_time, stamp.creation_time);
    assert_eq!(stamp.access_date, stamp.creation_date);
    // An even second has no tenths.
    assert_eq!(
        Stamp::pack(Some(at(2026, 11, 1, 12, 34, 56))).creation_tenths,
        0
    );
    // The first and the last moments FAT can store.
    let first = Stamp::pack(Some(at(1980, 1, 1, 0, 0, 0)));
    assert_eq!((first.creation_date, first.creation_time), (33, 0));
    let last = Stamp::pack(Some(at(2107, 12, 31, 23, 59, 58)));
    assert_eq!(last.creation_date, (127 << 9) + (12 << 5) + 31);
    assert_eq!(last.creation_time, (23 << 11) + (59 << 5) + 29);
}

#[test]
fn no_time_packs_to_the_zero_timestamp() {
    assert_eq!(Stamp::pack(None), Stamp::ZERO);
    assert_eq!(NoClock.now(), None);
    let mut clock = NoClock;
    assert_eq!((&mut clock).now(), None);
}

#[test]
fn a_time_outside_the_range_clamps_and_never_fails() {
    let low = Stamp::pack(Some(at(1970, 6, 15, 12, 0, 0)));
    assert_eq!(low, Stamp::pack(Some(at(1980, 1, 1, 0, 0, 0))));
    let high = Stamp::pack(Some(at(3000, 6, 15, 12, 0, 0)));
    assert_eq!(high, Stamp::pack(Some(at(2107, 12, 31, 23, 59, 58))));
    // Inside the years, each field clamps to its own range.
    let wild = Stamp::pack(Some(at(2000, 0, 0, 99, 99, 99)));
    assert_eq!(wild, Stamp::pack(Some(at(2000, 1, 1, 23, 59, 59))));
    let wilder = Stamp::pack(Some(at(2000, 99, 99, 0, 0, 0)));
    assert_eq!(wilder, Stamp::pack(Some(at(2000, 12, 31, 0, 0, 0))));
    // The boundary years are themselves valid.
    assert_ne!(
        Stamp::pack(Some(at(1980, 1, 1, 0, 0, 1))),
        Stamp::pack(Some(at(1979, 1, 1, 0, 0, 1)))
    );
    assert_eq!(
        Stamp::pack(Some(at(2107, 12, 31, 23, 59, 58))).creation_date >> 9,
        127
    );
}

#[test]
fn a_new_entry_takes_all_six_fields_and_a_modified_one_only_three() {
    let stamp = Stamp::pack(Some(at(2026, 11, 1, 12, 34, 57)));
    let mut slot = [0xEEu8; 32];
    stamp.write_new(&mut slot);
    assert_eq!(slot[13], 100);
    assert_eq!(
        u16::from_le_bytes([slot[14], slot[15]]),
        stamp.creation_time
    );
    assert_eq!(
        u16::from_le_bytes([slot[16], slot[17]]),
        stamp.creation_date
    );
    assert_eq!(u16::from_le_bytes([slot[18], slot[19]]), stamp.access_date);
    assert_eq!(u16::from_le_bytes([slot[22], slot[23]]), stamp.write_time);
    assert_eq!(u16::from_le_bytes([slot[24], slot[25]]), stamp.write_date);
    // The bytes between (the high cluster word) and around are not touched.
    assert_eq!(slot[0..13], [0xEE; 13]);
    assert_eq!(slot[20..22], [0xEE; 2]);
    assert_eq!(slot[26..32], [0xEE; 6]);
    let later = Stamp::pack(Some(at(2027, 1, 2, 3, 4, 6)));
    later.write_modified(&mut slot);
    assert_eq!(slot[13], 100, "the creation tenths stay");
    assert_eq!(
        u16::from_le_bytes([slot[16], slot[17]]),
        stamp.creation_date
    );
    assert_eq!(u16::from_le_bytes([slot[24], slot[25]]), later.write_date);
    assert_eq!(u16::from_le_bytes([slot[18], slot[19]]), later.access_date);
}

#[test]
fn a_moved_entry_copies_every_time_field_and_nothing_else() {
    let mut from = [0u8; 32];
    for (i, byte) in from.iter_mut().enumerate() {
        *byte = i as u8 + 1;
    }
    let mut to = [0xEEu8; 32];
    copy_times(&from, &mut to);
    assert_eq!(to[13..20], from[13..20]);
    assert_eq!(to[22..26], from[22..26]);
    assert_eq!(to[0..13], [0xEE; 13]);
    assert_eq!(to[20..22], [0xEE; 2]);
    assert_eq!(to[26..32], [0xEE; 6]);
}

#[test]
fn a_clock_lent_by_reference_reports_what_the_clock_reports() {
    struct Fixed(Option<Timestamp>);
    impl Clock for Fixed {
        fn now(&mut self) -> Option<Timestamp> {
            self.0
        }
    }
    let mut clock = Fixed(Some(at(2026, 11, 1, 12, 34, 56)));
    assert_eq!((&mut clock).now(), Some(at(2026, 11, 1, 12, 34, 56)));
    clock.0 = None;
    assert_eq!((&mut clock).now(), None);
}

#[test]
fn the_last_year_stores_as_itself_and_only_later_years_clamp() {
    let mid = Stamp::pack(Some(at(2107, 6, 15, 12, 0, 0)));
    assert_eq!(mid.creation_date, (127 << 9) + (6 << 5) + 15);
    assert_eq!(mid.creation_time, 12 << 11);
    let early = Stamp::pack(Some(at(1980, 6, 15, 12, 0, 0)));
    assert_eq!(early.creation_date, (6 << 5) + 15);
}
