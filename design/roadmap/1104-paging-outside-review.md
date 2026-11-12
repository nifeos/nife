---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1104. `paging`: what an outside maintainer would fix before the move

Raised by a read-only outside review of `crates/paging`, as a stranger meeting it for the first
time would. It changes nothing; it lists what the next lanes change, graded P0 to P2, and the two
one-way doors that need the architect. A recording lane assigns the number.

No review package exists for this crate in `notes/crate-reviews/`, so there is no settled baseline
to inherit. The one ruling the crate itself records is its name: the crate root's `Name:` block
states that `paging` is ratified as a standard term that is already right. This block reopens that
name for one reason only, an external fact and not a later rule: the name is taken on crates.io
(Decision 1). Every other name below is provisional, and the rulings this tree has since made
(`design/naming.md`: spelled-out words, units spelled out, standard-library spellings, kept foreign
names carry an alias) are applied as written.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its helpers (`standalone.py`,
`spelled_out.py`, `unit_symbols.py`, `falsify.py`), cargo-mutants, Kani, cargo-llvm-cov. No new
tool.

## Goal

A stranger who finds this crate in its own repository can tell in one screen what it is for. The
stranger can build it for the three bare-metal targets and prove it from the README alone. The
stranger meets no name that needs a decoder and follows no link into another tree. Every safety
claim sits where the `unsafe` lives. The repository's template gates are green for the crate's own
reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1104-paging-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1104-paging-outside-review/prior-art.md).

## The items

Each item: what is wrong, where, why it matters, the fix, the effort (S under a day, M a few
days, L more) and the door (one-way: a published name, wire format, identity, layout or dependency;
two-way: everything else).

### P0

1. The manifest cannot stand alone. `crates/paging/Cargo.toml` lines 1 to 9 carries `publish = false`,
`license.workspace = true` and `lints.workspace = true`, and no `rust-version`, `description`,
`repository`, `readme`, `keywords` or `categories`. There is no README and no license file in the
crate directory. The template's `standalone.py` lists each of these as a finding and `check /
standalone` is red on day one, and `cargo publish` would refuse. Fix: inline `license = "MIT OR
Apache-2.0"`, the `[lints]` table with `unsafe_op_in_unsafe_fn`, `missing_docs` and
`unexpected_cfgs` with `cfg(kani)`, `rust-version = "1.87"` (the floor `u64::is_multiple_of` sets;
edition 2024 alone would allow 1.85), the metadata fields, the two license files and a README (item
11). Drop `publish = false` in the release that publishes. Effort S. Two-way, except that the
published `rust-version` floor is a promise users rely on, so set it from an MSRV build rather than
from this reading.

2. Another crate already holds the name on crates.io. `paging` resolves to an unrelated crate
   ("Paginator", 0.1.0, 2,774 downloads all-time, 28 recent, last updated three years before this
   review). No release workflow can publish under this name, and the repository `nifeos/paging`
   would carry a name its crate cannot have. One-way. Decision 1 below.

3. Every falsification record points into nife. All 38 harness records read `` Falsification:
   replayable `crates/paging/falsifications/<module>.<harness>.patch` `` (for example
   `src/page_size.rs` via `src/verification.rs` line 24, `src/domain.rs` line 121, `src/aarch64.rs` line 328), and
   every patch diffs `a/crates/paging/src/...`. The template's `falsify.py --check` requires
   `falsifications/<module>.<harness>.patch` and applies with `git apply -p1` from the crate root,
   so every record fails the form check and no patch would apply in the replay. The proofs are green
   and the patches are real, but the gate that proves the patches still bite is red. The four
   `nth_allocation.*.patch` files falsify host tests, which the template does not replay at all.
   nife's own lint may (unverified). Fix: rewrite the 38 record lines, strip `crates/paging/` from
   the 42 patch paths, and run `falsify.py` without `--check` once. Then either add a `cargo test`
   replay step for the four host-test patches or record in BUGS that a person replays them by hand.
   Effort S. Two-way.

