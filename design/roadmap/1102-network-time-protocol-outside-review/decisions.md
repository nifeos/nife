# Decisions 1 to 3: the case behind each ruling

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md). The rulings
themselves are in the milestone's "Decisions for the architect".

## Decision 1. Does the crate keep a dependency on `clock_contract` for one constant?

Background: the only thing the crate imports is `clock_contract::NS_PER_SECOND`, the number
1,000,000,000. The import exists so that all of nife shares one definition of the second. After the
move the line would become a crates.io dependency on `clock_contract`, which is a nife wire contract
with its own release cadence. `clock_contract`'s review package already proposes renaming the
constant under the units rule, so the dependency inherits that break. nife's rule approves any
nifeos crate as a dependency in advance, so the question is not permission but shape.

Recommendation: no dependency. A private `const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000` in
`src/lib.rs`, one line, spelled under the units rule. A wire-format crate that a stranger pulls in
for RFC 5905 should bring nothing else into their `cargo tree`, and a shared definition of a
physical constant protects nobody from drift: the second does not move.

Prior art: `sntpc`'s core has no dependency for this. `ntp-proto` carries many because it also
carries NTS and the algorithms. `core::time::Duration` defines `NANOS_PER_SEC` privately and exports
no constant.

Example: with the dependency, `cargo tree -p network_time_protocol` for a user's embedded project
shows `clock_contract`. That crate's README is about a seqlock-protected clock page in an operating
system they have never heard of. A `cargo update` can break their build when that crate renames a
constant. Without it, the tree shows one crate.

## Decision 2. The public names the template's gates refuse, and the ones a later rule reopens

Background: the template has no baseline; each of its naming checks fails at a ceiling of 0. Three
public names fail today (items 4 and 5) and five more do not meet rules that came after their names.
All are two-way until the first publish and one-way after it. The move cannot land green without the
first three. Ratified or recorded status does not shield a name from a later rule at its release
review.

Recommendation, one line each, in the order a ruling would take them:

| today | recommended | rule | status today |
|---|---|---|---|
| `PACKET_LEN` | `PACKET_BYTES` | a length carries its unit | no block |
| the British form of the low-bit randomizer | `randomize_low_bits` | American spelling; say what is low | no block |
| `Reject::Unsynchronised`, `leap::UNSYNCHRONISED` | `Unsynchronized`, `UNSYNCHRONIZED` | American spelling; RFC 5905 writes "unsynchronized" | recorded (RFC 5905's leap indicator 3) |
| `UNIX_MINIMUM` | `MINIMUM_UNIX_SECONDS` | a limit opens `MINIMUM_` and ends with its unit | ratified; reopened by the later rule |
| `UNIX_LIMIT` (exclusive) | `MAXIMUM_UNIX_SECONDS` (inclusive, one less) | same rule; std's `MAX` is inclusive | provisional |
| `UNIX_DELTA` | `UNIX_EPOCH_DELTA_SECONDS` | a quantity ends with its unit | no block |
| `DEFAULT_POLL` | `DEFAULT_POLL_EXPONENT` | say what the number is (log2 seconds) | provisional |
| `Short` | `ShortFormat` | a generic word; the RFC's term is "NTP short format" | no block |
| `abs_ticks` | drop it (`ticks().unsigned_abs()` is one call) or `unsigned_abs_ticks` | std's spelling | no block |

Changing `UNIX_LIMIT` to an inclusive maximum changes two comparisons (`>=` becomes `>`), the two
Kani harnesses that state the window and three tests. It is still the same window. If the architect
prefers the exclusive form, `UNIX_SECONDS_LIMIT` keeps the unit and the word.

Prior art: Rust's `u32::MAX` and `Duration::MAX` are inclusive maxima. OpenBSD's `ntpd` names the
epoch constant `JAN_1970` (unverified). `ntpd-rs` names it `EPOCH_OFFSET` (unverified). The RFC's
own names are `MAXDISP`, `MAXSTRAT`, `NOSYNC`, which the crate already spells out as
`MAXIMUM_ROOT_DISTANCE`, `MAXIMUM_STRATUM` and the leap constant.

Example: a user reading `Timestamp::from_unix(seconds, nanoseconds)` and its documentation today
meets `UNIX_MINIMUM..UNIX_LIMIT` and has to learn that `LIMIT` is exclusive. With
`MINIMUM_UNIX_SECONDS..=MAXIMUM_UNIX_SECONDS` the range reads the way every std range does.

## Decision 3. Do `Query::new` and the low-bit randomizer leave before the first publish?

Background: `Query::with_nonce` puts 64 random bits on the wire and keeps the true send time
locally, and the crate's documentation explains at length why this is free and strictly stronger.
`Query::new` and the low-bit randomizer are the weaker path for a caller with no random source. No caller
in nife uses them. Once published, removing them is a breaking change.

Recommendation: remove both now. A caller with no random source has no business proposing a wall
clock from an unauthenticated protocol, and the crate's own documentation says as much. Two public
items that exist only to steer callers away are surface without a user. If the architect wants the
weak path for an embedded user with a weak entropy source, keep the low-bit randomizer under the spelled
name and drop `Query::new`. Then the only constructor takes a nonce, and the caller decides what to
put in it.

Prior art: chrony randomizes the transmit timestamp by default (the crate's documentation;
unverified here). `ntpd-rs` generates a random origin for every request (unverified). `sntpc` sends
the caller's timestamp (its documentation does not describe a nonce).

Example: a user who writes `Query::new(now)` because it is the first constructor in the
documentation ships a client whose replies an off-path attacker forges with about 4,096 guesses on
a microsecond clock, as the low-bit randomizer documentation itself computes.
