//! **The NTPv4 wire format** (RFC 5905), as pure computation: no socket, clock, service or policy.
//!
//! The clock service owns the offset and the authority to set it; these 48 bytes and the arithmetic
//! over them are here. Unauthenticated NTPv4 only, with no NTS (RFC 8915) and no RFC 5905
//! symmetric-key MAC: an off-path attacker must guess the 64-bit nonce [`Query::accept`] checks and
//! an on-path one reads it, so a time from here is untrusted input the client may propose and never
//! set. [`Packet::parse`] is total on 48 bytes; [`Query::accept`] alone rejects. A timestamp counts
//! from 1900 in binary fractions ([`Timestamp`]), its seconds wrap on 7 February 2036 at 06:28:16
//! UTC ([`Timestamp::to_unix`]), and offsets are modular ([`Interval`]). See notes/ntp.md.

//! Name: ratified 2026-10-18 (calef, a crate-naming session). Kept: the NTPv4 standard's own full
//! name, so its `_protocol` is the standard's word and not nife's contract suffix. His answer was
//! "`network_time_protocol`". The stem `network_time` was ruled 2026-09-13: a standard's name stays
//! whole for a format or hardware and expands for a network protocol. Refused `ntp_proto` (milestone
//! 265 (`_proto` is a truncation)). The program `ntp` is `network_time_client` and its siblings.

#![no_std]

/// The UDP port NTP runs on. Here for the client that will eventually open the socket; this crate
/// never opens one.
pub const PORT: u16 = 123;

/// The wire size of an NTP packet with no extension fields and no MAC, in bytes.
///
/// We require exactly this and reject anything else ([`Reject::Length`]). A longer packet carries
/// extension fields or a MAC, and since this crate implements neither, accepting one would mean
/// silently ignoring authentication data a server went to the trouble of computing. Fail closed:
/// we send 48 bytes, a compliant server answers with 48 bytes, and anything else is not a reply to
/// the question we asked.
pub const PACKET_LEN: usize = 48;

/// The version this crate speaks and requires in a reply. NTPv4 (RFC 5905). A server echoes the
/// version it was asked in, so a reply that is not 4 did not come from a server we spoke to.
pub const VERSION: u8 = 4;

/// **Seconds from the NTP epoch (1 January 1900, 00:00:00 UTC) to the Unix epoch (1 January 1970).**
///
/// 70 years of 365 days plus the 17 leap days from 1904 to 1968 inclusive: 25567 days, 2208988800
/// seconds. 1900 itself is not a leap year (divisible by 100, not by 400), which is the arithmetic
/// this constant is most often got wrong by one day over.
pub const UNIX_DELTA: u64 = 2_208_988_800;

/// The number of NTP fraction ticks in one second: 2<sup>32</sup>. One tick is about 233 ps, which
/// is finer than a nanosecond, which is why nanoseconds survive a round trip through a
/// [`Timestamp`] and arbitrary tick values do not.
pub(crate) const TICKS_PER_SEC: u64 = 1 << 32;

const NANOS_PER_SEC: u64 = 1_000_000_000;

/// **The oldest Unix second [`Timestamp`] can represent**, under the era pivot: 0, the Unix epoch.
///
/// The pivot's own lower edge is 20 January 1968, but this crate's Unix seconds are unsigned (the
/// clock service has no use for a pre-1970 wall clock and a signed second only invites a sign bug),
/// so the reachable floor is the Unix epoch itself.
pub const UNIX_MIN: u64 = 0;

/// **The first Unix second [`Timestamp`] cannot represent**: 4233462144, which is 26 February 2104
/// at 09:42:24 UTC.
///
/// Past this the era pivot on [`Timestamp::to_unix`] would alias the value onto a date 136 years
/// earlier, so [`Timestamp::from_unix`] refuses it rather than returning a wrong answer.
/// `UNIX_MIN..UNIX_LIMIT` is exactly the range over which Unix → NTP → Unix is the identity, and
/// that is the property Kani proves.
pub(crate) const UNIX_LIMIT: u64 = (TICKS_PER_SEC - UNIX_DELTA) + (1 << 31);

/// The highest stratum a synchronised server may claim (RFC 5905 `MAXSTRAT`). 1 is a reference
/// clock, 15 is fifteen hops from one, 16 means "unsynchronised" and anything above is reserved.
/// A reply outside `1..=15` is not a time source ([`Reject::Stratum`]).
pub(crate) const MAX_STRATUM: u8 = 15;

/// The root distance we refuse outright, in [`Short`] units: 16 seconds, RFC 5905's `MAXDISP`.
///
/// This is deliberately far looser than the RFC's `MAXDIST` selection threshold (1 s by default).
/// The distinction is whose job it is: **this crate rejects the impossible, the clock service
/// applies policy.** A server whose root distance exceeds the protocol's own maximum dispersion is
/// describing something the protocol cannot mean; a server 200 ms away over a bad path is merely a
/// poor sample, and deciding what to do with a poor sample is the service's call, made with the
/// other samples in front of it.
pub(crate) const MAX_ROOT_DISTANCE: Short = Short(16 << 16);

/// The poll interval a request advertises, as log2 seconds: 6, which is 64 seconds. The server does
/// not act on it for a unicast client; it exists so the packet is well-formed and so a server that
/// rate-limits can see we are not asking often.
pub(crate) const DEFAULT_POLL: i8 = 6;

/// The NTP mode field (RFC 5905 §7.3). Three bits, in the low bits of the first octet.
pub mod mode {
    /// Symmetric active. We never send it and never accept it.
    pub const SYMMETRIC_ACTIVE: u8 = 1;
    /// Symmetric passive.
    pub const SYMMETRIC_PASSIVE: u8 = 2;
    /// **Client.** What a request is.
    pub const CLIENT: u8 = 3;
    /// **Server.** What a reply to a client request must be, and the only mode
    /// [`super::Query::accept`] will take.
    pub const SERVER: u8 = 4;
    /// Broadcast. Unsolicited by definition, so accepting one would mean taking time from a machine
    /// we never asked. Refused.
    pub const BROADCAST: u8 = 5;
}

/// The leap indicator (RFC 5905 §7.3). Two bits, the top of the first octet.
pub mod leap {
    /// No warning: the server is synchronised and the day is 86400 seconds long.
    pub const NONE: u8 = 0;
    /// The last minute of the day has 61 seconds (a leap second is being inserted).
    pub const INSERT: u8 = 1;
    /// The last minute of the day has 59 seconds (a leap second is being deleted).
    pub const DELETE: u8 = 2;
    /// **The clock is not synchronised.** The server is telling us its own time is not to be
    /// trusted, and [`super::Query::accept`] believes it.
    pub(crate) const UNSYNCHRONISED: u8 = 3;
}

// =================================================================================================
// Timestamps
// =================================================================================================

