---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: 37
decision_dependencies: 34
machine_requirements: none
specific_machine: none
needs_person: no
---
# ext4 through rsext4, under milestone 37's crash injector

Raised 2026-10-31 (UTC) by lane `lane/981-ufs2-read-only-server`, from the ruling recorded on
#2927: RedoxFS stays the launch store, and the next measurement is ext4 through `rsext4`. If it
passes, one ext4 server may become both the launch store and the reader of Linux drives that
milestone 190 (ext4, read and write) wants. If it fails, RedoxFS stays.

Reuse: `rsext4` 0.9.2 (Starry-OS, Apache-2.0, `no_std`, depending only on `bitflags` and
`thiserror`), which writes ext4 with `metadata_csum` and carries a jbd2 journal. The injector, the
workload and the outcome classes of milestone 37 (Prove RedoxFS's crash consistency). e2fsprogs as
the host-side oracle under §136 (a mature foreign implementation earns its place as an oracle, not
as a dependency). The lane writes only the harness glue that puts `rsext4`
behind milestone 37's write log.

## What is already measured

On 2026-10-31 (UTC), on a Linux host with e2fsprogs 1.47.2, `rsext4` built unchanged for
`targets/{aarch64,riscv64,x86_64}-unknown-nife.json`. On images `mkfs.ext4` made, which carry
`metadata_csum`, it wrote a file, a directory, a symlink, a hard link, a rename and an extended
attribute, and `e2fsck -fn` exited 0 after each. It refuses the `encrypt` feature. Its journal was
not exercised.

## The measurement

1. Put `rsext4` behind milestone 37's write log. The injector records every block write the
   engine makes during the seven-operation workload, then replays a prefix with a power cut, a torn
   final write, or a write the device acknowledged and dropped. It takes a `redoxfs::Disk`
   today. The glue is a `rsext4` block device over the same log.
2. Run the three sweeps with the journal on. After each fault, Linux replays the journal (a
   kernel mount in a VM, or `e2fsck -fy`, which replays as the kernel does), and then `e2fsck -fn`
   judges the volume. Each fault point lands in milestone 37's buckets: a state that really existed,
   refused at mount or read, or silently wrong.
3. Run the negative control. With the journal off, the same sweeps must produce failures; if they
   do not, the injector does nothing.
4. Report against RedoxFS's numbers: 93 power-cut points and 372 torn points all recovering a
   real state, and 186 lying-device cases with 0 silently wrong.

## The fork this measurement will raise

ext4 checksums metadata, not file data. A dropped data write will read back as stale bytes, which
milestone 37 counts as silently wrong. The power-cut and torn sweeps can match RedoxFS. The
lying-device sweep cannot reach zero unless file data gains a checksum. Whether a nonzero
lying-device count still passes is the architect's to decide, with the numbers in hand.

## Done when

The three sweeps and the control run in CI on the host, a note records every bucket's count beside
RedoxFS's, and the fork above has a ruling.

## BUGS

- Nothing here runs `rsext4` under nife; the builds are link-only.
- `rsext4`'s repository was last pushed 2026-04-02, while crates.io carries releases through
  2026-09-21, so where its development happens is not yet established.
