---
status: BUILT
raised: 2026-08-23
built: 2026-09-04
---
# 159. A real hardware entropy source: the JH7110's TRNG

The entropy service from milestone 56 (secrets, credentials, and the entropy to make them safe)
has a virtio-rng backend, which exists in QEMU and not on the VisionFive 2 the tree boots. The
StarFive JH7110's TRNG is the real-hardware source, and this milestone drives it.

The driver's own logic can be written and host-tested without silicon; whether it actually produces
usable entropy, and at what rate, can only be verified by reading real bits off a real TRNG.

## Where it stands

Built. On 2026-09-04 a confined userspace driver on radon, the `StarFive` VisionFive 2 (JH7110,
riscv64), served TRNG bytes to a client through a capability that names no device. It did so on two
boots, and the first draw differed across them. On 2026-09-16 milestone 306 (time the hw-entropy
step) measured it at 955,223 bytes/s on the board. What stays open is a decision rather than work:
whether these bytes need a health test before anything security-shaped trusts them.

Getting there took four lanes and three bench runs. Each is told in full in an appendix:

- [`wiring.md`](0159-jh7110-trng-driver/wiring.md): what the first two lanes built (2026-08-24 and
  2026-09-01), and why the status did not move after either.
- [`bench-procedure.md`](0159-jh7110-trng-driver/bench-procedure.md): the ordered bench procedure,
  and a table of what each possible `hw entropy` line means.
- [`bench-runs.md`](0159-jh7110-trng-driver/bench-runs.md): the first two bench runs of
  2026-09-04. One found a gated block and a service that reported ready on zeros; the other found
  a tour whose success line could never print.
- [`register-sequence.md`](0159-jh7110-trng-driver/register-sequence.md): the third lane's
  bring-up changes, taken from Linux, `NetBSD` and the TRM.

## Why this is the same shape of gap as 53 and 157

