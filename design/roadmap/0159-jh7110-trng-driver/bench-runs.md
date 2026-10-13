# The first two bench runs, 2026-09-04: a dead block, then a wrong tour

An appendix to [milestone 159 (a real hardware entropy source: the JH7110's
TRNG)](../0159-jh7110-trng-driver.md). The main page is complete without it. Both runs came before
the run that served (see the main page), and each found a defect the main page summarizes.

## The bench ran it, 2026-09-04, and the failure is cleanly attributed

The confined driver ran on silicon for the first time. Two boots of radon, byte-identical:

```
hw entropy  : FAILED: JH7110 TRNG at 0x1600c000 (tree says starfive,trng, status disabled):
              report 0x524e475550, bring-up diagnostic 0x0000000000000000,
              draws 0/0 bytes, first-all-zero true, draws-differ false
```

Transcript: `target/board/radon-2026-09-04-trng-bringup.log`.

The fix from milestone 239 (radon's device tree does not describe the TRNG, so a working driver
never runs) works. `tree says starfive,trng` is the vendor U-Boot spelling, matched by the second
arm 239 added on 2026-09-03. Every boot before that read `skipped`. The device-tree half of this
milestone is done.

The diagnostic is the all-zero case. The [outcome table](bench-procedure.md) routes it to
milestone 220 (this kernel drives no clock or reset controller, and the first real device will
need one) rather than to a defect here. The register window read as nothing, most likely because the block's
clocks are gated or its reset is not deasserted.

Two independent signals agree, which is what makes this a diagnosis rather than a guess. The
device tree marks the node `status disabled`, and the register window reads zero. Milestone 239
deliberately *reports* `status` without acting on it, because that same tree lies about the S7
core; here the tree and the hardware say the same thing.

### What it establishes for fatal risk 6, which is more than "FAILED" suggests

`report 0x524e475550` is `entropy_proto::READY`, ASCII `RNGUP`. That crate's own comment says a
bring-up failure reports `0xDEAD_0000_0000_0000 | step` *instead*. So the service did not report a
failure. It reported ready.

So on real silicon, for the first time, a userspace driver started from the archive as an EL0
process. It reached the device through a capability that names no device, and completed its
bring-up far enough to send `READY`. What it could not do is get a non-zero byte out of a block
nothing has powered.

Risk 6 is *"a capability-confined userspace driver cannot drive real hardware at real speed."*
This run splits it. Confined is demonstrated on silicon. Drives real hardware is blocked on
milestone 220. At real speed stays unmeasured, and unmeasurable until the block is on.

### A defect in this milestone, found by the same line

The service sent `READY` while holding 32 bytes of zeros. `entropy_proto::READY`'s own doc says it
is sent *"once the device is up and its first bytes are in hand"*. Two paragraphs later the same
file argues that a caller who cannot be given randomness must find out, because the alternative is
*"the exact silent-degradation failure"* it exists to prevent.

A service that reports ready on a dead device is that failure. The tour caught it only because it
prints the draws beside the report and checks `first-all-zero`. A client that trusted `READY`
would have consumed zeros believing them random. That is this milestone's to fix, not milestone
220's. It was worth fixing before 220 landed rather than after, because once the clock works the
symptom disappears and the defect does not.

### Fixed 2026-09-04, and the fix is wider than the report word

`entropy_proto::readiness` now decides the readiness word from the bytes. It returns `READY` only
when the first bufferful has a nonzero byte in it, and `0xDEAD_0000_0000_0000 | step` otherwise.
Two shared steps (`0x10` nothing arrived, `0x11` everything that arrived was zero) cannot collide
with a backend's own. All three backends call it: the JH7110 driver, the virtio-rng one, and the
`RDSEED`/`RNDRRS` instruction one. The last had the same shape of bug and had simply never met a
dead device.

Fixing the report word alone would have left the defect in place, which is the part worth
recording. A report reaches whoever wired the service; a client only ever sees a reply. So a
service that reported dead and went on serving its zero buffer would still have handed those zeros
out as randomness to every client that was not watching the handshake. A backend that draws an
all-zero first bufferful is therefore condemned for the boot and answers `NO_ENTROPY` to everything
after it. A backend that drew *nothing* is not condemned. It has told the truth at every step, its
replies already say `NO_ENTROPY` while it stays dry, and it recovers by itself if the device starts
answering.

`system_initializer` already gated the login stack on this word (`entropy_ready = verdict ==
entropy_proto::READY`). So on radon as it stood, the real init now declines to build a credential
stack on a gated TRNG instead of building one on zeros.

