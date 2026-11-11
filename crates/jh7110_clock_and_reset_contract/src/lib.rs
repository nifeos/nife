//! **The JH7110 clock and reset wire**: the request and reply words a client and the clock and
//! reset server exchange, defined once for the server, the starters that call it and their tests.
//!
//! The JH7110 boots most of its blocks with their clocks gated and their resets asserted, and the
//! words that change that sit in three windows shared by every block of a domain. One server holds
//! those windows. A client holds an endpoint capability whose badge is its device's register base
//! physical address, as the device tree's `reg` gives it, and the server maps that badge to the
//! device's own clocks and resets and to nothing else. No request word names a device.
//!
//! A `CALL` carries two words and its reply two. Request word 0 carries the operation in bits
//! 63:56 ([`OPERATION_SHIFT`]) and, for the two reads, the device's own clock or reset index in bits
//! 7:0. Request word 1 carries [`SET_RATE`]'s most hertz. Reply word 0 is a [`Status`], reply word 1
//! the operation's answer. The wire is §320 (the JH7110 clock and reset wire) in
//! `design/decisions/`. See notes/jh7110-clock-and-reset.md.
//!
//! # BUGS
//!
//! - The server answers [`VERSION_QUERY`] with [`Status::UnknownOperation`]: the reply milestone
//!   1000 (message contracts carry bind-time version checks) defines does not exist yet, so a
//!   client built against another version is not refused at bind.
//!
//! Name: provisional. The crate the wire serves, `jh7110_clock_and_reset`, with the contract suffix
//! `_contract`.

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// This interface's identity in a program's `INTERFACES` note: 24, the JH7110 clock and reset
/// requests and replies. The number is this interface's for good: no other interface takes it,
/// and a format change is a new version, never a new number. A program declares the pair of this
/// id and [`VERSION`].
///
/// Name: provisional.
pub const INTERFACE_ID: u64 = 24;

/// The wire's version, one for the operation set, the statuses and the reply layouts together. A
/// break bumps it; an addition (a new operation, a new status) does not.
///
/// Name: provisional.
pub const VERSION: u32 = 1;

/// Where a request packs its operation: bits 63:56 of word 0, where `entropy_contract` puts its
/// own, so the contracts read alike.
///
/// Name: provisional.
pub const OPERATION_SHIFT: u32 = 56;

/// **Ask which versions the server serves.** Reserved for the query milestone 1000 (message
/// contracts carry bind-time version checks) defines; no arguments.
///
/// Name: provisional.
pub const VERSION_QUERY: u64 = 0;

/// **Bring the device up**: enable each of its clocks, each parent choice taken before its clock's
/// enable, then release each of its resets, in the server's plan order. No arguments. Idempotent,
/// since firmware may have started the block, so a client whose call failed calls again. Reply
/// word 1 is the [`Summary`], packed, whatever the status.
///
/// Name: provisional.
pub const BRING_UP: u64 = 1;

/// **Read one of the device's clock words**, by the device's own clock index in word 0 bits 7:0
/// (its position in the plan, not the domain's clock number). Reply word 1 is the raw word.
///
/// Name: provisional.
pub const READ_CLOCK: u64 = 2;

/// **Read one of the device's reset status words**, by the device's own reset index in word 0 bits
/// 7:0. Reply word 1 is the raw status word, in which a set bit is a line out of reset.
///
/// Name: provisional.
pub const READ_RESET: u64 = 3;

/// **Run the device's one rated clock at no more than word 1's hertz.** Reply word 1 is the hertz
/// it now runs at. Served only for a device with a rated clock, today the card clock of a Secure
/// Digital (SD) and Multi-Media Card (MMC) controller.
///
/// Name: provisional.
pub const SET_RATE: u64 = 4;

/// **Build request word 0**: `operation` and, for the two reads, the device's own `index`.
///
/// ```
/// use jh7110_clock_and_reset_contract::{READ_CLOCK, index, operation, request};
///
/// let w0 = request(READ_CLOCK, 2);
/// assert_eq!((operation(w0), index(w0)), (READ_CLOCK, 2));
/// ```
///
/// Name: provisional.
#[must_use]
pub const fn request(operation: u64, index: u8) -> u64 {
    ((operation & 0xff) << OPERATION_SHIFT) | index as u64
}

/// The operation of request word 0.
///
/// Name: provisional.
#[must_use]
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

/// The device's own clock or reset index in request word 0, bits 7:0.
///
/// Name: provisional.
#[must_use]
pub const fn index(w0: u64) -> u8 {
    (w0 & 0xff) as u8
}

