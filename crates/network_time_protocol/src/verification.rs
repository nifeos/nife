use super::*;

/// The nanosecond bound this harness proves the round trip over: 2^16, not the whole range.
///
/// The property is two multiplications composed, which a bit-blasting model checker handles badly:
/// with kissat, 2^16 takes 9 s, 2^20 takes 212 s, and 2^30 was killed at 10 minutes, twice. The
/// full claim is proved by running the code on all 10^9 values in 0.6 s
/// ([`super::tests::every_nanosecond_survives_the_round_trip`]); this harness guards the low
/// corner, where 1 ns is 4.29 ticks and truncating instead of rounding turns it into 0. See
/// notes/ntp/what-is-proved.md.
const PROVED_NANOSECONDS: u64 = 1 << 16;

/// **Nanoseconds survive the fixed-point conversion**, for every value under [`PROVED_NANOSECONDS`].
/// See that constant for what is and is not covered and why.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.nanoseconds_survive_the_fixed_point.patch`
#[kani::proof]
fn nanoseconds_survive_the_fixed_point() {
    let nanoseconds: u32 = kani::any();
    kani::assume((nanoseconds as u64) < PROVED_NANOSECONDS);
    assert_eq!(
        ticks_to_nanoseconds(nanoseconds_to_ticks(nanoseconds)),
        nanoseconds
    );
}

/// **The era pivot is exact over the whole representable window**: for every Unix second in
/// [`UNIX_MINIMUM`]`..`[`UNIX_LIMIT`], 1970 to 2104, encoding and decoding returns the second it
/// started with. Quantified rather than sampled, and the window spans the 2036 rollover, so this
/// covers the era-0 branch, the era-1 branch, and the boundary between them in one proof. All
/// 4.2 billion of them, in a third of a second, because this half is addition.
///
/// It asserts the seconds; the nanoseconds are the sweep named on [`PROVED_NANOSECONDS`]. The halves
/// compose because `from_unix` builds `(seconds << 32) | ticks` with `ticks < 2^32`, disjoint
/// bits that `to_unix` reads back through separate accessors.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.the_era_pivot_is_exact_over_the_window.patch`
#[kani::proof]
fn the_era_pivot_is_exact_over_the_window() {
    let seconds: u64 = kani::any();
    kani::assume(seconds < UNIX_LIMIT);

    let t = Timestamp::from_unix(seconds, 0).expect("in range must encode");
    assert_eq!(t.to_unix().0, seconds);
    assert_eq!(t.fraction(), 0);
}

/// **Decoding is total, and it never produces an invalid sub-second.** For every one of the
/// 2^64 bit patterns a packet can carry: `to_unix` does not panic, and the nanosecond it returns
/// is under 1e9. The second half is the clamp in [`ticks_to_nanoseconds`], and it matters because a
/// nanosecond field of exactly 1e9 is the kind of thing that panics inside a `Duration`
/// constructor three layers away from the packet that caused it.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.decoding_any_wire_value_is_total.patch`
#[kani::proof]
fn decoding_any_wire_value_is_total() {
    let bits: u64 = kani::any();
    let (_seconds, nanoseconds) = Timestamp::from_bits(bits).to_unix();
    assert!((nanoseconds as u64) < NS_PER_SECOND);
}

/// **Out of range is refused, never aliased.** For every `seconds`, `from_unix` returns `Some` if
/// and only if the value is inside the window. This is the other half of the era decision: the
/// encoder does not quietly wrap a 2105 timestamp onto 1969.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.out_of_range_is_refused.patch`
#[kani::proof]
fn out_of_range_is_refused() {
    let seconds: u64 = kani::any();
    let nanoseconds: u32 = kani::any();
    let encoded = Timestamp::from_unix(seconds, nanoseconds);
    assert_eq!(
        encoded.is_some(),
        seconds < UNIX_LIMIT && (nanoseconds as u64) < NS_PER_SECOND
    );
}

/// **Parse then serialise is the identity, on every 48-byte input there is.** Not "on a
/// well-formed packet": there is no such qualifier, because parse judges nothing. That is what
/// makes this provable over 2^384 inputs and what keeps every rejection in one function.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.parse_then_serialise_is_the_identity.patch`
#[kani::proof]
#[kani::unwind(49)]
fn parse_then_serialise_is_the_identity() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let p = Packet::parse(&wire).expect("48 bytes always parse");
    assert_eq!(p.to_bytes(), wire);
}

