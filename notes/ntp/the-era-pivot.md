# The 2036 problem in detail: the epoch, the wrap, and why the pivot is fixed

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, after the parent's
"the pivot we chose".*

An appendix to [`notes/ntp.md`](../ntp.md), which states the pivot and its table. This file holds
the arithmetic behind the epoch constant, the case against a nearest-era rule, and the exact window
`Timestamp::from_unix` accepts.

## Two things that catch people out

An NTP timestamp is 64 bits: 32 of seconds since 1 January 1900, 32 of binary fraction.

The epoch is not Unix's. They differ by 2,208,988,800 seconds, which is 25,567 days: 70 years of
365 plus the 17 leap days from 1904 to 1968. 1900 is not a leap year (divisible by 100 and not by
400), and assuming it is puts the constant one day out.

The seconds field wraps, on 7 February 2036 at 06:28:16 UTC. 32 bits of seconds is 136 years, and
the field alone cannot say which 136 years it means. Something has to decide, and the choice is
visible in the decoded output of every timestamp the machine ever handles.

## Why not the nearest era

The alternative is real and is what several implementations do: pick the era that puts the
timestamp nearest to the time you already believe it is. It is more flexible and it is strictly
worse here, for three reasons.

1. It makes decoding depend on the clock. The same bytes parse to different instants depending on
   when you ask, which is a property no parser should have.
2. It makes the function untestable without injecting a "now", and therefore unprovable: Kani
   quantifies over inputs, and a hidden input is not one.
3. It is wrong exactly when it matters. This machine boots believing it is January 1970. A
   nearest-era heuristic on a machine whose clock is wrong picks the wrong era with total
   confidence. The entire reason this crate exists is that the clock is wrong.

## The window

The representable window in Unix seconds is `0 .. 4_233_462_144`, the epoch to 2104-02-26 09:42:24
UTC. It is clipped below at 1970 because the crate's Unix seconds are unsigned.
`Timestamp::from_unix` refuses anything outside it rather than wrapping, so the failure mode is
`None` instead of a date 136 years off.

The offset and delay arithmetic is the piece that is easy to miss. Differences are taken as
`wrapping_sub` on the raw 64-bit values and read back as signed. That is what makes an exchange
straddling the 2036 boundary come out as three seconds instead of minus 136 years, and there is a
test that does exactly that.
