# The kernel's file-service wiring

This appendix to [`notes/fs-server.md`](../fs-server.md) carries the design behind
`kernel/src/user/fs_service/`. That code wires the block server, the FS server, the caretakers and
their clients, for the test suite and the boot.

## A grant is three processes and one frame

A per-file grant is what `run wc report.txt` resolves to, and the suite wires it the way the shell
would:

```text
  FS server ──file IPC──► fs_file_caretaker ──narrowed file IPC──► the confined program
               (a directory)          (one file, one direction)
```

The caretaker holds the directory capability. The program holds an endpoint to the caretaker and
nothing that names the FS server. So "it cannot reach a second file" is a fact of its capability
table, not a check it is trusted to pass. A directory grant has the same shape with
`fs_subtree_caretaker`:

```text
  FS server ──file IPC──► fs_subtree_caretaker ──narrowed file IPC──► the confined program
           (the image root)         (one subtree, one rights set)
```

A set grant uses `fs_nameset_caretaker`, which answers only for the names in a set:

```text
  FS server ──file IPC──► fs_nameset_caretaker ──narrowed file IPC──► the confined program
           (the image root)         (one directory, and only the names in the set)
```

All three processes share one frame. Every request on both hops is a blocking `CALL`. So the client
is parked in its own call while the caretaker uses the page. A second frame would
buy a copy and no isolation, since the client is entitled to the bytes either way.

That argument fails at startup, and the ordering in `wait_for_caretaker` is the fix. A caretaker
stages its granted name in the shared page and then blocks in a `CALL` to the FS server. When the
same call wired the service, the FS server is still parked in its readiness `SEND`. That `CALL`
waits until the sentinel is drained. A client that already existed had that whole window to write
its own first name over the page, and the caretaker, which will not serve a hole, would die and
leave its client blocked forever. So the wiring drains the service, waits for the caretaker's own
sentinel, and only then spawns the program.

A set does not fit in `START` registers, so the set grant encodes it into a frame of its own,
mapped read-only into the caretaker before it is spawned. No client can reach that frame, so it
needs none of the ordering above. `ARG_MAX` returns here as the size of a capability, bounded by
`filesystem_contract::nameset::MAX_NAMES` at both ends.

## Two directories for one program

`start_granted_two_dirs` starts two subtree caretakers for one program:

```text
  FS server ──file IPC──► fs_subtree_caretaker A ──narrowed file IPC──►
           (the image root)      (subtree A, one rights set)          the confined program
                      └────────► fs_subtree_caretaker B ──narrowed file IPC──►  (slot 0: A
                                 (subtree B, one rights set)                     slot 1: B
                                                                                  slot 2: report)
```

Which directory is which is the capability table slot: slot 0 is always the first grant, slot 1
the second, and slot 2 the report. How the shell
would encode a second directory grant to the progenitor is left open for whoever wires this into
the shell. Both caretakers map the same file-channel frame. That is safe, because the program is one
thread with at most one `CALL` in flight.

A pair is built once and reused. A caretaker never exits, so every pair holds its regions for the
boot. The suite's region table is a fixed 256, and it already runs close to that. A caretaker keeps
no state between clients, so a second client of the same pair holds the same grant.

## Stacks a client needs

`spawn_fs_client` maps up to eight extra stack pages. The hand-written `fs_test_client` roles need
none. A shell carries a path stack, a parsed path and a listing buffer by value, and with no extra
pages it overflowed by 192 bytes. Each pipeline stage's endowment can carry a whole `FileGrant`, and
four pages then overflowed by 48 bytes. Both presented as a data abort on the client's `sp`, then
the lost-wakeup watchdog. The test was still waiting for a report from a dead process. The
shell's stack is sized by the largest value `grant_plan` hands it, so a field added there can be a
page needed here.

## The disk the FS server mounts

The FS server serves the second virtio block device. A machine with no virtio block device at all
is an installed system. There the FS server mounts the NVMe disk, through the server
`non_volatile_memory_express_service` already runs. It speaks the same `blk` contract with the
same transfer geometry. The test for that arm is device 0, not device 1. `cargo xtask uefi-test`
attaches one virtio disk and an NVMe controller. There, "there is no device 1" would hand the FS
server a disk with no filesystem on it. The NVMe arm also requires that this boot came from that disk.
RedoxFS writes on mount, and a stick booted on an installed machine changed 3 blocks of it.

The FS server is given the whole NVMe disk, not the nife data partition. That works because
`redoxfs`'s `FileSystem::open` scans blocks `0..65536` for its header and takes the block it finds
as the filesystem's origin, which is why `components/src/system_installer.rs` puts the data
partition first. The cost is that the server can address the EFI system partition and the partition
table. A partition-bounded mount needs a base-block field on the `blk` wire or a program in the
middle, and the installer's `BUGS` carries the argument.

## Client windows and the server's mapping list

Every client window is in one physically contiguous run. So one `PageFrame` capability names the
whole pool, and the progenitor slices a window per client (`abi::page_frame::SLICE`). The main FS server
maps every window; a client's badge picks which one it reads.

With every window mapped, a server's list of `Mapping`s is about 5.6 KiB, and on a thread's stack it
took the spawn closure's frame to 6.7 KiB, over the 4 KiB guard page `script/stack-frame-check`
holds every kernel frame under. The kernel has no heap, so the lists live in a static, one per FS
server slot, each claimed once by one server thread.
