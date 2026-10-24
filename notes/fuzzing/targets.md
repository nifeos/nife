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
