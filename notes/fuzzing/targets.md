# The parser targets, one by one

*Name: provisional (lane comments/929-rest-01, 2026-10-24 UTC). An appendix to
[`notes/fuzzing.md`](../fuzzing.md), whose table says in a line why each target earns its place. This
file keeps the longer reasons each target's own doc comment points to.*

## `device_tree_blob_walk`

The walkers are where the parser's state lives: a depth counter, two 16-entry per-depth cell-count
stacks, and a slot for the node being walked. Each is indexed by a number the blob controls. A
walker is an unbounded loop over a symbolic blob, which is where bounded model checking stops, so the
Kani harnesses deliberately stay with the leaf readers.

A blob that parses is not a blob that is safe to walk, and the kernel calls every accessor on the
same blob during boot. So the target calls them all. The count grows with the parser; the rule is
that the target tracks the parser's public surface.

## `elf_parse`

"Run this binary" is something a user asks for by name, so this parse has the widest supply of
hostile inputs. The whole-parse totality proof failed on an O(n^2) overlap loop over up to 64
program headers, unrolled 64 deep against symbolic slice offsets. That is past what the solver can
do at a useful input size. The builds for the three machines differ only in the one `u16` the
machine check compares, so the paths past it are the same paths on every host.

## `globally_unique_identifier_partition_table`

A partition table is read from a disk somebody else formatted, with somebody else's tool, possibly
years ago and possibly on purpose. Every field the parser trusts is one an attacker with a disk
image gets to choose.

The crate is the most heavily checked parser in the tree. It has eight Kani harnesses and exhaustive
mutation tests that flip every byte of a real header and entry array to every other value, 460,000
validations. Both explore single fields around a valid table. The interesting arithmetic needs
several hostile fields at once: `entry_count * entry_size`, `alternate_lba - entry_array_blocks()`,
and `last_usable_lba > block_count - backup_reserved`. A single-byte mutation cannot reach it,
because changing any field breaks the CRC, and proving that the CRC catches it is the mutation
test's job. A table that survives the CRC and then makes one of those wrap is what a fuzzer is for.

The fixtures use 512-byte blocks. 4K-native disks exist, and the parser takes the block size from
the header block's length, so a second split would be worth adding.

## `file_allocation_table_boot_sector`

A FAT volume arrives on a stick a stranger formatted, and its boot sector says where every table,
the root directory and each cluster lie. A wrong number there sends every later read to the wrong
sector, or past the volume. The crate's four Kani harnesses already cover `BootSector::parse` on
every 512-byte sector, since the parse has no loop. What they cannot reach is the device path: a
4096-byte sector, a device whose sector size disagrees with the volume, and the accessors a reader
calls once the parse has said yes. So the target reads each input at all four sector sizes through
a `SectorDevice` over the bytes, and checks the same geometry the harnesses prove on everything
accepted. The dosfstools-made fixtures seed it unmodified.

Then it opens the volume and walks its tree: every directory's slots, and every short entry's
chain, four directories deep. Each chain must yield only data clusters, no more than the volume
holds, and a walk that fails with `TooLong` must have yielded exactly that many. Kani proves the
walk's bound on volumes of up to eight clusters; this holds it on whole images. A budget of 16,384
steps per input keeps a hostile volume that claims millions of clusters from turning one input into
a timeout. `fuzz/seeds/file_allocation_table_boot_sector/small-tree.img`, a 6 KiB FAT12 volume
`fuzz/seeds/make-fat12-tree.py` writes, seeds the walk, since the boot sectors hold no tree.

## `file_allocation_table_writer`

The reader target holds the parser to what a stranger's stick can say. This one holds the writer to
what it promises on any volume. Operations decoded from the input (create a file, write, set a
length, seek and write, remove, make and remove a directory, rename) run against either a hostile
128 KiB image or an empty volume the crate formats. Names come from a small alphabet, so they
collide, nest and need long-name slots. Nothing may panic or write outside the image. On the
formatted volume, after `sync`, the root still lists, holds no more entries than the writer made,
and the shutdown bit reads clean. `fuzz/seeds/file_allocation_table_writer`
holds one seed of each kind: the reader's small tree behind the hostile-image byte, and a short
script of operations behind the formatted byte.

## `capability_manifest_note_decode`

Whoever built a program wrote the manifest note inside its ELF, and the progenitor decodes it to
decide what the program holds. The progenitor holds all userspace authority, so nothing confines it
from these bytes. Kani proves `decode` total and proves its output encodes back, but only to 57
bytes, which end at the zone count byte of versions 2 and 3. The zone walk and the service byte lie
past that bound, and `decode_subtree_grants` and `decode_verbs` have no harness.

Every input goes to all three decoders. On acceptance the target writes the descriptor back out and
requires the input byte for byte, because the crate promises one encoding per manifest. It writes
the first 56 bytes with the crate's `encode`, then the zones and service byte the accessors report,
or the scope's own note, or the verb records. It also holds the invariants the crate states:

- No note carries `reboot`, `sync`, `installer`, `catalog`, `indexes`, `activation_directory` or
  `console_input`.
- Zones are canonical and strictly ascending, and version 2 has at least one.
- A refusal by `decode` is a refusal by `zones` and `service`.
- The decoder refuses a valid descriptor with a byte cut or added.
- Verbs ascend, and `grants_for` finds each.

Mutation alone seldom keeps a zone walk valid, since one flipped length byte shifts every later
zone. So the seeds are valid manifests of every version that the crate encodes
(`fuzz/seeds/capability_manifest_note_decode/`, which `crates/capability_manifest_note/tests/
fuzz_seed.rs` holds to the encoders), and a custom mutator builds more. It makes a fresh zoned
descriptor from a pool of canonical zones, or a verb table, and sometimes breaks one thing. A
sixty-second smoke run left versions 2 and 3 past the count byte in its corpus.

## `domain_name_system_reply`

The real captures in `crates/domain_name_system/tests/fixtures` seed the target unmodified, and
`script/fuzz` passes that directory read-only, as it passes every seed directory.

## `http_response_feed`

The digest decides whether a package may run, but it cannot stop a head that panics the reader, and
a panic in the progenitor takes the prompt with it. The target asserts properties rather than only
waiting for a panic. notes/fuzzing.md's BUGS section records that three of the older targets assert
nothing, and that the one with a real property is the one that found a silent bug.

## `nifefs_roundtrip` and `package_archive_roundtrip`

Fuzzing `Fs::parse` for panics would burn CI time rediscovering a theorem Kani already proves. What
can go wrong unseen is the writer and reader disagreeing. A name over `NAME_LEN` was once silently
truncated, so two distinct files could land under one name and the second would win. A data-loss
bug in an archive format is worse than a panic, and much quieter.

The package target applies the same shape to `package_archive`. Its digest check matters most: a
package that fails its own digest would be refused for no reason a user could act on. That is the
quietest way for a trust mechanism to become noise.
