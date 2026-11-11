# Three proofs in `kernel/src/arch/aarch64/iommu.rs` (milestones 255 and 1066)

*Name: provisional, minted 2026-10-11 (UTC) by lane/fourth-ten-longest-notes. That lane moved the
main page's two per-file proof sections here under §212 (a prose budget). The SMMUv3 pair is from
milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it), and the IO APIC pair is from
milestone 304 (`cargo kani -p kernel` only ever compiled one architecture). Each file takes its name from
the code it proves. calef names files.*

An appendix to [`notes/kernel-proofs.md`](../kernel-proofs.md), which states what is proved. Two
came from milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it), and the third from
milestone 1066 (the IOMMU device tables grow past 64 devices), which made the stream table two-level.

The SMMUv3 driver is aarch64's IOMMU. It has no `asm!` in it at all. What it computes is which
physical addresses a device may touch, which is what `design/fatal-risks/README.md` risk 7 rests on.
The word arithmetic was inline in `attach`, tangled with the volatile writes that make that function
unreachable. It is now five functions in the same file, and `attach` is shorter for it. Nothing
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
- `no_stream_can_reach_another_streams_tables`. `stream_position` is the whole of the addressing
  for both tables: which leaf, and the offset into that leaf's STE frame and its CD frame. It is
  the only thing between a requester id from the bus and a raw write. Over every `u32` and every
  table width `initialize` can choose, it places exactly the ids below the width and refuses the
  rest with `None`, never a panic. Every placed entry has a level-one descriptor and lies inside its
  leaf's single frame, and no two entries share a byte.
- `the_level_one_table_names_exactly_the_leaf`. A level-one descriptor shares its low word with
  `SPAN`. For every page-frame leaf below 2^52, `L2Ptr` reads back as the leaf entire, and `SPAN`
  says the leaf holds exactly the 64 STEs one frame holds. The `STRTAB_BASE_CFG` word carries the
  width and split the kernel indexes by, and the two-level format.

All three were falsified, and the falsifications are the argument. Each patch leaves every test in
the tree green. QEMU's `virt` board puts RAM at `0x4000_0000`, so bit 31 of a domain root is clear
and `ttb >> 31` agrees with `ttb >> 32` on this board. A leaf offset left at the linear table's
`rid * 64` agrees with the right one for every stream below 64. And QEMU reads `SPAN` as `2^SPAN`
entries where the specification says `2^(SPAN - 1)`, so a `SPAN` one short passes under QEMU and
halves every leaf on silicon. `script/falsifications --sweep kernel` replays all three.

What this does not cover, said here as well as at the harness. The register offsets and bit
constants are unproved and unprovable in this tree. A harness asserting the constant it was given is
a tautology, and if Arm IHI 0070 was misread then the code and the proof are wrong together. That is
why `the_iommu_faults_a_dma_that_escapes_the_domain` in `kernel/src/virtio.rs` is not made redundant
by this. And a property proved of the SMMUv3 is not proved of the other two IOMMUs.
`riscv64/iommu.rs` writes its device context in 64-bit stores with no split at all, so this property
has no counterpart there. `x86_64/` is not even compiled under Kani on an aarch64 host.
