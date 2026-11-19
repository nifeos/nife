# Ruled decisions

An appendix to [the milestone](../1096-interfaces-note-outside-review.md). The four decisions
below are the rules in force.

## The owner string `nife` in a crate that may not say `nife`

Background. Every nife manifest note has the owner `nife`, and the macro places the note in
`.note.nife.manifest`. The standing rule says a crate on the externalization list names neither nife
nor its programs, and `lint_externalized_crates.py` and `script/crate-move` both fail on the word.
Five of this crate's nine hits are the owner constant, the section name and a test asserting the
bytes. `capability_manifest_note`, on the same list with 37 hits, has the same constant. Changing
the owner changes the bytes of every built program and every reader; it is the one-way side.

Ruled 2026-11-14 UTC (decision D1): the owner and the section name keep their bytes, for both note
crates. Bytes that already sit on a wire or in built programs may spell the word after the crate
leaves; the exception covers wire literals only (owner strings, section names, magic values and the
tests that assert those bytes). The lint and the move check accept a line that ends with the
comment `// wire literal` (provisional spelling), refuse an unmarked mention, and the lint prints
every marked line. Prose around them says "the owner string" and does not repeat it. Reversing the
exemption costs one pull request; reversing a renamed owner costs a format version.

Prior art. The owner of an ELF note is the vendor's name and nothing else: `GNU` for
`.note.gnu.build-id` and `.note.ABI-tag`, `FreeBSD` for `.note.tag`, `NetBSD` for
`.note.netbsd.ident`, `Android` for `.note.android.ident`. Every third-party parser (readelf,
`object`, `goblin`) carries those strings as literals. Where nife differs: no other project forbids
its own name in its own format's crate; the rule exists so the crate stands alone, and a wire
constant does not stop it standing alone.

Example. After the ruling, `src/lib.rs` keeps `const OWNER: &[u8] = b"nife"; // wire literal` and the module doc reads "the owner string names the operating system that reads
the note".

## The public names, ratified before the move

Background. Every `Name:` block in the crate reads `provisional`, and `script/crate-standalone`
step 0 refuses a crate whose names are not ratified. The names are one-way at the tag that nife
pins. Twenty public names: `Error` (`NoVersion`, `UnknownVersion`, `WrongLength`, `BadField`),
`INTERFACES`, `INTERFACE_ID`, `INTERFACES_VERSION`, `MAXIMUM_INTERFACES`, `INTERFACE_RECORD`,
`Declared` (`id`, `version`), `interfaces_len`, `encode_interfaces`, `Interfaces` (`len`,
`is_empty`, `iter`, `version_of`, `check_served`), `decode_interfaces`, `Refusal` (`interface`,
`declared`, `served`), `interfaces_note_len`, `InterfacesNote` (`of`), `carry_interfaces!`.

Recommendation. Ratify them as they stand except four: `INTERFACE_RECORD` becomes
`INTERFACE_RECORD_BYTES`, `interfaces_len` becomes `descriptor_bytes`, `interfaces_note_len`
becomes `note_bytes`, and `Declared` becomes `DeclaredInterface`. The first three follow the unit
rule; the fourth gives a noun. `len`, `is_empty` and `iter` keep std's spelling. `version_of`
stays, since std's `get` would promise the record and this returns one field of it.

Prior art. std names a byte count `size_of` and an element count `len`; the unit rule's own example
is `MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`. `goblin::elf::note::Note` and
`object::read::elf::Note` name the record for what it is. Where nife differs: the unit is in the
name, not in a doc comment.

Example. `let bytes: [u8; descriptor_bytes(3)] = encode_interfaces(&list);` reads as 44 bytes for
three records; `interfaces_len(3)` reads as a count of three.

Ruled 2026-11-19 UTC (option A): the architect ratified the twenty names with the four renames,
`INTERFACE_RECORD_BYTES`, `descriptor_bytes`, `note_bytes` and `DeclaredInterface`, and no alias of
the old spellings. Sixteen names keep their spelling. Ratification takes effect for the `Name:`
blocks when the renames land; items 9 and 10 are that work, done by a rename lane. The names are
one-way at the first tag that nife pins, and ratifying them is what lets the move begin.

## The crate name

Ruled 2026-11-14 UTC (decision 3, option A): the crate and repository name `interfaces_note` is
ratified, and the crate's `Name:` block says so. The crates.io name waits on the publishing hold.

Background. The repository `nifeos/interfaces_note` takes the name at creation, which is the
one-way step; the crates.io name waits on the publishing hold.

Recommendation. Keep `interfaces_note`. It follows the sibling `capability_manifest_note` (what the
note holds, then `_note`) and the ruled type name `INTERFACES`. The alternative, if the name should
say what the records are, is `interface_versions_note`.

Prior art. Note crates and types take their names from the note's type: `abi-tag`, `build-id`,
`gnu-property`. Linux names the module table that is this crate's closest relative `__versions`.
Where nife differs: the `_note` suffix says the carrier, which the alternatives above do not.

Example. The README's first line under either name: "An ELF note that lists the interface versions
a program was built against, and a decoder that refuses every other spelling of that list."

## Review clearance and the repository

Background. Clearing a crate review is the go to create its repository under nifeos. This crate
has no review package in `notes/crate-reviews/`; milestone 1096 is its review. It leaves last in
step 3 of milestone 1017 (the contract crates leave in this order), after the three crates ahead of
it. The repository name is one-way: a repository that exists has a name others link to.

Ruled 2026-11-19 UTC (option A, the maintainer's provisional ruling): clear the review once the
four renames land. The architect then gives the go to create `nifeos/interfaces_note`, with no tag
and no publication, and the maintainer asks for that go from milestone 1096. Clearing now with the move waiting
(B), and holding until a dry run also stages the move's own items 1 to 5, 11 and 15 (C), were the
alternatives.

Prior art. In this tree `generational_table`, `block_roster` and `file_allocation_table` each moved
on a cleared review with no tag. Where nife differs: nothing outside nife links the crate.

Example. Once the renames land, `nifeos/interfaces_note` opens with the README of item 2, and nife
pins it by revision.