/// **What reply word 0 says**, a small non-negative number. A kernel failure of the `CALL` itself
/// is a small negative, enormous as a `u64`, so [`Status::from_word`] answers `None` for it and the
/// two never collide.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Done.
    Done = 0,
    /// The badge names no device this server serves, badge 0 included.
    UnknownDevice = 1,
    /// An operation this server does not know.
    UnknownOperation = 2,
    /// An enable bit did not read back set.
    ClockNotRunning = 3,
    /// A reset did not read released within the poll bound.
    ResetHeld = 4,
    /// A read named a clock or reset beyond the device's own.
    IndexOutOfRange = 5,
    /// [`SET_RATE`] on a device with no rated clock.
    NoRatedClock = 6,
    /// No divider gives a rate at or below the one asked.
    NoRateAtOrBelow = 7,
}

impl Status {
    /// Every status, in number order.
    ///
    /// Name: provisional.
    pub const ALL: [Status; 8] = [
        Status::Done,
        Status::UnknownDevice,
        Status::UnknownOperation,
        Status::ClockNotRunning,
        Status::ResetHeld,
        Status::IndexOutOfRange,
        Status::NoRatedClock,
        Status::NoRateAtOrBelow,
    ];

    /// The status as reply word 0.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn to_word(self) -> u64 {
        self as u64
    }

    /// **Read reply word 0**: the status the server sent, or `None` for a word no server sends,
    /// a kernel error from the `CALL` among them.
    ///
    /// ```
    /// use jh7110_clock_and_reset_contract::Status;
    ///
    /// assert_eq!(Status::from_word(0), Some(Status::Done));
    /// assert_eq!(Status::from_word(-4i64 as u64), None); // the kernel refused the CALL
    /// ```
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn from_word(word: u64) -> Option<Status> {
        if word < Status::ALL.len() as u64 {
            Some(Status::ALL[word as usize])
        } else {
            None
        }
    }
}

/// **What a bring-up did**, reply word 1 of [`BRING_UP`], sent whatever the status so a failure is
/// diagnosable. Bits 7:0 count the clocks the plan enables, 15:8 those reading back as running,
/// 23:16 the resets in the plan, 31:24 those released. Bit 32 says the device was already up
/// before the call. Bits 47:40 count steps the server refused. The rest is zero.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// The clocks the plan enables.
    pub clocks_in_plan: u8,
    /// Of those, the ones whose enable bit read back set.
    pub clocks_running: u8,
    /// The resets the plan releases.
    pub resets_in_plan: u8,
    /// Of those, the ones that read released within the poll bound.
    pub resets_released: u8,
    /// Every clock read enabled and every reset released before the call wrote anything.
    pub already_up: bool,
    /// Steps the server refused to store: a plan and a domain that disagree.
    pub refused: u8,
}

/// The bits [`Summary::pack`] may set. A word with any other bit set came from no server.
const SUMMARY_BITS: u64 = 0xffff_ffff | 1 << 32 | 0xff << 40;

impl Summary {
    /// **Pack the summary into reply word 1.**
    ///
    /// ```
    /// use jh7110_clock_and_reset_contract::Summary;
    ///
    /// // The GMAC's bring-up of §320's example: six clocks enabled and running, two resets
    /// // released.
    /// let s = Summary {
    ///     clocks_in_plan: 6,
    ///     clocks_running: 6,
    ///     resets_in_plan: 2,
    ///     resets_released: 2,
    ///     already_up: false,
    ///     refused: 0,
    /// };
    /// assert_eq!(s.pack(), 0x0202_0606);
    /// assert_eq!(Summary::unpack(s.pack()), Some(s));
    /// ```
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn pack(&self) -> u64 {
        self.clocks_in_plan as u64
            | (self.clocks_running as u64) << 8
            | (self.resets_in_plan as u64) << 16
            | (self.resets_released as u64) << 24
            | (self.already_up as u64) << 32
            | (self.refused as u64) << 40
    }

    /// **Read reply word 1 of a bring-up**, or `None` when a bit outside the layout is set.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn unpack(word: u64) -> Option<Summary> {
        if word & !SUMMARY_BITS != 0 {
            return None;
        }
        Some(Summary {
            clocks_in_plan: word as u8,
            clocks_running: (word >> 8) as u8,
            resets_in_plan: (word >> 16) as u8,
            resets_released: (word >> 24) as u8,
            already_up: word >> 32 & 1 != 0,
            refused: (word >> 40) as u8,
        })
    }

    /// **The status a bring-up with this summary answers**: [`Status::ClockNotRunning`] when a
    /// clock did not read back running, else [`Status::ResetHeld`] when a reset stayed held, else
    /// [`Status::Done`]. A clock comes first because the plan enables every clock before it
    /// releases any reset, and a reset behind a gated clock may never release.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn status(&self) -> Status {
        if self.clocks_running < self.clocks_in_plan {
            Status::ClockNotRunning
        } else if self.resets_released < self.resets_in_plan {
            Status::ResetHeld
        } else {
            Status::Done
        }
    }
}

#[cfg(test)]
mod tests;
