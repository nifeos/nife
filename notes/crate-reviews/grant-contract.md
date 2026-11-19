# Review package: `grant_contract`

*Name: provisional, minted by lane milestone/1094-grant-plan-rulings-built. The `grant_contract`
package of [crate-reviews.md](../crate-reviews.md), written with
`script/crate-review-packet grant_contract`. It is the second of two for the split in milestone 1094
(`grant_plan`: what an outside maintainer would fix before it leaves); [grant-plan.md](grant-plan.md) is the
first.*

## In one paragraph

`grant_contract` is what a program links to say what it may ask for, and what the planner reads
to decide. It holds the manifest types: `Manifest`, the file and directory declarations, the rights
bits and the authorities a program asks for. It also holds the slot constants a program finds its
holdings in, and the verb narrowing that turns a manifest into the subset a verb needs. It is `no_std` with no
allocator and no `unsafe`, and depends on `program_argument_contract`. `grant_plan` and nine other
packages in nife link it.

## Identity

| item | value |
|---|---|
| crate | `grant_contract`; the maintainer ruled the name as built on 2026-11-19 UTC, and `script/names` still reports it provisional until the architect ratifies it |
| interface | `INTERFACE_ID` 17, `VERSION` 1 |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; none |
| license | inherited from the nife workspace; no license files in the crate |
| `publish` | `false` |
| size | four files under `src/`: `lib.rs` 55 lines, `manifest.rs` 727, `manifest/tests.rs` 99, `verbs.rs` 407 |
| consumers in nife | `components`, `capability_manifest_note`, `grant_plan`, `program_asks` (dev), `program_table`, `swish`, `system_initializer`, `timetable`, `fixtures`, `fuzz`, `jig`, `std_exerciser`, `system_tests` (dev), `xtask` |

## Public API

| group | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| slots | `CATALOG_SLOT`, `DIAGNOSTICS_SLOT`, `DOMAIN_SLOT`, `ENTROPY_SLOT`, `INDEX_SOURCES_SLOT`, `INSTALLER_REPLY_SLOT`, `INSTALLER_SLOT` and the rest of the slot list |
| declarations | `FileGrantDeclaration`, `DirectoryGrantDeclaration` (`Forbidden`, `Required`) |
| manifest | `Manifest`, with `is_well_formed`; `RIGHTS_BITS_DO_NOT_OVERLAP`, the compile-time check that the rights bits share no bit |
| verbs | `verbs::ManifestAuthorities` (a `u16` set with `union`, `contains`, `bits`, `from_bits`), `VerbGrant`, `narrow_by_verb`, `verb_key`, `delegated_within`, `verb_of_page`, `MAXIMUM_VERB_BYTES` |

The packet lists every item with its signature.

## Proofs and falsification patches

None. The crate has no Kani harness and no falsification patch. Tests cover the manifest's
well-formedness and the verb narrowing; `grant_plan`'s tests exercise both again through the
planner. The wire identity is the `INTERFACE_ID` and `VERSION` pair above, which the elf note of
milestone 597 (a program carries its manifest in an ELF note) reads.

## Names

The crate has 46 names with a block: 22 ratified and 24 provisional. The provisional ones were
minted by this lane under the two rulings of milestone 1094.

| name | before | reason |
|---|---|---|
| `verbs::MAXIMUM_VERB_BYTES` | `MAXIMUM_VERB` | unit |
| `RIGHTS_BITS_DO_NOT_OVERLAP` | `THE_RIGHTS_BITS_SHARE_NO_BIT` | says the property, not the phrase |
| private field `length_bytes` | `len` | unit |
| the block of each item in `manifest.rs` and `verbs.rs` that cites the new spellings | | provisional until the architect ratifies |

The lane added no alias of an old nife spelling.

## Naming rules

Checked with `helpers/name_checks.py`: no finding.

## License

As for `grant_plan`: inherited today, written into the crate's own manifest by the move, with both
license files. The crate adapts no source.

## What the release still needs

| item | owner |
|---|---|
| A README, with an example a doctest runs. | the move lane |
| `rust-version`, `repository`, and the license files. | the move lane |
| A review of 51 milestone citations, 29 section citations, 17 nife paths and 4 provenance lines in shipped text. | the move lane |
| An entry in `crates/externalization-list.txt`, which names `grant_plan` and not this crate. | a lane, after clearance |
| The tag `v0.1.0`. | calef |

## Proposed items

1. P1-5: a Kani harness for the manifest's well-formedness and for the verb narrowing, with a
   falsification patch for each.
2. P1-9 and P1-9b: documentation for a stranger, then `missing_docs` denied at the crate root.
3. P1-13: an undeclared widening option in `Manifest::is_well_formed`.
4. P1-14: a mutation run on the crate.

## Open questions

None for this package. The clearance covers this crate with `grant_plan`.

## Result of `script/crate-standalone`

The run fails with 17 findings. They are the crate name and 55 item names not yet ratified
(`python3 helpers/name_release.py --draft grant_contract` prints the table), the missing README,
license files and `rust-version`, and the citations listed above. The two path dependencies are
sibling crates in nife. The build and proofs are skipped because the copy needs them.

## Recommendation

Ready for review.
