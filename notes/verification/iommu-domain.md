# The IOMMU domain, and what the DMA proof does not establish

An appendix to [notes/verification.md](../verification.md), following
[direct-memory-access-validator.md](direct-memory-access-validator.md). It holds the domain-builder proofs of milestone 35 (prove the
DMA-confinement boundary), and the address path no descriptor proof can see.

## The IOMMU domain: proved where it can be, tested where it cannot

Milestone 35's third item was to confirm the IOMMU domain builder has a *maps-exactly-the-grant*
proof. The builder is `paging::domain::build_identity_domain`, from milestone 16b, part of milestone 16
(real hardware + IOMMU-backed driver isolation). The property is
the hardware sibling of the validator property: the device's DMA domain maps precisely the granted
frames and nothing else. The first pass at the milestone declined it. The property is a `Mapper`
build-and-translate round trip, a symbolic IOVA walking a *built* four-level table. That is the
BMC-over-real-memory wall the parent already declined for the ELF parser and for `Mapper` itself.

That was the right diagnosis of the wrong target. The correction is on the record because it is the
parent's own rule being applied: prefer refactoring the logic to shrinking the proof. The domain is
"an identity map over exactly the granted pages", and the *page set* is loopless arithmetic that
needs no tables at all. Factored out (`grant_pages`, `grant_page`), it proves in a quarter of a
second. The builder now calls it instead of `map_range`'s unchecked `va + i * PAGE_SIZE`, so the
proved page set is the page set that runs.

Six in `crates/paging/src/domain.rs`:

| Harness | Property |
|---|---|
| `an_enumerated_page_lies_inside_the_grant` | **soundness, the security direction**: every page the domain maps is page-aligned and lies wholly inside a granted region, so no ungranted byte becomes device-reachable (and since IOVA == PA, no ungranted physical memory is translatable) |
| `every_whole_page_of_the_grant_is_enumerated` | **completeness, the functional direction**: every whole page of a grant is mapped, proved *constructively* (the witness index is `(iova - base) / PAGE_SIZE`, so it says which iteration maps it) |
| `the_enumeration_is_injective` | no page is enumerated twice, so a legal grant cannot fail its own build against `AlreadyMapped` |
| `the_grant_enumeration_is_total` | neither entry point panics or overflows, for any base, size, or index |
| `a_page_index_below_the_count_always_resolves` | the builder's defensive `ok_or` is dead code, provably |
| `a_grant_the_domain_cannot_express_is_refused` | the two inputs that could produce an over-map (an unaligned base, an end that wraps `u64`) are refused, so the builder maps nothing rather than something rounded |

Completeness is there because without it the property is half a property. A domain that mapped
nothing at all would satisfy soundness perfectly. It would confine the device by starving it,
surfacing as a mysterious device fault rather than a refusal. Proving both directions is what makes
"*exactly* the grant" mean what it says.

One proof covers both IOMMUs, which is the right shape for a §19 (architectural parity is a tenet)
gate. The page set does not depend on the page-table format. So the same harnesses underwrite the
SMMUv3 (VMSAv8-64) domain on aarch64 and the RISC-V IOMMU (Sv39) domain on riscv. No second
harness, no parity gap.

The residual, named rather than implied. These prove the page set the builder *asks* the mapper for.
They do not prove "`Mapper::map` writes exactly one leaf for the page it is told and touches nothing
else". That is the build-and-translate round trip, and it stays on the wall. Four things underwrite
it. First, the proved walk arithmetic: `distinct_pages_take_distinct_paths` (so an ungranted page
cannot alias a granted leaf), `index_is_always_in_bounds`, the leaf codec keeping address and
permissions apart (proved for *both* formats in `aarch64.rs` and `sv39.rs`), and the
two-halves-disjoint gate. Second, `domain.rs`'s build-and-translate tests on both formats
(`aarch64_domain_confines_a_region`, `sv39_domain_confines_a_region`,
`two_disjoint_regions_map_and_the_gap_does_not`). Third, milestone 16b's end-to-end attacker test,
in which the hardware faults an escaping DMA. So the page set is proved, and the mapper writing it
faithfully is tested and composed. That is the honest line, and it is a better line than the first
pass drew.

