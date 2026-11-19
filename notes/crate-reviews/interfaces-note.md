# Review package: `interfaces_note`

*Name: provisional. The `interfaces_note` package of [crate-reviews.md](../crate-reviews.md), the
review that [milestone 1096 (what an outside maintainer would fix before it leaves)](../../design/roadmap/1096-interfaces-note-outside-review.md)
records item by item.*

## In one paragraph

An ELF note, type `INTERFACES` (4), that lists the interface versions a program was built against:
a version word, a count, then records of a `u64` interface id and a `u32` version in strictly
ascending id order. The crate writes the note at compile time (`carry_interfaces!`), decodes it from
untrusted bytes without a panic (`decode_interfaces`) and checks a declared version against the
served one (`check_served`), where an absent note or an absent entry passes. It is `no_std`, with no
allocator, no `unsafe` and no dependency, and three Kani harnesses prove the decoder and the
encoder against each other.

## Identity

| item | value |
|---|---|
| crate | `interfaces_note`, ratified as the crate and repository name |
| note type | `INTERFACES` = 4 |
| interface id | 21, with `INTERFACES_VERSION` 1 |
| owner string and section | kept as wire literals, each line marked `// wire literal` |
| size | one `src/lib.rs` of about 630 lines: library, Kani harnesses and tests |
| consumers in nife | the programs of `components` (through `carry_interfaces!`), `system_initializer` and `capability_manifest_note` (its tests) |

## Public API

| group | items |
|---|---|
| constants | `INTERFACES`, `INTERFACE_ID`, `INTERFACES_VERSION`, `MAXIMUM_INTERFACES`, `INTERFACE_RECORD_BYTES` |
| types | `Error`, `DeclaredInterface`, `Interfaces`, `Refusal`, `InterfacesNote` |
| sizes | `descriptor_bytes(count)`, `note_bytes(count)`, both byte counts |
| write | `encode_interfaces`, `InterfacesNote::of`, `carry_interfaces!` |
| read | `decode_interfaces`, `Interfaces::{len, is_empty, iter, version_of, check_served}` |

## Names

Twenty public names. The architect ratified all twenty with four renames (milestone 1096, decision
2), and this lane built the four with no alias of the old spellings:

| was | is |
|---|---|
| `INTERFACE_RECORD` | `INTERFACE_RECORD_BYTES` |
| `interfaces_len` | `descriptor_bytes` |
| `interfaces_note_len` | `note_bytes` |
| `Declared` | `DeclaredInterface` |

Every changed name carries a provisional `Name:` block, and the `Name:` blocks of the sixteen names
that keep their spelling are unchanged; ratifying them is the architect's step. The static that
`carry_interfaces!` emits is now `CARRIED_INTERFACES_NOTE`, provisional (milestone 1096, BUGS), so
that the identifier no longer spells the system's name.

## Proofs and falsification patches

Three harnesses, each with one replayable patch under `falsifications/`:
`a_decoded_interface_list_encodes_back_to_its_bytes`, `an_encoded_interface_list_decodes_back_to_itself`
and `decode_interfaces_never_panics`. The patches follow the renames.

## What the move still carries

Items 1 to 5, 11 and 15 of the milestone are the move's own work: the manifest, the README and
license files, the citations and passive voice, the fuzz opt-in and the mutation exemptions.

## Proposed items

None.

## Open questions

None. The go to create `nifeos/interfaces_note`, with no tag and no publish, is the architect's.

This package clears nothing. Ready for review.
