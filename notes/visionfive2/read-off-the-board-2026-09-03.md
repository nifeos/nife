# Read off the board, 2026-09-03

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md). Three of the facts here were guesses
until this session, the first bench session that drove radon from a script rather than by hand.

## `boot.scr.uimg` works

Milestone 218 (every boot of the VisionFive 2 needs a human typing four commands into U-Boot) had
never run on the board. It does:

```
Found U-Boot script /boot.scr.uimg
nife: boot.scr is driving this boot, milestone 218
```

It took fourteen point seven seconds from power to the end of the boot tour, with nothing typed.
That was the last piece standing between this project and an unattended run.

## Two environment values nobody had ever read

Milestone 218's lane had to leave these as assumptions:

```
scriptaddr  = 0x43900000
fdt_addr_r  = 0x46000000
```

`fdt_addr_r` is the interesting one. It sits below `0x8000_0000`, which is the parent's DTB caveat
with a number under it at last. The extlinux fallback puts the device tree outside the kernel's
boot page table. That is why the manual path moves it to `0x8600_0000`, and why the boot script
does the same.

## The TRNG is not in the tree

The driver from milestone 159 (a real hardware entropy source: the JH7110's TRNG) reported
`hw entropy : skipped`. The prompt seemed to confirm why rather than leaving it inferred:

```
StarFive # fdt print /soc/rng@1600c000
libfdt fdt_path_offset() returned FDT_ERR_NOTFOUND
```

`fdt list /soc` returns 56 nodes, and none of them was read as a random number generator. The
absence looked specific rather than general. `crypto@16000000` and `sec_dma@16008000`, the TRNG's
neighbors in the same security block, are both described. That went to milestone 239 (radon's
device tree does not describe the TRNG, so a working driver never runs).

## Correction, 2026-09-03: the node is there

The conclusion above is the part that was wrong. Milestone 239 went to the firmware's own source
rather than re-reading the board, and the node is there. The tree radon hands us is built from
`arch/riscv/dts/jh7110.dtsi` in StarFive's U-Boot fork, and that file spells it:

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

The source is starfive-tech/u-boot, branch `JH7110_VisionFive2_devel`, commit `bfbdce9b86a2` of
2023-01-06. That was the last change to the file before this board's `Feb 12 2023` firmware build,
and it is unchanged at that branch's head. It was read 2026-09-03.

So every observation above holds, and none of them meant what they were read to mean.
`fdt print /soc/rng@1600c000` failed because the node is not called that, twice over. It is
`trng`, not `rng`, and its unit address carries an upper-case C, so even `/soc/trng@1600c000`
misses. And the driver skipped because it matched mainline's `starfive,jh7110-trng` only, against a
tree that says `starfive,trng`. The neighbors were visible for the same reason they are visible in
that file: `crypto@16000000` and `sec_dma@16008000` are spelled the same way in both.

The `status = "disabled"` is U-Boot's, not the board's. StarFive's own Linux enables the identical
node from `jh7110-common.dtsi` (`&trng { status = "okay"; };`). Their kernel driver had already
moved to `starfive,jh7110-trng` in December 2022, two months before that firmware was built.
U-Boot's control DTB is a stale fork of the vendor's own hardware description. Nobody on their side
noticed, because Linux on this board reads its own DTB and never sees U-Boot's.

Milestone 239 taught `crates/jh7110_entropy`'s `discover` both spellings. It also made it carry the
`status` it found, so the next boot answers this rather than inferring it. None of that has run on
the board. The two commands that settle it are in that milestone's block.

## A second number in the same boot

The tour reported `capability slots: 4 of 24 at peak`, where QEMU reports 21. That is not a
different measurement. Milestone 230 (`script/shell-check` is red on `main`, on both architectures,
and nothing says so) established that init builds the login stack only when it has an entropy
client. No TRNG node meant no entropy, no login stack, and a much smaller peak. The 24-slot ceiling
was sized against a QEMU boot richer than the real board's. The correction above does not change
that until a boot proves the driver reaches bytes: a node found is not a device driven.