/// **An NTP 64-bit timestamp**: 32 bits of seconds since 1 January 1900, 32 bits of binary fraction.
///
/// Held as the raw 64-bit wire value, so a parse-serialise round trip is the identity by
/// construction and there is no "which representation is canonical" question to get wrong.
///
/// The two conversions that matter are [`Timestamp::from_unix`] and [`Timestamp::to_unix`], and the
/// era rule they share is documented on the latter.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// The all-zero timestamp. On the wire this means "unknown" rather than "1900": a server that
    /// has never synchronised sends zero, and a client's first request carries zero in the fields
    /// it cannot yet fill. [`Query::accept`] refuses a reply whose transmit timestamp is zero.
    pub const ZERO: Timestamp = Timestamp(0);

    /// Wrap a raw wire value (the 64 bits as they appear in the packet, host-order).
    pub const fn from_bits(bits: u64) -> Timestamp {
        Timestamp(bits)
    }

    /// The raw wire value.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// The seconds field: seconds since 1900, modulo 2<sup>32</sup>. Interpreting it needs the era
    /// rule; see [`Timestamp::to_unix`].
    pub const fn seconds(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The fraction field, in units of 2<sup>-32</sup> s.
    pub(crate) const fn fraction(self) -> u32 {
        self.0 as u32
    }

    /// Is this the "unknown" value? See [`Timestamp::ZERO`].
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// **Unix seconds and nanoseconds to an NTP timestamp.** `None` if `nanos` is not a valid
    /// sub-second (`>= 1_000_000_000`) or if `secs` is outside `UNIX_MIN..`[`UNIX_LIMIT`], where the
    /// era pivot could not read the result back.
    ///
    /// Refusing out of range rather than wrapping is the deliberate half of the 2036 handling: the
    /// conversion is only ever the identity's inverse inside the window, so outside it we return
    /// nothing instead of a value that would decode to a date 136 years off.
    pub const fn from_unix(secs: u64, nanos: u32) -> Option<Timestamp> {
        if nanos as u64 >= NANOS_PER_SEC || secs >= UNIX_LIMIT {
            return None;
        }
        // Wrapping is correct and intended here: for secs in range, the sum exceeds 2^32 exactly
        // when the instant is in era 0 (before 2036), and truncating to 32 bits is what the wire
        // format is. The era is recovered by the pivot on the way back.
        let ntp_secs = ((secs + UNIX_DELTA) % TICKS_PER_SEC) as u32;
        Some(Timestamp(
            ((ntp_secs as u64) << 32) | nanos_to_ticks(nanos) as u64,
        ))
    }

    /// **An NTP timestamp to Unix seconds and nanoseconds.** Total: every one of the 2<sup>64</sup>
    /// bit patterns decodes to some instant, because on the wire every one of them is one.
    ///
    /// The 32-bit seconds field rolls over every 136 years, so RFC 5905 §6's era rule is a fixed
    /// pivot, not a guess against the current time: high bit set (seconds ≥ 2<sup>31</sup>) is era
    /// 0, counted from 1900 (20 January 1968 to 7 February 2036 at 06:28:16 UTC); clear is era 1,
    /// counted from the rollover (to 26 February 2104). The window is
    /// [`UNIX_MIN`]`..`[`UNIX_LIMIT`], clipped at the Unix epoch because the seconds returned are
    /// unsigned, and the crate expires in 2104. See notes/ntp/the-era-pivot.md for why a
    /// nearest-era rule is worse.
    pub const fn to_unix(self) -> (u64, u32) {
        let s = self.seconds() as u64;
        let secs = if s >= (1 << 31) {
            // Era 0. UNIX_DELTA > 2^31, so this subtraction stays non-negative only for s at or
            // above the delta; below it (1968 to 1970) the instant predates the Unix epoch and we
            // saturate at 0 rather than return a negative second. That range is unreachable from
            // from_unix, so it costs the round trip nothing.
            s.saturating_sub(UNIX_DELTA)
        } else {
            // Era 1: add a full rollover before subtracting the epoch delta.
            s + TICKS_PER_SEC - UNIX_DELTA
        };
        (secs, ticks_to_nanos(self.fraction()))
    }

    /// **Replace the low `bits` of the fraction with random bits**, for use as the transmit
    /// timestamp of a request.
    ///
    /// [`Query::accept`] requires the reply's origin to match it exactly, so it is all an off-path
    /// attacker has to guess, and RFC 5905 §3 says to randomise its low-order bits. How many are
    /// random is the clock's precision, which the caller knows and this crate does not: a
    /// microsecond clock leaves about 12, which is 4096 guesses. `bits >= 32` randomises the whole
    /// fraction. With a real random source, prefer [`Query::with_nonce`] and its 64 random bits.
    pub const fn randomise_low(self, bits: u32, random: u32) -> Timestamp {
        if bits == 0 {
            return self;
        }
        let mask: u32 = if bits >= 32 {
            u32::MAX
        } else {
            (1 << bits) - 1
        };
        Timestamp((self.0 & !(mask as u64)) | (random & mask) as u64)
    }

    /// The signed difference `self - other`, in fraction ticks, taken **modulo 2<sup>64</sup>** and
    /// read back as signed.
    ///
    /// This is the RFC's arithmetic and it is not an implementation detail. Two absolute decodings
    /// subtracted would go wrong across the 2036 rollover by 2<sup>32</sup> seconds; wrapping
    /// subtraction is right there and everywhere else, as long as the true difference is under
    /// 68 years, which for the four timestamps of one exchange it always is.
    pub const fn since(self, other: Timestamp) -> Interval {
        Interval((self.0.wrapping_sub(other.0) as i64) as i128)
    }
}

/// Nanoseconds to fraction ticks, **rounded to nearest**. `nanos` must be under 1e9; the caller
/// checks.
///
/// Rounding, not truncating, and that is load-bearing rather than fussy. A tick is finer than a
/// nanosecond (2<sup>32</sup> > 1e9), so nanoseconds ought to survive a round trip, but truncating
/// in both directions loses one: 1 ns truncates to 4 ticks, and 4 ticks truncates back to 0 ns.
/// Rounding to nearest in both directions keeps the error under half a tick each way, which is under
/// an eighth of a nanosecond in total, so the value always lands back on the nanosecond it started
/// on. Kani proves it for all 1e9 of them.
const fn nanos_to_ticks(nanos: u32) -> u32 {
    let n = nanos as u64;
    (((n * TICKS_PER_SEC) + NANOS_PER_SEC / 2) / NANOS_PER_SEC) as u32
}

/// Fraction ticks to nanoseconds, rounded to nearest, clamped to a valid sub-second.
///
/// The clamp catches the two tick values (2<sup>32</sup>-2 and 2<sup>32</sup>-1) whose rounded
/// nanosecond is a whole second. Neither is producible by [`nanos_to_ticks`], so clamping cannot
/// disturb the nanosecond round trip; it exists so that **every** 64-bit pattern off the wire
/// decodes to a sub-second a `Duration` will accept, rather than one in 2<sup>31</sup> hostile
/// packets producing a nanosecond field of 1e9 for something downstream to trip over.
const fn ticks_to_nanos(ticks: u32) -> u32 {
    let n = ((ticks as u64 * NANOS_PER_SEC) + (TICKS_PER_SEC / 2)) / TICKS_PER_SEC;
    if n >= NANOS_PER_SEC {
        (NANOS_PER_SEC - 1) as u32
    } else {
        n as u32
    }
}

/// **The NTP short format**: 16 bits of seconds, 16 of fraction. Used for root delay and root
/// dispersion, which are durations rather than instants and never need 136 years of range.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct Short(pub u32);

