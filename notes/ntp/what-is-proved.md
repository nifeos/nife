# What the wire-format crate proves, and where a solver was the wrong tool

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, after the parent's
own heading.*

An appendix to [`notes/ntp.md`](../ntp.md). It holds the seven Kani harnesses over
`crates/network_time_protocol`, and the nanosecond round trip that a model checker could not finish
and an exhaustive test could.

## The seven harnesses

Seven Kani harnesses, run by `script/verify`:

| harness | what it quantifies over |
|---|---|
| `the_era_pivot_is_exact_over_the_window` | every Unix second from 1970 to 2104, all 4.2 billion: encode then decode is the identity, across both eras and the boundary |
| `nanoseconds_survive_the_fixed_point` | nanoseconds under 2^16 (see below) |
| `decoding_any_wire_value_is_total` | all 2^64 timestamp bit patterns: no panic, and the nanosecond returned is always a valid sub-second |
| `out_of_range_is_refused` | every input to `from_unix`: `Some` exactly inside the window, never an aliased answer |
| `parse_then_serialise_is_the_identity` | all 2^384 48-byte packets |
| `an_unmatched_origin_is_always_rejected` | all 2^384 packets: no combination of the other 40 bytes gets one past the nonce check |
| `accepting_is_total_and_a_sample_is_coherent` | all 2^384 packets and any three timestamps: `accept` never panics or overflows, and an accepted sample has a non-negative delay, a stratum in 1..=15, mode 4, and a non-zero transmit |

The three 2^384 harnesses are the ones a model checker exists for. The domain is unenumerable and
the code is fed by the network.

## The nanosecond round trip is the exception

The property is `ticks_to_nanos(nanos_to_ticks(n)) == n`, and it is real. A tick is 2^-32 s, about
233 ps, so nanoseconds fit inside ticks, but only if both directions round to nearest. Truncate
either way and 1 ns becomes 4 ticks becomes 0 ns. Kani is bad at it, because a bit-blasting model
checker is bad at multiplication and this is two of them composed. Measured on an M-series laptop
with kissat:

| bound on `n` | solver time |
|---|---|
| 2^16 | 9 s |
| 2^20 | 212 s |
| 2^30 (the real range) | killed at 10 minutes, twice |

That is about four times the work per bit, so the full range is out of reach by roughly five
orders of magnitude. One restatement was tried: the division by its defining inequality
(`t*d <= x < (t+1)*d`), which replaces a division circuit with a shift-and-add one. It did not
help, and that is what identified the multiplication rather than the division as the cost.

The answer was not a cleverer harness. The domain is 10^9 values, and running the code on every
one of them takes 0.6 s. So the test does that. `every_nanosecond_survives_the_round_trip` is
exhaustive, which is a *complete* verification of the function, strictly stronger than any bounded
solver result. The crate gets `opt-level = 2` in the dev profile to keep it that quick, the same way
`measured_boot` does for SHA-256. The bounded Kani harness stays as the in-gate regression guard on
the low corner where the truncation bug lives.

The general rule this is an instance of is worth keeping. A model checker is the tool for domains
too big to enumerate, not a better tool for domains that are not. Check first whether you can try
all of them.
