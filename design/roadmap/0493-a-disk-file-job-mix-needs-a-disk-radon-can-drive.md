---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: a-disk-file-job-mix-needs-a-disk-radon-can-drive
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: riscv64 silicon with a disk
specific_machine: none
needs_person: yes
---
# 493. A disk-file job mix needs a disk radon can drive, and a file service that takes more than one client at a time

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `a-disk-file-job-mix-needs-a-disk-radon-can-drive`, filed 2026-09-19, on calef's
instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the
proposal's own, unedited except for this paragraph: the argument is its author's and promotion is
not the moment to improve it. Written by milestone 168 (a multi-tasking workload benchmark)'s lane, which was asked to add AIM7's disk-
file category as a second mix and found two blockers, neither of which is a job-mix change.

The first blocker is a block driver for radon. The second is
a change to how the file service shares memory with its clients, which is a wire agreement between
two programs and so is an architect's call rather than a lane's.

In brief. Milestone 168's mix now covers AIM7's compute, user-memory, pipe, page-mapping and process-creation
categories (see its block). The disk-file category is the one left, and the proposal that asked for it
(absorbed into 168's block on 2026-09-19) said it should be a second mix, so a run without a disk
stays comparable with one that has. That shape is still right. What stops it is below, checked
against the tree rather than assumed.

## What stops it, measured

**1. Nothing nife runs on radon can read a disk.** Every file-service path in the kernel starts at
`crate::virtual_io::find_block_device_n` (`kernel/src/user/fs_service.rs`, `wire_servers` and
`start_crash`), and radon has no virtio. The NVMe driver exists
(`kernel/src/user/non_volatile_memory_express_service.rs`) and its test self-skips on the board
because only QEMU attaches a controller (`notes/visionfive2.md`, the milestone 145 section). No SD
or eMMC driver exists. So a disk mix built today would run only under QEMU, and **no QEMU number is a
result** for this instrument (milestone 168's gate). It would be a job whose only output is fiction.

2. The file service shares one channel among all its clients. `fs_service::ensure` wires one
FS server with one contiguous file channel (`file_channel()`, `file_system_contract::file_system::TRANSFER_PAGES`
pages) and `spawn_fs_client` maps that **same** channel into every client. That is safe today because
each client is started one at a time by a test and every request is a blocking `CALL`. It is not
safe for 32 tasks issuing requests at once: task A writes its request payload into the shared page,
task B overwrites it before A's `CALL` is served. The job would measure a data race.

## The options

1. One FS server and one disk per task. No protocol change: `wire_servers` already knows how to
   build a second server on a second disk (`start_crash` does). Costs 32 disks and 32 servers at the
   top of the sweep, which measures 32 independent filesystems rather than contention on one, and
   AIM7's disk jobs contend on one filesystem. Cheap to build, wrong shape.
2. A channel per client on one server. The FS server accepts a per-client channel at connect
   (for example, a client grants its own frame on its first request), so N clients contend on one
   server and one disk, which is the AIM7 shape. Costs a `file_system_contract` change, which every
   file client in the tree speaks. This is the fork.
3. Serialize file jobs in the mix behind a lock held across the request. Refused: it turns the
   one category that should show kernel blocking under load into a queue the workload itself
   builds, and the number would measure the lock.

Recommendation: none, deliberately, because option 2 is a wire format and AGENTS.md
says an irreversible fork arrives with options rather than a winner. What can be said
is that option 1's lower cost is no argument for it: at equal cost it still loses, on the shape
reason above.

## Why it matters for fatal risk 4

`design/fatal-risks/README.md`'s risk 4 is decided by milestone 168's sweep on radon. Two of the three jobs
that block deep in a kernel path (map and spawn) are now in the mix; the file job is the third. A
flat curve without it is still evidence, weaker than a flat curve with it, and milestone 168's
block says so where the verdict will be read. Nothing here blocks the first bench evening.

## What is blocked until it is answered

- The disk-file mix itself, on (1) a radon block driver and (2) the file-service channel decision.
- Nothing else. Milestone 168's first bench evening runs the seven-job mix without a disk.

## Index row

Milestone 168's mix now covers AIM7's compute, user-memory, pipe, page-mapping and process-creation
categories (see its block).
