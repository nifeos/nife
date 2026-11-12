# P1: before the tag

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

## P1: before the first tag

8. The shipped text points back into nife.
   Finding: the template's `standalone` counts four milestone citations, nine paths into the nife
   tree, four section citations, seven provenance-block lines and one line naming the architect. The
   root `//!` block quotes the architect's answer and names the person. The item docs cite
   `notes/ntp.md`, `notes/ntp/the-era-pivot.md`, `notes/ntp/what-is-proved.md`,
   `notes/undefined-behavior.md`, `notes/mutation-testing.md`, `notes/verification.md`,
   `script/verify`, `measured_boot`, milestones 51 and 211 and a pull request number, and speak of
   "the clock service" as though the reader knew which one.
   Evidence: `Cargo.toml` lines 7 to 13. `src/lib.rs` lines 3, 9, 11 to 15, 53, 56, 67, 80, 93, 201,
   326, 547, 690, 827 to 832, 1124, 1183. `src/verification.rs` lines 10 and 122.
   Change: the move strips every `Name:` block, as ruled. Confirm the strip reaches the `//!` form
   at line 11, which is a paragraph of the crate root and not a `///` block (unverified). Rewrite
   each remaining citation to point at the README or `docs/`, and "the clock service" to "the caller
   that owns the clock". The `Cargo.toml` comment becomes the `description`.
   Door: two-way. Effort: S.

9. The crate does not say it is safe Rust.
   Finding: no `unsafe` anywhere (`unsafe_scan` passes), but no `#![forbid(unsafe_code)]`. The
   forbid makes the claim the compiler's, and it is the form the template's fuzz opt-in reads
   without a scan.
   Evidence: `src/lib.rs` line 17.
   Change: `#![forbid(unsafe_code)]` under `#![no_std]`. Then opt the repository into the weekly
   fuzz (`public-crash-inputs: true`, which the template allows only for a crate with no `unsafe`).
   Add one `fuzz/` target that feeds arbitrary bytes and three arbitrary timestamps to
   `Query::accept`. The target asserts the Kani harness's coherence conditions on an `Ok`. Kani
   already covers the domain. The fuzz target exercises the compiled code on a hosted runner at no
   cost to a person.
   Door: two-way. Effort: S.

10. The exhaustive sweep depends on a workspace profile override that does not move.
    Finding: `every_nanosecond_survives_the_round_trip` runs the conversion on all 10^9 inputs and
    takes 0.6 s only because the workspace manifest sets `opt-level = 2` for this package. The
    workspace comment says the unoptimised run is about a minute (unverified here). In the new
    repository `check / tests` runs it once, `check / mutation-diff` and the weekly `mutation` run
    it once per mutant (185 mutants), and `weekly / miri` already takes the strided path.
    Evidence: `Cargo.toml` (workspace) lines 284 to 290. `src/lib.rs` lines 826 to 854.
    Change: carry the optimisation in the crate's own manifest (`[profile.test] opt-level = 2`, or
    `[profile.dev.package.network_time_protocol]` since the crate is its own workspace root), and
    measure `check / coverage` at that level before relying on it, because line coverage under
    optimisation can merge lines (unverified). If coverage suffers, keep the full sweep behind a
    feature or an environment variable, run it in `proofs` beside Kani, and let the default test
    walk the same prime stride Miri does.
    Door: two-way. Effort: S.