### P1

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
0, refuses 84 definitions. Public among them: `is_user_page_va` (`src/page_format.rs` line 43),
`PageFormat::entry_pa` (`src/page_format.rs` line 66), the `va` and `pa` parameters of every `PageFormat`
method and every `Mapper` method (`src/mapper.rs` lines 138 to 451), `PageSize::largest_fitting`
(`src/page_size.rs` line 53), `DmaRegion` (`src/domain.rs` line 12), `pat::MSR` (`src/x86_64.rs` line 23) and
`AMD_VI_PR` (`src/x86_64.rs` line 331, refused as `pr`; it is "present"). The rule (`design/naming.md`,
"Acronyms and abbreviations") lists `va`, `pa`, `dma` and `msr` by name, and says an integer
constant is never kept. Not caught by the helper but the same rule by judgment: `phys_to_ptr` and
`alloc_page_frame` as parameters (`src/mapper.rs` lines 66 to 67, `src/domain.rs` lines 81 to 82), `attrs`
(`src/aarch64.rs` line 111, `src/sv39.rs` line 55, `src/x86_64.rs` line 107), and `kernel_rodata` and `user_rodata`
(`src/flags.rs` line 84, `src/flags.rs` line 125), where "read-only data" is the spoken phrase. Fix: one
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

8. The proofs say "over every constructor" and cover eight of ten. Every `all` array in the
format harnesses (`src/aarch64.rs` lines 402 to 411, `src/aarch64.rs` lines 433 to 442, `src/aarch64.rs` lines 481 to 490,
`src/sv39.rs` lines 468 to 477, `src/sv39.rs` lines 500 to 509, `src/sv39.rs` lines 544 to 553, `src/x86_64.rs` lines 1000 to 1009,
`src/x86_64.rs` lines 1037 to 1046, `src/x86_64.rs` lines 1072 to 1081, `src/x86_64.rs` lines 1133 to 1142) and
`flag_tests::ALL` (`src/flag_tests.rs` lines 3 to 12) omit `Flags::write_combining` and
`Flags::user_write_combining`. On `Ia32e` those two are the only flags that take the PWT-alone path
(`src/x86_64.rs` lines 118 to 123), so the W^X, round-trip and address-field proofs never see that branch.
The host tests do (`src/eighth_pass_attacks.rs` lines 56 to 67, `src/x86_64.rs` lines 614 to 635), and
`src/flags.rs` lines 41 to 47 already records half of this gap in its BUGS. Fix: a `Flags::ALL` constant of
ten, with one test that its length equals the number of public constructors, used by every harness
and test. The harness docs then say "every constructor" truthfully. Effort S. Two-way.

9. The mutation exemptions and their reasons live in nife's `.cargo/mutants.toml`. Lines 328-388
   there hold 33 regexes of the form `^crates/paging/src/<file>\.rs:...`, each justified by a
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

### P2: later

