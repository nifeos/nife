# The JH7110 TRNG: sources, bring-up and health testing

This is the reference behind `crates/jh7110_entropy`, which holds the TRNG's register layout, its
device-tree query and the decision of whether a generation succeeded. Nothing here has been
checked against real silicon. Every fact below comes from documentation and other drivers, so the
milestone's gate stays HARDWARE.

## Sources

- binding: Linux's `starfive,jh7110-trng.yaml`, mainline as of 2026-08-24. It gives the
  compatible string, the `reg` window, two clocks (`hclk`, `ahb`), one reset and one interrupt.
  Its example is the fixture's model: `rng@1600C000 { reg = <0x1600C000 0x4000>; interrupts = <30>; }`.
- driver: Linux's `drivers/char/hw_random/jh7110-trng.c`, mainline as of 2026-08-24. The
  register offsets and bit positions are transcribed from its `#define`s.
- ds: the *JH7110 Datasheet*, v1.67 (2025-02-14), section 2.8.2 "TRNG". The whole section
  lists four features: a ring-oscillator source, LFSR post-processing, self re-seeding, and
  256-bit output. It makes no NIST SP 800-90B, FIPS 140 or AIS-31 claim.
- trm: the *JH7110 Technical Reference Manual*, Preliminary V2 (2023-04-24), "TRNG > Control
  Registers", fetched 2026-09-04. It settles that `ISTAT` is write-1-to-clear, that zero disables
  `AUTO_RQSTS` and `AUTO_AGE`, and that `MODE.R256` picks four words or eight. It names two
  `ISTAT` bits no Linux driver defines, `AGE_ALARM` and `RQST_ALARM`. Its `STAT.LAST_RESEED`
  value `0x7` means unseeded. Its register map agrees with the driver everywhere they overlap.
  Bit positions live in the manual's figures, which are images, so the numbering comes from the
  two drivers and the manual supplies names and meanings.
- netbsd: NetBSD's `sys/arch/riscv/starfive/jh7110_trng.c`, revision 1.2, fetched 2026-09-04.
  It is the only driver that polls. It supplies the interrupt bit positions mainline omits
  (`RAND_RDY` 0, `SEED_DONE` 1, `AGE_ALARM` 2, `RQST_LOCKUP` 3, `LFSR_LOCKUP` 4, `GLOBAL` 31).
  Its poll path trusts `ISTAT.RAND_RDY` only when `STAT.SEEDED` is set, which is the gate the
  crate applies.

## The bring-up order

All three drivers (mainline, the vendor's, NetBSD's) agree on this sequence. A driver that skips a
step relies on a reset value nobody documented.

1. Write 0 to `AUTO_AGE` and `AUTO_RQSTS`. Otherwise a nonzero reset counter raises an alarm
   later, in a register the driver may not decode.
2. Clear `ISTAT` (write-1-to-clear). `RAND_RDY` is latched, so a driver that never acknowledges
   it sees "ready" at once on every request after the first and reads stale words.
3. Write `MODE_R256`. Without it, eight assembled words may claim 32 bytes from a block that
   answered 16.
4. Enable interrupts, or deliberately leave `IE` at zero to poll. `ISTAT` latches either way.
5. Write `CTRL_EXEC_RANDRESEED`, wait for `SEED_DONE` and acknowledge it. Until then
   `STAT.SEEDED` is clear and every poll answers unseeded.

Then, per generation: acknowledge any stale `RAND_RDY`, write `CTRL_GENE_RANDNUM`, poll
`(STAT, ISTAT)`, and acknowledge `RAND_RDY` once the words are read.

## Health testing

A hardware status bit that is merely read is nearly free. A software SP 800-90B suite
(repetition count, adaptive proportion) is real design and verification work. So the question was
whether the chip documents a health signal.

It documents one fault signal: `ISTAT.LFSR_LOCKUP`. Linux treats it as a single-event upset and
reseeds. The crate surfaces it as its own outcome rather than "not ready yet", so a caller can
count it, retry, or refuse to serve bytes past it.

It catches a stuck post-processing stage, not a degraded ring oscillator that keeps running.
Neither the datasheet nor the driver documents an SP 800-90B-class test. Whether this project
needs one before trusting these bytes for anything security-shaped is undecided. It is an
architect's call, and no section in `design/decisions/` records it yet.

## The driver's own choices

`components/src/jh7110_entropy.rs` polls `ISTAT` rather than taking PLIC line 30. A wrong interrupt
binding fails silently, as a hang. A wrong poll bound fails loudly, as `NO_ENTROPY`. Switching to
the interrupt once the board confirms the binding is reversible.

It draws the first 32 bytes before it reports, and the bytes decide the readiness word. A device
whose clock is gated reads as zeros, and an all-zero first generation reports
`STEP_FIRST_ALL_ZERO`. That device is then refused for the rest of the boot. A client only ever
sees a reply, so a service that reported dead and went on serving its zero buffer would hand those
zeros out as randomness to everyone not watching the handshake. A device that answered with nothing
is not refused. It has told the truth at every step, `take` answers `NO_ENTROPY` while it stays
dry, and it recovers by itself if it starts answering.

The readiness message has a fixed layout. Word 0 is the verdict. Word 1 holds the byte count, and
in its high half the `STAT` read right after the bring-up writes. Word 2 is always
`(STAT << 32) | ISTAT`, read at report time, whatever word 0 says. An all-zero pair says the
register window read as nothing: a gated clock, a reset still asserted, or a base that is not the
TRNG. A nonzero `STAT` with `SEEDED` clear says the device is alive and seeding did not finish.

One question about the bytes is open. `MODE.R256` has never been read back on silicon. If radon's
`STAT.R256`, in word 1's high half, reads clear, the upper four `RAND` words are not device output
and only 16 bytes per generation may be served. One line of a bench transcript closes it.
