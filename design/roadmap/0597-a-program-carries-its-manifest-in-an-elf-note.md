---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
promoted_from: a-program-carries-its-manifest-in-an-elf-note
---
# 597. A program carries its manifest in an ELF note

Built 2026-09-26 by lane `milestone/597-manifest-note`, promoted from the proposal
`a-program-carries-its-manifest-in-an-elf-note`, which the `maintainer/m2-ruling` lane raised
recording calef's ruling on DECISIONS §197 (a package is one archive file). *(Number and title
provisional: the integrator mints the number at merge, and the title is a draft until an architect
names it.)* It replaces the prototype in draft pull request #1319 (the price of an ELF-note manifest).

The gate the proposal carried (DECISION: the owner string, the type number and the encoding) was
answered on 2026-09-26; see "What was ruled".

## What was ruled

calef, 2026-09-26 (UTC): *"M2 is right."* A program's manifest travels inside its executable, as an
ELF note found through a `PT_NOTE` program header. §197 has the ruling and its reasons.

calef, 2026-09-26 (UTC), answering the maintainer's three names with *"Yes"*:

| | ruled |
|---|---|
| owner string | `nife` |
| note type | `1`, "manifest" |
| encoding | a fixed binary layout: a version word, then little-endian fields in a fixed order |

The owner and type carry `Name: ratified` blocks at `manifest_note::OWNER` and
`manifest_note::MANIFEST`. The field order of version 1 is this lane's design, below, and is a wire
format: changing it is a new version word.

## The descriptor, version 1

56 bytes. `crates/capability_manifest_note` is the only definition (AGENTS.md rule 7), with `encode` and
`decode`; its module documentation has the same table.

Amended in place on 2026-09-27 (UTC) on calef's ruling, before anything outside the tree had acted on version 1. `arg` also takes `2`, "words", for milestone 205 (how a foreign program is told what to do). The same day, and the same ruling (on #1360), `machine` and `share` amend version 1 in place too, for milestone 126 (the `procps` package): one zero byte (55) remains, so the next field is likely version 2. The module's table is current.

| offset | size | field | values |
|---|---|---|---|
| 0 | 4 | version | `1` |
| 4 | 1 | `arg` | 0 forbidden, 1 required |
| 5 | 1 | `mem` | 0 forbidden, 1 required |
| 6 | 1 | `file` | 0 forbidden, 1 read-only, 2 read-write |
| 7 | 1 | `dir` | 0 forbidden, 1 required |
| 8 | 8 | `mem` minimum pages | 0 unless `mem` is required |
| 16 | 8 | `mem` maximum pages | 0 unless required; at least the minimum |
| 24 | 1 | `output` | 0 silent, 1 words, 2 bytes, 3 bytes and diagnostics |
| 25 | 1 | `input` | 0 forbidden, 1 required |
| 26 | 1 | writes while reading | 0 or 1; 0 unless `input` is required |
| 27 | 1 | `dir`'s subtree option | 0 for none, else a declared letter |
| 28 to 34 | 1 each | `reports`, `interruptible`, `clock`, `domain`, `config`, `entropy`, `network` | 0 or 1 |
| 35 | 1 | `runtime` | 0 native, 1 std |
| 36 | 1 | option count | at most 16 |
| 37 | 16 | option letters | the first *count*, in bit order; the rest 0 |
| 53 | 1 | `machine` | 0 or 1 (milestone 126, the machine statistics page) |
| 54 | 1 | `share` | 0 or 1 (milestone 126, a view of the job budget) |
| 55 | 1 | zero | |

One manifest has one encoding. `decode` refuses an unknown version, any length but 56, a value
outside its field's range, and a nonzero byte where the layout says zero. The Kani harness
`a_decoded_manifest_encodes_back_to_its_bytes` proves that whatever `decode` accepts, `encode`
spells back byte for byte, over every 56-byte string. So no two byte strings mean one manifest,
and the shell and the progenitor cannot read one note two ways.

