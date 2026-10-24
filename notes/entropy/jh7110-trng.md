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

## The kernel's wiring

`kernel/src/user/entropy_service.rs` gives the TRNG driver less authority than any other entropy
backend: a request endpoint it receives on, a readiness endpoint it sends on once, and one page of
device memory. That page is the first `0x1000` of the binding's `0x4000` register window, mapped
device-typed. `jh7110_entropy::regs` reaches only `0x68`, and a driver that cannot name a register
cannot touch it. It gets no DMA page, because the device writes nothing to memory. It gets no `Irq`
capability, because the driver polls. And it gets no `Virtio` capability, because there is no
transport. With no DMA there is nothing for an IOMMU to confine, which is a narrower attack surface
rather than a weaker claim.

This is the shape of fatal risk 6's experiment (`design/fatal-risks/README.md`): an unprivileged
process holding one device page and two endpoints, driving a real device that is not virtio.
Whether it runs at real speed is what the bench measures.

The kernel enables the TRNG's two clocks and releases its reset before the spawn. The driver's
first act is to read `STAT`, and a gated block answers that read with zeros. The clock and reset
controller is admin plane and stays in the kernel
(`kernel/src/drivers/jh7110_clock_and_reset.rs`), on the argument of §86 (whether an NVMe driver can
leave the kernel). A virtio-rng needs no such step, since a QEMU device comes up running.