impl Short {
    /// The value in nanoseconds. Exact enough: a 16.16 tick is about 15.3 µs, so nothing is lost
    /// that was ever there.
    pub const fn nanos(self) -> u64 {
        (self.0 as u64 * NANOS_PER_SEC) >> 16
    }

    /// The whole-seconds part.
    pub const fn secs(self) -> u16 {
        (self.0 >> 16) as u16
    }
}

// =================================================================================================
// Intervals
// =================================================================================================

/// **A signed time interval, in NTP fraction ticks** (2<sup>-32</sup> s), held in `i128`.
///
/// `i128` rather than `i64` because the inputs are hostile. Two timestamps 68 years apart differ by
/// 2<sup>63</sup> ticks, and the offset formula adds two such differences, so an `i64` accumulator
/// would overflow on a packet an attacker can send for free. Widening costs nothing here (this is
/// arithmetic done once per sample, not per byte) and it means the computation is **total**: there
/// is no input, however absurd, on which it panics. Kani proves that.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct Interval(i128);

impl Interval {
    /// Zero.
    pub const ZERO: Interval = Interval(0);

    /// Wrap a tick count.
    pub const fn from_ticks(ticks: i128) -> Interval {
        Interval(ticks)
    }

    /// The interval in ticks of 2<sup>-32</sup> s.
    pub const fn ticks(self) -> i128 {
        self.0
    }

    /// The interval in nanoseconds, truncated toward zero. This is the form the clock service wants:
    /// it holds an offset it can add to a monotonic reading.
    pub const fn nanos(self) -> i128 {
        (self.0 * NANOS_PER_SEC as i128) / TICKS_PER_SEC as i128
    }

    /// The interval in microseconds, truncated toward zero. For logging, where nanoseconds are noise.
    pub const fn micros(self) -> i128 {
        (self.0 * 1_000_000) / TICKS_PER_SEC as i128
    }

    /// Is this interval negative?
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// The magnitude, in ticks.
    pub const fn abs_ticks(self) -> u128 {
        self.0.unsigned_abs()
    }
}

// =================================================================================================
// The packet
// =================================================================================================

/// **A parsed NTP packet.** The 48 bytes of RFC 5905 §7.3, decoded field by field and judged not at
/// all: [`Packet::parse`] cannot reject a 48-byte input, so every judgement is in one place
/// ([`Query::accept`]) and parse-then-serialise is the identity on every packet that exists.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Packet {
    /// Leap indicator, 2 bits. See [`leap`].
    pub leap: u8,
    /// Version number, 3 bits. See [`VERSION`].
    pub version: u8,
    /// Association mode, 3 bits. See [`mode`].
    pub mode: u8,
    /// Distance from a reference clock in hops. 1 is a reference clock itself, 0 is either
    /// "unspecified" or a kiss-o'-death, 16 is unsynchronised. See [`MAX_STRATUM`].
    pub stratum: u8,
    /// Log2 of the maximum interval between messages, in seconds.
    pub poll: i8,
    /// Log2 of the sender's clock precision, in seconds. Negative in practice: -23 is about 119 ns.
    pub precision: i8,
    /// Round-trip delay to the reference clock.
    pub root_delay: Short,
    /// Maximum error relative to the reference clock.
    pub root_dispersion: Short,
    /// Four bytes identifying the reference source. For stratum 1 it is ASCII naming the clock
    /// (`GPS`, `PPS`, `DCF`); for stratum 2 and up it is (in IPv4) the upstream server's address;
    /// for stratum 0 it is the **kiss code**, which is why it is bytes here and not a `u32`.
    pub reference_id: [u8; 4],
    /// When the server last set or corrected its own clock.
    pub reference: Timestamp,
    /// **T1.** The client's transmit timestamp, echoed back. The nonce, and the whole of plain NTP's
    /// off-path spoofing resistance.
    pub origin: Timestamp,
    /// **T2.** When the request arrived at the server, on the server's clock.
    pub receive: Timestamp,
    /// **T3.** When the reply left the server, on the server's clock.
    pub transmit: Timestamp,
}

impl Packet {
    /// **Decode 48 bytes.** The only way this fails is a wrong length; every 48-byte input is a
    /// packet, because on the wire every 48-byte input is one. Semantic checks live in
    /// [`Query::accept`].
    pub fn parse(wire: &[u8]) -> Result<Packet, Reject> {
        if wire.len() != PACKET_LEN {
            return Err(Reject::Length(wire.len()));
        }
        Ok(Packet {
            leap: wire[0] >> 6,
            version: (wire[0] >> 3) & 0b111,
            mode: wire[0] & 0b111,
            stratum: wire[1],
            poll: wire[2] as i8,
            precision: wire[3] as i8,
            root_delay: Short(be32(wire, 4)),
            root_dispersion: Short(be32(wire, 8)),
            reference_id: [wire[12], wire[13], wire[14], wire[15]],
            reference: Timestamp(be64(wire, 16)),
            origin: Timestamp(be64(wire, 24)),
            receive: Timestamp(be64(wire, 32)),
            transmit: Timestamp(be64(wire, 40)),
        })
    }

    /// **Encode 48 bytes.** The exact inverse of [`Packet::parse`] on any packet that parse
    /// produced. Fields wider than the wire allows (a `leap` above 3, a `version` above 7) are
    /// masked to their field width rather than rejected, because a `Packet` you built by hand with
    /// a nonsense field is a programming error, not a wire event, and the wire has no room to
    /// express it.
    pub fn to_bytes(&self) -> [u8; PACKET_LEN] {
        let mut w = [0u8; PACKET_LEN];
        w[0] = ((self.leap & 0b11) << 6) | ((self.version & 0b111) << 3) | (self.mode & 0b111);
        w[1] = self.stratum;
        w[2] = self.poll as u8;
        w[3] = self.precision as u8;
        w[4..8].copy_from_slice(&self.root_delay.0.to_be_bytes());
        w[8..12].copy_from_slice(&self.root_dispersion.0.to_be_bytes());
        w[12..16].copy_from_slice(&self.reference_id);
        w[16..24].copy_from_slice(&self.reference.0.to_be_bytes());
        w[24..32].copy_from_slice(&self.origin.0.to_be_bytes());
        w[32..40].copy_from_slice(&self.receive.0.to_be_bytes());
        w[40..48].copy_from_slice(&self.transmit.0.to_be_bytes());
        w
    }

    /// **The synchronisation distance to the root**, RFC 5905's λ: half the root delay plus the root
    /// dispersion. The server's own claim about how far from a real clock it is. Compared against
    /// [`MAX_ROOT_DISTANCE`] by [`Query::accept`]; saturating, so a server claiming the maximum of
    /// both fields gets a large answer rather than a wrapped small one.
    pub(crate) const fn root_distance(&self) -> Short {
        Short((self.root_delay.0 / 2).saturating_add(self.root_dispersion.0))
    }

    /// Is the reference id a kiss code? True exactly when the stratum is 0, which is what makes the
    /// four bytes ASCII (`DENY`, `RSTR`, `RATE`) instead of an address. See [`Reject::KissOfDeath`].
    pub(crate) const fn is_kiss_of_death(&self) -> bool {
        self.stratum == 0
    }
}

