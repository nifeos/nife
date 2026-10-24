# Appendices to notes/kernel-proofs.md

[`notes/kernel-proofs.md`](../kernel-proofs.md) is the page to read. Each file here holds detail it
links to, moved out under §212 (a prose budget) so the main page stays inside
its word budget.

*Name: provisional, minted 2026-09-25 by milestone 432 (the RISC-V IOMMU driver has no
counterpart to the SMMU's proofs) as §212's parent-named sibling directory of
`notes/kernel-proofs.md`, whose own name is provisional too. Naming is calef's.*

`smmu-proofs` and `x86-irq-proofs` Name: provisional, minted 2026-10-11 (UTC) by
lane/fourth-ten-longest-notes. That lane moved the main page's two per-file proof sections here
under §212 (a prose budget). The SMMUv3 pair is from milestone 255 (a quarter of `kernel/src/arch/`
has no assembly in it), and the IO APIC pair is from milestone 304 (`cargo kani -p kernel` only ever
compiled one architecture). Each file is named for the code it proves. calef names files.

- [`smmu-proofs.md`](smmu-proofs.md): the two SMMUv3 harnesses, their falsifications and what they
  do not cover.
- [`x86-irq-proofs.md`](x86-irq-proofs.md): the two vector-band harnesses, the GSI wrap one of them
  found, and why both are `attested`.
