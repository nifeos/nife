---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: radon (the device tree radon's own firmware hands over is the subject)
needs_person: yes
---
# 239. radon's device tree does not describe the TRNG, so a working driver never runs

The premise in that title is false and the title is kept anyway, because milestone 241 (what a
fourth board would have to be for, so that GICv3 is bought rather than justified) cites this
milestone by that name. The sections below say what is actually true of the board that runs
milestone 159 (a real hardware entropy source: the JH7110's TRNG). The token follows milestone
218's (every boot of the VisionFive 2 needs a human typing four commands into U-Boot) precedent.
The route is built and has not run on radon, so the status says what the outcome is rather than
what was built.

Everything that does not need the board is built, host-tested and merged; what
remains is two commands at a U-Boot prompt and one boot, and QEMU cannot stand in, because the
`virt` machine has no JH7110 anything.

In brief. radon booted nife hands-free on 2026-09-03 (milestone 218 (every boot of the VisionFive 2
needs a human typing four commands into U-Boot)) and the tour reported:

```
hw entropy  : skipped (this machine's tree describes no starfive,jh7110-trng; QEMU virt has none)
```

The line was true. The conclusion drawn from it, that the tree omits the device, was not. The tree
radon hands us does describe the TRNG. It describes it under a different node name and a different
`compatible` string, and calls it disabled.

## What it actually is

`fdt addr ${fdtcontroladdr}` hands nife the vendor U-Boot's own control device tree: U-Boot 2021.10,
`Build: jenkins-VF2_515_Branch_SDK_Release-24`, dated `Feb 12 2023`, from the board's own banner in
`crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-manual-boot.log`. That tree is
compiled from `arch/riscv/dts/jh7110.dtsi` in StarFive's U-Boot fork, and that file says:

```
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

Read at commit `bfbdce9b86a2` (2023-01-06), the last change to that file before the flashed
firmware's build date, and unchanged at that branch's head:
<https://github.com/starfive-tech/u-boot/blob/bfbdce9b86a2/arch/riscv/dts/jh7110.dtsi>. Mainline
Linux spells the same device `rng@1600c000`, `compatible = "starfive,jh7110-trng"`, and that is what
`crates/jh7110_entropy` was written against.

Every observation from the bench holds, and none of them meant what it was read to mean.

| What was seen on 2026-09-03 | What it was read as | What it was |
|---|---|---|
| `fdt print /soc/rng@1600c000` -> `FDT_ERR_NOTFOUND` | no such device in the tree | no such **path**: the node is `trng`, not `rng`, and its unit address carries an **upper-case C**, so `/soc/trng@1600c000` misses too |
| `fdt list /soc` -> 56 nodes, no random number generator | the device is absent | `trng@1600C000` is one of the 56; it was not recognized |
| `crypto@16000000` and `sec_dma@16008000` present, their neighbor absent | a specific omission, so a firmware build choice | both are spelled identically in the two trees, so both matched a reader's eye where the third did not |
| `hw entropy : skipped (... describes no starfive,jh7110-trng ...)` | the tree names no TRNG | the tree names no `starfive,jh7110-trng`, which is a narrower claim than the sentence reads as, and it was the exact truth |

**The three shapes of fix this block was minted with are all answers to a question that was not the
question.** Loading a fuller DTB from the card, carrying an overlay, and accepting the vendor tree's
limits all assume a missing node. Nothing is missing.

## Why the vendor tree spells it that way, which decides whether to trust it

`starfive,trng` is not a different device and it is not a different register block. It is a stale
fork, and there are two pieces of evidence rather than an argument:

- StarFive's own kernel driver had already moved on. `drivers/char/hw_random/starfive-trng.c` in
  `starfive-tech/linux` matched `starfive,jh7110-trng` at commit `202b558ae34c`, 2022-12-14, two
  months before this firmware was built
  (<https://github.com/starfive-tech/linux/blob/202b558ae34c/drivers/char/hw_random/starfive-trng.c>).
  Its register `#define`s are `CTRL` 0x00, `STAT` 0x04, `MODE` 0x08, `SMODE` 0x0C, `IE` 0x10,
  `ISTAT` 0x14, `RAND0`..`RAND7` 0x20..0x3C, `AUTO_RQSTS` 0x60, `AUTO_AGE` 0x64, with the same
  `CTRL`/`STAT`/`ISTAT` bit positions mainline's `jh7110-trng.c` carries. Two drivers written
  against one IP block agree completely, which is what makes accepting the vendor string a claim
  about a spelling rather than a claim about silicon.
- Nobody on their side had a reason to notice. Linux on this board is handed the kernel
  package's own DTB. It never sees U-Boot's, so U-Boot's copy could drift for a year without
  costing StarFive anything. It costs us, because nife takes U-Boot's tree and nothing else.

**And the `status = "disabled"` is U-Boot's, not the board's.** StarFive's Linux enables the
identical node from `jh7110-common.dtsi` (`&trng { status = "okay"; };`), along with `&crypto` and
`&sec_dma`. U-Boot marks it disabled because U-Boot has no driver for it.

## What was built, and the one decision inside it

`machine_discovery::jh7110::entropy::discover` tries mainline's `starfive,jh7110-trng` first and the vendor's
`starfive,trng` second, reads every property against the string that matched, and carries two new
facts out: which spelling it was, and what the node's `status` says.

**`status` is reported and not acted on**, which is the decision. Refusing a node the firmware calls
disabled would be correct device-tree semantics and wrong here, and this tree already has the reason
written down in the other direction. The same DTB marks the S7 monitor core `status = "okay"` and
claims it has an Sv39 MMU, and both are false (notes/visionfive2.md, "Second bench stop"). A tree
that lies about `status` in one direction has not earned obedience in the other. So the boot tour's
failure line now carries the tree's own two words (`tree says starfive,trng, status disabled`)
beside the bring-up diagnostic, because those two facts together are what triage needs. An all-zero
register window on a node the firmware calls disabled is a clock or a reset nobody ungated. That is
milestone 220's (a clock and reset controller, because the first real device needs one) territory
rather than a driver bug.

The skip line names both strings now. The old wording was the exact truth and was still read as a
stronger claim than it made, which is a defect in the line rather than in the reader. Observed on a
riscv64 boot under QEMU on 2026-09-03 (`script/soak --arch riscv64`), which is the only machine
available to say it:

```
hw entropy  : skipped (this machine's tree names no TRNG: neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng; QEMU virt has neither)
```

**The fixture is transcribed, not captured**:
`crates/machine_discovery/tests/fixtures/jh7110-trng-vendor-uboot.dts`, from the firmware's own source at
the commit above, wrapped in the `/soc` node with the two-cell `#address-cells`/`#size-cells` that
were measured off the board and are already committed in
`crates/machine_discovery/tests/fixtures/visionfive2-uboot-control.dts`. `dtc` warns
`simple-bus unit address format error, expected "1600c000"` when compiling it, which is the
upper-case C being genuinely irregular rather than a transcription slip.

## What is left, and it is a bench session

Nothing in this milestone has run on radon. The board was powered off when the work was done and
there was no bench session. Two commands and one boot settle all of it.

1. At the `StarFive #` prompt, with a serial terminal that can type (`screen /dev/cu.usbmodem*
   115200`; `script/board-console` deliberately cannot type):

   ```
   StarFive # fdt addr ${fdtcontroladdr}
   StarFive # fdt print /soc/trng@1600C000
   ```

   **The upper-case C is required** and is the whole reason the first attempt missed. If that path
   is not found, fall back to `fdt list /soc` and read the whole list rather than scanning it for a
   word.

2. Then let the board boot normally (the card's `boot.scr.uimg` needs nothing typed) and read the
   `hw entropy` line, which is the last line of the tour before the banner.

| What comes back | What it means |
|---|---|
| `fdt print` shows the node, `compatible = "starfive,trng"`, `status = "disabled"` | **The finding above is confirmed.** Whatever the `hw entropy` line then says is about the device, not about the tree |
| `hw entropy : JH7110 TRNG at 0x1600c000 served 32+32 bytes ...` | **Milestone 159's line, and fatal risk 6 answered.** Record it whole; the tree question is closed and this block is BUILT |
| `hw entropy : FAILED: ... (tree says starfive,trng, status disabled): ... bring-up diagnostic 0x0000000000000000 ...` | The node was found and the register window read as nothing. The expected outcome if the firmware never ungated the block's clocks or deasserted its reset, which is exactly what `status = "disabled"` predicts. **This routes to milestone 220**, not back here |
| `hw entropy : FAILED: ... bring-up diagnostic 0x<nonzero> ...` | The device answered and the driver's sequence is wrong. Milestone 159's own table decodes the word |
| `hw entropy : skipped (... neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng ...)` | **The node really is absent from the running firmware's tree**, whatever its source says, and this block's three original shapes of fix come back. Capture the tree (`fdt print` to a file, or the raw dump route notes/visionfive2.md's PLIC fixture used) before deciding anything |
| `fdt print` finds the node but the boot still skips | A discovery bug, not a tree question. The fixture test says the decode works on this shape, so suspect the `/soc` wrapper's cells or a node deeper than `crates/device_tree_blob`'s `MAX_DEPTH` of 16 |

## Follow-on

- **Done.** `machine_discovery::jh7110::entropy::discover` accepts both spellings, carries which one matched and the
  node's `status`, and is host-tested against a fixture transcribed from the firmware's own source.
- **Done.** notes/visionfive2.md's "The TRNG is not in the tree" section and its "To measure at the
  bench" item 9 ask the question that can actually be answered.
- **Recorded.** Milestone 159's step-4 outcome table still quotes the old skip line
  (`... describes no starfive,jh7110-trng; ...`) and still reads that line as "the board's own
  device tree has no TRNG node". Both are stale; the corrected table is the one above, not 159's,
  and the maintainer owns reconciling them.
- **Milestone 220.** A clock and reset controller. If the bench run comes back with an all-zero
  bring-up diagnostic, that is not a defect in this milestone or in 159: it is the firmware never
  having ungated a block it calls disabled, and 220 is where it goes. This milestone's work makes
  that outcome distinguishable from "no such device", which it was not before.
- **Refused.** Shipping a fuller DTB from the card, or an overlay, or a `fdt mknode` sequence in the
  boot script. All three were priced and all three lost to a four-line decoder change. They make
  this project the author of a hardware description, and they change every discovery input at once
  on a board nobody can currently test. The boot-script route depends on the vendor
  U-Boot's `fdt` subcommands and on a node path with an upper-case C in it. The one thing they would
  buy over the decoder change is flipping `status` to `okay`, and that buys nothing: U-Boot has no
  driver for this block either way, so no `fdt set` ungates a clock.
- **Refused.** Further work on the device-tree source itself. The worry that a tree describing less
  than the chip has is a limit on every future driver does not hold: the vendor tree describes 56
  nodes under `/soc` including this one. What it has is stale *names*, and the answer to a stale
  name is to match both, at the one place that reads it.

## BUGS

- Nothing here has touched silicon. The node's presence in the running firmware's tree is
  inferred from that firmware's source at the right vintage, not read off the board. It is a strong
  inference (the model string, the two neighbor nodes, and the PLIC node in
  `crates/machine_discovery/tests/fixtures/visionfive2-uboot-control.dts` all match that source) and
  it is still an inference. The `fdt print` above is what turns it into a fact.
- **The firmware could have been built from a tree this repository has not read.** The banner names
  a Jenkins build of a release branch, not a commit, so "the last change before Feb 12 2023 on
  `JH7110_VisionFive2_devel`" is a reasonable reading of which source went in and not a proven one.
  The last row of the table above is what that possibility looks like from the boot tour.
- Accepting `starfive,trng` is a claim about a register layout, made from two drivers agreeing.
  Neither driver was run. If the vendor tree's node were somehow a different block at the same
  address, this change would hand a confined userspace process a mapping of it, which is the same
  authority milestone 159 already grants and no more.
- **`status` is read and deliberately ignored.** A future device whose tree honestly says
  `disabled`, meaning it, will be driven anyway. That is the right trade for this board and it is
  not obviously right for the next one; the field is there so a caller can change its mind without
  changing the decoder.
- The `device_tree_blob` crate's `MAX_DEPTH` is 16 and this fixture's node sits at depth 3. The real control
  DTB's `/soc/trng@1600C000` is at the same depth, so the limit is not in play, but nothing in the
  test proves that about the real tree because the real tree is not in the test.

## Index row

**the title's premise is false and the block says so**: the vendor U-Boot's tree describes the
device as `trng@1600C000`, `compatible = "starfive,trng"`, `status = "disabled"`, where mainline
says `rng@1600c000`/`starfive,jh7110-trng`. `discover` now matches both spellings and carries the `status` it found. A route was taken 2026-09-03 and radon was powered off, so like milestone 218
the token says what the outcome is rather than what was built; the block carries the two commands
that settle it
