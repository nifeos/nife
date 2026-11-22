# Upstream item 2: the truncate fix

Appendix to [milestone 190](../../0190-ext4-read-write.md), upstream item 2. Name provisional.

This is a draft. Nobody has filed anything here. The architect reviews the exact text below, and
filing it is a separate step.

- Repository and branch: `rcore-os/tgoskits`, `dev`.
- Base commit: `6e80b940d46167a59a795e158bb17a7eb28333af` ("feat(ivc): add fragmented Message V1
  full-duplex transport (#2068)"). No commit after it on `dev` touches `fs/rsext4`.
- The change: [`2-truncate-fix.patch`](2-truncate-fix.patch), two commits, `git format-patch`
  output against that base.

## Title

```
fix(rsext4): make a depth-2 truncate succeed and keep the file intact when it fails
```

## Body (Chinese, as filed)

```markdown
## 问题

`truncate_inode` 在缩小文件时有两个问题。

1. 当盘区树深度为 2，而新的文件末尾位于某个一级索引节点覆盖的第一个块之前时，缩小操作返回
   `Corrupted { op: "extent:empty_index" }`。`remove_extent` 的第二阶段删除该索引节点下的全部叶子之后，
   仍然把这个已经没有条目的索引节点写回磁盘；`write_node_to_block` 校验节点时拒绝条目数为 0 的索引节点，
   于是整个缩小操作失败。用 1,500 个单块盘区（每隔一个块写一个块）建出根节点含两个一级索引节点的树，
   再缩小到 100 字节，即可稳定复现。
2. 缩小失败时文件内容被改动。缩小操作先把保留下来的末尾块中新文件末尾之后的字节清零（与 Linux 的顺序一致），
   再提交元数据事务。事务失败时元数据回滚，文件保持原来的大小，但被清零的字节不在回滚范围内，
   因此一次失败的 truncate 会丢失从新文件末尾到该块末尾的数据。

## 改动

两个提交，各自独立：

1. `fix(rsext4): free an emptied extent index node instead of writing it back`
   在 `remove.rs` 的索引分支中，外部索引节点删除最后一个子节点后不再写回。
   父节点看到 `empty` 后会立即删除并释放这个块，所以写回本身没有意义，而且写出的是非法节点。
   根节点没有自己的块，保持原有处理：第三阶段把空的根变成空的叶子。
2. `fix(rsext4): restore the zeroed EOF tail when a single-transaction truncate fails`
   `zero_mapped_inode_tail_with_undo` 在清零时保存被替换的字节，返回 `ZeroedTail`。
   盘区路径和传统间接块路径在单事务提交失败时，通过 `restore_tail_on_error` 把这些字节写回数据块缓存。
   原来的 `zero_mapped_inode_tail` 保留为不需要撤销的调用方的包装。

## 每一步的理由

- 不写回空索引节点，而不是放宽校验：条目数为 0 的外部索引节点在 ext4 中是非法的，
  `e2fsck` 也会报告，校验是对的；错误在于写出它。父节点已经负责释放该块，跳过写回不会遗漏任何持久化。
- 保留“先清零再解除映射”的顺序，用撤销副本实现原子性，而不是改成提交之后再清零：
  这个顺序与 Linux 一致，原有注释也说明它让损坏的隐藏分支在修改任何数据之前就在预检阶段失败。
  改变顺序会改变这两点，所以选择在失败时恢复。代价是每次缩小多复制最多一个块的字节。
- 只在单事务路径恢复：需要重新启动的缩小操作分多个事务提交，较早的事务可能已经持久化，
  此时文件大小已经变化，恢复旧字节反而会把已经超出文件末尾的数据写回。这条路径保持原有行为，
  代码中有注释说明。
- 恢复本身失败时返回缩小操作自己的错误：用恢复的错误替换它会隐藏缩小失败的原因。
  这一点是取舍，如果维护者希望两种错误都能看到，可以改成别的报告方式。

## 验证

- 新增两个测试，位于 `fs/rsext4/tests/file_operations.rs`：
  `shrinking_extent_inode_past_a_whole_index_node_frees_that_node` 建出深度为 2 的树并缩小到 100 字节，
  要求操作成功、数据正确、释放的块多于数据块数，并在卸载、重新挂载后再次读取；
  `failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block` 让 inode 写入失败，
  要求 truncate 返回 `Io`，并且文件的每个字节都与原来相同。
- 两个测试在去掉对应修复时都失败：第一个返回 `Corrupted { op: "extent:empty_index" }`，
  第二个在第 100 个字节处发现差异；加上修复后都通过。
- 在一个由 `mkfs.ext4` 创建的镜像上，对深度为 2 的文件，以每个一级索引节点的起始块前后各两个块加
  100 字节为目标逐一缩小，修复后每次都成功，卸载后 `e2fsck -fn` 返回 0。
- `cargo fmt`、`cargo xtask clippy --package rsext4` 通过。`cargo xtask test --since 6e80b940d`
  中，除 `fs/rsext4/tests/filesystem_growth.rs` 的两个测试和 ax-fs-ng 的
  `root_growth_preserves_other_partitions_and_ext4_contents` 之外全部通过；这三个测试在基准提交上、
  不加本改动时以同样方式失败，与本改动无关（本改动不涉及文件系统扩容）。

## 未验证的内容

- 传统间接块路径的恢复没有单独的测试：它与盘区路径使用同一个辅助函数，失败注入方式相同。
  如果需要，可以补一个对应测试。
- 需要重新启动的多事务缩小在失败时仍会留下被清零的尾部，原因见上文。
- 没有运行 QEMU 或板卡测试：改动只影响 rsext4 的宿主可测行为，没有运行时启动路径变化。
```

## Body (English rendering, for review)

```markdown
## Problem

`truncate_inode` has two problems when it shrinks a file.

1. When the extent tree has depth 2 and the new end of file lies before the first block a level-1
   index node covers, the shrink returns `Corrupted { op: "extent:empty_index" }`. After phase 2 of
   `remove_extent` deletes every leaf under that index node, it still writes the index node, now
   with no entries, back to disk. `write_node_to_block` validates the node and refuses an index
   node with 0 entries, so the whole shrink fails. A tree built from 1,500 single-block extents
   (one block written in every two), whose root holds two level-1 index nodes, shrunk to 100
   bytes, reproduces it every time.
2. A failed shrink changes the file's contents. The shrink first zeroes the bytes after the new
   end of file in the block it keeps (the same order as Linux), then commits the metadata
   transaction. When the transaction fails, the metadata rolls back and the file keeps its old
   size, but the zeroed bytes are outside the rollback, so a failed truncate loses the data from
   the new end of file to the end of that block.

## Change

Two commits, each standing alone:

1. `fix(rsext4): free an emptied extent index node instead of writing it back`
   In the index branch of `remove.rs`, an external index node whose last child was removed is no
   longer written back. The parent removes and frees this block as soon as it sees `empty`, so
   the write served no purpose, and what it wrote was an invalid node. The root has no block of
   its own and keeps its existing handling: phase 3 turns an empty root into an empty leaf.
2. `fix(rsext4): restore the zeroed EOF tail when a single-transaction truncate fails`
   `zero_mapped_inode_tail_with_undo` saves the bytes it replaces while zeroing and returns a
   `ZeroedTail`. When the single-transaction commit fails, the extent path and the legacy
   indirect-block path write those bytes back into the data block cache through
   `restore_tail_on_error`. The original `zero_mapped_inode_tail` stays, as a wrapper for callers
   that need no undo.

## The reasoning of each step

- Not writing back the empty index node, rather than relaxing the validation: an external index
  node with 0 entries is invalid in ext4, and `e2fsck` reports it too, so the validation is right
  and the mistake is writing such a node. The parent already frees the block, so skipping the
  write loses nothing that had to be made durable.
- Keeping "zero, then detach mappings" and getting atomicity from an undo copy, rather than
  zeroing after the commit: the order matches Linux, and the existing comment says it lets a
  corrupt hidden branch fail in the preflight before any data is changed. Changing the order
  would change both, so the fix restores on failure instead. The cost is copying at most one
  block's bytes per shrink.
- Restoring only on the single-transaction path: a restarted shrink commits in several
  transactions, and an earlier one may already be durable. The file's size has then changed, and
  restoring the old bytes would write back data that already lies past the end of file. That path
  keeps its existing behavior, with a comment in the code saying why.
- When the restore itself fails, the shrink's own error is returned: replacing it with the
  restore's error would hide why the shrink failed. This is a trade-off; if the maintainers want
  both errors visible, the reporting can change.

## Verification

- Two new tests, in `fs/rsext4/tests/file_operations.rs`.
  `shrinking_extent_inode_past_a_whole_index_node_frees_that_node` builds a depth-2 tree and
  shrinks it to 100 bytes. It requires the shrink to succeed, the data to be correct and more
  blocks to be freed than there were data blocks, then reads the file again after an unmount and
  a remount. `failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block` makes the
  inode write fail, and requires the truncate to return `Io` and every byte of the file to equal
  what it was.
- Both tests fail without their fix: the first returns `Corrupted { op: "extent:empty_index" }`,
  and the second finds a difference at byte 100. With the fix both pass.
- On an image `mkfs.ext4` made, a depth-2 file was shrunk to each target in turn: two blocks on
  either side of the first block of every level-1 index node, plus 100 bytes. With the fix every
  shrink succeeds, and `e2fsck -fn` returns 0 after the unmount.
- `cargo fmt` and `cargo xtask clippy --package rsext4` pass. `cargo xtask test --since
  6e80b940d` passes except for two tests in `fs/rsext4/tests/filesystem_growth.rs` and ax-fs-ng's
  `root_growth_preserves_other_partitions_and_ext4_contents`. Those three fail the same way at the
  base commit without this change, and are unrelated to it (it does not touch filesystem growth).

## Not verified

- The restore on the legacy indirect-block path has no test of its own: it uses the same helper as
  the extent path, with the same failure injection. A matching test can be added if wanted.
- A restarted, multi-transaction shrink still leaves its tail zeroed when it fails, for the reason
  above.
- No QEMU or board test was run: the change affects only rsext4 behavior that host tests reach,
  and changes no runtime boot path.
```

## Test evidence

Run on the base commit above with the two commits applied, on an x86_64 Linux host with
e2fsprogs 1.47.2. Each test ran with its own commit's source change reverted (red), then as
committed (green), through `cargo test -q -p rsext4 --features host-test --test file_operations
<test>`, the profile `cargo xtask test` runs for rsext4:

```
== commit 1 (9df283fad) with remove.rs from the base
thread 'file_functional_tests::shrinking_extent_inode_past_a_whole_index_node_frees_that_node' panicked at fs/rsext4/tests/file_operations.rs:2056:14:
shrinking a depth-2 tree must succeed: Ext4Error { kind: Corrupted, context: Some(Operation { op: "extent:empty_index" }) }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 49 filtered out; finished in 2.34s
== commit 1 (9df283fad) as committed
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out; finished in 2.74s
== commit 2 (cc66f1115) with zero.rs and resize.rs from commit 1
thread 'file_functional_tests::failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block' panicked at fs/rsext4/tests/file_operations.rs:2101:9:
assertion `left == right` failed: a failed truncate must leave every byte of the file as it was
  left: Some(100)
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 50 filtered out; finished in 0.06s
== commit 2 (cc66f1115) as committed
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 50 filtered out; finished in 0.08s
```

`cargo xtask clippy --package rsext4`: `passed checks: 3, failed checks: 0, skipped checks: 0`.
`cargo fmt --check -p rsext4`: clean.

`cargo xtask test --since 6e80b940d` selected 20 packages and exited 1. rsext4 passed 544 tests
and failed 2, both in `tests/filesystem_growth.rs`
(`growth_preserves_files_and_exposes_new_groups_to_linux` and
`growth_recovers_commit_checkpoint_and_flush_interruptions`). ax-fs-ng failed one,
`root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents`, in two feature
profiles. All three fail the same way at the base commit without these changes, so they predate
the patch; they test filesystem growth, which the patch does not touch.
Every other selected package passed.

The depth-2 sweep in the verification list ran from a scratch test outside the patch, on images
`mkfs.ext4 -b 4096` made; it is not part of the submission.
