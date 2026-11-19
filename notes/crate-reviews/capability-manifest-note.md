# Review package: `capability_manifest_note`

*Name: provisional. The `capability_manifest_note` package of the index
[crate-reviews.md](../crate-reviews.md), in its own file as the other packages are. It follows the
shape of [universal-serial-bus.md](universal-serial-bus.md).*

## The package

Name and version. `capability_manifest_note` `0.1.0`, edition 2024, `rust-version` 1.88, license
`MIT OR Apache-2.0`, `publish = false`. The crate name was ratified on 2026-10-18 (UTC). The
repository is `nifeos/capability_manifest_note`, which does not exist yet. The crate is on
`crates/externalization-list.txt`, in step 6 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
after `machine_statistics_contract`, `domain_name_resolution_contract` and `grant_plan`. The review
that this package answers is
[milestone 1082 (the outside review of `capability_manifest_note`)](../../design/roadmap/1082-capability-manifest-note-outside-review.md).

API. The ELF note a program carries to ask for authority, its compile-time encoder and its strict
decoder. One claim runs through it, in both directions: one manifest has exactly one encoding, so
what the compiler accepts into a program's bytes the reader accepts, and what the reader refuses
the compiler refuses. It is `no_std`, allocates nothing and has no `unsafe` function; the five
`unsafe` sites are the `link_section` attributes of the two macros.

| area | what it holds |
|---|---|
| identity | `OWNER` (the owner string), the three note types `MANIFEST`, `SUBTREE_GRANTS`, `VERBS`, `NOTE_SET_VERSION`, `INTERFACE_ID` |
| manifest descriptor | `BARE_VERSION`, `ZONED_VERSION`, `SERVICED_VERSION`, `DESCRIPTOR_BYTES`, `NOTE_BYTES`, `MAXIMUM_ZONES`, `MAXIMUM_ZONE_BYTES`, `encode`, `encode_zoned`, `encode_serviced`, `decode`, `asks_service`, `zones`, `Zones`, `ZonesIter`, `Note`, `ZonedNote`, `ServicedNote`, the four `*_bytes` functions |
| subtree grants note | `SUBTREE_GRANTS_VERSION`, `SUBTREE_GRANTS_BYTES`, `SUBTREE_GRANTS_NOTE_BYTES`, `Scope`, `SubtreeGrantsNote`, `decode_subtree_grants` |
| verb table note | `VERBS_VERSION`, `MAXIMUM_VERBS`, `VERB_RECORD_BYTES`, `verbs_descriptor_bytes`, `encode_verbs`, `decode_verbs`, `Verbs`, `VerbsIter` |
| macros | `carry!` (emits the static `MANIFEST_NOTE`) and `carry_subtree_grants!` (emits `SUBTREE_GRANTS_NOTE`) |
| errors | `Error` (`NoVersion`, `UnknownVersion`, `WrongLength`, `BadField`), with `Display` and `core::error::Error` |

The packet (`script/crate-review-packet capability_manifest_note`) lists the public signatures. Nine
public signatures name a `grant_contract` type (`Manifest`, `ManifestAuthorities`, `VerbGrant`).
That crate is the organization's own and is approved in advance; no nife type and no nife error
enum appears in a signature.

What a stranger may find surprising:

- The owner string is `nife`, and so is the section name `.note.nife.manifest`. They are wire
  literals: every program built so far carries those bytes, and the README says once that the owner
  string is the name of the operating system that defines the note. The lines that hold one end with
  `// wire literal`.
- A reader refuses a descriptor version it does not know and never skips it.
- Version 2 and version 3 descriptors both exist so that notes already built keep their bytes.
- The statics the macros emit are `MANIFEST_NOTE` and `SUBTREE_GRANTS_NOTE`, so one program may
  carry one `carry!` and one `carry_subtree_grants!` and no more of either.

Proofs. Seven Kani harnesses, each with a falsification patch in `falsifications/`.

| harness | bound | what it proves |
|---|---|---|
| `a_decoded_manifest_encodes_back_to_its_bytes` | every version 1 descriptor | whatever `decode` accepts, `encode` spells back byte for byte |
| `an_encoded_manifest_decodes_to_itself` | every manifest a descriptor can spell | whatever `encode` builds, `decode` reads back as the same manifest |
| `an_inverted_memory_grant_does_not_encode` | every inverted memory range | `encode` refuses a minimum above a maximum |
| `decode_never_panics` | up to one byte past version 1's length | `decode` is total |
| `decode_never_panics_through_a_serviced_descriptor` | six zone layouts, every length from 56 to 62 bytes | `decode`, `zones` and `asks_service` are total through a version 2 or 3 descriptor |
| `a_decoded_verb_table_finds_each_verb` | tables of one or two verbs of one or two bytes | `grants_for` finds every verb `decode_verbs` accepts, with its grants |
| `decode_subtree_grants_never_panics` | up to one byte past 8 bytes | `decode_subtree_grants` is total |

Outside the bounds: a zone walk with a count, a length byte or a length chosen freely (each variant
ran out of memory under a 12 GiB cap, and the harness documentation says so), a verb table of more
than two verbs, and a zone of more than the bytes the layouts use. The fuzz target
`capability_manifest_note_decode` and its 14 seeds reach those; the target is still in nife's fuzz
workspace and the move gives the crate its own (item 8 of the milestone). The weekly fuzz opt-in
stays off, and the project's own host fuzzes the crate.

Dependencies. Two, both the organization's own and approved in advance: `grant_contract` (the type a
note carries) and `domain_name_resolution_contract` (the zone limits and spelling). Dev-dependencies:
`interfaces_note`, which holds the two notes to one header, and `program_table`, whose catalog the
round-trip test encodes. Each is a path dependency with a `version`, and the move converts each to
`git` and `rev`.

