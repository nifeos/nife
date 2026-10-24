# `RENAME`: the two atomicities, the refusals, and our own checks

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, after the verb.*

An appendix to [notes/dir-capability.md](../dir-capability.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). The main page keeps the wire and the rights; this is the
rest of the verb's design.

## The two atomicities, stated apart (DECISIONS §42 (a filesystem declares what it offers))

§42's point is that saying "atomic" and letting the reader assume the stronger one is the mistake
POSIX made, so:

- Concurrency-atomic: yes. No observer sees the state where the name is in both places or
  neither. The reason is structural rather than a lock: the FS server's serve loop runs one request
  to completion before it receives the next, so inside the server there is no concurrent observer at
  all.
- Crash-atomic: yes, and measured. The whole rename runs inside one `fs.tx`, which reaches the
  platter through one commit in RedoxFS's header ring. That is a design claim until something cuts
  the power, so the rename is now the last operation of the workload in
  `redoxfs_server/tests/crash_consistency.rs`, whose sweep cuts the device at every write the workload
  makes and mounts what is left. A recovery holding the file under both names, or under neither, is
  a state that never existed and fails the sweep. Both names are in that test's `NAMES`, so a
  snapshot reads both and cannot miss either case.

## What is deliberately not offered, and why each refusal is loud

§42's operative rule is no silent degradation: a verb that is not offered fails loudly and the
application decides, and what it must never receive is an unsafe operation wearing a safe one's
name.

- `renameat2`'s `EXCHANGE` and `NOREPLACE`. They work on ext4, btrfs, XFS, f2fs and tmpfs and
  nowhere else, and emulating `NOREPLACE` with link-then-unlink is racy. Offering them would make
  behavior backend-specific, which §42 forbids.
- Cross-filesystem move. A different verb: copy-then-unlink, a different object with a different
  identity, non-atomic by nature. It cannot be reached through this verb anyway, because both
  handles are minted by one server bound to one image.
- Moving a *directory* between two directories is `EINVAL`; a directory renames in place fine.
  POSIX's guard against making a directory its own descendant is a path-prefix test, and this
  contract has no paths to take a prefix of. The equivalent here is an ancestry walk inside the
  server, which is real recursion in a process whose stack is measured at about three quarters used
  (§27). Renaming within one directory cannot create a cycle, because the node's parent does not
  change, and that is exactly where the line is drawn.

## Two checks that are ours rather than the engine's

RedoxFS's `rename_node` passes the *destination's* own type to `remove_node`, so it never compares
the two kinds: it will happily rename a file over a directory. POSIX will not, and §42 says an
offered verb means one thing on every backend, so the kind comparison is done at our boundary: file
over directory is `EISDIR`, directory over file is `ENOTDIR`. A non-empty destination directory is
the engine's `ENOTEMPTY`. Renaming a name onto itself is a successful no-op, which is POSIX's answer
and the safe one: the alternative removes the only link and then has nothing left to relink.

Both names go through `check_component`, so `..` means nothing here either.