const fn be32(w: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([w[at], w[at + 1], w[at + 2], w[at + 3]])
}

const fn be64(w: &[u8], at: usize) -> u64 {
    u64::from_be_bytes([
        w[at],
        w[at + 1],
        w[at + 2],
        w[at + 3],
        w[at + 4],
        w[at + 5],
        w[at + 6],
        w[at + 7],
    ])
}

// =================================================================================================
// The exchange
// =================================================================================================

/// **Why a reply was not an answer to our question.**
///
/// One variant per check [`Query::accept`] makes, so a client can tell a broken server from an
/// attack, and a kiss-o'-death (an instruction to back off) from a failure to retry.
///
/// # Examples
///
/// The nonce check runs before any of the packet is believed, so an off-path flood of well-formed
/// replies gets nowhere:
///
/// ```
/// use network_time_protocol::{Packet, Query, Reject, Timestamp, VERSION, leap, mode};
///
/// let sent = Timestamp::from_unix(1_800_000_000, 0).unwrap();
/// let query = Query::with_nonce(sent, Timestamp::from_bits(0x9e37_79b9_7f4a_7c15));
///
/// // A perfect packet from a perfect server, claiming the year 2000. It is simply not ours.
/// let spoof = Packet {
///     leap: leap::NONE,
///     version: VERSION,
///     mode: mode::SERVER,
///     stratum: 1,
///     origin: Timestamp::from_bits(0), // the attacker had to guess, and 64 bits is a lot
///     receive: Timestamp::from_unix(946_684_800, 0).unwrap(),
///     transmit: Timestamp::from_unix(946_684_800, 1).unwrap(),
///     ..Packet::default()
/// };
/// let dest = Timestamp::from_unix(1_800_000_000, 1_000_000).unwrap();
/// assert_eq!(query.accept(&spoof.to_bytes(), dest), Err(Reject::OriginMismatch));
///
/// // An unsolicited broadcast does not even get that far: mode 5 is not a reply.
/// let broadcast = Packet { mode: 5, ..spoof };
/// assert_eq!(query.accept(&broadcast.to_bytes(), dest), Err(Reject::Mode(5)));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reject {
    /// Not 48 bytes. See [`PACKET_LEN`] for why nothing else is accepted.
    Length(usize),
    /// The version is not [`VERSION`]. A server echoes the version it was asked, so this reply is
    /// not to our request.
    Version(u8),
    /// The mode is not [`mode::SERVER`]. Broadcast and symmetric packets are unsolicited by
    /// construction; taking time from one would mean taking it from a machine we never asked.
    Mode(u8),
    /// **Stratum 0: a kiss-o'-death.** Not a time, an instruction. The payload is the four-byte kiss
    /// code: `RATE` (you are polling too fast, back off), `DENY` or `RSTR` (go away and do not come
    /// back). A client that treats this as a mere failure and retries is the abusive client the
    /// packet exists to stop, so it is its own variant.
    KissOfDeath([u8; 4]),
    /// The stratum is above [`MAX_STRATUM`]: 16 is the server saying it is unsynchronised, above
    /// that is reserved. Either way it is not a time source.
    Stratum(u8),
    /// The leap indicator is [`leap::UNSYNCHRONISED`]. The server is telling us not to trust it.
    Unsynchronised,
    /// **The origin timestamp is not the nonce we sent.** The load-bearing check: this is what an
    /// off-path attacker, who cannot see our request, has to guess. Also what stops a stale reply to
    /// an earlier request being read as an answer to this one.
    OriginMismatch,
    /// The server's transmit timestamp is zero, which on the wire means "I do not know what time it
    /// is". Believing it would set our clock to 1900.
    TransmitZero,
    /// The server received the request after it sent the reply (T3 < T2). Impossible, so the
    /// timestamps are fabricated or the server is broken.
    ServerOrderReversed,
    /// We received the reply before we sent the request (T4 < T1). Same, on our side of the wire.
    ClientOrderReversed,
    /// The round trip took less time than the server spent thinking, so the delay is negative and
    /// the offset computed from it is meaningless.
    NegativeDelay,
    /// The server's own claimed distance from a reference clock exceeds [`MAX_ROOT_DISTANCE`].
    RootDistance,
}

/// **One accepted sample**: the two numbers the protocol exists to produce, and the packet they came
/// from.
///
/// The packet rides along because the clock service needs it to apply policy. Stratum, root distance
/// and leap indicator are how it decides whether to believe a sample, and that decision is the
/// service's, not this crate's; here they have only been checked for being possible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sample {
    /// **How far our clock is behind the server's**, so a reader adds it. RFC 5905's θ:
    /// `((T2 - T1) + (T3 - T4)) / 2`, which cancels the path delay exactly when the path is
    /// symmetric and is wrong by half the asymmetry when it is not. Truncated toward zero by the
    /// halving, which is under a quarter of a nanosecond.
    pub offset: Interval,
    /// **The round-trip time, less the server's turnaround.** RFC 5905's δ:
    /// `(T4 - T1) - (T3 - T2)`. Never negative in an accepted sample ([`Reject::NegativeDelay`]),
    /// and the bound on how wrong `offset` can be: the true offset is within ±δ/2 of it.
    pub delay: Interval,
    /// The reply, in full, for the policy that is not this crate's to apply.
    pub response: Packet,
}

/// **A request in flight**, holding the two things a reply has to be checked against: the nonce we
/// put on the wire, and the local time we actually sent at.
///
/// Separating them buys real security; see [`Query::with_nonce`].
///
/// # Examples
///
/// One exchange: the client is 2 seconds behind the server and the path takes 20 ms each way.
///
/// ```
/// use network_time_protocol::{Packet, Query, Timestamp, VERSION, leap, mode};
///
/// // T1: what our clock said when we sent. The nonce is separate, and is 64 random bits.
/// let sent = Timestamp::from_unix(1_800_000_000, 0).unwrap();
/// let nonce = Timestamp::from_bits(0x9e37_79b9_7f4a_7c15);
/// let query = Query::with_nonce(sent, nonce);
///
/// // On the wire the transmit field is the nonce, not the time; a server never interprets it.
/// let request = Packet::parse(&query.request()).unwrap();
/// assert_eq!(request.transmit, nonce);
/// assert_eq!(request.mode, mode::CLIENT);
///
/// // A server whose clock is 2 seconds ahead of ours, taking 1 ms to turn the request around.
/// let reply = Packet {
///     leap: leap::NONE,
///     version: VERSION,
///     mode: mode::SERVER,
///     stratum: 2,
///     origin: request.transmit, // echoed, and this is the whole off-path defence
///     receive: Timestamp::from_unix(1_800_000_002, 20_000_000).unwrap(), // T2
///     transmit: Timestamp::from_unix(1_800_000_002, 21_000_000).unwrap(), // T3
///     ..Packet::default()
/// };
///
/// let dest = Timestamp::from_unix(1_800_000_000, 41_000_000).unwrap();
/// let sample = query.accept(&reply.to_bytes(), dest).unwrap();
///
/// assert_eq!(sample.offset.nanos() / 1_000_000, 2_000);
/// assert_eq!(sample.delay.nanos() / 1_000_000, 40);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Query {
    nonce: Timestamp,
    sent: Timestamp,
    poll: i8,
}