Dependents in nife: `components`, `program_asks`, `system_initializer`, `fixtures`, `fuzz`, `jig`,
`redoxfs_server`, `std_exerciser`, `xtask`, and `program_table` (dev). None names the macros'
statics.

Naming rules. `python3 helpers/name_checks.py capability_manifest_note`: 0 unratified names failing
a naming rule (spelled-out words, register and size names, units, question shape, British spelling,
contract names). `script/names capability_manifest_note` lists every `Name:` block.

Provenance. The crate's own design: no text from a source with a notice to carry. The prior-art
rows of the milestone's appendix marked unverified are from memory, and the README cites only the
owners `FreeBSD`, `GNU` and `FDO`.

References to nife in shipped text. `python3 helpers/lint_externalized_crates.py` counts 2 hits for
the crate, below its ceiling, which this pull request lowers from 5 to 2, and lists 5 marked wire
literals in `src/lib.rs`, 3 in `src/tests.rs` and none elsewhere.

Result of `script/crate-standalone`. Seven failures, all expected before the move. The check copies the
crate alone, and its four path dependencies (`grant_contract`, `domain_name_resolution_contract`,
`interfaces_note`, `program_table`) have not left the tree yet, so the manifest check fails once per
dependency, the build is skipped and the proofs are skipped because the copy does not build. The seventh
failure is the names check: 56 names are not yet ratified, which is the list in the table above for this
review. The harness and falsification steps could not run. The move lane reruns the check once the sibling
crates are available.

Result of `script/crate-move --dry-run`. Three findings, each for the move lane; template `v8`, the 15 tracked files copied, the `Name:` blocks
removed from 3 files and the 7 falsification patches regenerated for `-p1` and applying with no offset. The
`build` step fails at `cargo generate-lockfile` because the two path dependencies,
`grant_contract` and `domain_name_resolution_contract`, are siblings that have not left yet; it clears when each is
a `git` dependency with a `version`. The `prose caps` step refuses the README at 1382 words, and
the `passive voice` step finds 8 passives in 1520 words across 12 lines (the README is first). The
`mentions nife` step reports none, and the nife-side steps are the workspace member and the
mutation baseline row. The fuzz target links other crates of the tree, so it moves with the crate's own
`fuzz/` workspace (item 8).

Open items before the move. These are in the milestone and belong to the move lane:

- P0-2: the stranger run, recorded in `docs/stranger-test.md` of the new repository.
- P1-8: the crate's own `fuzz/` workspace with the target and its 14 seeds, the seed test reading
  `CARGO_MANIFEST_DIR`, and the target dropped from nife's workspace.
- The README under the template's prose caps and passive-voice ratchet (1382 words; 8 passives in 1520).
- The consumers' lines converted from path dependencies to `git` and `rev` with `version = "0.1.0"`.

Recommendation. Ready for review. The names below are the only ones the architect has ratified
beyond those the crate already carried; every other name is provisional. The clearance is the go for
creating `nifeos/capability_manifest_note`; no tag and no publication is asked.

## Names

Every public name of the crate and its state. The architect ratifies; the state is as the crate's
`Name:` blocks say.

### Ratified

| name | ruling |
|---|---|
| `capability_manifest_note` | 2026-10-18 |
| `OWNER`, `MANIFEST` | 2026-09-26 |
| `MAXIMUM_VERBS` | 2026-10-27 |
| `asks_service` | 2026-11-19, as built (a function that answers yes or no takes a third-person verb phrase) |
| `BARE_VERSION` | 2026-11-19, as built (the layout with nothing added) |

### Provisional

| name | what it is |
|---|---|
| `MANIFEST_NOTE`, `SUBTREE_GRANTS_NOTE` | the statics `carry!` and `carry_subtree_grants!` emit, renamed so that no identifier spells the project word; a two-way door, ruled provisionally by the maintainer |
| `DESCRIPTOR_BYTES`, `NOTE_BYTES`, `SUBTREE_GRANTS_BYTES`, `SUBTREE_GRANTS_NOTE_BYTES`, `VERB_RECORD_BYTES`, `MAXIMUM_ZONE_BYTES` | lengths with their unit; ruled to stand as built on 2026-11-19 in the milestone's record, and the `Name:` blocks stay provisional |
| `zoned_descriptor_bytes`, `serviced_descriptor_bytes`, `zoned_note_bytes`, `serviced_note_bytes`, `verbs_descriptor_bytes` | the functions of the same rule |
| `NOTE_SET_VERSION`, `INTERFACE_ID` | the interface identity of the note set; `INTERFACE_ID` 7 is recorded as ratified in the milestone |
| `ZONED_VERSION`, `SERVICED_VERSION`, `MAXIMUM_ZONES` | the later descriptor versions and the zone count limit |
| `Zones`, `ZonesIter`, `Verbs`, `VerbsIter` | the views and their iterators, named as std names an iterator beside its collection |
| `ZonedNote`, `ServicedNote`, `encode_zoned`, `encode_serviced`, `zones`, `has_root`, `check_serviced` | the zone and service half of the manifest note |
| `verification`, `tests` | the modules |

### No `Name:` block yet

`Error`, `Note`, `encode`, `decode`, `SUBTREE_GRANTS`, `SUBTREE_GRANTS_VERSION`, `Scope`,
`SubtreeGrantsNote`, `decode_subtree_grants`, `VERBS`, `VERBS_VERSION`, `encode_verbs`,
`decode_verbs`, `Verbs::grants_for` and the methods `of`, `declared`, `iter`, `len`, `is_empty`.
`python3 helpers/name_release.py --draft capability_manifest_note` prints the full table, and a
release names each of these before the first tag.

Ready for review.
