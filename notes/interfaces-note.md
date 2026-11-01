# The INTERFACES note, explained

*Name: provisional, minted 2026-11-01 (UTC) by lane/the-interfaces-note-explained.*

A one-page account, for a reader who has not followed milestone 956 (the linked interfaces carry
checkable versions), of what the `INTERFACES` note is and what `crates/interfaces_note` does. The
plan, the id table and the rulings are in
[the milestone's block](../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md);
the byte layout is in the crate's module documentation. This page was read against that code.

## What it is for

A program is built against shared crates that define formats another, separately released part
reads at run time: the syscall numbers (`abi`), the clock page (`clock_contract`), the sink framing
(`byte_sink_contract`) and so on. Once a package leaves this repository, a program and the system
that runs it can be built from different commits. The note is how a program says, inside its own
bytes, which version of each such format it was built against, so the other side can check.

## What a program writes

One line at module scope in the program's root, the macro `interfaces_note::carry_interfaces!`,
given a list of `interfaces_note::Declared { id, version }` with each id read from the interface
crate's `INTERFACE_ID` and each version from that crate's version constant. The macro places one
static, `InterfacesNote`, in the section `.note.nife.manifest`, which the link script
(`crates/user_mode_runtime/link.ld`) gathers into the output section `.note.nife` and its
`PT_NOTE` segment, beside any manifest note the program carries. The note is computed by a `const
fn`, so a list whose ids do not strictly ascend, or that holds more than 64 entries, is a compile
error.

The bytes, all little-endian words:

| bytes | what |
|---|---|
| 0 to 3 | name size, 5 (`nife` and its NUL) |
| 4 to 7 | descriptor size, `8 + 12 * count` |
| 8 to 11 | note type, `INTERFACES` = 4 |
| 12 to 19 | `nife`, then NUL padding to 8 bytes |
| 20 to 23 | descriptor version, `INTERFACES_VERSION` = 1 |
| 24 to 27 | count |
| then | `count` records of 12 bytes: the id (`u64`), then the version (`u32`) |

The header is the one every nife manifest note has: owner `nife`, with a type per kind of note
(`MANIFEST` 1, `SUBTREE_GRANTS` 2, `VERBS` 3, `INTERFACES` 4). `decode_interfaces` refuses an
unknown descriptor version, a count over 64, a length the count does not predict, and ids that do
not strictly ascend, so one list has exactly one encoding. Two Kani harnesses in the crate state
that a decoded list encodes back to its bytes and that decoding never panics.

## The id table

Every interface owns a number for good. The one table is the "Interface ids" section of the
milestone's block: `abi` is 1, `address_space_map` 2, `machine_statistics_contract` 4,
`byte_sink_contract` 6, `capability_manifest_note` 7, `clock_contract` 8, and so on to
`user_mode_runtime` at 20; 3 is retired (it was `glob`). An interface is the module that holds
`INTERFACE_ID`, so `filesystem_contract` holds two, `blk` at 14 and `fs` at 15. A new interface
takes the next unused number by adding a row there, and a number is never reused.

## What a version bump means

A format change is a new version under the same id, never a new id. Every interface is at version 1
today. The check at load compares by equality: the syscall ABI is never served in two versions at
once, so a program built against `ABI_VERSION` 1 is refused by a system serving 2, and a program
built against 2 is refused by one serving 1. For every other interface, a bump today changes the
recorded bytes and nothing refuses on it; a check at bind time for message contracts is a separate
proposal, [the message contracts carry bind-time version checks](../design/roadmap/proposals/message-contracts-carry-bind-time-version-checks.md).

## What is checked at load, and where

The kernel does not read this note; nothing under `kernel/` names it. The progenitor does, in
`crates/system_initializer/src/image_request.rs`, function `serves_declared_abi`, which
`endowed_image` in the same file calls after decoding the manifest note. It finds the note by owner
`nife` and type 4. No note passes. A note that does not decode refuses. A note that decodes is
asked `check_served(abi::INTERFACE_ID, abi::ABI_VERSION)`: an entry for id 1 that is not the
served version refuses, and a list with no entry for id 1 passes. Only the ABI entry is compared.

A refusal makes `endowed_image` return `None`. The spawn loop
(`crates/system_initializer/src/spawn_loop.rs`) answers the request with
`spawnproto::SPAWN_REFUSED_BY_MANIFEST` (`u64::MAX - 3`), and the shell prints `refused: the
progenitor read those bytes' own manifest, and it does not allow this line`. The `Refusal` value
that names both versions stops inside the progenitor: the reply carries a status word and no text.

This runs only on the image path, when the shell sends a program's bytes from a file
(`Wiring::image`). A program the progenitor starts from the archive built with the kernel is not
checked; the milestone's BUGS section records that.

## Why a leaf crate

`capability_manifest_note`, which holds the other note types, depends on `grant_plan` (the shell's
planner) and on `domain_name_resolution_contract`. Carrying the list through it would link both
into every program, and the check would then make the program declare them, though it uses
neither; `disk_surveyor` and `disk_partitioner` link neither today. `interfaces_note` depends on
nothing. It repeats the owner string `nife` rather than import it, and a test in
`capability_manifest_note` (`the_interfaces_note_shares_the_manifest_notes_header`) holds the two
headers equal.

## What keeps a list honest

`helpers/interface_declarations_check.py`, which `script/lint` runs with its selftest, reads each
program of `process-tools`, `core-tools` and `disk-tools`, works out the crates it links from its
source and `cargo metadata`, and fails when a list misses an interface the program links or names
one it does not, when a version is read from a module other than its id's, when an `INTERFACE_ID`
in the tree disagrees with the table, or when a linked crate is not classified. It reads source
text, not the linked bytes. The crate `interfaces_note` itself has no id: the check lists it as
owed one, so no list declares it, and the note does not declare its own format. Its descriptor's
first word is its own version.

## Which programs carry it

All fifteen programs of the first cut: `date`, `disk_partitioner`, `disk_surveyor`, `free`,
`pgrep`, `pmap`, `printenv`, `ps`, `rm`, `slabtop`, `top`, `uptime`, `uuid`, `vmstat` and `wc`.
Derived with

```sh
git grep -l 'interfaces_note::carry_interfaces!' -- '*.rs' | grep -v '^crates/interfaces_note/'
```

which names exactly the `programs` of `packages/process-tools.package.toml`,
`packages/core-tools.package.toml` and `packages/disk-tools.package.toml`.

## A worked example: `uptime`

`components/src/uptime.rs` declares nine interfaces, each at version 1: `abi` (1),
`address_space_map` (2), `byte_sink_contract` (6), `clock_contract` (8),
`counter_frequency_contract` (9), `current_cpu_contract` (10), `entropy_contract` (12),
`socket_contract` (19) and `user_mode_runtime` (20). It carries no manifest note of its own, so this
is the only note in its `.note.nife` section: 136 bytes, the 20-byte header and owner, then a
descriptor of `8 + 9 * 12` = 116 bytes. Built for `aarch64-unknown-none-softfloat` and dumped with
`readelf -x .note.nife`, the section begins

```text
05000000 74000000 04000000 6e696665 00000000   name size 5, descriptor 0x74, type 4, "nife"
01000000 09000000                              version 1, count 9
01000000 00000000 01000000                     id 1 (abi), version 1
02000000 00000000 01000000                     id 2 (address_space_map), version 1
```

and the seven records after run 6, 8, 9, 10, 12, 19 (`0x13`) and 20 (`0x14`), each at version 1.
Sent to the progenitor as a file, those bytes are accepted while `abi::ABI_VERSION` is 1, and
refused with `SPAWN_REFUSED_BY_MANIFEST` by a progenitor built with any other value.

## Not verified here

- The Kani harnesses were not run for this page; `script/verify` runs them.
- The refusal was read in code and its host tests (`image_request.rs`'s tests module), not
  observed on a booted system.
