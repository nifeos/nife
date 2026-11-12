# Prior art

An appendix to [the milestone](../1104-paging-outside-review.md).

How the crate sits against what a crates.io user already knows. Recent downloads are the last 90
days at the time of the review, from the crates.io API.

| crate | recent / all-time downloads | formats | walk | TLB obligation | what a user coming from it will miss here |
|---|---|---|---|---|---|
| `x86_64` 0.15.5 | 1.01M / 5.93M | IA-32e only | `Mapper<S>` trait per page size; `OffsetPageTable`, `RecursivePageTable`, `MappedPageTable` | `MapperFlush<S>` is `#[must_use = "Page Table changes must be flushed or ignored."]` with `flush()` and a safe `ignore()`; `map_to` is `unsafe` | `update_flags`, `set_flags_p*_entry`, `translate_page` per size, `identity_map`, `PageAlreadyMapped` carrying the frame |
| `aarch64-paging` 0.12.3 | 83.3k / 242k | AArch64 VMSA, stage 1 and 2, EL3/EL2/EL1 regimes | `Mapping` over a `Translation` trait; `IdMap`, `LinearMap` behind `alloc` | not read for this review (unverified) | `modify_range`, `walk_range`, `activate()` (unverified: taken from the crate's landing page) |
| `page_table_multiarch` 0.6.2 | 7.5k / 118k | x86_64, AArch64, ARMv7-A, Sv39, Sv48, LoongArch64 | `PageTable64<M, PTE, H>` with a `PagingHandler` for `alloc_frame`, `dealloc_frame`, `phys_to_virt`; a cursor API | not read for this review (unverified) | Sv48, 32-bit tables, `dealloc_frame`, `query` returning the page size |
| `page_table_entry` 0.6.2 | 9.0k / 141k | the entry types behind the crate above | none | none | a shared `GenericPTE` trait a user can implement for a new format |

Outside Rust: Linux keeps one walk per architecture behind `pgtable.h` and batches invalidation in
`mmu_gather`. FreeBSD and OpenBSD keep it in `pmap(9)` (`pmap_enter`, `pmap_remove`, `pmap_protect`)
with the TLB shootdown inside. SeL4 proves its VSpace operations in Isabelle and makes page tables
kernel objects a capability names. None of those is a library another kernel can link.

Where this crate differs: one walk over six formats, two of them IOMMU formats (`Vtd`, `AmdVi`),
chosen by a type parameter. No instructions emitted, so the host tests build real tables in host
memory and the same code runs under Kani. A caller cannot safely ignore `TlbFlush` (dropping it
panics; only `flush` or the `unsafe` `assume_no_stale_entry` discharge it), where `x86_64` lets a
caller `ignore()` it. A present leaf is never overwritten (`AlreadyMapped` forces
break-before-make). W^X is a property of the `Flags` constructors, with no writable-and-executable
constructor to call. 38 proofs, each shipped with the patch that makes it fail. Zero dependencies.
What it costs: no permission change on a live mapping, no block split, no reclamation of an emptied
table, the 4 KiB granule only, and the smaller method surface in the last column.

The concrete example: with `x86_64`, `unsafe { mapper.map_to(page, frame, flags, &mut alloc) }?
.ignore()` compiles and leaves a stale translation if the page was live. Here the same shape is
`mapper.unmap(va)?` followed by `mapper.map(...)`, and the `TlbFlush` that the `unmap` returned
panics the moment the program drops it undischarged, which `tests/mapping.rs` pins
(`dropping_the_tlb_obligation_is_fatal`).