11. The surviving mutants need the exemptions file.
    Finding: five mutants survive and nife's triage calls each equivalent (`|` to `^` over disjoint
    bits in `Packet::to_bytes` twice, `Timestamp::from_unix` and `Timestamp::randomise_low`; `%` to
    `+` in `from_unix` where the `as u32` truncation makes the two agree). The template's `mutation
    / mutation` fails on one survivor unless `.github/mutants-exempts` lists it with a reason, and
    `release / gate` reads that verdict.
    Evidence: `notes/project-metrics/mutation-census.csv` latest row. `mutation-triage.csv` lines
    592 to 595.
    Change: an exemptions file carrying the five with the triage's reasons. Or, for the four `|`/`^`
    survivors, a test that sets overlapping bits through the public surface is impossible by
    construction, so the exemption is the honest record.
    Door: two-way. Effort: XS.

12. The totality claim has a hole a caller can construct.
    Finding: `Interval`'s documentation says "there is no input, however absurd, on which it
    panics", and that holds for every interval `since` and `accept` produce (each fits in 64 bits).
    `Interval::from_ticks` is public and takes any `i128`, and `nanoseconds` and `microseconds`
    multiply by 10^9 and 10^6 before dividing, so an interval above about 1.7 × 10^29 ticks
    overflows: a panic in a debug build and a wrapped answer in release. Reasoned, not run.
    Evidence: `src/lib.rs` lines 302 to 335.
    Change: either `saturating_mul` in both accessors, or narrow the public constructor to `i64`
    ticks (the widest value the crate ever produces) and keep `i128` as the private accumulator. The
    second is cleaner and is what the type's documentation already argues. A Kani harness over
    `from_ticks(any i64).nanoseconds()` then proves the accessor total.
    Door: `from_ticks`'s parameter type is a public signature, two-way today. Effort: XS.

13. Two checks the RFC's reference implementation makes are absent or differ.
    Finding: RFC 5905 Appendix A's `packet()` refuses `rootdelay / 2 + rootdisp >= MAXDISP`. The
    crate refuses `> MAXIMUM_ROOT_DISTANCE`, so a root distance of exactly 16 s passes here and not
    there (the test `a_server_claiming_an_impossible_root_distance_is_refused` pins the crate's
    choice). The same routine refuses `reftime > xmt`, a reference timestamp later than the transmit
    timestamp. The crate does not look at `reference` at all. Both verified against the RFC text.
    Evidence: `src/lib.rs` lines 716 to 718 and 435 to 441.
    Change: match the RFC's `>=` and say so in the constant's documentation. Add
    `Reject::ReferenceAfterTransmit` with a modular comparison through `since`, a test and a line in
    the `accept` documentation's list. Both are cheap and both close a gap a stranger comparing
    against the RFC will find.
    Door: a new `Reject` variant is a public enum change, two-way today. Effort: XS.

14. The replay test is the caller's, and the documentation does not say so.
    Finding: RFC 5905's `receive()` drops a packet whose transmit timestamp equals the last one
    accepted. The crate is stateless, so this test cannot live here. A caller that reuses a `Query`
    across attempts, or runs long, owns it. nife's client draws a fresh nonce per attempt, so a
    replay fails the origin check there, and nothing says that a different caller must do the same.
    Evidence: `src/lib.rs` lines 681 to 690. `components/src/network_time_client.rs` line 153.
    Change: one paragraph on `Query::accept` and in the README's BUGS: a `Query` is for one request.
    A reply to an earlier `Query` fails the origin check, and a caller that keeps one nonce across
    requests must refuse a repeated `transmit` itself.
    Door: two-way. Effort: XS.

15. A missing proof: nothing shows `accept` ever accepts.
    Finding: An `accept` that refuses every packet satisfies
    `accepting_is_total_and_a_sample_is_coherent`. Unit tests pin the acceptance path on one
    hand-assembled reply and one rollover case. Tests cover the offset and delay values, and no
    proof does.
    Evidence: `src/verification.rs` lines 106 to 138.
    Change: a harness `a_consistent_exchange_is_accepted`. It takes symbolic T1 to T4, constrained
    so that the modular differences are non-negative and the server turnaround stays within the
    round trip. It builds a packet from them with version 4, mode 4, stratum in 1 to 15, a non-zero
    transmit and a root distance under the maximum. Assert `Ok`, `delay.ticks() == (T4 - T1) - (T3 -
    T2)` and `offset.ticks() == ((T2 - T1) + (T3 - T4)) / 2` in wrapping `i64` arithmetic. Its
    falsification patch turns the stratum `>` into `>=`. Kani handles this in seconds since it is
    addition.
    Door: two-way. Effort: S.