The judgment, stated where it can be argued with. An all-zero bufferful is legitimate output with
probability 2^-2048 (virtio), 2^-256 (JH7110) or 2^-64 (the instruction backend). So refusing one
is a correctness claim about a random variable. It is recorded as a `BUGS` entry in
`entropy_proto`, in `components/src/entropy.rs` and in `components/src/jh7110_entropy.rs` rather
than left implicit. A false "the device is dead" costs one boot's entropy; a false "the device is
alive" costs every secret derived from it.

It stops at bring-up, deliberately. A source that answers once and degrades, or whose register file
latches and repeats a nonzero answer, still passes. That is continuous health testing, which
[§137 (a hardware TRNG with no published health-test
claim)](../../decisions/0137-trng-health-tests.md) owns as `PROPOSED`. Its hard half is the failure
action for a *running* service, since refusing to serve is a denial of service that can brick a
boot. A readiness handshake does not have that problem, because nothing depends on the service at
the moment it reports. That is why this could ship without 137 and does not pre-empt it.

The boot tour keeps its own two-draw client-side check. A tour that only repeated the service's
verdict would have caught nothing on 2026-09-04.

## The second bench run, 2026-09-04: the device worked and the tour was wrong

Milestone 220 landed the clock and reset driver and radon was booted again. Transcript:
`target/board/radon-2026-09-04-clock-and-first-entropy.log`.

```
hw clock   : JH7110 STG CRG at 0x10230000 (named by this machine's device tree):
             clocks 0x00000000,0x00000000 -> 0x80000000,0x80000000 (running);
             reset 3 assert 0x007ffffe -> 0x007ffff6, status 0x00000009 (released, 1 polls)
hw entropy : FAILED: JH7110 TRNG at 0x1600c000 (tree says starfive,trng, status disabled):
             report 0x524e475550, bring-up diagnostic 0x0000000000000020,
             draws 8/8 bytes, first-all-zero false, draws-differ true
```

Milestone 220's premise is confirmed. The clocks read `0x00000000` before and `0x80000000` after,
so bit 31 was genuinely clear and the block genuinely was gated. The section above predicted that
and it was right.

And then the `hw entropy` line said FAILED while every condition it names was satisfied. The report
word is `READY`, the first draw is not zeros, both draws are full, and the two differ.

### The failure was in the tour, and the number that misled everyone was not a register

`entropy_proto` carries `MAX_BYTES = 8` per exchange and `Wiring::get` is exactly one exchange, so
`w.get(32, &mut a)` returns 8, never 32. The tour then required `na == 32`. Its success line was
unreachable on any device, working or dead, from the day it was written.

It survived three days because this branch runs on exactly one machine in the world. QEMU's `virt`
has no TRNG node, so CI takes the `skipped` arm and never evaluates the condition. The first radon
boot failed earlier (on the gated clock) than the check that was broken.

The `0x20` was the number 32. The driver's third report word was the byte count when the report
said `READY`, and a `(STAT << 32) | ISTAT` snapshot otherwise. So the same field meant two things,
depending on a word printed beside it. Read as a register it says `ISTAT` bit 5, which no Linux
driver, no `NetBSD` driver and the TRM all fail to name, and an hour went into that bit. Read as
what it was, it says the pool held all 32 bytes it had generated. There is no evidence this device
has ever set `ISTAT` bit 5, and the entry that claimed otherwise is corrected here rather than
quietly dropped.

The diagnostic is now unconditionally the register snapshot. `Wiring::fill` loops until a buffer
is full and the tour uses it. That also makes the two draws stronger than they were: 32 bytes is
exactly one generation, so draw `a` empties the driver's pool and draw `b` forces a second trip to
the hardware. Two 8-byte draws both came out of one buffer, so `a != b` proved only that a cursor
advanced. And `kernel::user::entropy_tests::a_fill_gathers_across_round_trips` asks the virtio
backend for 32 bytes in CI, because nothing in this tree had ever asked this protocol for more than
one word.

### What the run establishes for fatal risk 6

- Confined: demonstrated 2026-09-03, and again here. The driver holds two rendezvous capabilities
  and one page of device memory, no IRQ, no DMA, no `Virtio` capability.
- Drives real hardware: demonstrated on 2026-09-04, which is the half that was blocked. A confined
  EL0 process wrote a JH7110 register, polled it, and handed a client bytes that were not zero and
  that changed between draws. It did so through a capability that names no device. The clock work
  that made it possible was milestone 220's.
- At real speed: still unmeasured on silicon. It was "measurable for the first time" here, and the
  instrument to measure it with landed on 2026-09-10. The step times itself now, so the bench
  procedure's step 5 asks for three numbers off the line rather than for a stopwatch.

At that point the success line had still never printed, which is why the status did not move then.
What printed was a FAILED line whose numbers, read correctly, describe a working device.
