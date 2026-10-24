# What `system_installer` writes, and why it is safe to interrupt

*Name: provisional. After the program it describes.*

An appendix to [`notes/installing.md`](../installing.md). It holds the design behind
`components/src/system_installer.rs`. The slot states are in [`notes/boot-slots.md`](../boot-slots.md).

## The roles

`a0` picks the role.

| role | what it does |
|---|---|
| `ROLE_SURVEY` | read the table back and say whether this disk already carries nife |
| `ROLE_INSTALL` | draw the ids, write the table, lay out the EFI system partition, copy the boot file |
| `ROLE_CONFIRM` | mark the boot slot this machine started from successful, so a good upgrade sticks |

Survey and confirm hold no entropy endpoint. So the only table either can write is one it read, and
confirm changes the attribute bits `boot_slot` owns and nothing else.

## The layout an install writes

An install writes, in order:

1. a GPT with the nife data partition first, two boot slots, and an EFI system partition last;
2. a FAT32 volume in that partition;
3. the boot file at `\EFI\BOOT\BOOTX64.EFI`, where it is the chooser;
4. the same file raw into slot 0, marked good (`boot_slot::State::installed`).

Slot 1 is laid out empty, so an upgrade never repartitions. The slots are partitions because UEFI
gives GPT attribute bits 48 to 63 to the owner of the type GUID, and `types::NIFE_BOOT` is ours.

Data goes first because `redoxfs` finds its header within the disk's first 65,536 blocks, so a
whole-disk server mounts it. `redoxfs_server`'s `mkfs` makes the filesystem, not the installer.

The boot file is the one this machine booted from, mapped read-only by the kernel. It seals the
kernel and the archive together, so they cannot be installed apart.

## Bounding the installed filesystem server

The installed filesystem server holds the whole disk, bounded only by the extent `mkfs` recorded.
Bounding the server needs a base-block field on the `blk` wire, which is calef's call. The other way
is a caretaker, which costs one IPC per block. `mkfs`'s `PartitionDisk` is the bounded shape.

## How confirm finds the slot

`slot` is the index among this disk's `NIFE_BOOT` partitions, in table order. That is how
`uefi_loader`'s chooser counted them before writing the index onto the kernel's command line. The
installer counts the same way, and that is the whole agreement between the two. The chooser's
`the_disk_with_slots` collects boot-type partitions in index order, and so does `confirm`'s loop.

A number the installer cannot match is a refusal, never a nearest slot. Confirming the wrong entry
would mark an image good that never ran. That is worse than not confirming at all, because the
machine would then keep an upgrade that does not work instead of rolling back to one that does.

## Why confirm is safe to run twice

Setting a bit that is already set writes the same bytes, so a second run writes a second copy of
the same table. `confirm` also returns before writing anything when it finds the slot already
successful. That is the cheap half of idempotence, and a reader should not rely on it. The property
that matters is the byte-level one below.

## Why confirm is safe to interrupt

The bits it changes are nine, inside one `u64`, inside one 128-byte entry, inside one 512-byte
logical block. They are never split across two writes. So the entry array on the disk is always
either the old entry or the new one, never a third thing.

What can be interrupted is the four-write sequence that puts the array and the two headers back.
The order keeps at least one complete, self-consistent copy of the table on the disk at every
instant: the backup is finished before the primary is touched. The primary's header goes last, so a
half-written primary fails its own CRC rather than pointing at an array it does not describe.

The worst outcome of an interrupted confirmation is one of two states. Either the slot is not
confirmed, which is the state the machine was in a moment earlier and rolls back safely. Or the
primary table does not parse beside a backup that does. Nothing reads the backup in that case yet;
`uefi_loader::chooser`'s `BUGS` records it, and records that the chooser's own write goes
primary-first.

An already-good slot is not written at all. An installed machine boots its confirmed slot every day,
and a write on every one of those boots is a write that can be interrupted on every one of them.