/// **A reply whose origin field is not our nonce is always rejected**, over every packet an
/// attacker could send. The load-bearing security check, proved rather than tested: no
/// combination of the other 40 bytes gets a packet past it.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.an_unmatched_origin_is_always_rejected.patch`
#[kani::proof]
#[kani::unwind(49)]
fn an_unmatched_origin_is_always_rejected() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let nonce: u64 = kani::any();
    let sent: u64 = kani::any();
    let dest: u64 = kani::any();

    let q = Query::with_nonce(Timestamp::from_bits(sent), Timestamp::from_bits(nonce));
    let origin = u64::from_be_bytes([
        wire[24], wire[25], wire[26], wire[27], wire[28], wire[29], wire[30], wire[31],
    ]);
    kani::assume(origin != nonce);
    assert!(q.accept(&wire, Timestamp::from_bits(dest)).is_err());
}

/// **Accepting is total, and an accepted sample has a non-negative delay.** For any 48 bytes and
/// any three timestamps: `accept` does not panic, does not overflow, and if it returns a sample
/// then the delay is non-negative, so the ±δ/2 bound a client reasons with is never derived
/// from a nonsense δ. This is the proof that a hostile packet cannot make the arithmetic
/// misbehave, which is the property the `i128` intervals exist for.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.accepting_is_total_and_a_sample_is_coherent.patch`
#[kani::proof]
#[kani::unwind(49)]
fn accepting_is_total_and_a_sample_is_coherent() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let nonce: u64 = kani::any();
    let sent: u64 = kani::any();
    let dest: u64 = kani::any();

    let q = Query::with_nonce(Timestamp::from_bits(sent), Timestamp::from_bits(nonce));
    if let Ok(s) = q.accept(&wire, Timestamp::from_bits(dest)) {
        // **The two predicate-shaped claims are stated over the numbers** (milestone 211).
        // `accept` refuses on `delay.is_negative()` and on `p.transmit.is_zero()`, so
        // asserting through those same two predicates proves only that the guard and the
        // assertion agree: an `is_negative` stuck at false satisfies both sides and a
        // negative delay reaches the caller with this harness still green.
        assert!(
            s.delay.0 >= 0,
            "an accepted sample carries a negative delay"
        );
        assert!(
            s.response.transmit.0 != 0,
            "an accepted sample carries an unset transmit timestamp",
        );
        assert!(s.response.stratum >= 1 && s.response.stratum <= MAXIMUM_STRATUM);
        assert!(s.response.mode == mode::SERVER);
    }
}

/// **A consistent exchange is accepted, with the RFC's offset and delay.** Without this, an
/// `accept` that refused every packet would satisfy
/// [`accepting_is_total_and_a_sample_is_coherent`]. From any four timestamps whose modular
/// differences are non-negative and whose round trip covers the server's turnaround, a version 4,
/// mode 4 reply with a stratum in 1 to 15, a non-zero transmit timestamp and a root distance under
/// the maximum is accepted, and its delay is `(T4 - T1) - (T3 - T2)` and its offset
/// `((T2 - T1) + (T3 - T4)) / 2`.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.a_consistent_exchange_is_accepted.patch`
#[kani::proof]
#[kani::unwind(49)]
fn a_consistent_exchange_is_accepted() {
    let t1: u64 = kani::any();
    let t2: u64 = kani::any();
    let t3: u64 = kani::any();
    let t4: u64 = kani::any();
    let nonce: u64 = kani::any();
    let stratum: u8 = kani::any();
    let root_delay: u32 = kani::any();
    let root_dispersion: u32 = kani::any();
    let server = t3.wrapping_sub(t2) as i64 as i128;
    let round = t4.wrapping_sub(t1) as i64 as i128;
    kani::assume(server >= 0 && round >= server);
    kani::assume(t3 != 0);
    kani::assume((1..=MAXIMUM_STRATUM).contains(&stratum));
    let packet = Packet {
        leap: leap::NONE,
        version: VERSION,
        mode: mode::SERVER,
        stratum,
        poll: 6,
        precision: 0,
        root_delay: Short(root_delay),
        root_dispersion: Short(root_dispersion),
        reference_id: [0; 4],
        reference: Timestamp::ZERO,
        origin: Timestamp::from_bits(nonce),
        receive: Timestamp::from_bits(t2),
        transmit: Timestamp::from_bits(t3),
    };
    kani::assume(packet.root_distance() < MAXIMUM_ROOT_DISTANCE);
    let q = Query::with_nonce(Timestamp::from_bits(t1), Timestamp::from_bits(nonce));
    let sample = q.accept(&packet.to_bytes(), Timestamp::from_bits(t4));
    let sample = sample.expect("a consistent exchange is refused");
    assert_eq!(sample.delay.ticks(), round - server);
    let to_server = t2.wrapping_sub(t1) as i64 as i128;
    let from_server = t3.wrapping_sub(t4) as i64 as i128;
    assert_eq!(sample.offset.ticks(), (to_server + from_server) / 2);
}