Two fields were settled by the layout rather than carried. The diagnostic stream's slot is
`grant_plan::DIAGNOSTICS_SLOT`, whose own documentation already says a manifest does not pick it.
A manifest's option letters became a value, `grant_plan::Flags`, because a decoded manifest has no
`'static` string to point at.

§170 (how a foreign program is told what to do) is being recorded as this lands, by
`maintainer/170-ruling`, and was not readable from this lane: that branch held only its claim
commit. The brief says the ruling adds named-file grants read-only, read-write or directory, and
"may create the named path". Version 1 carries the first three as `grant_plan::Manifest` has them
today (`file` 1 and 2, and `dir`). It does not carry "may create", because `Manifest` has no such
field yet; when it does, that is a version 2, or a fourth `file` value if calef prefers to widen
version 1 before anything outside this tree has written one. See Follow-on.

## What is built

1. The reader (`crates/elf/src/note.rs`), from #1319, split so two callers read one rule. The
   progenitor holds a whole image and asks `Elf::note`. The shell never holds all of a file (it
   streams it into frames a page at a time), so it finds the `PT_NOTE` headers in the first page
   (`NoteSegments::from_head`) and feeds each segment to the same `NoteSearch`. A duplicate note,
   in one segment or across two, is refused on both paths.
2. **The format** (`crates/capability_manifest_note`), above, and `carry!`, which places a program's note in
   `.note.nife.manifest` from a constant at compile time.
3. **The linker script** (`crates/user_mode_runtime/link.ld`) keeps that section in the `rodata`
   load segment and in a `PT_NOTE` of its own. `helpers/build-ripgrep.sh` derives its copy by
   substitution, so it inherits both. A program that carries no note gets an empty `PT_NOTE`, which
   reads as no manifest.
4. **The policy** (`grant_plan::image_manifest`), replacing #1320's `INSTALLED_MANIFEST_OF`. Vouched
   bytes get what their note declares, since the digest that vouched covers the note. Bytes with no
   note get `NO_NOTE_MANIFEST`, which is `uptime`'s. Unvouched bytes get `UNVOUCHED_MANIFEST`
   whatever their note says, per §219 (how the shell names an installed program to the spawner),
   and are refused if the note asks for an argument or memory.
5. **The progenitor** reads the note in its own copy of the bytes and endows through that function.
   A note that is there and unreadable refuses the bytes, and so does a request whose words do not
   fit the manifest it would be endowed with. Both answer `SPAWN_REFUSED_BY_MANIFEST`.
6. **The shell** reads the note before binding, refuses an unreadable one or one an image cannot
   carry at the prompt, and binds the line against it. `caps <path>` prints what the note asks
   beside what will be granted.

**A boot image's programs keep their compiled-in manifests.** Moving them is not trivial: the
shell binds a named program before it has any bytes, from `grant_plan::Prog`, and the progenitor
builds from its own archive, so reading their notes would mean reading every program's bytes at
the prompt. Three programs carry notes today: `greeting`, and two fixtures whose notes exist to be
refused or ignored. `uptime` carries none, deliberately, so the no-note default is exercised.

## The gate that proves it

`script/swish-check`, on aarch64, riscv64 and x86_64:

- `packages/greeting/0.1.0/greeting` prints `clock: held at slot 1, as its manifest note asked`.
  Under #1320's ceiling that slot was empty. `caps` of it prints the clock row and the note's ask.
- `caps packages/uptime/0.1.0/uptime` prints that it carries no manifest note and asks for its
  output alone: the no-note default.
- `installed/unvouched`'s note asks for entropy, the network and the process domain. `caps` prints
  that ask beside the ruling's three slots, and the run's census still reads `slots held: 0 1 2`.
