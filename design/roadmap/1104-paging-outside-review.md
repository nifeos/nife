---
status: BUILT
built: 2026-11-22
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1104. `paging`: what an outside maintainer would fix before the move

Raised by a read-only outside review of `crates/page_tables`, as a stranger meeting it for the first
time would. It changes nothing; it lists what the next lanes change, graded P0 to P2, and the two
one-way doors that need the architect. A recording lane assigns the number.

No review package exists for this crate in `notes/crate-reviews/`, so there is no settled baseline
to inherit. The one ruling the crate itself records is its name: the crate root's `Name:` block
states the name as ratified. The architect reopened that name for one reason only, an external fact
and not a later rule: `paging` is taken on crates.io (Decision 1, ruled). Every other name below is provisional, and the rulings this tree has since made
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

## Finding status

- P0-1, the manifest stands alone: At the move. The ruling of 2026-11-20 (UTC) gives the move lane
  item 1: the inline license and lints, `rust-version`, the metadata and the license files.
- P0-2, another crate holds the name on crates.io: Done, [#3594](https://github.com/nifeos/nife/pull/3594), the rename to `page_tables`.
- P0-3, falsification records point into nife: At the move. The move commit rewrites the 38 record
  lines and the patch paths that name `crates/page_tables/`, and gives the host-test patches a
  replay.
- P1-4, documentation for a stranger: At the move. The move lane rewrites the comments that cite
  sections, milestones and nife paths.
- P1-5, the spelled-out renames: Done, [#3806](https://github.com/nifeos/nife/pull/3806). The architect ratified the result with the other
  public names in [#3857](https://github.com/nifeos/nife/pull/3857).
- P1-6, `doc(alias)` on kept specification names: Done, [#3806](https://github.com/nifeos/nife/pull/3806) and [#3857](https://github.com/nifeos/nife/pull/3857). Every kept
  public name carries an alias for its plain or specification spelling.
- P1-7, `Mapper::new`'s safety contract and `Misaligned`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P1-8, `Flags::ALL` over every constructor: Done, [#3567](https://github.com/nifeos/nife/pull/3567), eleven constructors.
- P1-9, the mutation exemptions: At the move. The 33 exemptions move from nife's
  `.cargo/mutants.toml` to the new repository's `.github/mutants-exempts`.
- P1-10, overflowing range arithmetic: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P1-11, a README: At the move.
- P2-12a, `SPLIT_SHIFT` and `BadRegion` documented: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-12b, `WritableAndExecutable` out of `MapError`: Will not fix. The architect kept the variant
  when ratifying the public names, ruled 2026-11-20 (UTC); its doc says the kernel's mapping hold
  raises it and this crate never does.
- P2-13, the `walk` assertion: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-14, the leaf-size spelling: Done, [#3806](https://github.com/nifeos/nife/pull/3806). `PageSize::FourKibibytes`, `TwoMebibytes` and
  `OneGibibyte` carry `#[doc(alias = "Size4KiB")]` and siblings.
- P2-15a, `Display` and `core::error::Error` for `MapError`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-15b, the README's list of missing operations: At the move, with item 11.
- P2-16, documentation drift: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-17, the test pool shape: Done, [#3973](https://github.com/nifeos/nife/pull/3973). `tests/mapping.rs` builds every mapper from a pool
  the mapper borrows.
- P2-18, the Sv39 device bit and the T-Head source: Done, [#3567](https://github.com/nifeos/nife/pull/3567). The README's BUGS lines
  belong to item 11.
- P2-19, Miri's pointer casts: Done, [#3973](https://github.com/nifeos/nife/pull/3973). Each test pool returns the pointer it allocated
  for an address instead of casting the integer back.
- Decision 1, the published name: Done, [#3594](https://github.com/nifeos/nife/pull/3594), ruled 2026-11-14 (UTC).
- Decision 2, the spelling of the leaf sizes: Done, [#3806](https://github.com/nifeos/nife/pull/3806), option A, ratified with the other
  names in [#3857](https://github.com/nifeos/nife/pull/3857).
- Ratifying the public names: Done, [#3857](https://github.com/nifeos/nife/pull/3857), ruled 2026-11-20 (UTC).
- Clearing the review package: Done, [#3857](https://github.com/nifeos/nife/pull/3857), ruled 2026-11-20 (UTC). The repository
  `nifeos/page_tables` is approved with `publish = false`, no tag and no publish.

## Open decisions

### The spelling of the leaf sizes

Ruled 2026-11-19 (UTC), option A, by the maintainer under the standing units rule, and ratified
by the architect with the other public names on 2026-11-20 (UTC). The reasoning follows.

Background. `PageSize` names the three sizes a single page table entry can map: 4 KiB, 2 MiB and
1 GiB. Its variants are `Size4KiB`, `Size2MiB` and `Size1GiB`, copied from the `x86_64` crate,
the spelling a Rust kernel author has already seen. The units rule says a name spells its unit out
and treats the binary prefixes as symbols, so a name says `kibibytes`, `mebibytes` or
`gibibytes`. The template's unit check does not catch the variants, because it splits a name at
digits and never sees `kib` whole. Keeping the `x86_64` spelling is an exception to the rule,
which is why this is the architect's call.

Recommendation: A. The tree has one spelling for units, and an exception for one enum is a second
rule to remember. The rule's case argument (uppercase erases `K` against `k`) is weaker for a
camel-case variant, which keeps the letters. The stricter choice also allows loosening later: an
alias added later is compatible, a rename is not.

- A. Follow the rule: `PageSize::FourKibibytes`, `PageSize::TwoMebibytes` and
  `PageSize::OneGibibyte`, with `#[doc(alias = "Size4KiB")]` and its siblings for the reader who
  searches for the `x86_64` spelling.
- B. Keep `Size4KiB`, `Size2MiB` and `Size1GiB` as a kept foreign name, a term of art a reader
  searches for, and record the exception in [design/naming.md](../naming.md).
- C. Name the level and not the size, as Linux does, for example `PageSize::Page`,
  `PageSize::Block` and `PageSize::LargeBlock`.

Door: one-way at the first publish. Renaming an enum variant breaks every user.

Prior art. `x86_64` 0.15.5 spells `Size4KiB`, `Size2MiB` and `Size1GiB`. `page_table_multiarch`
0.6.2 spells `PageSize::Size4K` (from its landing page; the larger sizes unverified). Linux names
the level and not the size: `PAGE_SIZE`, `PMD_SIZE`, `PUD_SIZE`. Arm's documentation says
"page", "block" and the granule. nife differs in spelling the unit into every name that holds a
quantity. Concrete example: `m.map_span(va, pa, len, flags, PageSize::Size1GiB)` becomes
`m.map_span(virtual_address, physical_address, len, flags, PageSize::OneGibibyte)`.

Blocks: the first publish. The rename lane of item 5 can take it.

### Ratify the public names once the rename lane lands

The renames have landed provisionally; the package lists every public name with its status and
proposal for this one pass.

Background. `page_tables` builds and walks page tables for six formats: aarch64, Sv39, the T-Head
Sv39 variant, x86-64, Intel VT-d and AMD-Vi. Its public names are what every user compiles against,
and the move strips the `Name:` blocks that mark a name provisional, so each must read ratified
first. Item 5 renames `va` and `pa` to `virtual_address` and `physical_address`, and spells
out `is_user_page_virtual_address`, `entry_physical_address`, `DirectMemoryAccessRegion` and
others; the maintainer rules those provisionally. The fixes added `Flags::ALL`. Ratification is
the architect's by the naming rule.

Recommendation: A. One lane performs item 5, item 6's aliases and the leaf-size ruling, then the
architect ratifies the result in one pass, so the names read are the names that ship.

- A. Ratify after the rename lane lands, against the renamed source.
- B. Ratify item 5's proposed names now, before the renames.
- C. Ratify with changes the architect names; the rename lane applies them.

Door: one-way at the first publish. A published public name is a breaking change to rename.

Prior art. `x86_64` spells its addresses `VirtAddr` and `PhysAddr`, and `aarch64-paging` spells
`VirtualAddress` and `PhysicalAddress` (both recalled). nife takes the spelled-out form
for every parameter as well as every type. Concrete example: `PageFormat::entry_pa(entry)` reads
`PageFormat::entry_physical_address(entry)`.

Ruled: the architect ratifies the public names as built, with the renames `TranslationLookasideBufferFlush`, `aarch64::memory_attribute_indirection` and `x86_64::page_attribute_table`, the alias `DMA`, and `MapError::WritableAndExecutable` kept. The spelled-out forms keep each old spelling as a `doc(alias)`.

Blocks: the first publish.

### Clear the review package and approve the repository

Background. A crate leaves this tree for its own `nifeos` repository only when the architect
clears its review package, and that clearance approves the repository. `page_tables` serves
every kernel that maps memory on these six formats, so it meets the bar for leaving. The
architect ruled its published name on 2026-11-14 (UTC). The package is
`notes/crate-reviews/page-tables.md`, and items 1, 3, 4, 9 and 11 are the move's work.

Recommendation: A. A package written after the rename lane shows the crate as it will ship.

- A. A lane writes the package after the rename lane; the architect's clearance creates the
  repository.
- B. Clear on this outside review alone.
- C. Keep the crate in nife.

Door: one-way. Creating a public repository and moving the crate are outward steps.

Prior art. `aarch64-paging` and `page_table_multiarch` each live in their own repository and
publish on their own cadence. nife differs in reviewing a whole crate once, before its first
publish. Concrete example: the architect cleared `generational_table` on 2026-11-08 (UTC), and its
repository, `nifeos/generational_table`, dates from that day.

Ruled: A on 2026-11-20 (UTC). The architect clears the review and approves `nifeos/page_tables` with `publish = false`, no tag and no publish. The move lane takes items 1, 3, 4, 9 and 11.

Blocks: the move.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1104-paging-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1104-paging-outside-review/prior-art.md).

## The items

Each item: what is wrong, where, why it matters, the fix, the effort (S under a day, M a few
days, L more) and the door (one-way: a published name, wire format, identity, layout or dependency;
two-way: everything else).

### P0

1. The manifest cannot stand alone. `crates/page_tables/Cargo.toml` lines 1 to 9 carries `publish = false`,
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
   replayable `crates/page_tables/falsifications/<module>.<harness>.patch` `` (for example
   `src/page_size.rs` via `src/verification.rs` line 24, `src/domain.rs` line 121, `src/aarch64.rs` line 328), and
   every patch diffs `a/crates/page_tables/src/...`. The template's `falsify.py --check` requires
   `falsifications/<module>.<harness>.patch` and applies with `git apply -p1` from the crate root,
   so every record fails the form check and no patch would apply in the replay. The proofs are green
   and the patches are real, but the gate that proves the patches still bite is red. The four
   `nth_allocation.*.patch` files falsify host tests, which the template does not replay at all.
   nife's own lint may (unverified). Fix: rewrite the 38 record lines, strip `crates/page_tables/` from
   the 42 patch paths, and run `falsify.py` without `--check` once. Then either add a `cargo test`
   replay step for the four host-test patches or record in BUGS that a person replays them by hand.
   Effort S. Two-way.

### P1

The P1 items are in [the appendix](1104-paging-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1104-paging-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. Decision 1 has its ruling; the open ones are under "Open decisions".

### Decision 1. The crate's published name

Ruled 2026-11-14 (UTC), option A: the crate is named `page_tables`, in code and package alike.
`crates/paging` is `crates/page_tables`, the package and the library are `page_tables`, and every
caller says `page_tables::`. The old name carries no `doc(alias)`. The crate root's `Name:` block is
ratified for `page_tables`. No repository exists before the review clears. The reasoning follows.

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
(`kernel/src/user.rs`) became `use page_tables::{Flags, Half, MapError, Mapper};`, and the
repository is `nifeos/page_tables`.

## Done when

The checks are in [the appendix](1104-paging-outside-review/done-when.md).

## BUGS

- The template's `unit_symbols.py` does not see a binary prefix inside a camel-case variant
  (`Size4KiB` judged clean). Whatever the leaf-size decision rules, the check and the rule disagree today, and
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

## Follow-on

- **Recorded.** The move to `nifeos/page_tables`, which takes items 1, 3, 4, 9 and 11 and the
  README lines of items 15 and 18.

## Index row

An outside review of `page_tables` before it leaves for its own repository. It finds three P0 items: a
manifest that cannot stand alone, a crate name that another crate already holds on crates.io, and
falsification records that point into nife. It finds eight P1 items: documentation written for an
insider, 84 spelled-out-rule findings, kept names with no alias, and an incomplete `unsafe` contract
on `Mapper::new`. The rest are proofs that miss constructors, mutation exemptions in
the wrong file, overflowing range arithmetic, and no README. It finds eight P2 items. The architect
ruled the crate's published name on 2026-11-14 (`page_tables`), ratified the public names and
cleared the review on 2026-11-20, and the move does what remains.
