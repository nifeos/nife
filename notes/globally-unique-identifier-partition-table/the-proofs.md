# Proved, symbolically

The Kani harnesses behind
[`notes/globally-unique-identifier-partition-table.md`](../globally-unique-identifier-partition-table.md):
what each quantifies over, how long it takes, and the three whose reasoning needs spelling out.

`script/verify` runs these eight harnesses, 69 to 74 seconds in total on patagonia (Apple M3) under a
shared load on 2026-10-22 (UTC). The UUID type comes from the `uuid` crate, which carries no
harness here.

| harness | what it quantifies over | time |
|---|---|---|
| `crc32_matches_its_bitwise_definition` | every 8-byte input | 20 s |
| `a_single_byte_change_always_changes_the_crc` | every 8-byte buffer, every position, every replacement | 33 s |
| `an_entry_survives_the_round_trip` | all 2^1024 partition entries | 8 s |
| `a_guid_survives_the_on_disk_layout` | all 2^128 on-disk GUIDs | 3 s |
| `overlap_is_exactly_sharing_a_block` | every pair of LBA ranges (complete, not bounded) | 2 s |
| `a_headers_fields_survive_the_round_trip` | every value of all nine header fields | 3 s |
| `the_header_fields_partition_the_block` | every 512-byte block: no two fields share a byte | 2 s |
| `create_never_lays_out_a_table_parse_would_reject` | every disk size and every partition placement | 22 s |

Three of these need their reasoning spelled out, and one of them is a correction.

## Why the CRC harnesses are bounded at 8 bytes

CRC-32 is a shift-and-xor over every bit of the input, so the formula the solver gets grows with
the byte count. The bound does not buy "CRC-32 is correct for short inputs", which would be a nearly
worthless claim. It buys the *structure*: the table-driven implementation agrees with the bitwise
definition, and a changed byte always changes the answer. Both are properties of the algorithm
rather than of the length. The lengths that actually occur (92 bytes, 16 KiB) are covered
exhaustively in the parent, over bytes real tools wrote.

The table-versus-definition harness earns its keep because the table is derived by `const`
evaluation from the polynomial. A wrong table that happened to be self-consistent would round-trip
our own output perfectly. It would pass every test in the suite except the published check values,
and it would fail on every real disk.

## A symbolic CRC-32 is very expensive, and it decided the shape of two harnesses

The first attempt at the header round trip went through `encode_into` and `decode`, so the formula
carried two 92-byte CRC-32s over symbolic bytes. It ran 274 seconds without finishing. The
`script/verify` budget is about three minutes per harness, the same budget the DMA harnesses record
in notes/verification.md.

The measured reason is in the two CRC harnesses above. One table CRC plus one bitwise CRC over 8
symbolic bytes is 20 s. Two table CRCs over the same 8 bytes is 33 s. So the table version costs
roughly five times the bitwise one for the solver. A table lookup on a symbolic index is a 256-way
multiplexer, and there is one per byte from the first symbolic byte onward.

So `Header` grew a `decode_fields`/`encode_fields` pair, and the round trip is proved in two
halves. They compose for a reason you can read in three lines of source rather than assume.
Everything `encode_into` adds is four bytes at offset 16. Everything `decode` adds is checks. And
the CRC field is not one of the nine fields, so neither half can disturb the other. That took 3
seconds instead of 274, and it is a better decomposition anyway: the layout and the integrity are
different claims.

## The create-then-parse harness moved for the same reason

A byte-level version has four symbolic CRC chains in it and did not finish either. The byte-level
identity is proved instead where it can be proved *completely*, in `tests/real_disks.rs`.
`GloballyUniqueIdentifierPartitionTable::create` reproduces `sgdisk`'s 512-byte header, its backup
header and its 16 KiB array exactly.

What that test cannot do is vary the disk size. So the harness takes that dimension, and asserts
every geometry rule `GloballyUniqueIdentifierPartitionTable::parse` enforces on what
`GloballyUniqueIdentifierPartitionTable::create` produced, for every `u64` disk size. A writer that
emits a table its own reader rejects is the worst failure available to this crate, and an unusual
disk size is exactly where it would hide.

## The one that is complete

`overlap_is_exactly_sharing_a_block` is complete rather than bounded, because it is pure integer
comparison. It states the property as its definition (the intersection is non-empty) rather than
as the implementation, so the proof is not the code compared against itself.
