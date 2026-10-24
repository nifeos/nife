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
- uboot-dtsi: `arch/riscv/dts/jh7110.dtsi` in `starfive-tech/u-boot` at `bfbdce9b86a2`,
  <https://github.com/starfive-tech/u-boot/blob/bfbdce9b86a2/arch/riscv/dts/jh7110.dtsi>.
- vendor-driver: `drivers/char/hw_random/starfive-trng.c` in `starfive-tech/linux` at
  `202b558ae34c`,
  <https://github.com/starfive-tech/linux/blob/202b558ae34c/drivers/char/hw_random/starfive-trng.c>.

## The vendor spelling

The tree nife is handed on the VisionFive 2 is the vendor U-Boot's control DTB (U-Boot 2021.10,
per `crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-manual-boot.log`). StarFive's
fork spells the node this way (uboot-dtsi):

```text
trng: trng@1600C000 {
    compatible = "starfive,trng";
    reg = <0x0 0x1600C000 0x0 0x4000>;
    clocks = <&clkgen JH7110_SEC_HCLK>,
         <&clkgen JH7110_SEC_MISCAHB_CLK>;
    clock-names = "hclk", "miscahb_clk";
    resets = <&rstgen RSTN_U0_SEC_TOP_HRESETN>;
    interrupts = <30>;
    status = "disabled";
};
```

It is the same device, window and interrupt under a different string: a stale fork, not
different silicon. Accepting `starfive,trng` is a claim about the register layout, and the vendor
Linux driver at the firmware's own vintage (vendor-driver) backs it. It defines `CTRL` 0x00,
`STAT` 0x04, `MODE` 0x08, `SMODE` 0x0C, `IE` 0x10, `ISTAT` 0x14, `RAND0`..`RAND7` 0x20..0x3C,
`AUTO_RQSTS` 0x60 and `AUTO_AGE` 0x64. Its `CTRL`, `STAT` and `ISTAT` bit positions match mainline's
`jh7110-trng.c`. Two drivers written against one IP block agree completely.

`discover` reports the node's `status` and does not act on it. The vendor tree marks the TRNG
disabled because U-Boot has no driver for it, not because the silicon is absent; StarFive's own
Linux enables the node from `jh7110-common.dtsi` (`&trng { status = "okay"; };`). The same DTB
marks the S7 monitor core `okay` and claims an Sv39 MMU for it, and both are false
(notes/visionfive2.md, "Second bench stop"). So the crate says what the tree says, and the caller
neither refuses a device that is there nor trusts one that is not.

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