impl Query {
    /// **The plain form.** The transmit timestamp on the wire is the local send time, and the only
    /// unpredictability an attacker faces is whatever the clock's low fraction bits give (see
    /// [`Timestamp::randomise_low`], which the caller should apply first).
    pub const fn new(sent: Timestamp) -> Query {
        Query {
            nonce: sent,
            sent,
            poll: DEFAULT_POLL,
        }
    }

    /// **The hardened form, and it is worth understanding why it is free.**
    ///
    /// A server never interprets the client's transmit timestamp. It copies it into the origin field
    /// and does nothing else with it. So the value on the wire does not have to be a time at all: it
    /// can be 64 random bits, kept next to the true send time locally. The reply still has to match
    /// it exactly, and now an off-path attacker has 64 bits to guess instead of the dozen the clock's
    /// precision leaks. This is what chrony does by default, and it costs one extra field here.
    ///
    /// `sent` is the true local transmit time and is what the offset arithmetic uses as T1; `nonce`
    /// is what goes on the wire and what the reply's origin field is checked against.
    pub const fn with_nonce(sent: Timestamp, nonce: Timestamp) -> Query {
        Query {
            nonce,
            sent,
            poll: DEFAULT_POLL,
        }
    }

    /// Advertise a different poll interval (log2 seconds). See [`DEFAULT_POLL`].
    pub const fn with_poll(mut self, poll: i8) -> Query {
        self.poll = poll;
        self
    }

    /// The nonce this query put on the wire. A reply's origin field must equal it.
    pub const fn nonce(&self) -> Timestamp {
        self.nonce
    }

    /// The local time the request was sent (T1).
    pub const fn sent(&self) -> Timestamp {
        self.sent
    }

    /// **The 48 bytes to send**: a version 4, mode 3 client packet, all fields but the transmit
    /// timestamp zero. Zero is the honest value for the rest: we are a client, we have no stratum,
    /// no reference and no idea what time it is, and saying so is both correct and the smallest
    /// fingerprint.
    pub fn request(&self) -> [u8; PACKET_LEN] {
        Packet {
            leap: leap::NONE,
            version: VERSION,
            mode: mode::CLIENT,
            stratum: 0,
            poll: self.poll,
            precision: 0,
            root_delay: Short(0),
            root_dispersion: Short(0),
            reference_id: [0; 4],
            reference: Timestamp::ZERO,
            origin: Timestamp::ZERO,
            receive: Timestamp::ZERO,
            transmit: self.nonce,
        }
        .to_bytes()
    }

    /// **Check a reply and compute the sample.** `dest` is T4: the local time the reply arrived,
    /// read as close to the receive as the caller can manage.
    ///
    /// Refuses, in this order: a length other than 48; a version other than 4 or a mode other than
    /// server; an origin that is not our nonce, checked before anything else is believed; a
    /// kiss-o'-death, a stratum above [`MAX_STRATUM`] or an unsynchronised leap indicator; a zero
    /// transmit timestamp; timestamps against causality (T2 ≤ T3, T1 ≤ T4, a round trip no shorter
    /// than the turnaround); a root distance past [`MAX_ROOT_DISTANCE`]. Each is its [`Reject`]
    /// variant. None authenticates: an on-path attacker sees the nonce. See notes/ntp.md.
    pub fn accept(&self, wire: &[u8], dest: Timestamp) -> Result<Sample, Reject> {
        let p = Packet::parse(wire)?;

        if p.version != VERSION {
            return Err(Reject::Version(p.version));
        }
        if p.mode != mode::SERVER {
            return Err(Reject::Mode(p.mode));
        }
        // Before anything else about the packet is believed: is it ours?
        if p.origin != self.nonce {
            return Err(Reject::OriginMismatch);
        }
        if p.is_kiss_of_death() {
            return Err(Reject::KissOfDeath(p.reference_id));
        }
        if p.stratum > MAX_STRATUM {
            return Err(Reject::Stratum(p.stratum));
        }
        if p.leap == leap::UNSYNCHRONISED {
            return Err(Reject::Unsynchronised);
        }
        if p.transmit.is_zero() {
            return Err(Reject::TransmitZero);
        }
        if p.root_distance() > MAX_ROOT_DISTANCE {
            return Err(Reject::RootDistance);
        }

        // Modular differences throughout (Timestamp::since), so an exchange across the 2036
        // rollover is right rather than 136 years out.
        let server = p.transmit.since(p.receive); // T3 - T2
        if server.is_negative() {
            return Err(Reject::ServerOrderReversed);
        }
        let round = dest.since(self.sent); // T4 - T1
        if round.is_negative() {
            return Err(Reject::ClientOrderReversed);
        }
        let delay = Interval(round.0 - server.0);
        if delay.is_negative() {
            return Err(Reject::NegativeDelay);
        }

        let offset = Interval((p.receive.since(self.sent).0 + p.transmit.since(dest).0) / 2);

        Ok(Sample {
            offset,
            delay,
            response: p,
        })
    }
}

