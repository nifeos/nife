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

Ruled 2026-10-31 (UTC): option 1, a fourth note type, `INTERFACES = 4`, on the pattern of `SUBTREE_GRANTS` and `VERBS`. Milestone 956's steps 3 and 4 execute it. The progenitor's policy for a program with no such note stays accept, as today. The crate name `interface_version` (step 2) is still open.

Raised 2026-10-31 (UTC) by the lane `milestone/956-checkable-interface-versions`, at steps 3 and 4 of milestone 956 (the linked interfaces carry checkable versions). The lane stopped here. The plan names a wire format that the tree has already spent, and a wire format is an architect's call. Every name below is provisional.

Reuse: the proposal reuses the note machinery of milestone 597 (a program carries its manifest in an ELF note): the one-encoding decode, the Kani property, and the separate note types that `SUBTREE_GRANTS` and `VERBS` already use. No new dependency.

## What the tree does today

The plan says the manifest note gains "descriptor version 3": version 1's 56 bytes unchanged, then a counted (interface, version) list. Descriptor version 3 exists. `SERVICED_VERSION = 3` (milestone 911 (scripted installs grant only what the owner's approval file allows)) is version 2's layout plus a service byte. The next free descriptor version is 4.

Two further facts bear on the plan:

- A descriptor version is a whole layout. Versions 2 and 3 carry zones, and version 3 carries the service ask. A version 4 that adds a list must say how it composes with both. Without that, an encoder has two ways to write one manifest, and the one-encoding property breaks.
- Step 3 compares "the manifest note's declared ABI version" against `ABI_VERSION`. No version of the note carries one, so step 3 cannot run before the declaration exists. The plan says steps 1 through 3 stand alone. Step 3 does not; it depends on step 4.

The tree already has the other shape. A note type is per owner, and `capability_manifest_note` carries three: `MANIFEST` 1, `SUBTREE_GRANTS` 2 and `VERBS` 3, each with its own version constant, its own encoder, decoder and length. The grants and the verbs joined the manifest as separate notes, never as descriptor versions.

## The decision

Where does the (interface, version) list live?

**Recommended: a fourth note type, provisionally `INTERFACES = 4`, with `INTERFACES_VERSION = 1`.** Its descriptor is a count, then that many records of `INTERFACE_ID: u64` and `version: u32`, little-endian, in id order with no duplicates. The decode refuses anything else, which keeps one encoding. The ABI is one entry of the list, so step 3 reads the entry whose id is `abi`'s. A program with no such note declares nothing. What the progenitor does then (accept, as for every program today, or refuse once packages carry the stamp) is a second small ruling the lane asks for here.

Options:

1. A fourth note type, as recommended. Versions 1 to 3 of the descriptor stay untouched, the zone and service layouts do not multiply, and the stamp is one more note a build adds. A program the stamp does not reach keeps working. Cost: a program carries one more ELF note, and a reader walks one more.
2. Descriptor version 4: version 3's layout, then the counted list. Matches the plan's text and the ruling's wording ("descriptor version 3"). Cost: a manifest that asks for no service and no zones still pays the longer layout to say its interfaces, and every future descriptor field must choose again how to compose. Version 2 and 3 manifests could not declare interfaces at all.
3. Byte 55 as a coarse ABI revision. calef's ruling of 2026-10-29 (UTC, PR #2852, question 3) refused it. The list keeps the record whole.

Prior art, both recalled and not verified in this lane. Fuchsia stamps a coarse API level into a package (the shape proposal read RFC-0239; this lane did not re-read it). ELF's own `.note.gnu.property` holds a list of typed properties in one note, and option 1 gives each concern its own note.

Premise check: the ruling called its choice "descriptor version 3" before milestone 911 spent that number. Its substance holds under either option: a counted (interface, version) list, stamped mechanically, with versions 1 and 2 untouched. Only the container differs.

Reversibility: nothing outside this tree reads the list yet, because nothing writes it, so either choice is cheap today. It turns expensive once the first package carries the stamp.

Would we choose option 1 if both cost the same? Yes. A second note costs a reader a second walk. A composed descriptor costs every future field a composition rule.

## What this unblocks

On a yes: step 4 lands the note type, the encode and decode, and the Kani property, and the `INTERFACE_ID` constants. Step 3 then lands the spawn-time comparison. Steps 5, 6 and 8 wait on step 2's bind check, whose open point is next.

## A second, smaller point: where step 2's check lives

Step 2 puts `member(own, served)` and `Mismatch` "in each message contract". The first cut links no message contract. Its interfaces are a syscall surface, layout constants, a library and two shared pages (`abi`, `address_space_map`, `glob`, `machine_statistics_contract` and `block_roster`). Step 2 therefore serves the contracts the plan lists later. `filesystem_contract` stays out of scope while RedoxFS is replaced. Two homes are possible. One copy per contract crate repeats a ten-line function a dozen times, against rule 7's one definition. One small shared crate (provisionally `interface_version`, a new name) holds the function and the error once, and each contract depends on it. The lane recommends the shared crate and starts step 2 on a yes to the name, since a new crate name is the architect's call.
