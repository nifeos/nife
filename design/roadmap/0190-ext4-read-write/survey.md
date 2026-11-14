# Milestone 190 appendix: the candidates for ext4, and why each lost

An appendix to [milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the
host-side oracle)](../0190-ext4-read-write.md), under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This is every candidate for reading and writing ext4, in any language, and the reason
each one other than `rsext4` lost, recorded so a later reader can disagree with an argument rather
than rediscover it.

| candidate | language | writes | writes the journal | `metadata_csum` on write | needs shared-memory threads | license |
|---|---|---|---|---|---|---|
| `rsext4` | Rust, `no_std` with `alloc` | yes | yes, jbd2 with checksum v3 | yes | no | Apache-2.0 |
| `ext4-view` | Rust, `no_std` | no, an explicit non-goal | n/a | n/a | no | MIT OR Apache-2.0 |
| `ext4plus` | Rust, `no_std` | yes | no, and it tells you to disable journaling | read-only | no | MIT OR Apache-2.0 |
| `libext2fs` (e2fsprogs) | C | yes, mature | no, replay only | yes | no | LGPLv2 |
| `lwext4` | C | yes | no | yes | no | BSD-3 except two GPLv2 files |
| LKL (Linux Kernel Library) | C | yes, the reference | yes, real jbd2 | yes | yes | GPLv2 |
| write our own | Rust | eventually | eventually | eventually | no | ours |

## Two facts decided most of it

`metadata_csum` is not optional. It is `mke2fs`'s default, so a drive Linux formatted carries
it. An implementation that reads it and cannot update it on write produces metadata Linux flags as
corrupt. That disqualifies a candidate for this requirement on a concrete ground, not on maturity.

Of the C implementations, only Linux writes the journal. libext2fs replays a journal it finds
and never writes one, and fuse2fs, its own read and write client, says so in its manual:

> Warning: fuse2fs does not support using the journal. There may be file system corruption or data
> loss if the file system is not gracefully unmounted.
>
> -- fuse2fs(1), https://man7.org/linux/man-pages/man1/fuse2fs.1.html

Its source has no transaction start or commit calls. Among the C candidates only LKL writes a real
jbd2 journal, because LKL is Linux. `rsext4` is the one Rust implementation that writes one, and
the main page records what Linux's replay of it measured.

## The refusals, each with its reason

`lwext4`, refused on license and on the seam. It is the obvious embedded C candidate, with its
own block-device abstraction and a need for no more than `malloc` and string functions, tier two
of the libc tiers of milestone 36 (a foreign-language component, seam first). But `ext4_extents.c` and
`ext4_xattr.c` are GPLv2, which makes the library GPLv2 as distributed, and extents are what make
ext4 ext4, so the GPL files cannot be dropped. `deny.toml`'s allow-list is permissive-only. Its
block-device abstraction is also function pointers the Rust side would implement: a callback into
Rust, which §31 (the foreign-language seam) rule 2 forbids. Either objection alone is enough.

`ext4plus`, refused on `metadata_csum`. It cannot update the checksums on write. Its README also
recommends ramdisks for writing and warns of known bugs, and it is pre-0.1.0.

`ext4-view`, refused as read-only by design. It is a good reader and does not write, which is
half of this requirement.

LKL, refused on the syscall surface. `lkl_host_operations` requires `thread_create`,
`thread_join`, `thread_exit`, `thread_self`, `thread_stack`, four TLS operations, semaphores and
recursive mutexes, because LKL is a kernel linked into your address space with kthreads, workqueues
and softirq contexts. nife cannot provide that: `ThreadControlBlock::CONFIGURE` consumes the
address-space capability it binds, so no two thread control blocks share an address space, which
§105 (`std::thread::spawn` stays declined, until a customer needs it) records. Building shared
address spaces because a filesystem wants them would change the syscall surface sideways.