- After the owner vouches for `installed/unvouched` (#1340's `vouch`), the same bytes run endowed
  from their note: the census reads `0 7 9` (and 10 where there is a network), where D2 gave
  `0 1 2`.
- `installed/asks-an-arg 5` is refused by the progenitor, whose words are *"the
  progenitor read those bytes' own manifest, and it does not allow this line"*. Its note asks for
  an argument and nobody vouched for it.
- `installed/malformed-note` is the witness with its note's version word set to 9, refused at the
  prompt as *"carries a manifest note that cannot be read"*.

Proofs: four Kani harnesses on the reader, two on the format, each with a replayable falsification
that the sweep turned red. Host tests cover every field's range, every zero byte, each image
policy case, and the streaming reader agreeing with the whole-image reader.

## Falsifications

| claim | how it was made to fail |
|---|---|
| a note that asks more than its vouch allows is refused | host test `an_image_is_endowed_from_its_note_only_when_vouched`; swish-check's `installed/asks-an-arg 5` |
| a malformed note is refused | host tests on every field, `decode_never_panics`'s patch, the elf crate's four; swish-check's `installed/malformed-note` |
| a binary with no note gets the no-manifest default | host test `image_manifest(None, ..)`; swish-check's `caps` of `uptime` |
| an unvouched note grants nothing | the witness's census, with a note asking for all three |
| one manifest, one encoding | `a_decoded_manifest_encodes_back_to_its_bytes`, red when `decode` stops checking the trailing zeros |

## BUGS

- **The writable activation set is no longer harmless.** The boot prompt can write `activation/`,
  so it can vouch its own bytes, and a vouched program is now endowed from its own note, the
  network and the process domain included. This lane was told not to touch who may write
  `activation/`; the fork is `notes/who-may-write-the-activation-set.md`, whose cost of waiting
  this milestone raised.
- An image request carries an argument and a `--mem` count and nothing else a line designates.
  A note that declares a file, a directory, an input, an option, a supervised job, a second
  stream, a silent output or the `std` runtime is refused (`grant_plan::image_can_carry`) rather
  than run without it. `spawnproto`'s BUGS has each.
- An image's `Endowment` names a stand-in row, `grant_plan::IMAGE_ROW`, because an endowment
  names a `Prog`. Reading that row for an image reads the wrong manifest. It is marked as the
  exception it is; the fix is an endowment that holds its manifest.
- The shell reads a note only from a file whose program headers are in its first page, and only
  note segments of a page or less. Anything else reads as "no note" or is refused, and the
  progenitor then refuses a request that does not fit what it finds. It fails closed, and no
  program this tree links is affected.
- A foreign build carries a note only by linking an object that holds one (#1319 measured
  `-Clink-arg=note.o`). Nothing writes that object yet.
- Version 1 does not carry §170's "may create the named path", because `grant_plan::Manifest`
  does not yet; see "The descriptor, version 1".
- The shell and the progenitor read two copies of the file. A file changed between the two is
  judged on what reached the progenitor.

## Follow-on

- **Recorded.** "May create the named path" is in this block's BUGS: carried when §170's ruling
  is on `main` and `grant_plan::Manifest` carries it. It is a wire change here: a version 2, or a widening of
  version 1's `file` field if calef rules that before anything outside this tree writes a note.
  Checked 2026-09-26 against `maintainer/170-ruling`, which held only its claim commit.
- **Recorded.** An image request that can carry a file, a directory, an input and options, which
  is what lets a note declare them, is in `crates/grant_plan/src/spawnproto.rs`'s BUGS. It is the
  wiring a named program already has, over the image path.
- **Milestone 595.** A `std` image: the progenitor sizes an image's region before the bytes, and
  so the note, arrive. Milestone 595 (the shell runs a `std` program)'s Follow-on carries it.
- **Recorded.** The writable activation set is in `notes/packages.md`'s BUGS and in
  `notes/who-may-write-the-activation-set.md`, the architect's open fork.

## Index row

A program's manifest now travels in its own bytes, as an ELF note (owner `nife`, type 1, a
versioned fixed layout), read through a `PT_NOTE` program header. The shell binds a file's line
against it and the progenitor endows from it, vouched; unvouched notes grant nothing, and `caps`
shows the ask beside the grant. Proved on all three architectures at the prompt, with the format's
one-encoding property proved by Kani.
