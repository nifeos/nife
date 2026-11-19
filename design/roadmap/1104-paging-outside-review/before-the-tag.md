# P1: before the tag

An appendix to [the milestone](../1104-paging-outside-review.md).

4. The documentation assumes a reader inside nife. 52 lines in `src/` and `tests/` cite `§N`,
   `milestone N`, `notes/...` or `design/...`. 30 lines are `Name:` provenance blocks.
   `src/lib.rs` line 11 names the architect. `standalone.py` refuses all of them. A stranger cannot
   follow `notes/page-tables.md`, `MappingHold::record_mapping`, `revoke::revoke_region`,
   `framebuffer_driver`, `board_th1520`, `arch::x86_64::initialize`, `mmu::initialize` or
   `kernel/src/arch/x86_64/iommu.rs`. A doc comment points at each of them as if the reader had it
   (`src/mapper.rs` lines 35 to 43, `src/mapper.rs` line 372, `src/flags.rs` line 46, `src/flags.rs` line 146,
   `src/sv39.rs` line 147, `src/x86_64.rs` line 17, `src/x86_64.rs` line 235, `src/aarch64.rs` line 92). The hardware
   citations (Arm DDI 0487, the Intel SDM, AMD 48882 revision 2.62, QEMU's `intel_iommu_internal.h`,
   Linux's `pgtable-64.h`) are the ones to keep. Fix: rewrite each comment to say what the kernel
   does with the item in words, keep the hardware sources, move the history to git, and run
   `standalone.py` to zero. Effort M. Two-way.

5. Public names the spelled-out rule refuses. The template's `spelled_out.py`, at a ceiling of
0, refuses 84 definitions. Public among them: `is_user_page_va` (`src/page_format.rs` line 43) and `PageFormat::entry_pa` (`src/page_format.rs` line 66). The `va` and `pa` parameters of every `PageFormat` method and every `Mapper` method (`src/mapper.rs` lines 138 to 451). `PageSize::largest_fitting` (`src/page_size.rs` line 53), `DmaRegion` (`src/domain.rs` line 12) and `pat::MSR` (`src/x86_64.rs` line 23). `AMD_VI_PR` (`src/x86_64.rs` line 331, refused as `pr`; it is "present"). The rule (`design/naming.md`,
"Acronyms and abbreviations") lists `va`, `pa`, `dma` and `msr` by name, and says an integer
constant is never kept. The helper does not catch these, but the same rule applies by judgment. `phys_to_ptr` and `alloc_page_frame` as parameters (`src/mapper.rs` lines 66 to 67, `src/domain.rs` lines 81 to 82). `attrs` (`src/aarch64.rs` line 111, `src/sv39.rs` line 55, `src/x86_64.rs` line 107). `kernel_rodata` and `user_rodata` (`src/flags.rs` line 84, `src/flags.rs` line 125), where "read-only data" is the spoken phrase. Fix: one
rename lane: `virtual_address` and `physical_address` throughout, `is_user_page_virtual_address`,
`entry_physical_address`, `DirectMemoryAccessRegion`, `pat::MODEL_SPECIFIC_REGISTER` with
`#[doc(alias = "IA32_PAT")]`, `AMD_VI_PRESENT`, `physical_to_pointer`, `allocate_page_frame`,
`attributes`, `kernel_read_only_data` and `user_read_only_data`. Every new name provisional. The
kernel's call sites move with them. Effort M. Two-way before the first release. The maintainer rules
them provisionally, and the release review rechecks them.

6. Kept specification names carry no alias. `Aarch64`, `Sv39`, `Sv39Mae`, `Ia32e`, `Vtd`, `AmdVi`,
   `TlbFlush`, the modules `mair` and `pat`, and the register-bit constants (`AF`, `SH_INNER`,
   `PXN`, `UXN`, `NG`, `PPN_SHIFT`, `PWT`, `PCD`, `XD`, `DTE_TV`) keep their source's spelling. The
   rule allows this for a term a reader searches for. The condition is that the doc comment opens
   with the full name and source and the item carries `#[doc(alias = "<plain-English name>")]`. The
   crate has zero `doc(alias)` attributes. `Ia32e` (`src/x86_64.rs` line 99) and `Sv39Mae`
   (`src/sv39.rs` line 150) state their source. The rest do not. `TlbFlush` is the one judgment call: the
   rule keeps an acronym only where nobody says the expansion as a phrase, and "translation
   lookaside buffer" is rarely spoken, so the recommendation is to keep it with an alias. Fix: one
   pass adding the opening line and the alias to each kept name. Effort S. Two-way.

7. `Mapper::new`'s safety contract omits the allocator. `src/mapper.rs` lines 77 to 88 requires a zeroed,
   page-aligned `root` and a `phys_to_ptr` that satisfies the type's contract, and says nothing
   about `alloc_page_frame`. The safe methods `map` and `map_block` then write
   `(*(self.phys_to_ptr)(new)).entries = [0; ENTRIES]` through a `*mut PageTable` built from
   whatever the allocator returned (`src/mapper.rs` lines 159 to 167, `src/mapper.rs` lines 226 to 231). A frame that
   lacks 4 KiB alignment, lacks exclusive ownership, or lacks a route through `phys_to_ptr` makes
   safe code reach undefined behavior. `build_identity_domain` (`src/domain.rs` lines 75 to 78) states
   exactly this condition. `Mapper::new` does not, and every kernel call site argues only the root.
   Fix: state in `Mapper::new`'s `# Safety` that every frame the allocator returns is page-aligned,
   exclusively owned by the mapper from that moment, reachable through `phys_to_ptr`, and not a
   frame any live leaf maps. Add `debug_assert!(new.is_multiple_of(PAGE_SIZE))` at both sites or
   return `MapError::Misaligned` for a misaligned frame. Effort S. Two-way.

8. The proofs say "over every constructor" and miss some. Every `all` array in the format harnesses omits `Flags::write_combining` and `Flags::user_write_combining`. The arrays are at `src/aarch64.rs` lines 402 to 411, 433 to 442 and 481 to 490. More are at `src/sv39.rs` lines 468 to 477, 500 to 509 and 544 to 553. The rest are at `src/x86_64.rs` lines 1000 to 1009, 1037 to 1046, 1072 to 1081 and 1133 to 1142. `flag_tests::ALL` (`src/flag_tests.rs` lines 3 to 12) omits them too. On `Ia32e` those two are the only flags that take the PWT-alone path
(`src/x86_64.rs` lines 118 to 123), so the W^X, round-trip and address-field proofs never see that branch.
The host tests do (`src/eighth_pass_attacks.rs` lines 56 to 67, `src/x86_64.rs` lines 614 to 635), and
`src/flags.rs` lines 41 to 47 already records half of this gap in its BUGS. Fix: a `Flags::ALL` constant of
every constructor (eleven), with one test that its length equals the number of public
constructors, used by every harness
and test. The harness docs then say "every constructor" truthfully. Effort S. Two-way.

9. The mutation exemptions and their reasons live in nife's `.cargo/mutants.toml`. Lines 328-388
   there hold 33 regexes of the form `^crates/page_tables/src/<file>\.rs:...`, each justified by a
   `THE_*_SHARE_NO_BIT` compile-time assertion (`src/share_no_bit.rs`). In the new repository the
   paths become `src/...`, and the template reads `.github/mutants-exempts`, not `mutants.toml`.
   Without the port, the first full run reports those 33 as survivors. The ledgers record this
   crate's history: 39 survivors in the first baseline, 17 killed and 22 equivalent, and a later
   inflow of 18, 8 killed and 10 equivalent (`notes/mutation-testing/baseline-ledger.md` line 22,
   `notes/mutation-testing/inflow-2026-10-12-triage.md` line 31; the inflow columns were not re-derived).
   The file lists 644 mutants today. Fix: port each exemption with its reason into the template's
   format and run one full `mutation.yml` inside the window. Expect the `pat::VALUE` constant-level
   `|` (`src/x86_64.rs` line 34) to survive, as nife's note at `mutants.toml` lines 385 to 387 says it does, and
   exempt it with that reason. Effort S to M. Two-way.

10. Address arithmetic that can overflow. `map_range` computes `va + i * PAGE_SIZE` and `pa + i
* PAGE_SIZE` (`src/mapper.rs` line 285), and `map_span` computes `va + done` and `pa + done`
(`src/mapper.rs` lines 269 to 270). `PageFormat::index` computes `9 * (Self::LEVELS - 1 - level)`
(`src/page_format.rs` line 104). With overflow checks on, which the workspace's release profile sets, a
`count` or `len` that runs past `u64::MAX` panics instead of returning a `MapError`. Without them it
wraps into the low half and maps a page nobody asked for. The kernel's callers derive counts from
link-time symbols and page-aligned user requests, so this is inferred to be unreachable today (not
proven). `domain.rs` already refuses the same shape with `MapError::BadRegion`
(`src/domain.rs` lines 50 to 52) and says in its comment that it avoids `map_range`'s unchecked arithmetic.
Fix: `checked_add` and `checked_mul`, returning `MapError::BadRegion`, in both functions, with a
test at `u64::MAX - PAGE_SIZE`. A `const` assertion or a doc line that `index` is defined for `level
< LEVELS`. Effort S. Two-way.

11. There is no README, and the examples need twenty hidden lines. The crate root's docs
    (`src/lib.rs` lines 1 to 11) are four sentences and a pointer into nife. The two `Mapper` doctests
    (`src/mapper.rs` lines 107 to 137, `src/mapper.rs` lines 298 to 325) hide the table allocation behind `# ` lines,
    so the rendered example omits the one thing a user has to supply. The scaffold's
    `README-crate.md` requires `## EXAMPLES` and `## BUGS`, the build and prove commands, the Kani
    install line at the template's version, measured run time and memory, and one real falsification
    example. Fix: write it. Include a visible `Mapper` example over a `Vec<Box<PageTable>>` and a
    table of the six formats (levels, split bit, block sizes, what `leaf_flags` can report). Add the
    "what is proven" list in a user's words. State the limits: no permission change, no block split,
    no table reclamation, the 4 KiB granule, and the `SPLIT_SHIFT` fiction on the IOMMU formats.
    Effort M. Two-way.