// =================================================================================================
// Host tests. Milliseconds, no emulator: this crate is arithmetic and byte layout, and both are the
// kind of thing that is cheap to pin and expensive to debug from a boot log.
// =================================================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// A server reply, assembled by hand from the RFC 5905 §7.3 field layout rather than captured
    /// off a wire, and said plainly because the difference matters: these bytes prove the decoder
    /// agrees with the *specification*, which is the thing a capture could not tell us (a capture
    /// only proves we agree with whatever produced it).
    ///
    /// It describes a stratum 2 server synchronised to 17.253.14.251, with a 50.8 ms root delay and
    /// 23.4 ms root dispersion, replying to a request sent at 2026-07-30 12:00:00 UTC (plus a random
    /// nonce fraction) with a server clock 0.5 s ahead of ours.
    const REPLY: [u8; PACKET_LEN] = [
        0x24, 0x02, 0x06, 0xe9, 0x00, 0x00, 0x0d, 0x01, // LI/VN/mode, stratum, poll, prec
        0x00, 0x00, 0x05, 0xfe, 0x11, 0xfd, 0x0e, 0xfb, // root delay, root dispersion, refid
        0xee, 0x15, 0xba, 0xb7, 0x1c, 0x8a, 0x3d, 0x00, // reference
        0xee, 0x15, 0xbb, 0x40, 0x5b, 0x3f, 0x7a, 0x91, // origin (T1, our nonce)
        0xee, 0x15, 0xbb, 0x40, 0xde, 0x31, 0x24, 0x8d, // receive (T2)
        0xee, 0x15, 0xbb, 0x40, 0xde, 0x4b, 0x5b, 0x70, // transmit (T3)
    ];

    /// T1, as it appears in the packet above.
    const T1: Timestamp = Timestamp::from_bits(0xee15_bb40_5b3f_7a91);
    /// T4: the reply arrived 23.1 ms after we sent, on our clock.
    const T4: Timestamp = Timestamp::from_bits(0xee15_bb40_6129_5c42);

    fn query() -> Query {
        Query::new(T1)
    }

    /// Mutate a copy of the good reply, so every rejection test differs from an acceptance by
    /// exactly the bytes under test.
    fn tweak(f: impl FnOnce(&mut [u8; PACKET_LEN])) -> [u8; PACKET_LEN] {
        let mut w = REPLY;
        f(&mut w);
        w
    }

    /// 1 January 1970 00:00:00 UTC is NTP seconds 0x83AA7E80, a value any NTP reference can check.
    /// The usual error, treating 1900 as a leap year, is exactly one day.
    #[test]
    fn the_epoch_delta_is_the_number_everyone_gets_wrong() {
        assert_eq!(UNIX_DELTA, 0x83AA_7E80);
        let epoch = Timestamp::from_unix(0, 0).unwrap();
        assert_eq!(epoch.seconds(), 0x83AA_7E80);
        assert_eq!(epoch.fraction(), 0);
        assert_eq!(epoch.to_unix(), (0, 0));
    }

    /// NTP seconds 0 is the rollover instant, 7 February 2036 06:28:16 UTC, Unix second 2085978496;
    /// a naive decoder makes it 1 January 1900. The second before it, NTP seconds 0xFFFFFFFF, is in
    /// era 0, and the two are adjacent in Unix time although the seconds field wrapped between
    /// them.
    #[test]
    fn the_2036_rollover_decodes_to_2036_and_not_to_1900() {
        let rollover = Timestamp::from_bits(0);
        assert_eq!(rollover.to_unix(), (2_085_978_496, 0));

        let before = Timestamp::from_bits(0xFFFF_FFFF << 32);
        assert_eq!(before.to_unix(), (2_085_978_495, 0));

        assert_eq!(rollover.to_unix().0 - before.to_unix().0, 1);
    }

    #[test]
    fn the_pivot_has_edges_and_they_are_where_the_docs_say() {
        // Just inside the window round trips.
        let last = Timestamp::from_unix(UNIX_LIMIT - 1, 999_999_999).unwrap();
        assert_eq!(last.to_unix(), (UNIX_LIMIT - 1, 999_999_999));
        // Just outside is refused rather than aliased: UNIX_LIMIT would encode to the same seconds
        // field as 1968-01-20.
        assert!(Timestamp::from_unix(UNIX_LIMIT, 0).is_none());
        assert!(Timestamp::from_unix(u64::MAX, 0).is_none());
        // A nanosecond field that is not a sub-second is a caller bug, not a rounding opportunity.
        assert!(Timestamp::from_unix(0, 1_000_000_000).is_none());
    }

    /// **Every nanosecond there is, checked by running the code on all 10^9 of them**: 0.6 s, so
    /// exhaustive, where Kani cannot finish (notes/ntp/what-is-proved.md has the measurements).
    /// It needs the optimised build `[profile.dev.package.network_time_protocol]` in the workspace
    /// manifest supplies, as `measured_boot` has for SHA-256. Under Miri, about a thousand times
    /// slower, it walks every 999,983rd value (a prime, so the samples miss any power-of-two
    /// structure) plus the edges, and "Miri-clean" means those values
    /// (notes/undefined-behavior.md).
    #[test]
    fn every_nanosecond_survives_the_round_trip() {
        #[cfg(miri)]
        let domain = (0..1_000_000_000u32).step_by(999_983).chain([999_999_999]);
        #[cfg(not(miri))]
        let domain = 0..1_000_000_000u32;
        for n in domain {
            assert_eq!(
                ticks_to_nanos(nanos_to_ticks(n)),
                n,
                "{n} ns did not survive"
            );
        }
        // The same thing through the public conversion, on the cases that break a truncating
        // implementation. 1 ns is 4.29 ticks: truncate both ways and it comes back 0.
        for n in [0, 1, 2, 999_999_998, 999_999_999] {
            let t = Timestamp::from_unix(1_785_412_800, n).unwrap();
            assert_eq!(t.to_unix(), (1_785_412_800, n));
        }
        assert_eq!(nanos_to_ticks(1), 4);
        assert_eq!(ticks_to_nanos(4), 1);
    }

    #[test]
    fn a_hostile_fraction_still_decodes_to_a_valid_sub_second() {
        // The two tick values whose rounded nanosecond is a whole second. Nothing this crate emits
        // produces them; a packet off the wire can carry them, and a nanos field of 1e9 would blow
        // up a Duration downstream.
        assert_eq!(ticks_to_nanos(u32::MAX), 999_999_999);
        assert_eq!(ticks_to_nanos(u32::MAX - 1), 999_999_999);
    }

    #[test]
    fn the_hand_assembled_reply_decodes_field_by_field() {
        let p = Packet::parse(&REPLY).unwrap();
        assert_eq!(p.leap, leap::NONE);
        assert_eq!(p.version, 4);
        assert_eq!(p.mode, mode::SERVER);
        assert_eq!(p.stratum, 2);
        assert_eq!(p.poll, 6);
        assert_eq!(p.precision, -23);
        assert_eq!(p.root_delay, Short(0x0D01));
        assert_eq!(p.root_dispersion, Short(0x05FE));
        assert_eq!(p.reference_id, [17, 253, 14, 251]);
        assert_eq!(p.origin, T1);
        // 0x0D01 in 16.16 is 3329/65536 s = 50.796 ms.
        assert_eq!(p.root_delay.nanos(), 50_796_508);
        // The reference timestamp is 137 s before T1: the server last corrected itself at 11:57:43.
        assert_eq!(p.reference.to_unix().0, 1_785_412_663);
        assert_eq!(p.receive.to_unix(), (1_785_412_800, 867_937_359));
    }

    #[test]
    fn parse_then_serialise_is_the_identity() {
        assert_eq!(Packet::parse(&REPLY).unwrap().to_bytes(), REPLY);
        // Including on bytes that are not a sensible packet at all: parse judges nothing.
        let junk = [0xFFu8; PACKET_LEN];
        assert_eq!(Packet::parse(&junk).unwrap().to_bytes(), junk);
        let zeros = [0u8; PACKET_LEN];
        assert_eq!(Packet::parse(&zeros).unwrap().to_bytes(), zeros);
    }

    #[test]
    fn a_request_is_a_version_4_client_packet_carrying_only_the_nonce() {
        let nonce = Timestamp::from_bits(0x1234_5678_9abc_def0);
        let w = Query::with_nonce(T1, nonce).request();
        assert_eq!(w[0], 0b00_100_011, "LI 0, VN 4, mode 3");
        assert_eq!(w[1], 0, "no stratum: we are a client, not a source");
        assert_eq!(w[2] as i8, DEFAULT_POLL);
        assert_eq!(
            &w[3..24],
            &[0u8; 21],
            "a client claims no precision, no root distance, no reference"
        );
        assert_eq!(
            &w[24..40],
            &[0u8; 16],
            "origin and receive are ours to leave empty"
        );
        assert_eq!(&w[40..48], &nonce.bits().to_be_bytes());
        // And it is exactly one datagram's worth, which is what makes the strict length check on
        // the reply symmetric rather than arbitrary.
        assert_eq!(w.len(), PACKET_LEN);
    }

    /// Checkable by hand: the server clock is 0.5 s ahead, the request took 11.5 ms to arrive, the
    /// server spent 0.4 ms, and the reply took 11.2 ms back.
    ///
    /// ```text
    ///   offset = ((T2-T1) + (T3-T4))/2 = (0.5115 + 0.4888)/2 = 0.50015 s
    ///   delay  = (T4-T1) - (T3-T2)     = 0.0231 - 0.0004    = 0.0227 s
    /// ```
    #[test]
    fn the_offset_and_delay_are_the_numbers_the_rfc_formula_gives() {
        let s = query().accept(&REPLY, T4).unwrap();
        // 1 ns under 500_150_000: the fixed-point representation of 0.5115 s is not exact.
        assert_eq!(s.offset.nanos(), 500_149_999);
        assert_eq!(s.delay.nanos(), 22_700_000);
        assert_eq!(s.offset.micros(), 500_149);
        assert!(!s.delay.is_negative());
        assert_eq!(s.response.stratum, 2);
    }

    /// The bound the protocol promises. The path is asymmetric by 0.3 ms, so the true offset
    /// (0.5 s) and the computed one differ by 0.15 ms, inside delay/2 (11.35 ms). A client that
    /// trusts an offset further than this is trusting the network.
    #[test]
    fn the_offset_is_within_half_the_delay_of_the_truth() {
        let s = query().accept(&REPLY, T4).unwrap();
        let truth = 500_000_000i128;
        assert!((s.offset.nanos() - truth).abs() <= s.delay.nanos() / 2);
    }

    /// The reason the arithmetic is modular. T1 is two seconds before the rollover (seconds field
    /// 0xFFFFFFFE) and T4 one second after (0x00000001), so the raw seconds go DOWN across the
    /// exchange: decoding then subtracting gives a delay of minus 136 years, wrapping gives three
    /// seconds. Round trip 3 s and turnaround 1.5 s make the delay 1.5 s, and the offset
    /// ((T2-T1) + (T3-T4))/2 = (1 + -0.5)/2 = 0.25 s. T2 and T3 straddle the era boundary.
    #[test]
    fn an_exchange_across_the_2036_rollover_comes_out_right() {
        let t1 = Timestamp::from_bits(0xFFFF_FFFE_0000_0000);
        let t2 = Timestamp::from_bits(0xFFFF_FFFF_0000_0000);
        let t3 = Timestamp::from_bits(0x0000_0000_8000_0000);
        let t4 = Timestamp::from_bits(0x0000_0001_0000_0000);

        let mut p = Packet::parse(&REPLY).unwrap();
        p.origin = t1;
        p.receive = t2;
        p.transmit = t3;
        let s = Query::new(t1).accept(&p.to_bytes(), t4).unwrap();
        assert_eq!(s.delay.nanos(), 1_500_000_000);
        assert_eq!(s.offset.nanos(), 250_000_000);
        assert_eq!(t2.to_unix().0 + 1, t3.to_unix().0);
    }

    #[test]
    fn a_reply_to_someone_else_is_refused() {
        // The load-bearing check. An attacker who cannot see our request has to produce this field,
        // and one bit wrong is enough.
        let w = tweak(|w| w[31] ^= 1);
        assert_eq!(query().accept(&w, T4), Err(Reject::OriginMismatch));
        // A zero origin (a server that echoed nothing) is not a pass either.
        let w = tweak(|w| w[24..32].fill(0));
        assert_eq!(query().accept(&w, T4), Err(Reject::OriginMismatch));
        // And the check is against the NONCE, not the send time: with a random nonce, a reply
        // echoing our true send time is a forgery.
        let nonce = Timestamp::from_bits(0xdead_beef_0bad_f00d);
        let q = Query::with_nonce(T1, nonce);
        assert_eq!(q.accept(&REPLY, T4), Err(Reject::OriginMismatch));
        // while one echoing the nonce is accepted, and the arithmetic still uses the true T1.
        let w = tweak(|w| w[24..32].copy_from_slice(&nonce.bits().to_be_bytes()));
        assert_eq!(q.accept(&w, T4).unwrap().offset.nanos(), 500_149_999);
    }

    /// A broadcast packet is unsolicited, so nobody should believe it, even carrying our origin
    /// timestamp as it does here and passing the check usually thought of as "the" check.
    #[test]
    fn the_wrong_mode_is_refused_even_when_everything_else_is_perfect() {
        for m in [
            mode::SYMMETRIC_ACTIVE,
            mode::SYMMETRIC_PASSIVE,
            mode::CLIENT,
            mode::BROADCAST,
            6,
            7,
        ] {
            let w = tweak(|w| w[0] = (w[0] & !0b111) | m);
            assert_eq!(query().accept(&w, T4), Err(Reject::Mode(m)));
        }
        // A version that is not 4 did not come from a server answering our request.
        let w = tweak(|w| w[0] = (w[0] & !0b111000) | (3 << 3));
        assert_eq!(query().accept(&w, T4), Err(Reject::Version(3)));
    }

    /// Stratum 0 with the kiss code in the reference id. A client that reads RATE as "try again"
    /// is the client the packet exists to stop.
    #[test]
    fn a_kiss_of_death_is_reported_as_itself_and_not_as_a_failure() {
        for code in [b"RATE", b"DENY", b"RSTR"] {
            let w = tweak(|w| {
                w[1] = 0;
                w[12..16].copy_from_slice(code);
            });
            assert_eq!(query().accept(&w, T4), Err(Reject::KissOfDeath(*code)));
        }
    }

    #[test]
    fn a_server_that_says_it_is_lost_is_believed() {
        // Stratum 16 is "unsynchronised", above it is reserved.
        let w = tweak(|w| w[1] = 16);
        assert_eq!(query().accept(&w, T4), Err(Reject::Stratum(16)));
        let w = tweak(|w| w[1] = 255);
        assert_eq!(query().accept(&w, T4), Err(Reject::Stratum(255)));
        // The leap indicator says the same thing in two bits.
        let w = tweak(|w| w[0] |= leap::UNSYNCHRONISED << 6);
        assert_eq!(query().accept(&w, T4), Err(Reject::Unsynchronised));
        // A leap second pending is NOT a rejection: the clock is fine, the day is not 86400 s long,
        // and interpreting that is the calendar's problem.
        let w = tweak(|w| w[0] = (w[0] & 0b0011_1111) | (leap::INSERT << 6));
        assert!(query().accept(&w, T4).is_ok());
    }

    #[test]
    fn a_zero_transmit_timestamp_is_not_a_time() {
        let w = tweak(|w| w[40..48].fill(0));
        assert_eq!(query().accept(&w, T4), Err(Reject::TransmitZero));
    }

    #[test]
    fn timestamps_that_disagree_with_causality_are_refused() {
        // T3 < T2: the server sent the reply before it got the request.
        let w = tweak(|w| {
            let t2 = REPLY[32..40].to_vec();
            w[32..40].copy_from_slice(&REPLY[40..48]);
            w[40..48].copy_from_slice(&t2);
        });
        assert_eq!(query().accept(&w, T4), Err(Reject::ServerOrderReversed));

        // T4 < T1: the reply arrived before we sent. Our side of the wire, our clock.
        let early = Timestamp::from_bits(T1.bits() - 1);
        assert_eq!(
            query().accept(&REPLY, early),
            Err(Reject::ClientOrderReversed)
        );

        // The round trip shorter than the server's own turnaround: each half is causally fine and
        // the pair is impossible, so this is the check the two above do not make.
        let mut p = Packet::parse(&REPLY).unwrap();
        p.receive = T1;
        p.transmit = Timestamp::from_bits(T1.bits() + (1 << 32)); // a full second of thinking
        assert_eq!(
            query().accept(&p.to_bytes(), T4), // T4 is only 23 ms after T1
            Err(Reject::NegativeDelay)
        );
    }

    #[test]
    fn a_server_claiming_an_impossible_root_distance_is_refused() {
        // Half the root delay plus the dispersion, against MAXDISP. The saturating add is why the
        // maximum of both fields rejects rather than wrapping to a small, passing number.
        let w = tweak(|w| {
            w[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
            w[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        });
        assert_eq!(query().accept(&w, T4), Err(Reject::RootDistance));
        assert_eq!(Packet::parse(&w).unwrap().root_distance(), Short(u32::MAX));
        // 16 seconds exactly is inside; a tick past it is not.
        let mut p = Packet::parse(&REPLY).unwrap();
        p.root_delay = Short(0);
        p.root_dispersion = MAX_ROOT_DISTANCE;
        assert!(query().accept(&p.to_bytes(), T4).is_ok());
        p.root_dispersion = Short(MAX_ROOT_DISTANCE.0 + 1);
        assert_eq!(query().accept(&p.to_bytes(), T4), Err(Reject::RootDistance));
    }

    /// Extension fields and a MAC make a longer packet. This crate implements neither, and quietly
    /// ignoring authentication data a server computed is the failure this refuses to have.
    #[test]
    fn a_reply_of_the_wrong_size_is_refused_rather_than_truncated() {
        let mut long = [0u8; PACKET_LEN + 20];
        long[..PACKET_LEN].copy_from_slice(&REPLY);
        assert_eq!(query().accept(&long, T4), Err(Reject::Length(68)));
        assert_eq!(query().accept(&REPLY[..47], T4), Err(Reject::Length(47)));
        assert_eq!(query().accept(&[], T4), Err(Reject::Length(0)));
    }

    #[test]
    fn randomising_the_low_bits_touches_only_the_low_bits() {
        let t = Timestamp::from_unix(1_785_412_800, 500_000_000).unwrap();
        let r = t.randomise_low(12, 0xFFFF_FFFF);
        assert_eq!(r.seconds(), t.seconds(), "the second must not move");
        assert_eq!(r.fraction() >> 12, t.fraction() >> 12);
        assert_eq!(r.fraction() & 0xFFF, 0xFFF);
        // Zero bits is a no-op rather than a mask of width zero minus one.
        assert_eq!(t.randomise_low(0, 0xFFFF_FFFF), t);
        // And 32 or more takes the whole fraction, which is the most this can do; a full 64 bits of
        // nonce needs Query::with_nonce.
        assert_eq!(t.randomise_low(32, 0x1234_5678).fraction(), 0x1234_5678);
        assert_eq!(t.randomise_low(99, 0x1234_5678).fraction(), 0x1234_5678);
    }

    #[test]
    fn the_short_format_reads_as_16_dot_16() {
        assert_eq!(Short(1 << 16).secs(), 1);
        assert_eq!(Short(1 << 16).nanos(), 1_000_000_000);
        assert_eq!(Short(1 << 15).nanos(), 500_000_000);
        assert_eq!(MAX_ROOT_DISTANCE.secs(), 16);
    }

    // Milestone 85 survivors. The exhaustive sweeps quantify over the CONVERSIONS; these are the
    // edges and accessors the sweeps never touch. See notes/mutation-testing.md.

    /// `UNIX_LIMIT` is a wall with two sides. Checks read off the constant move with it, so the
    /// wall is also pinned as a date worked by hand: era 0's 2^32 seconds less the 1900-to-1970
    /// offset, plus the half-era the pivot grants, is 2104-02-26T09:42:24Z. A date well inside it
    /// survives the round trip.
    #[test]
    fn the_range_limit_is_exactly_where_the_docs_say() {
        assert!(Timestamp::from_unix(UNIX_LIMIT - 1, 0).is_some());
        assert!(Timestamp::from_unix(UNIX_LIMIT, 0).is_none());
        assert_eq!(UNIX_LIMIT, 4_233_462_144);
        const Y2100: u64 = 4_102_444_800; // 2100-01-01T00:00:00Z
        assert_eq!(
            Timestamp::from_unix(Y2100, 0).map(|t| t.to_unix()),
            Some((Y2100, 0))
        );
    }

    /// An interval answers for its own sign and magnitude. `nanos()` was the only accessor any
    /// test read, so `ticks`, `abs_ticks` and `is_negative` could return constants.
    #[test]
    fn an_interval_knows_its_sign_and_magnitude() {
        let earlier = Timestamp::from_unix(1_785_412_800, 0).unwrap();
        let later = Timestamp::from_unix(1_785_412_801, 0).unwrap();
        let forward = later.since(earlier);
        assert_eq!(forward.ticks(), 1i128 << 32);
        assert_eq!(forward.abs_ticks(), 1u128 << 32);
        assert!(!forward.is_negative());
        let backward = earlier.since(later);
        assert_eq!(backward.ticks(), -(1i128 << 32));
        assert_eq!(backward.abs_ticks(), 1u128 << 32);
        assert!(backward.is_negative());
        // Zero is not negative: the boundary the < sits on.
        assert!(!earlier.since(earlier).is_negative());
    }

    /// RFC 5905's lambda with a delay big enough that halving and remainder-by-2 differ; the
    /// fixture reply's tiny delay could not tell them apart.
    #[test]
    fn root_distance_halves_the_delay_it_is_given() {
        let mut p = Packet::parse(&REPLY).unwrap();
        p.root_delay = Short(6 << 16);
        p.root_dispersion = Short(1 << 16);
        assert_eq!(p.root_distance().secs(), 4);
    }

    /// Stratum 15 is the last hop that still counts as synchronised (RFC 5905 MAXSTRAT); the
    /// rejection tests only ever probed 16, so > could rot to >= and refuse a legal server.
    #[test]
    fn the_fifteenth_stratum_is_still_a_time_source() {
        let w = tweak(|w| w[1] = MAX_STRATUM);
        assert!(query().accept(&w, T4).is_ok());
        let w = tweak(|w| w[1] = MAX_STRATUM + 1);
        assert_eq!(query().accept(&w, T4), Err(Reject::Stratum(16)));
    }
}

// =================================================================================================
// Machine-checked proofs (Kani). Behind `#[cfg(kani)]`, so an ordinary build/test never sees them;
// only `cargo kani` (script/verify) sets that cfg. See notes/verification.md and notes/ntp.md.
//
// The three things worth proving here are the three things the crate docs call easy to get wrong:
// the fixed-point conversion, the era pivot, and the fact that no hostile packet can make the
// arithmetic misbehave. Kani proves absence of panics and arithmetic overflow along the way, which
// on a parser fed by the network is half the point.
// =================================================================================================
#[cfg(kani)]
mod verification;
