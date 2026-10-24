# The crate's modules: the reasons behind their doc comments

The detail behind the doc comments in `crates/globally_unique_identifier_partition_table`, for
[`notes/globally-unique-identifier-partition-table.md`](../globally-unique-identifier-partition-table.md).
Each section names the module it serves.

## CRC-32

`crc.rs` keeps two implementations of one CRC. `crc32` is table-driven because the exhaustive
mutation tests run it a hundred thousand times over a 16 KiB entry array. Eight shifts per byte
would turn a two-second test into a twenty-second one. The table is derived at compile time from the
polynomial, so the derivation is a second place a bug can hide. The bitwise form stays beside it
for that reason, with a Kani harness holding the two equal: the optimization and its definition side
by side, rather than the optimization alone and a comment claiming it is right.

There is no room to be creative about which CRC-32 it is. If it is wrong by one bit, no real disk
parses.

## The header's declared size

`Header::decode` accepts a declared header size above 92 and covers the extra bytes with the CRC.
In practice nothing writes anything but 92. A revision that grew the header would also have to
change the revision field, and the crate refuses any revision but 1.0 outright. So the larger size
is accepted on paper and never met.

## Type GUIDs and unique GUIDs

Every entry carries two GUIDs. The type GUID says what the partition is for and is a shared
constant, from `guid::types`. The unique GUID names one partition, is generated once, and is never
reused. Confusing the two is how an operating system comes to believe two partitions are the same
one.

`NIFE_BOOT` was ratified the day it was minted. A type GUID is a value two programs agree on, so
ratification is what turns a lane's proposal into a number the project has committed to. From then
on it changes the way `NIFE_DATA` would, which is not at all.

## Where to read

`span.rs` exists because the block service moves one 4096-byte filesystem block per request, while
a GPT counts 512-byte logical blocks. The transfer unit is 4096 because that is what RedoxFS reads,
and it keeps a mount's device round trips affordable. So reading LBA 1 means reading transfer block
0 and taking bytes 512 to 1024 of it. The 33 blocks of the backup table straddle a boundary that
depends on the disk's size.

## Miri and the sweeps

The exhaustive corruption sweeps skip under Miri. Each parse re-CRCs the 16 KiB entry array, and an
interpreter runs that polynomial around a thousand times slower than silicon. The header sweep takes
four seconds native and would take most of a day. Miri checks memory rules, not CRC completeness,
and the clean-fixture tests walk every code path the sweeps walk. So "Miri-clean" for this crate
means those paths.

## Regenerating the fixtures

Each fixture is the first 34 and last 33 blocks of a 64 MiB disk image with 512-byte blocks. That
is the whole primary table and the whole backup table, without the space between.
`tests/real_disks.rs` reads them.

```sh
# sgdisk (brew install gptfdisk)
dd if=/dev/zero of=gpt64.img bs=1m count=64
sgdisk -n 1:2048:+4M  -t 1:EF00 -c 1:"EFI system"   gpt64.img
sgdisk -n 2:0:+10M    -t 2:8300 -c 2:"linux root"   gpt64.img
sgdisk -n 3:0:0 -t 3:EC5CC08B-D749-4434-AC38-A274C50385BA -c 3:"cricker data" gpt64.img
sgdisk -U 1A2B3C4D-5E6F-4718-9A0B-C1D2E3F40506 gpt64.img
sgdisk -u 1:11111111-2222-4333-8444-555555555501 \
       -u 2:11111111-2222-4333-8444-555555555502 \
       -u 3:11111111-2222-4333-8444-555555555503 gpt64.img
dd if=gpt64.img of=sgdisk-64m.head bs=512 count=34
dd if=gpt64.img of=sgdisk-64m.tail bs=512 skip=131039 count=33

# macOS, no root needed for a disk image
dd if=/dev/zero of=apple64.img bs=1m count=64
hdiutil attach -nomount -imagekey diskimage-class=CRawDiskImage apple64.img   # prints /dev/diskN
diskutil partitionDisk /dev/diskN GPT MS-DOS ESP 16M HFS+ MACDATA 0b
hdiutil detach /dev/diskN
dd if=apple64.img of=apple-64m.head bs=512 count=34
dd if=apple64.img of=apple-64m.tail bs=512 skip=131039 count=33
```
