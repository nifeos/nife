---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: 956, 597
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# The manifest note declares the interface versions: a separate note type

Raised 2026-10-31 (UTC) by the lane `milestone/956-checkable-interface-versions`, at steps 3 and 4 of milestone 956 (the linked interfaces carry checkable versions). The lane stopped here because the plan names a wire format that the tree has already spent, and a wire format is an architect's call. Every name below is provisional.

Reuse: the proposal reuses the note machinery of milestone 597 (a program carries its manifest in an ELF note): the one-encoding decode, the Kani property, and the separate note types that `SUBTREE_GRANTS` and `VERBS` already use. No new dependency.

## What the tree does today

The plan says the manifest note gains "descriptor version 3": version 1's 56 bytes unchanged, then a counted (interface, version) list. Descriptor version 3 exists. `SERVICED_VERSION = 3` (milestone 911, scripted installs grant only what the owner's approval file allows) is version 2's layout plus a service byte. The next free descriptor version is 4.

Two further facts bear on the plan:

- A descriptor version is a whole layout. Versions 2 and 3 carry zones, and version 3 carries the service ask. A version 4 that adds a list must say how it composes with both, or an encoder has two ways to write one manifest, which breaks the one-encoding property.
- Step 3 compares "the manifest note's declared ABI version" against `ABI_VERSION`. No version of the note carries one, so step 3 cannot run before the declaration exists. The plan's claim that steps 1 through 3 stand alone is wrong for step 3; it depends on step 4.

The tree already has the other shape. A note type is per owner, and `capability_manifest_note` carries three: `MANIFEST` 1, `SUBTREE_GRANTS` 2 and `VERBS` 3, each with its own version constant, its own encoder, decoder and length. The grants and the verbs joined the manifest as separate notes, not as descriptor versions.

## The decision

Where does the (interface, version) list live?

**Recommended: a fourth note type, provisionally `INTERFACES = 4`, with `INTERFACES_VERSION = 1`.** Its descriptor is a count, then count records of `INTERFACE_ID: u64` and `version: u32`, little-endian, sorted by id, no duplicates, the decode refusing anything else (one encoding). The ABI is one entry of the list, so step 3 reads the entry whose id is `abi`'s. A program with no such note declares nothing, and the progenitor's policy for that case (accept, as every program today, or refuse once packages are stamped) is a second small ruling the lane asks for with this one.

Options:

1. A fourth note type, as recommended. Versions 1 to 3 of the descriptor stay untouched, the zone and service layouts do not multiply, and the stamp is one more note a build adds. A program the stamp does not reach keeps working. Cost: a program carries one more ELF note, and a reader walks one more.
2. Descriptor version 4: version 3's layout, then the counted list. Matches the plan's text and the ruling's wording ("descriptor version 3"). Cost: a manifest that asks for no service and no zones still pays the longer layout to say its interfaces, and every future descriptor field must choose again how to compose. Version 2 and 3 manifests could not declare interfaces at all.
3. Byte 55 as a coarse ABI revision. Refused by calef's ruling of 2026-10-29 (UTC, PR #2852, question 3); listed so the record is complete.

Prior art. Fuchsia stamps a coarse API level into a package (read from RFC-0239 in the shape proposal, not re-read here). ELF's own `.note.gnu.property` carries a list of typed properties in one note, which is the shape of option 1 with a note per concern. Mark both as recalled, not verified in this lane.

Premise check: the ruling called its choice "descriptor version 3" before milestone 911 spent that number. The ruling's substance, a counted (interface, version) list stamped mechanically and versions 1 and 2 untouched, holds under either option; only the container differs.

Reversibility: nothing outside this tree reads the list yet, because nothing writes it, so either choice is cheap today and expensive after the first package stamps it.

Would we choose option 1 if both cost the same? Yes. Two notes cost a reader a second walk; a composed descriptor costs every future field a composition rule.

## What this unblocks

On a yes: step 4 lands the note type, the encode and decode, and the Kani property, and the `INTERFACE_ID` constants. Step 3 then lands the spawn-time comparison. Steps 5, 6 and 8 wait on step 2's bind check, whose open point is next.

## A second, smaller point: where step 2's check lives

Step 2 puts `member(own, served)` and `Mismatch` "in each message contract". The first cut links no message contract (`abi`, `address_space_map`, `glob`, `machine_statistics_contract` and `block_roster` are a syscall surface, layout constants, a library and two shared pages), so step 2 serves the contracts the plan lists later, and `filesystem_contract` is out of scope while RedoxFS is replaced. Two homes are possible. One copy per contract crate repeats a ten-line function a dozen times, against rule 7's one definition. One small shared crate (provisionally `interface_version`, a new name) holds the function and the error once, and each contract depends on it. The lane recommends the shared crate, and starts step 2 on a yes to the name, because a new crate name is the architect's.
