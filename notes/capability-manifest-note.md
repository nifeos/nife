# The manifest note: what no note can spell, and the verb table

`crates/capability_manifest_note` is the wire format of the note a program carries to say what
authority it asks for. The descriptor layouts are in the crate's module documentation; this page
holds the reasons behind what the layout leaves out, and the verb table's design. How a manifest is
granted is in [component-manifest.md](component-manifest.md).

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
