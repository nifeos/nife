//! **Time on a volume**: the clock a writer must supply, and the packed form a directory entry
//! stores.
//!
//! The crate owns no clock, and a write that stamps a time needs one, so every caller that writes
//! supplies a [`Clock`]. A caller with no time source says so with [`NoClock`], and the entries it
//! writes carry the zero timestamp, the same bytes a `mkfs` with no clock has always written.
//!
//! FAT stores a date as 16 bits (year from 1980, month, day), a time as 16 bits (hour, minute and
//! seconds in units of two), and a creation time's odd second as a tenths-of-a-second byte of 0 or
//! 100. It stores no time zone. In nife the time a clock supplies is UTC (ruled 2026-11-01 UTC). The
//! crate stores exactly the fields its clock reports and converts nothing, so it cannot check that a
//! clock gives UTC; that is the caller's duty. A reader that treats FAT times as local time, such as
//! Windows, shows a stored UTC value as local time. A read takes no clock and never writes.
//!
//! A time FAT cannot store is refused, not adjusted: the operation fails with `TimeOutOfRange`
//! before it writes a byte. A clock whose time is not trustworthy reports `None` and gets the zero
//! timestamp.
//!
//! Name: provisional. The clock half of a writer, ruled required on 2026-11-01 (UTC).

/// **A calendar date and time of day**, as plain numbers with no time zone.
///
/// FAT stores a date and a time of day with no time zone. In nife the value is UTC (ruled 2026-11-01
/// UTC). The crate stores these fields as given and converts nothing, so it cannot check that they
/// are UTC. A reader that treats FAT times as local time, such as Windows, shows a UTC value as
/// local time.
///
/// The fields need not be valid. A writer refuses a time FAT cannot store (see [`Clock`]) with
/// `TimeOutOfRange`, and writes nothing.
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
/// The time a clock supplies must be UTC (ruled 2026-11-01 UTC). FAT stores a date and a time of
/// day with no time zone, the crate writes the fields it is given and converts nothing, and it
/// cannot check that a clock gives UTC. A reader that treats FAT times as local time, such as
/// Windows, shows a UTC value as local time. A read takes no clock and never writes.
///
/// [`Clock::now`] returns `None` when there is no time to report, and the entry then carries the
/// zero timestamp. A clock whose time is not trustworthy (one that has not been set, or has lost
/// power) returns `None`; it does not guess. A time FAT cannot store is refused: a year before 1980
/// or after 2107, or a field outside its range (month 1 to 12, day 1 to 31, hour 0 to 23, minute
/// and second 0 to 59). The operation then fails with `TimeOutOfRange` before it writes a byte,
/// and the writer stays usable. The crate does not check a day against its month.
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
/// the zero timestamp in every entry it stamps, so an image built with it is reproducible. It
/// reports no time, so the question of a time zone does not arise (a clock that does report a time
/// must report UTC; see [`Clock`]).
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

/// **A time FAT cannot store**: what a clock reported was out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutOfRange;

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

    /// **Pack `now`**, or refuse it. `None` packs to [`Stamp::ZERO`]. A time FAT cannot store (a
    /// year outside 1980 to 2107, a month outside 1 to 12, a day outside 1 to 31, an hour past 23, a
    /// minute or second past 59) is [`OutOfRange`]. The check runs before any byte is written.
    pub(crate) fn pack(now: Option<Timestamp>) -> Result<Stamp, OutOfRange> {
        let Some(now) = now else {
            return Ok(Stamp::ZERO);
        };
        let legal = now.year >= FIRST_YEAR
            && now.year <= LAST_YEAR
            && (1..=12).contains(&now.month)
            && (1..=31).contains(&now.day)
            && now.hour <= 23
            && now.minute <= 59
            && now.second <= 59;
        if !legal {
            return Err(OutOfRange);
        }
        let date =
            ((now.year - FIRST_YEAR) << 9) + (u16::from(now.month) << 5) + u16::from(now.day);
        let time =
            (u16::from(now.hour) << 11) + (u16::from(now.minute) << 5) + u16::from(now.second / 2);
        Ok(Stamp {
            creation_tenths: (now.second % 2) * 100,
            creation_time: time,
            creation_date: date,
            access_date: date,
            write_time: time,
            write_date: date,
        })
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
