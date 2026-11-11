# Blocks: 2 MiB and 1 GiB leaves

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/verification.md](../verification.md). It holds the proofs `crates/paging`
gained on 2026-09-19 for milestone 161 (the x86_64 kernel port), when the direct map moved to
block leaves.

## What changed, and the shape of the proof

`crates/paging` gained a second and third leaf size, so the direct map stops costing 0.2% of RAM in
page tables. On QEMU's 256 MiB x86 machine that went from 560 KiB to 60 KiB; at 4 GiB, from 8,252 KiB
to 64 KiB. The proofs follow the domain builder's shape ([iommu-domain.md](iommu-domain.md)). The
decision that makes a block safe is pulled out as loopless arithmetic and proved for every input,
and the walk that writes it stays tested.

| Harness | Property |
|---|---|
| `verification::a_chosen_leaf_is_aligned_and_inside_the_span` | **soundness**: every leaf `Mapper::map_span` writes is aligned at both ends, no larger than the caller allowed, and lies wholly inside what is left of the span, so blocks map no byte the equivalent pages would not |
| `verification::the_chosen_leaf_is_the_largest_that_fits` | **completeness**: no fitting larger leaf is passed over; soundness alone is satisfied by always answering 4 KiB |
| `x86_64` / `aarch64` / `sv39` `::a_block_keeps_address_and_permissions_apart` | each format's block encoding, over every `u64` address and every `Flags` constructor: the address in its architectural field, nothing set below the block's alignment (reserved on x86, `RES0` on aarch64, a misaligned-superpage fault on Sv39), the format's block marker as a literal bit pattern, and the flags round-tripping; on x86 also W^X on the hardware bits |
| `x86_64` / `aarch64` / `sv39` `::a_table_entry_is_never_a_block` | no table pointer ever reads as a block, so the walk never stops at a table or descends into a block |

Every assertion is spelled in literals. The reason is milestone 211 (a harness that states its
property through the function under test cannot see that function break), and milestone 307 (which
assertion actually fires when a confinement claim is broken). The five `replayable` records were
swept red on 2026-09-19 (`script/falsifications --sweep paging`: 13 swept, 0 survivors).

## A lesson that cost a quarter of an hour

`PageSize::largest_fitting` is two `if`s and a mask test, and proves in 0.05 s. Written as a loop
over the sizes testing alignment with `%`, CBMC was still solving
`the_chosen_leaf_is_the_largest_that_fits` after ten minutes. Both changes went in together, so which one mattered
is not isolated. The harnesses themselves still use `%` (`is_multiple_of`) in their assumptions and
prove in well under a second, which points at the loop.

The residual is the usual one. The proofs cover the leaf choice and the encodings, not `Mapper`
writing the block into a built table. That is tested on all three formats
(`crates/paging/tests/blocks.rs`) and booted.