Three further costs. The rest of the host-operations surface: `jmp_buf_set` and `longjmp` in
assembly for three ISAs, a timer service with callbacks, and `ioremap` and `iomem_access` for a
virtio-mmio shim onto the block server. GPLv2, and therefore vendoring Linux. And `arch/lkl`'s
documented hosts are POSIX and Windows userspace, so nobody has shown that it builds for a
freestanding non-x86 target at all, and §19 (architectural parity is a tenet) requires all three.

This refusal is not permanent. If shared-memory threading is ever built because it is right on its
own merits, LKL becomes a lane that unlocks far more than ext4. The refusal concerns what may force
that decision, not the destination.

libext2fs as a shipped dependency, refused. The case for it is an argument from effort, and
under the tenet's test, *would we still choose it if both options cost the same?*, it loses. A Rust
implementation wins on memory safety in a parser eating bytes from drives we did not write. It
wins on being host-testable and reachable by Kani, on needing no libc tier, on carrying no LGPL
into the image, and on parity across three ISAs by construction. §83 (when the same thing exists in
C and in Rust, take the Rust one) says so directly. The objection of §34 (RedoxFS is the primary
filesystem, on three conditions) to littlefs applies too: it would put a foreign component in the
storage path. Its value is as the oracle, under §136 (a mature foreign implementation earns its
place as an oracle, not as a dependency).

Writing our own, refused. §46 (thin primitives or whole subsystems; we write everything in
between) rule 4 prefers depending where correctness is won by exposure rather than by reading a
specification, and a filesystem's on-disk format is the exposure case. Its 2026-10-04 amendment
makes taking the default for file formats. A young implementation of a hostile-input parser with
nothing to check it against is not trustworthy in any language, and `rsext4` has had exposure ours
would not.

## Prior art, read rather than recalled

libext2fs has already run as a filesystem server on a capability microkernel. Paul Boddie
integrated it into L4Re: a filesystem server program, per-file resource objects, IPC between clients
and the server, and a custom `io_manager` whose read and write functions issue IPC calls to a block
server. That is the `ext4_server` shape milestone 140 (mount a drive this system did not create)
chose, built on a system in the same family. He deferred the C library work as too much of a
challenge at that stage, with L4Re's uclibc available; nife has no such libc, which is another
reason the C stays on the host.

The `io_manager` seam is 17 function pointers, of which a real backend implements about six
(`open`, `close`, `set_blksize`, `read_blk64`, `write_blk64`, `flush`). It runs off POSIX upstream:
on Windows `windows_io_manager` replaces `unix_io_manager`, so the abstraction carries load.

A borg repository is itself a transaction log, which is why a gap between journal commits costs
this workload little. Segment files are append-only, written sequentially, about 500 MB each. Every
log entry carries a CRC32, a `COMMIT` tag ends a transaction, and borg discards an incomplete one
when it reopens the repository. `borg check` verifies every chunk cryptographically.

Sources:

- fuse2fs(1): https://man7.org/linux/man-pages/man1/fuse2fs.1.html
- fuse2fs source: https://github.com/tytso/e2fsprogs/blob/master/misc/fuse2fs.c
- `struct io_manager`: https://raw.githubusercontent.com/tytso/e2fsprogs/master/lib/ext2fs/ext2_io.h
- libext2fs on L4Re: https://blogs.fsfe.org/pboddie/?p=2457
- The EXT2FS Library manual: http://fs.csl.utoronto.ca/~sunk/libext2fs.html
- `rsext4`: https://github.com/rcore-os/tgoskits/tree/dev/fs/rsext4
- `ext4-view`: https://github.com/nicholasbishop/ext4-view-rs
- `ext4plus`: https://lib.rs/crates/ext4plus
- `lwext4`: https://github.com/gkostka/lwext4
- LKL `host_ops.h`: https://raw.githubusercontent.com/lkl/linux/master/arch/lkl/include/uapi/asm/host_ops.h
- borg data structures: https://borgbackup.readthedocs.io/en/stable/internals/data-structures.html
