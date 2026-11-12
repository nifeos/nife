# Prior art

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

How the crate sits against what a crates.io user already knows. Recent downloads are the last 90
days at the time of the review, from the crates.io API.

| crate or program | recent / all-time downloads | I/O | era rule | what a user coming from it will miss here |
|---|---|---|---|---|
| `rsntp` 4.1 | 338k / 4.29M | owns the socket (`std`, sync and async) | unverified | a one-call client; `std` types |
| `sntpc` 0.11 | 151k / 885k | socket and clock behind two caller traits; `no_std` | nearest of three eras relative to the caller's clock; silently wrong past 68 years (its own documentation) | `get_time` in one call; `KissOfDeathCode` as an enum; `dispersion` as a feature |
| `ntp-proto` 1.9 (ntpd-rs, Pendulum) | 35k / 95k | sans-I/O, `std` | unverified | NTS, extension fields, the filter and selection algorithms; its documentation says it is not meant as a public interface |
| `ntp` 0.5 | 6.7k / 254k | owns the socket | unverified | nothing a user would want: last release 2018 |
| OpenBSD `ntpd` `client.c`, chrony | n/a | the program | nearest era against the local clock (unverified) | chrony sends a random transmit timestamp by default (the crate's own documentation; unverified here) |
| RFC 5905 Appendix A `packet()` | n/a | reference implementation | unverified | `rootdelay / 2 + rootdisp >= MAXDISP` and `reftime > xmt` both refused; a duplicate `xmt` refused (items 13 and 14) |

Where this crate differs, it has no socket, no clock and no policy. It compiles for the host and for three bare-metal targets with no dependency but one constant.

The era pivot is a pure function of the bytes. The same packet decodes to the same instant on a machine that believes it is 1970 and on one that knows it is 2026. The crate carries a stated expiry (26 February 2104) instead of a hidden one.

The nonce is separate from the send time (`Query::with_nonce`), so an off-path forger guesses 64 bits. Three Kani harnesses cover every one of the 2^384 possible 48-byte replies: the parse round trip, the nonce check, and the totality and coherence of `accept`. A run over all 10^9 inputs verifies the nanosecond round trip.

What it costs: no NTS and no symmetric-key MAC. It takes 48 bytes only, so it refuses a reply that carries either. It keeps no replay state (item 14) and does no server selection or clock filtering. It uses `u8` fields where other crates offer enums. A stranger has to read `notes/ntp.md` to understand the public surface (item 8).

The concrete example is `sntpc`'s era rule. Its documentation says the era is the nearest of three to the caller's `timestamp_sec()`. It also says that a pivot more than half an era off gives a silently wrong result. A machine booting with its clock at the Unix epoch asks a 2026 server and gets a 2026 reply. The seconds field of that reply (era 0, high bit set) is nearer to 1970 than to 2104 in that rule. The result is right by luck of the pivot's width, and the same bytes would give a wrong result after 2036.

This crate's `Timestamp::to_unix` reads the high bit and nothing else. `the_era_pivot_is_exact_over_the_window` proves the round trip for every second from 1970 to 2104.