Every one of these properties was falsified before it was believed. Round the page count up and
soundness fails. Round it down and completeness fails, while soundness correctly still holds (an
under-map is safe). Drop the wrap refusal and soundness fails. One falsification corrected a claim in
the code. Soundness rests on `grant_pages` *flooring*, not on `grant_page`'s partial-page guard,
which cannot fire for any index the builder passes. The comment there now says so. A reader
hardening the wrong line would have thought the guard was the load-bearing one.

## What the DMA proof does NOT establish: addresses that never enter a descriptor

This is the part to read before repeating "DMA confinement is proved". Said without it, that
sentence is wrong in a way that matters.

The proof is about descriptor chains. `validate_and_shadow` sees the descriptors a driver publishes
in a virtqueue, and the harnesses quantify over every one of them. That is the whole address surface
for a disk and for a NIC. Every byte those devices touch is named by a descriptor the kernel
validated and copied into a shadow the driver cannot reach.

It is not the whole address surface for a GPU. Milestone 29 (a display terminal) found this, and
DECISIONS §29 (the framebuffer is a bigger grant) records it. virtio-gpu's *backing* addresses ride
inside a `RESOURCE_ATTACH_BACKING` command payload, not in a descriptor. The kernel bounds the
descriptor carrying that command, so the payload is in-region bytes. The addresses *inside* it are
bytes the transport does not parse. The validator therefore structurally cannot see them, and no
amount of proving it harder changes that: the addresses are not in its input. Teaching it to parse
them would push virtio-gpu knowledge into the layer DECISIONS §18 (PCIe transport) keeps
device-neutral. It would also start a per-device arms race with the next device class that carries
addresses in a payload.

So the two paths have genuinely different evidence, and conflating them is the error to avoid:

| Path | What confines it | Strength of the evidence |
|---|---|---|
| Addresses in **descriptors** (disk, NIC, and the GPU's own command ring) | the shadow-ring validator, plus the IOMMU where present | **machine-checked for every input** (`crates/direct_memory_access_validator`), plus end-to-end attacker tests on both ISAs and both transports |
| Addresses in a **command payload** (virtio-gpu backings) | the IOMMU, and *only* the IOMMU | **the barrier's allow-list is proved exact; the hardware honoring it is attacker-tested.** `an_enumerated_page_lies_inside_the_grant` and `every_whole_page_of_the_grant_is_enumerated` prove the domain maps exactly the granted pages, which is the property that makes an out-of-grant payload address untranslatable; `the_iommu_refuses_the_gpu_a_framebuffer_outside_the_drivers_grant` then points a backing at a frame left out of the domain and asserts the IOMMU's fault queue recorded a fault there, on both ISAs |

The middle column of that second row is the one useful thing this milestone could prove about the
payload path. It is worth naming rather than leaving as a side effect of item 3. A payload-borne
address is stopped by having no translation in the device's domain. So "the domain maps exactly the
grant" is exactly the property that barrier rests on. Proving it moved the payload path from "tested
end to end" to "the allow-list is proved exact, the hardware honoring it is tested end to end". That
is a narrowing, not a closing. The transport still cannot see these addresses, and the enforcement
is still the hardware's.

And the consequence that made milestone 35 load-bearing in the first place cuts the other way here.
The reason to prove the validator now, rather than later, is that milestone 16a's board has no IOMMU
(the VisionFive 2; notes/target-hardware.md). So on first silicon the validator stops being defense
in depth and becomes the sole DMA confinement. That argument works for the descriptor path precisely
because the validator covers it. For the payload path it inverts. On a board with no IOMMU, nothing
covers it: not the validator (the addresses are not in its input), and not the hardware (there is
none). A display driver on the VisionFive 2 is therefore either *trusted* with all of physical
memory, or the transport grows a virtio-gpu-aware check and pays the §18 cost knowingly. That is a
decision for whoever sequences 16a. What this note owes them is that it is a decision and not an
oversight. The same holds under HVF, where PCIe DMA runs unconfined by standing default.

Stated for a skeptic in one sentence, which is how it should be stated:

> Every address that reaches a device through a virtqueue descriptor is provably confined to the
> driver's grant; addresses that reach a device inside a command payload are confined by the IOMMU
> alone, that confinement is tested rather than proved, and on a board without an IOMMU it does not
> exist.
