# The manifest note: a program's requested authority as ELF note bytes

`crates/capability_manifest_note` is the wire format of the note a program carries to say what
authority it asks for. This page holds the descriptor layouts and the reasons behind the crate's
refusals; the crate's doc comments link here. How a manifest is granted is in
[component-manifest.md](component-manifest.md).

## Where the manifest travels

§197 (a package is one archive file) asked where a manifest travels, and calef ruled option M2 on
2026-09-26 (UTC): inside the executable, as a note behind a `PT_NOTE` program header. So it is in
the bytes the progenitor hashes and cannot drift from the code. The crate is the wire format between
the program that carries a note (`carry!`) and the two that read one: the shell, which binds a line
against it, and the progenitor, which endows from it (AGENTS.md rule 7). Finding the note is
`crates/elf`'s job (`elf::Elf::note`, `elf::NoteSearch`); the crate starts at the descriptor.

| | |
|---|---|
| owner | `OWNER`, `nife` (with its NUL, `namesz` 5) |
| type | `MANIFEST`, `1`, "manifest" |
| descriptor | `DESCRIPTOR_LEN` bytes, the layout below |
| section | `.note.nife.manifest`, which `crates/user_mode_runtime/link.ld` keeps in a `PT_NOTE` inside the read-only load segment |

One manifest has exactly one encoding, and `decode` enforces it. An unknown version, an
out-of-range value, a nonzero reserved byte or a wrong length all refuse. Whatever it accepts,
`encode` turns back into the same bytes (`verification::a_decoded_manifest_encodes_back_to_its_bytes`),
so two readers cannot disagree. A change is a new version word, not a tolerated extension.

## The descriptor, version 1

A version word, then little-endian fields in a fixed order (the encoding calef ratified
2026-09-26). Every field is one of `grant_plan::Manifest`'s, in bytes; nothing here is a new
authority.

| offset | size | field | values |
|---|---|---|---|
| 0 | 4 | version | `1` |
| 4 | 1 | `arg` | 0 forbidden, 1 required; hears words (milestone 205): 2 read-only, 3 read-write, 4 create |
| 5 | 1 | `mem` | 0 forbidden, 1 required |
| 6 | 1 | `file` | 0 forbidden, 1 read-only, 2 read-write |
| 7 | 1 | `dir` | 0 forbidden, 1 required |
| 8 | 8 | `mem` minimum pages | 0 unless `mem` is required |
| 16 | 8 | `mem` maximum pages | 0 unless `mem` is required; at least the minimum |
| 24 | 1 | `output` | 0 silent, 1 words, 2 bytes, 3 bytes and diagnostics (at `grant_plan::DIAGNOSTICS_SLOT`) |
| 25 | 1 | `input` | 0 forbidden, 1 required |
| 26 | 1 | `input` writes while reading | 0 or 1; 0 unless `input` is required |
| 27 | 1 | `dir`'s subtree option | 0 for none, else one of the declared option letters; 0 unless `dir` is required |
| 28 | 1 | `reports` | 0 or 1 |
| 29 | 1 | `interruptible` | 0 or 1 |
| 30 | 1 | `clock` | 0 or 1 |
| 31 | 1 | `domain` | 0 or 1 |
| 32 | 1 | `config` | 0 or 1 |
| 33 | 1 | `entropy` | 0 or 1 |
| 34 | 1 | `network` | 0 or 1 |
| 35 | 1 | `startup` | 0 native, 1 std |
| 36 | 1 | option count | at most `grant_plan::MAX_DECLARED_SHORT_OPTIONS` (16) |
| 37 | 16 | option letters | the first *count* are the letters in bit order, the rest 0 |
| 53 | 1 | `machine` | 0 or 1 (milestone 126, the machine statistics page) |
| 54 | 1 | `share` | 0 or 1 (milestone 126, a view of the job budget) |
| 55 | 1 | zero | |

Byte 55 is the last zero.

## The descriptor, version 2: a manifest and the resolver zones it asks for

calef, 2026-10-19 (UTC), on #2405 (milestone 892 (a std program at the prompt holds the network
and a resolver)'s Q1, "approve version 2?"): "Yes". Version 1's 56 bytes at the same offsets,
with `2` in the version word, then:

| offset | size | field | values |
|---|---|---|---|
| 56 | 1 | zone count | 1 to `MAX_ZONES` |
| 57 | 1 + *n* | each zone: its length *n*, then *n* bytes of dotted ASCII | *n* at most `ZONE_MAX`; 0 is the root zone |

The descriptor ends with the last zone, so `descsz` need not be a multiple of four; `ZonedNote`
pads after it. One request is still one encoding: a manifest that asks for no zone is
version 1, so a version 2 count of 0 is refused, and the zones are canonical (`zones` says
how). Zones are what the program asks for; the owner grants them at install, and nothing here
grants anything (§252 (a resolver grant is one zone per client badge)).

## What no note can spell

`encode` panics on the manifests the layout has no spelling for, and in a constant that panic is a
compile error rather than a note nobody can read. They are a declared second stream at a slot other
than `grant_plan::DIAGNOSTICS_SLOT`, and a subtree option that is not one of the declared letters
or is declared without a directory grant. They also include `grant_plan::Manifest::reboot` and
`sync`, and, since milestone 809 (the package client becomes a program), `installer` and `catalog`.
No image may be endowed those, and `jig` is endowed from the table.

`reboot` and `sync` have no byte on purpose (milestone 805 (`reboot` at the prompt), DECISIONS §251
(restarting the machine is a kernel object the progenitor hands out)). A note is how installed bytes
ask for authority, and `grant_plan::image_can_carry` refuses both to every image. A byte for either
would spell a request no reader may grant. Leaving it out keeps the layout's last zero byte for
version 2 and keeps the wire format unamended. The one program that declares them is endowed from
`grant_plan`'s table, which carries no note.

## The verb table, note type 3

`VERBS` (milestone 809, its option V2) carries a list of (verb, grants) pairs,
`grant_plan::VerbGrant`'s bytes. Each entry names one word a line may give after the program's name,
and which of the manifest's progenitor-endowed authorities that line is granted
(`grant_plan::narrow_by_verb`). A word not in the table is granted none of them. It is its own note
rather than a field of `MANIFEST`, because a manifest is one fixed-length value every reader copies,
and a table of words is not. Most programs carry none.

The image's own `jig` is endowed from `grant_plan`'s table, which carries the same list, so nothing
in the image carries this note yet. It is written, with its decoder, so a distribution can declare
one for a program it packages (basalt, at milestone 810 (`ripgrep` is packaged in basalt and
installed with `jig`)). Until a reader honors it, the progenitor refuses installed bytes that carry
it, rather than endow them with every verb's grants at once.

The descriptor is a version word, a count word, and exactly that many records, each the word
zero-padded to `grant_plan::MAX_VERB` bytes, then the grants' bits little-endian, then two zero
bytes. Its length varies, as calef ruled on #1903 (2026-10-10 UTC). One table still has one
encoding, because the records are sorted ascending by their words' bytes and each verb appears once.
