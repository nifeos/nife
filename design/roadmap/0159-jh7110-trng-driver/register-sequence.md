# What the third lane changed, 2026-09-04, none of it run on silicon at the time

An appendix to [milestone 159 (a real hardware entropy source: the JH7110's
TRNG)](../0159-jh7110-trng-driver.md). The main page is complete without it. The run that served,
later the same day, is on the main page.

Read this as "matches upstream's order, host-tested, unverified on hardware." radon was powered
down for all of it and no part of a JH7110 exists in QEMU, so nothing below had met the device when
it was written.

The prior art was fetched rather than recalled, which mattered. `crates/jh7110_entropy` was
transcribed from a *summary* of Linux's driver and recorded three of its own facts as unconfirmed.
Three sources settle them, all cited in the crate with URLs and fetch dates:

- mainline `jh7110-trng.c`;
- the JH7110 TRM's TRNG register page (new to this tree);
- `NetBSD`'s `jh7110_trng.c` (also new, and the most useful, because it is the only one of the
  three that polls, which is what this driver does).

The changes:

- `ISTAT` is `R/W1C`, per the TRM's register map. The crate said this was "not confirmed from the
  summarized driver source", and the driver therefore never wrote the register. That is a real
  defect and it is fixed. `RAND_RDY` is latched, so an unacknowledged one makes every generation
  after the first appear complete instantly. The driver then reads the `RAND` words without the
  device having refilled them. Silicon had not seen this bug, because the tour never reached a
  second generation: draw `b` came out of the buffer draw `a` had left. Looping the tour to 32
  bytes is exactly what would have exposed it, so the two fixes had to land together.
- `MODE.R256` is now written, and `STAT` after init is reported so a bench session can check it
  read back. The width a JH7110's TRNG resets to is a build-time parameter of the silicon
  (`BUILD_CONFIG.PRNG_LEN_AFTER_RST`), so it cannot be assumed from documentation. If the block is
  in 128-bit mode, only `RAND0..RAND3` are the answer, and half of every 32 bytes this driver serves
  is not device output. This was the one open correctness question about the bytes, and one line of
  a bench transcript would close it. (It did: see the main page.)
- `AUTO_AGE` and `AUTO_RQSTS` are zeroed. That is how the TRM says the two reseed-reminder alarms
  are disabled, and what Linux's default module parameters do.
- `STAT.SEEDED` now gates `RAND_RDY`. `jh7110_trng::interpret` takes `STAT` and returns a new
  `Outcome::Unseeded`. This is `NetBSD`'s gate: `RAND_RDY` is a latch that can stand from before
  this driver ran, while `SEEDED` is live state. The TRM confirms an unseeded core is a state the
  block reports (`STAT.LAST_RESEED == 0x7`, "Unseeded (zeroized state)"). A Kani harness pins that
  no unseeded snapshot reaches `Ready`.
- Two `ISTAT` bits nobody had named: `AGE_ALARM` (2) and `RQST_ALARM` (3). They are documented in
  the TRM and defined by `NetBSD`, and absent from both Linux drivers and therefore from this
  crate. A status word with unnamed bits is one a bench session cannot read, which is the whole of
  how the `0x20` went wrong ([the second bench run](bench-runs.md)).
- `IE` is left at zero, and now with evidence rather than a shrug. The TRM's wording on whether
  `ISTAT` latches with interrupts disabled is not decisive. The board settled it: on 2026-09-04 a
  reseed and a generation both completed and were detected by polling `ISTAT`, with `IE` never
  written. Measured, not inferred, and recorded at `IE_GLBL_EN`.
