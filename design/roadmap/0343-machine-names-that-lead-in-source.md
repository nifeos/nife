---
status: BUILT
raised: 2026-09-03
built: 2026-10-03
---
# 343. Source comments that lead with a machine name instead of the hardware

Filed 2026-09-03 as an unnumbered proposal from DECISIONS §143 (a machine's
name is not a hardware fact, and source comments should say the hardware), decided the same day;
numbered 2026-09-19 by milestone 433. **Premise re-checked 2026-09-19 and most of it has been done,
by nobody in particular.** Ten of the twelve uses are now in §143's shape and two are not. Both
comments the proposal singled out as carrying a machine name with no hardware at all have been
fixed. The module documentation of `kernel/src/soak.rs` now reads "on JH7110 silicon (radon, the board it was seen on)".
The documentation of `is_cycle_counter_grantable` in `kernel/src/arch/riscv64/timer.rs` now reads "the VisionFive 2's OpenSBI build (radon)".
`notes/README.md`'s index line is fixed too, and it was the smallest and most valuable part:
`target-hardware.md` is now indexed as "Where nife could actually run, **and what the three bench
machines are named**", so a stranger who meets `radon` has a route. `crates/multicast_dns_protocol`
went with the crate on 2026-09-15. The last two of the nine that gloss the name but lead with
it were fixed 2026-10-03 (see `## Follow-on`).

Those two comments were the last of the twelve, and both are closed (see `## Follow-on`).

The inventory as filed on 2026-09-03, kept because it is the account of how the twelve were
classified and because it is what the remaining two are measured against. §143 decided that a source
comment states the hardware and may name a machine only as a trailing gloss where the instance is
genuinely the point. The tree mostly did this already; twelve uses needed bringing into line, and
they were of three kinds. The status line above says which of them are still open.

Two carry the fact with no hardware at all, and both are really claims about the SoC or about
silicon-versus-emulation, which is what the reader needs:

- `kernel/src/soak.rs`: *"on radon, in `wake_load_aware`"*
- `kernel/src/arch/riscv64/timer.rs`: *"recorded that as unknown on radon and it is still
  unknown"*

Nine gloss the name but lead with it (`"on radon (the StarFive VisionFive 2)"`), where the
relevant half belongs first.

One was example data, `crates/multicast_dns_protocol`'s `host: "patagonia"`, which §143 explicitly puts
outside the rule. That crate was retired on 2026-09-15, so the case no longer exists in the tree. Changing it is a preference (a household name in a public crate's documentation),
not a consequence of the decision, and whoever does this should say which they acted on.

And `notes/README.md` indexes `target-hardware.md` as "Where nife could actually run", which does
not advertise that it is where the machine names are defined. A stranger who meets `radon` and wants
to resolve it has no obvious route. One clause fixes it, and it is the smallest and most valuable
part of this.

What it must not do. `notes/` and `design/` keep their machine names: about a hundred uses, and
they are load-bearing there because a measurement series has to assert that the same physical machine
was held constant. A sweep that treated this as a rename would destroy the thing that makes
notes/soak.md's nine-boot spread evidence at all. §143's `BUGS` records that the boundary is a
judgment, so this wants a reader rather than a `sed`, which is the lesson of the blind rename
already on AGENTS.md's record.

## Follow-on

- **Done.** The last two comments are in §143's shape. The module documentation of `kernel/src/arch/riscv64/iommu.rs`
  now reads "the JH7110 (radon, the `VisionFive` 2) has no IOMMU at all", hardware first. The
  `xtask/src/main.rs` comment no longer exists. It went when `xtask` was split into modules
  and the cargo-feature pack (`5d210e035`). Grepping `kernel/`, `crates/`, `components/`,
  `fixtures/`, `xtask/`, `script/` and `helpers/` for a machine name followed by a parenthesized
  gloss finds no comment that leads with the name and omits the hardware.
- **Done.** The other ten uses and the `notes/README.md` index line were brought into line
  incrementally between 2026-09-03 and 2026-09-19 by lanes editing those files for other reasons,
  which is why this block is `PARTIAL` rather than `NOT-STARTED`. Verified by reading each site named
  in the proposal: `kernel/src/soak.rs`, `kernel/src/arch/riscv64/timer.rs` and `notes/README.md`'s
  `target-hardware.md` entry. `crates/multicast_dns_protocol`'s `host: "patagonia"` example datum,
  which §143 put outside the rule anyway, went with the crate's retirement on 2026-09-15.

## Index row

DECISIONS §143 decided that a source comment states the hardware and may name a machine only as a
trailing gloss where the instance is genuinely the point. The reason is that a machine's name is not
a hardware fact and a reader who meets `radon` in a comment about a SoC erratum learns nothing. Twelve uses in
source needed bringing into line when this was filed, plus one index line in `notes/README.md` that
did not advertise where the machine names are defined. Ten of the twelve and the index line have
since been fixed incrementally by lanes touching the files for other reasons, and the last two
(the module documentation of `kernel/src/arch/riscv64/iommu.rs`, and an `xtask` comment that has since been deleted) were
closed 2026-10-03. `notes/` and `design/` keep their machine
names, about a hundred uses, because a measurement series has to assert that the same physical
machine was held constant. A sweep that treated this as a rename would destroy what makes
notes/soak.md's nine-boot spread evidence at all. §143's own `BUGS` records that the boundary is a
judgment, so this wants a reader rather than a `sed`.