The P2 items are in [the appendix](1104-paging-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. Each is one decision, with the background first and a recommendation.

### Decision 1. The crate's published name

Background. A crate on crates.io has one name forever: the registry never reassigns a name, and a
transfer needs the current owner's cooperation. This crate's root records `paging` as a ratified
name, chosen as a standard term a reader already knows. The crates.io API shows that an unrelated
crate owns `paging`. It is a paginator for lists, version 0.1.0, with 2,774 downloads all-time and
28 in the last 90 days, last updated three years before this review. Nothing in this tree can
publish under it, and a repository named `nifeos/paging` would carry a name its crate cannot have.
The ruling is not reopened by a later rule. A fact outside the tree blocks it.

Recommendation. Name the crate `page_tables`. It is free on crates.io (verified), it is a noun
for what the crate is (the thing it builds, in every format), one word will not do because `page`
alone names something else, and it follows the crate rule's `snake_case`. Also free and
verified: `page_table_formats`, `portable_page_tables`, `page_table_mapper`,
`page_table_walk`. Taken: `page_table` (0.0.6, 146k downloads) and `address_translation`.

The other option is to ask the owner of `paging` to transfer it. crates.io has no squatting policy
that forces a transfer. The outcome is the owner's and the wait is open-ended. The move waits until
it lands. Not recommended as the plan. A letter can go out in parallel.

Prior art. `aarch64-paging` and `page_table_multiarch` name the thing (the tables) or the
activity with the architecture; neither uses the bare word. Linux calls the per-architecture
layer `pgtable`, the BSDs `pmap`.

Concrete example. The kernel's `use paging::{Flags, Half, MapError, Mapper};`
(`kernel/src/user.rs`) becomes `use page_tables::{Flags, Half, MapError, Mapper};`, and the
repository is `nifeos/page_tables`.

### Decision 2. The spelling of the leaf sizes

Background. `PageSize` has three variants, `Size4KiB`, `Size2MiB` and `Size1GiB`. The units rule
says a name holding a quantity spells its unit out and that the binary prefixes are symbols, so a
name says `kibibytes`, `mebibytes` or `gibibytes`. The variants' own doc says the `x86_64` crate
spells the same three sizes this way, which is true and is the one million downloads a month a
Rust kernel author has already seen. The template's unit check does not catch the variants,
because it splits a name at digits and never sees `kib` whole. Once published, the variant names
are a one-way door: renaming them is a breaking change for every user.

Recommendation. Follow the rule: `PageSize::FourKibibytes`, `PageSize::TwoMebibytes`,
`PageSize::OneGibibyte`. The rule exists because uppercase erases the case a symbol depends on, and
a variant is camel case, so the `K`, `M` and `G` survive here and the argument is weaker than for a
constant. The recommendation still follows the rule because the tree has one spelling for units and
an exception for one enum is a second rule to remember. The restrictive choice is also the one that
allows loosening later (an alias is compatible; a rename is not). The alternative, keeping the
`x86_64` crate's spelling as a kept foreign name with an alias, is defensible under "a
specification's term of art that a reader searches for", and `Size2MiB` is what that reader types.

Prior art. `x86_64` 0.15.5: `Size4KiB`, `Size2MiB`, `Size1GiB`. `page_table_multiarch` 0.6.2:
`PageSize::Size4K` and larger (the landing page shows `Size4K`; the rest unverified). Linux:
`PAGE_SIZE`, `PMD_SIZE`, `PUD_SIZE`, named for the level and not the size. aarch64 documentation:
"page", "block" and the granule.

Concrete example. `m.map_span(va, pa, len, flags, PageSize::Size1GiB)` becomes
`m.map_span(virtual_address, physical_address, len, flags, PageSize::OneGibibyte)`.

## Done when

The checks are in [the appendix](1104-paging-outside-review/done-when.md).

## BUGS

- The template's `unit_symbols.py` does not see a binary prefix inside a camel-case variant
  (`Size4KiB` judged clean). Whatever Decision 2 rules, the check and the rule disagree today, and
  the fix is the template's, not this crate's.
- The template replays Kani falsifications only. The four host-test patches under
  `falsifications/nth_allocation.*` would have no gate in the new repository unless someone adds
  one. Whether nife's lint replays them today was not checked.
- Item 10's overflow comes from reasoning over the code and the kernel's call sites. No failing
  input demonstrates it. The fix is cheap enough to land without one.
- The Kani run time and peak memory the README must state were not measured here.
- The `aarch64-paging` and `page_table_multiarch` rows of the prior-art table come from their
  docs.rs landing pages only. Their error variants and TLB handling carry an unverified mark.
- Nobody ran a mutation census; the figures quoted are the ledgers'.

## Index row

An outside review of `paging` before it leaves for its own repository. It finds three P0 items: a
manifest that cannot stand alone, a crate name that another crate already holds on crates.io, and
falsification records that point into nife. It finds eight P1 items: documentation written for an
insider, 84 spelled-out-rule findings, kept names with no alias, and an incomplete `unsafe` contract
on `Mapper::new`. The rest are proofs that cover eight of ten constructors, mutation exemptions in
the wrong file, overflowing range arithmetic, and no README. It finds eight P2 items. Two one-way
doors go to the architect: the crate's published name and the spelling of the leaf sizes.
