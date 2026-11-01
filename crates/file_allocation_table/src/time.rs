//! **Time on a volume**: the clock a writer must supply, and the packed form a directory entry
//! stores.
//!
//! The crate owns no clock, and a write that stamps a time needs one, so every caller that writes
//! supplies a [`Clock`]. A caller with no time source says so with [`NoClock`], and the entries it
//! writes carry the zero timestamp, the same bytes a `mkfs` with no clock has always written.
//!
//! FAT stores a date as 16 bits (year from 1980, month, day), a time as 16 bits (hour, minute and
//! seconds in units of two), and a creation time's odd second as a tenths-of-a-second byte of 0 or
//! 100. It stores no time zone. This crate stores exactly what its clock reports, and a host that
//! reads the volume shows the stored time as local time, so a caller chooses whether its clock
//! reports local time or UTC.
//!
//! Name: provisional. The clock half of a writer, ruled required on 2026-11-01 (UTC).

/// **A calendar date and time of day**, as plain numbers with no time zone.
///
/// The fields need not be valid: a writer clamps an out-of-range value to the nearest time FAT can
/// store (see [`Clock`]), and never fails a write over it.
///
/// Name: provisional. Year, month, day, hour, minute and second, with no nife type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp {
    /// The year, for example 2026. FAT stores 1980 to 2107.
    pub year: u16,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31. The crate does not check the day against the month.
    pub day: u8,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 59. FAT keeps two-second resolution, and a creation time keeps the odd
    /// second in its tenths byte.
    pub second: u8,
}

/// **Where a writer gets the time.** A volume that writes holds one.
///
/// [`Clock::now`] returns `None` when there is no time to report (a real clock that has not been
/// set), and the entry then carries the zero timestamp. A time before 1980 is stored as
/// 1980-01-01 00:00:00 and one after 2107-12-31 23:59:58 as that moment, and any other field
/// outside its range is clamped to it. No write fails over a time.
///
/// The crate takes the clock as a type parameter and not as a trait object, so it needs no
/// allocator and no `dyn` indirection, and a caller's clock is a plain value.
///
/// # Examples
///
/// ```
/// use file_allocation_table::{Clock, Timestamp};
///
/// /// A clock a test sets by hand.
/// struct Fixed(Option<Timestamp>);
///
/// impl Clock for Fixed {
///     fn now(&mut self) -> Option<Timestamp> {
///         self.0
///     }
/// }
/// ```
///
/// Name: provisional. The ruling's clock; the method reports `Option` so "no time yet" is a value.
pub trait Clock {
    /// The current time, or `None` when there is none to report.
    ///
    /// Name: provisional. As `std::time::SystemTime::now`, with the empty case a value.
    fn now(&mut self) -> Option<Timestamp>;
}

/// **The explicit "no time"**: a clock that always reports none. A volume written with it carries
/// the zero timestamp in every entry it stamps, so an image built with it is reproducible.
///
/// Name: provisional. The ruling's name for the caller that has no time source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NoClock;

impl Clock for NoClock {
    fn now(&mut self) -> Option<Timestamp> {
        None
    }
}

impl<C: Clock + ?Sized> Clock for &mut C {
    fn now(&mut self) -> Option<Timestamp> {
        (**self).now()
    }
}

/// The first year FAT stores.
const FIRST_YEAR: u16 = 1980;
/// The last year FAT stores.
const LAST_YEAR: u16 = 2107;

/// **The six time fields of a directory entry, packed**: what a stamp writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    /// `DIR_CrtTimeTenth`: 0 or 100, the odd second of the creation time.
    pub(crate) creation_tenths: u8,
    /// `DIR_CrtTime`.
    pub(crate) creation_time: u16,
    /// `DIR_CrtDate`.
    pub(crate) creation_date: u16,
    /// `DIR_LstAccDate`.
    pub(crate) access_date: u16,
    /// `DIR_WrtTime`.
    pub(crate) write_time: u16,
    /// `DIR_WrtDate`.
    pub(crate) write_date: u16,
}

impl Stamp {
    /// The zero timestamp: every field zero.
    pub(crate) const ZERO: Stamp = Stamp {
        creation_tenths: 0,
        creation_time: 0,
        creation_date: 0,
        access_date: 0,
        write_time: 0,
        write_date: 0,
    };

    /// **Pack `now`**, clamping what FAT cannot store. `None` packs to [`Stamp::ZERO`].
    pub(crate) fn pack(now: Option<Timestamp>) -> Stamp {
        let Some(now) = now else {
            return Stamp::ZERO;
        };
        let (year, month, day, hour, minute, second) = if now.year < FIRST_YEAR {
            (FIRST_YEAR, 1, 1, 0, 0, 0)
        } else if now.year > LAST_YEAR {
            (LAST_YEAR, 12, 31, 23, 59, 58)
        } else {
            (
                now.year,
                now.month.clamp(1, 12),
                now.day.clamp(1, 31),
                now.hour.min(23),
                now.minute.min(59),
                now.second.min(59),
            )
        };
        let date = ((year - FIRST_YEAR) << 9) + (u16::from(month) << 5) + u16::from(day);
        let time = (u16::from(hour) << 11) + (u16::from(minute) << 5) + u16::from(second / 2);
        Stamp {
            creation_tenths: (second % 2) * 100,
            creation_time: time,
            creation_date: date,
            access_date: date,
            write_time: time,
            write_date: date,
        }
    }

    /// Write all six fields into the 32-byte short entry `slot`: a new entry, created, written and
    /// accessed now.
    pub(crate) fn write_new(&self, slot: &mut [u8]) {
        slot[13] = self.creation_tenths;
        slot[14..16].copy_from_slice(&self.creation_time.to_le_bytes());
        slot[16..18].copy_from_slice(&self.creation_date.to_le_bytes());
        self.write_modified(slot);
    }

    /// Write the write time, the write date and the access date into `slot`, and leave the
    /// creation fields: an entry changed now.
    pub(crate) fn write_modified(&self, slot: &mut [u8]) {
        slot[18..20].copy_from_slice(&self.access_date.to_le_bytes());
        slot[22..24].copy_from_slice(&self.write_time.to_le_bytes());
        slot[24..26].copy_from_slice(&self.write_date.to_le_bytes());
    }
}

/// Copy the time fields of the short entry `from` into `to`: a moved entry keeps its times.
pub(crate) fn copy_times(from: &[u8], to: &mut [u8]) {
    to[13..20].copy_from_slice(&from[13..20]);
    to[22..26].copy_from_slice(&from[22..26]);
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
