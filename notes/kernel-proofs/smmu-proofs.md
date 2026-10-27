# Two proofs in `kernel/src/arch/aarch64/iommu.rs` (milestone 255)

*Name: provisional, minted 2026-10-11 (UTC) by lane/fourth-ten-longest-notes. That lane moved the
main page's two per-file proof sections here under §212 (a prose budget). The SMMUv3 pair is from
milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it), and the IO APIC pair is from
milestone 304 (`cargo kani -p kernel` only ever compiled one architecture). Each file is named for
the code it proves. calef names files.*

An appendix to [`notes/kernel-proofs.md`](../kernel-proofs.md), which states what is proved. These
two came from milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it).

The SMMUv3 driver is aarch64's IOMMU. It has no `asm!` in it at all. What it computes is which
physical addresses a device may touch, which is what `design/fatal-risks/README.md` risk 7 rests on.
The word arithmetic was inline in `attach`, tangled with the volatile writes that make that function
unreachable. It is now three functions in the same file, and `attach` is shorter for it. Nothing
moved out of `arch/`, which is milestone 193's option A held to and milestone 244's refusal of
option B applied again.

- `the_smmu_is_handed_exactly_the_tables_the_kernel_built`. The context descriptor's TTB0 and
  the stream table entry's `S1ContextPtr` are each a physical address split across two 32-bit words.
  Each shares its low word with control bits (V, CONFIG and S1FMT sit under the STE's pointer).
  Take every page-frame `ttb` and every 64-byte-aligned `ctxptr` below 2^52. Both fields read back
  from the built words, at the bit positions Arm IHI 0070 gives them, are exactly the input. The
  control field is what it was written as rather than what an overlapping address bit made it. The
  second half is the worse failure. An address bit landing on CONFIG turns the stream to bypass,
  which is translation switched off rather than pointed somewhere wrong.
- `no_stream_can_reach_another_streams_tables`. `table_offset` is the whole of the addressing
  for both tables. Its `assert!` is the only thing between a device-tree-supplied `StreamID` and
  a raw write. For every pair of ids the bound admits, each entry lies inside the single frame that
  holds the table, and no two entries share a byte.

Both were falsified, and the falsifications are the argument. Each patch leaves every test in
the tree green. QEMU's `virt` board puts RAM at `0x4000_0000`, so bit 31 of a domain root is clear
and `ttb >> 31` agrees with `ttb >> 32` on this board. And the one PCIe disk's requester id is far
below 64, so raising `STRTAB_LOG2` to 7 writes off the end of a frame that nothing ever addresses.
`script/falsifications --sweep kernel` replays both.

What this does not cover, said here as well as at the harness. The register offsets and bit
constants are unproved and unprovable in this tree. A harness asserting the constant it was given is
a tautology, and if Arm IHI 0070 was misread then the code and the proof are wrong together. That is
why `the_iommu_faults_a_dma_that_escapes_the_domain` in `kernel/src/virtio.rs` is not made redundant
by this. And a property proved of the SMMUv3 is not proved of the other two IOMMUs.
`riscv64/iommu.rs` writes its device context in 64-bit stores with no split at all, so this property
has no counterpart there. `x86_64/` is not even compiled under Kani on an aarch64 host.