Milestones 53 (network and storage on real silicon) and 157 (real display output on the board:
U-Boot's `simple-framebuffer` handoff) both exist because virtio only carries an emulator's
paravirtual devices, and real hardware has none. Entropy is the same story one subsystem over.
`entropy_service`, from milestone 56 (secrets, credentials, and the entropy to make them safe),
speaks to a `Virtio` capability, proven end to end in QEMU. Until this milestone it had never run
against real hardware, because nothing here had a driver for the JH7110's TRNG.

## What was built

- The device, confirmed from documentation on 2026-08-24. The JH7110 datasheet (v1.67), section
  2.8.2, documents a TRNG module ("Ring-oscillator based entropy source... LFSR based digital post
  process... self re-seeding... 256-bit random number generation"). Linux carries a mainline driver
  (`drivers/char/hw_random/jh7110-trng.c`) and a device-tree binding (`starfive,jh7110-trng`,
  `reg = <0x1600C000 0x4000>`, `interrupts = <30>`, clocks `hclk`/`ahb`, one reset line).
- `crates/jh7110_entropy`: the register layout, the DTB-discovery query, and the byte pool
  (`jh7110_trng::Pool`), all host-tested. Discovery finds a tree shaped like the binding's own
  example, and correctly finds nothing on QEMU's riscv64 `virt` board. Milestone 239 (radon's
  device tree does not describe the TRNG, so a working driver never runs) taught it the vendor
  U-Boot's `starfive,trng` spelling.
- A driver, not a new protocol. `components/src/jh7110_entropy.rs` is a new backend behind the
  existing service, speaking `entropy_proto` unchanged. That is the same relationship milestone
  157's framebuffer driver has to rung one's `gfx_proto`. Rule 2 applies: it takes a base address
  and knows nothing else.
- The wiring. `entropy_service`'s `Bus` enum has a `Jh7110` variant, and the riscv64 boot tour
  spawns it when the machine's device tree describes the device. The driver holds two rendezvous
  capabilities and one page of device memory: no IRQ, no DMA page, no `Virtio` capability.
- A falsifiable boot-tour line, `hw entropy`. It plays a client over the same endpoint any client
  would hold, and passes only if the service says `READY`, both 32-byte draws come back full, the
  first is not all zeros, and the two differ. On QEMU, which has no TRNG node, it prints `skipped`,
  and a system test pins that the wiring refuses rather than mapping an unnamed address.
- A readiness verdict decided from the bytes. `entropy_proto::readiness` reports `READY` only when
  the first bufferful has a nonzero byte in it. A backend whose first bufferful is all zero is
  condemned for the boot and answers `NO_ENTROPY`. All three backends call it.
- The bring-up sequence upstream has: `ISTAT` acknowledged as `R/W1C`, `MODE.R256` written and
  read back, the reseed-reminder alarms off, and `STAT.SEEDED` gating `RAND_RDY` (with a Kani
  harness).

## It served, 2026-09-04, and the experiment is run

The success line printed. Two boots of radon, transcript at
`target/board/radon-2026-09-04-trng-success.log`:

```
hw entropy : JH7110 TRNG at 0x1600c000 served 32+32 bytes to a client through a capability
             that names no device; first draw 3faa07e1.., second differs;
             STAT after init 0x00040308 (256-bit: all eight RAND words are the answer)
```

Four things had to be true at once, and each was fixed separately:

- milestone 239 taught `discover` the vendor U-Boot's `starfive,trng` spelling;
- milestone 220 (this kernel drives no clock or reset controller, and the first real device will
  need one) clocked the block and released its reset;
- this milestone's `fill` replaced a `get` that asked for 32 bytes down a channel carrying 8, so
  the tour's success line had been unreachable on any device, working or dead, since the day it
  was written;
- `ISTAT`'s `R/W1C` clear stopped the second generation returning the first one's latched register
  file.

It is a measurement rather than an anecdote, which is what the bench procedure asks for. Two
boots, and the first draw differs across them:

| boot | first draw | second draw |
|---|---|---|
| 1 | `3faa07e1` | differs |
| 2 | `731191ba` | differs |

So the device is reseeded per boot rather than returning a constant baked into silicon or a stale
register file. Each boot's two draws differ from each other, so the pool is genuinely refilled
between them.

And the one question no documentation could settle is answered. `STAT after init 0x00040308`
reports 256-bit mode, so all eight `RAND` words are the answer and no byte-count correction is
owed. It is a build-time silicon parameter; the lane that wrote `MODE.R256` recorded that one bench
line would close it, and it did.

## What it establishes for fatal risk 6, and what it does not

Risk 6 is *a capability-confined userspace driver cannot drive real hardware at real speed*, and it
has three parts:

- Confined: demonstrated 2026-09-03. An EL0 process from the archive, reaching the device through a
  capability that names no device.
- Drives real hardware: demonstrated 2026-09-04, reproducibly, on the tree's only confined driver
  for a real non-virtio device.
- At real speed: the tour has timed the step since 2026-09-10, first under QEMU against
  virtio-rng, which gives the denominator radon's figure is read against. Milestone 306 ran it on
  the board on 2026-09-16: 955,223 bytes/s, 8.4 us per round trip, bring-up 562 us. Its block has
  the numbers and the caveats.

Nothing here says the driver is fast, and the block should not be quoted as if it did.

## What is still open

- A rate and quality argument, measured rather than assumed. The datasheet documents exactly one
  hardware fault signal (`ISTAT.LFSR_LOCKUP`, an SEU in the post-processing stage), and this
  driver reads it. Neither the datasheet nor the Linux driver documents anything like a NIST SP
  800-90B health test or a compliance claim (no FIPS, no AIS-31) over the raw bitstream. Whether
  this tree needs one before trusting these bytes for anything security-shaped is a real design
  question. It is not decided here (a developer does not edit `design/decisions/`). It is
  [§137 (a hardware TRNG with no published health-test
  claim)](../decisions/0137-trng-health-tests.md), `PROPOSED`, and the full argument is in
  `crates/jh7110_entropy/src/lib.rs`'s "Health testing" section.
- The board's own device tree has never been captured to a file. Milestone 239's fix proves the
  node is there under the vendor spelling; a dump would make it a fixture for the discovery test
  (step 6 of the [bench procedure](0159-jh7110-trng-driver/bench-procedure.md)).

## BUGS

- The readiness check refuses an all-zero first bufferful, which is legitimate output with
  probability 2^-2048 (virtio), 2^-256 (JH7110) or 2^-64 (the instruction backend). A false "dead"
  costs one boot's entropy; a false "alive" costs every secret derived from it. The trade is
  recorded as a `BUGS` entry in `entropy_proto`, `components/src/entropy.rs` and
  `components/src/jh7110_entropy.rs`.
- The check stops at bring-up. A source that answers once and degrades, or whose register file
  latches and repeats a nonzero answer, still passes. Continuous health testing belongs to §137.
- `POLL_TRIES` and `LOCKUP_RETRIES` bound loop iterations, not time, so what they bound depends on
  the core and on what the compiler did to the loop. A real timeout needs a clock the driver does
  not hold. Recorded in the driver's `BUGS`.
- The 128-bit-mode question was recorded as a `BUGS` entry in `components/src/jh7110_entropy.rs`,
  to stay until a bench session read `STAT.R256` off the board. The 2026-09-04 success line read
  it (`0x00040308`, bit 3 set), but the entry still says nobody has.

## Follow-on

- **Done.** The tour's unreachable success condition, the ambiguous diagnostic word, and the missing
  bring-up steps: this branch (`milestone/159-trng-sequence`), pull request #729.
- **Recorded.** The 128-bit-mode question and the iteration-bounded polling loops, both in
  `BUGS` above and in the driver's own.
- **Milestone 306.** Timing the `hw entropy` step, which is the "at real speed" half of fatal risk 6.
  Its QEMU half was built 2026-09-10, exercised against virtio-rng because QEMU has no JH7110. The
  one boot of radon that remained happened on 2026-09-16: 955,223 bytes/s, 8.4 us per round trip,
  bring-up 562 us. The proposal was promoted to milestone 306 then, because a proposal whose work
  is finished is not a proposal.
- **Decision.** Whether these bytes need a NIST SP 800-90B-class health test before anything
  security-shaped trusts them: `design/decisions/0137-trng-health-tests.md`, already `PROPOSED`
  and untouched by this milestone.

## What this does not decide

The stopgap question of §120 (a QEMU-only virtio-rng stopgap for the interactive boot) is separate
and not reopened by this milestone. Whether the interactive boot should grant *any* entropy source
(virtio-rng or this driver) before there is a real customer for interactive login is §120's call,
declined for now. Building this driver does not require revisiting that. Landing it does not by
itself grant it to the boot path.

## What it unblocks

The real-hardware half of milestone 56's own claim: an entropy service this tree can trust on the
board it actually ships to, not only in QEMU. Downstream of that, whenever §120's stopgap question
is revisited with a real customer, the answer can be "real hardware entropy," not only "QEMU's
virtio-rng."

## Index row

Surfaced while investigating milestone 49's boot-wiring fork (DECISIONS §120): the entropy service
(milestone 56) only has a virtio-rng backend, which does not exist on real silicon. A confined
userspace driver drove the TRNG on radon on 2026-09-04 and a client got real, differing bytes
through a capability naming no device, which is fatal risk 6's "drives real hardware" half; the
tour still printed FAILED, because its success line asked for 32 bytes down a protocol carrying 8
and was unreachable on any device. Fixed, with the bring-up steps upstream has and this driver
lacked, and the success line then printed on two boots of radon the same day.
