# NTP: the appendices to notes/ntp.md

[`notes/ntp.md`](../ntp.md) carries the wire format and its checks, the era pivot, the client's
capability shape, the nonce, the one-shot design and what neither half does. These files carry the
detail its prose budget has no room for.

- [`the-era-pivot.md`](the-era-pivot.md): the epoch arithmetic, the case against a nearest-era rule,
  and the exact window `Timestamp::from_unix` accepts.
- [`what-is-proved.md`](what-is-proved.md): the seven Kani harnesses, and the nanosecond round trip
  that a solver could not finish and an exhaustive test could.
- [`the-client-tests.md`](the-client-tests.md): the test server and what it does not prove, the
  witness and the false "same binary" argument, the six kernel tests and their mutations, and the
  hang one test used to show on its own.

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, which split the
parent under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

The stems here were minted the same day by the same lane. Each is named for its content, and calef
names files.
