# Upstream item 2a: free an emptied extent index node

Appendix to [milestone 190](../../0190-ext4-read-write.md), upstream item 2, first of its two
submissions (bug 2's `extent:empty_index` failure). Name provisional.

Both submissions are upstream, each with the title and Chinese body below: the Issue as
[rcore-os/tgoskits#2826](https://github.com/rcore-os/tgoskits/issues/2826) and the pull request as
[rcore-os/tgoskits#2827](https://github.com/rcore-os/tgoskits/pull/2827), whose one commit is the
patch beside this file.

- Repository and branch: `rcore-os/tgoskits`, `dev`.
- Base commit: `0353572d1e2cca79e25a2805deeff4521449a5c1` ("fix(x86_vlapic): stop the APIC timer
  without yielding in the vCPU entry window (#2819)"), the head of `dev` at filing. The patch beside
  this file dates from the earlier base `6e80b940d` and applies to this one unchanged.
- The change: [`2a-free-empty-extent-index.patch`](2a-free-empty-extent-index.patch), one commit,
  `git format-patch` output. It applies to the base alone. The companion [item
  2b](2b-restore-truncate-tail.md) adds its test at the same place in
  `fs/rsext4/tests/file_operations.rs`, so 2b went upstream after this one and rebases on it once
  this one lands.
- Issue settings, which upstream's contributing guide (section 3.7) requires in the sidebar:
  Assignees `calef`, labels `bug` and `q4`, Type `Bug`, Milestone `2026Q4`. GitHub dropped these at
  creation, because the filing account has no triage permission on the repository; the guide asks a
  maintainer to fill them in.

## Issue (filed)

### Title

```
rsext4: truncate of a depth-2 extent tree fails with extent:empty_index
```

### Body (Chinese, as filed)

```markdown
## 当前问题

`truncate_inode` 缩小一个盘区树深度为 2 的文件时，如果新的文件末尾位于某个一级索引节点覆盖的第一个块之前，
操作返回 `Ext4Error { kind: Corrupted, context: Some(Operation { op: "extent:empty_index" }) }`，文件无法缩小。

- 代码版本：`dev` 分支 `6e80b940d`；crates.io 上的 rsext4 0.9.2 同样复现。
- 环境：x86_64 Linux 宿主，`rust-toolchain.toml` 指定的 nightly，`cargo test -p rsext4 --features host-test`
  使用的内存块设备。
- 复现步骤：在新建的文件系统上创建一个文件，每隔一个块写入一个块，共写 1500 个块。每个块成为一个单块盘区，
  树的深度为 2，根节点含两个一级索引节点。然后调用 `truncate_inode` 把文件缩小到 100 字节。
- 实际结果：每次都返回上面的错误。期望结果：缩小成功，文件剩下 100 字节，第二个索引节点及其叶子被释放。

已有排查：`remove_extent` 的第二阶段删除某个外部索引节点下的全部叶子之后，仍把这个已经没有条目的索引节点写回磁盘。
`write_node_to_block` 校验节点时拒绝条目数为 0 的索引节点，于是整个删除失败。父节点在看到子节点为空时本来就会删除并释放它。

## 修改方案

交付范围：在 `fs/rsext4/src/extents_tree/remove.rs` 的索引分支中，外部索引节点删除最后一个子节点后不再写回，
由父节点照常删除并释放该块。根节点没有自己的块，保持原有处理（第三阶段把空的根变成空的叶子）。
不放宽校验。ext4 格式文档（4.2.3 Extent Tree）只把 `eh_entries` 定义为头部之后有效条目的个数，没有说明索引节点能否没有条目；
依据是 `e2fsck` 的检查：一级索引节点的 `eh_entries` 为 0 时，e2fsprogs 1.47.2 的 `e2fsck -fn` 报告
`Failed to iterate extents in inode 12 (op EXT2_EXTENT_DOWN, blk 2070, lblk 0): No 'down' extent`。

不包含：截断失败后被清零的文件尾部（另一个 Issue），以及盘区插入的其他问题。

依赖与兼容性：无新依赖；不改变磁盘格式，只是不再写出一个非法节点。负责人：@calef。期望季度：2026Q4。

## 测试验证

验收条件：在 `fs/rsext4/tests/file_operations.rs` 新增一个测试，按上面的步骤建出深度为 2 的树并缩小到 100 字节，
要求操作成功、内容正确、释放的块多于数据块数，并在卸载、重新挂载后再次读取相同内容。
去掉修复时该测试失败并报告 `extent:empty_index`，加上修复后通过。
`cargo xtask clippy --package rsext4`、`cargo fmt` 与 `cargo xtask test --since <基准>` 中的 rsext4 测试通过。
```

### Body (English rendering, for review)

```markdown
## Current problem

When `truncate_inode` shrinks a file whose extent tree has depth 2, and the new end of file lies
before the first block a level-1 index node covers, it returns
`Ext4Error { kind: Corrupted, context: Some(Operation { op: "extent:empty_index" }) }` and the file
does not shrink.

- Code version: `dev` at `6e80b940d`; rsext4 0.9.2 on crates.io reproduces it too.
- Environment: an x86_64 Linux host, the nightly `rust-toolchain.toml` names, and the in-memory
  block device `cargo test -p rsext4 --features host-test` uses.
- Steps: on a new filesystem, create a file and write one block in every two, 1500 blocks in all.
  Each block becomes a single-block extent, and the tree has depth 2 with two level-1 index nodes
  in its root. Then call `truncate_inode` to shrink the file to 100 bytes.
- Actual result: the error above, every time. Expected: the shrink succeeds, the file keeps 100
  bytes, and the second index node and its leaves are freed.

Investigation so far: after phase 2 of `remove_extent` deletes every leaf under an external index
node, it still writes that node, now with no entries, back to disk. `write_node_to_block`
validates the node and refuses an index node with 0 entries, so the whole removal fails. The
parent already deletes and frees a child it sees as empty.

## Proposed change

Scope: in the index branch of `fs/rsext4/src/extents_tree/remove.rs`, an external index node
whose last child was removed is no longer written back, and the parent removes and frees its block
as before. The root has no block of its own and keeps its existing handling (phase 3 turns an
empty root into an empty leaf). The validation stays. The ext4 format documentation (4.2.3 Extent Tree)
defines `eh_entries` only as the number of valid entries following the header, and it does not say
whether an index node may have none. The authority is `e2fsck`: when a level-1 index node has
`eh_entries` set to 0, `e2fsck -fn` from e2fsprogs 1.47.2 reports
`Failed to iterate extents in inode 12 (op EXT2_EXTENT_DOWN, blk 2070, lblk 0): No 'down' extent`.

Out of scope: the file tail a failed truncate leaves zeroed (a separate Issue), and other extent
insertion problems.

Dependencies and compatibility: no new dependency; the on-disk format does not change, and the
only difference is that an invalid node is no longer written. Owner: @calef. Target quarter:
2026Q4.

## Test plan

Acceptance: a new test in `fs/rsext4/tests/file_operations.rs` builds the depth-2 tree as above
and shrinks it to 100 bytes. It requires the shrink to succeed, the contents to be correct and
more blocks to be freed than there were data blocks, and reads the same contents again after an
unmount and a remount. Without the fix the test fails with `extent:empty_index`; with it the test
passes. `cargo xtask clippy --package rsext4`, `cargo fmt` and the rsext4 tests in
`cargo xtask test --since <base>` pass.
```

## Pull request (filed)

### Title

```
fix(rsext4): free an emptied extent index node instead of writing it back
```

### Body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#2826

`truncate_inode` 缩小盘区树深度为 2 的文件时，如果新的文件末尾位于某个一级索引节点覆盖的第一个块之前，
返回 `Corrupted { op: "extent:empty_index" }`。`remove_extent` 的第二阶段删除该索引节点下的全部叶子之后，
仍然把这个已经没有条目的索引节点写回磁盘；`write_node_to_block` 校验节点时拒绝条目数为 0 的索引节点，
于是整个缩小操作失败。本 PR 只修复这一点；截断失败后被清零的尾部由另一个 PR 处理。

## 修改方案

一个提交，只改 `fs/rsext4/src/extents_tree/remove.rs` 的索引分支：外部索引节点删除最后一个子节点后不再写回。

- 为什么不写回而不是放宽校验：ext4 格式文档（4.2.3 Extent Tree）没有说明索引节点能否没有条目，
  依据是 `e2fsck` 的检查：一级索引节点的 `eh_entries` 为 0 时，e2fsprogs 1.47.2 的 `e2fsck -fn` 报告
  `Failed to iterate extents in inode 12 (op EXT2_EXTENT_DOWN, blk 2070, lblk 0): No 'down' extent`。校验是对的，错误在于写出它。
- 为什么跳过写回是安全的：父节点看到 `empty` 后会立即删除并释放这个块，写回的内容随后就被丢弃，
  跳过它不会遗漏任何需要持久化的数据。
- 根节点没有自己的块，保持原有处理：第三阶段把空的根变成空的叶子。

兼容性：不改变磁盘格式，也不改变任何公开接口；受影响的只有删除盘区时会清空外部索引节点的情况。

## 测试验证

环境：x86_64 Linux 宿主，`rust-toolchain.toml` 指定的 nightly，e2fsprogs 1.47.2。

- 新增测试 `shrinking_extent_inode_past_a_whole_index_node_frees_that_node`
  （`fs/rsext4/tests/file_operations.rs`）：每隔一个块写一个块，共 1500 个，建出根节点含两个一级索引节点、
  深度为 2 的树，再缩小到 100 字节；要求操作成功、数据正确、释放的块多于数据块数，并在卸载、重新挂载后再次读取。
- `cargo test -p rsext4 --features host-test --test file_operations shrinking_extent_inode_past`：
  去掉本修复时失败，`shrinking a depth-2 tree must succeed: Ext4Error { kind: Corrupted, context: Some(Operation { op: "extent:empty_index" }) }`；
  加上修复后通过。
- `cargo fmt`、`cargo xtask clippy --package rsext4` 通过。
- `cargo xtask test --since 0353572d1`：rsext4 只有 `tests/filesystem_growth.rs` 中的两个测试失败，ax-fs-ng 的
  `root_growth_preserves_other_partitions_and_ext4_contents` 失败，其余全部通过。这三个测试在基准提交上同样失败，
  原因是宿主的 `mke2fs` 默认启用 `orphan_file`，而扩容拒绝该特性（`fs/rsext4/src/ext4/grow/layout.rs` 的特性检查），
  与本改动无关。

未验证：没有运行 QEMU 或板卡测试，改动只影响宿主测试可以覆盖的 rsext4 行为，不涉及启动路径。
```

### Body (English rendering, for review)

```markdown
## Current problem

Linked Issue: #<to be filled>

When `truncate_inode` shrinks a file whose extent tree has depth 2, and the new end of file lies
before the first block a level-1 index node covers, it returns
`Corrupted { op: "extent:empty_index" }`. After phase 2 of `remove_extent` deletes every leaf
under that index node, it still writes the index node, now with no entries, back to disk.
`write_node_to_block` validates the node and refuses an index node with 0 entries, so the whole
shrink fails. This pull request fixes only that; another pull request handles the tail a failed
truncate leaves zeroed.

## Change

One commit, in the index branch of `fs/rsext4/src/extents_tree/remove.rs` alone: an external
index node whose last child was removed is no longer written back.

- Why skip the write rather than relax the validation: the ext4 format documentation (4.2.3
  Extent Tree) does not say whether an index node may have no entries, so the authority is
  `e2fsck`. When a level-1 index node has `eh_entries` set to 0, `e2fsck -fn` from e2fsprogs
  1.47.2 reports `Failed to iterate extents in inode 12 (op EXT2_EXTENT_DOWN, blk 2070, lblk 0): No 'down' extent`. The validation is right, and the mistake is writing such a node.
- Why skipping the write is safe: the parent removes and frees this block as soon as it sees
  `empty`, so whatever the write stored is discarded at once, and skipping it loses nothing that
  had to be made durable.
- The root has no block of its own and keeps its existing handling: phase 3 turns an empty root
  into an empty leaf.

Compatibility: the on-disk format and every public interface stay the same; the change affects
only removals that empty an external index node.

## Test results

Environment: an x86_64 Linux host, the nightly `rust-toolchain.toml` names, e2fsprogs 1.47.2.

- New test `shrinking_extent_inode_past_a_whole_index_node_frees_that_node`
  (`fs/rsext4/tests/file_operations.rs`): it writes one block in every two, 1500 in all, which
  builds a depth-2 tree with two level-1 index nodes in its root, and shrinks it to 100 bytes. It
  requires the shrink to succeed, the data to be correct and more blocks to be freed than there
  were data blocks, then reads the file again after an unmount and a remount.
- `cargo test -p rsext4 --features host-test --test file_operations shrinking_extent_inode_past`:
  without this fix it fails with `shrinking a depth-2 tree must succeed: Ext4Error { kind:
  Corrupted, context: Some(Operation { op: "extent:empty_index" }) }`; with it, it passes.
- `cargo fmt` and `cargo xtask clippy --package rsext4` pass.
- `cargo xtask test --since 0353572d1`: in rsext4 only two tests in `tests/filesystem_growth.rs`
  fail, plus ax-fs-ng's `root_growth_preserves_other_partitions_and_ext4_contents`; everything
  else passes. Those three fail the same way at the base commit, because the host's `mke2fs`
  enables `orphan_file` by default and growth refuses that feature (the feature check in
  `fs/rsext4/src/ext4/grow/layout.rs`), so they are unrelated to this change.

Not verified: no QEMU or board test ran; the change affects only rsext4 behavior host tests reach,
and no boot path.
```

## Test evidence

Before filing, every check the pull request cites ran again on `0353572d1`, the head of `dev` then,
with the patch applied. Without the fix the new test failed with the same `extent:empty_index`
error, and with it the test passed. `cargo xtask clippy --package rsext4` passed its 3 checks, and
`cargo fmt --check` was clean. `cargo xtask test --since 0353572d1` failed only the three growth
tests, and they fail the same way on unchanged `dev`. The runs below are the earlier ones, on
`6e80b940d`.

Run on the base commit above with the patch applied, on an x86_64 Linux host with e2fsprogs
1.47.2. The test ran with the commit's source change reverted (red), then as committed (green),
through `cargo test -q -p rsext4 --features host-test --test file_operations <test>`, the profile
`cargo xtask test` runs for rsext4. Then `cargo xtask clippy --package rsext4` and
`cargo fmt --check -p rsext4`:

```
== cc399e11c with remove.rs from the base
thread 'file_functional_tests::shrinking_extent_inode_past_a_whole_index_node_frees_that_node' panicked at fs/rsext4/tests/file_operations.rs:2056:14:
shrinking a depth-2 tree must succeed: Ext4Error { kind: Corrupted, context: Some(Operation { op: "extent:empty_index" }) }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 49 filtered out; finished in 2.18s
== cc399e11c as committed
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out; finished in 1.76s
passed checks: 3, failed checks: 0, skipped checks: 0
cargo fmt --check -p rsext4: clean
```

On the commit as it now stands, `cargo test -p rsext4 --features host-test`, with `MKE2FS_CONFIG`
pointing at a copy of `/etc/mke2fs.conf` without `orphan_file`, passed all 656 tests.

`cargo xtask test --since 6e80b940d`, run before the last edits to this commit, which changed only
comments, selected 20 packages and exited 1. rsext4 passed 543 tests and failed 2, both in
`tests/filesystem_growth.rs`; ax-fs-ng failed
`root_growth_preserves_other_partitions_and_ext4_contents` in two feature profiles. ax-block failed
6 of its `block::cache` tests in that run (`independent global sync cannot borrow maintenance`);
ax-block does not depend on rsext4, and it passed 188 of 188 in each of three reruns on the same
branch. Every other selected package passed. The growth failures come from this host's
`mke2fs.conf`, which enables `orphan_file`; `layout.rs`'s feature check refuses any compatible
feature outside its list. At the base commit the same three tests fail the same way. With
`MKE2FS_CONFIG` pointing at a copy of `/etc/mke2fs.conf` without `orphan_file`, they pass:

```
== host mke2fs.conf
thread 'growth_recovers_commit_checkpoint_and_flush_interruptions' panicked at fs/rsext4/tests/filesystem_growth.rs:562:65:
thread 'growth_preserves_files_and_exposes_new_groups_to_linux' panicked at fs/rsext4/tests/filesystem_growth.rs:225:5:
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
== MKE2FS_CONFIG without orphan_file
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 99.90s
== ax-fs-ng with MKE2FS_CONFIG without orphan_file
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 222 filtered out; finished in 7.10s
== ax-fs-ng with the host mke2fs.conf
thread 'root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents' panicked at fs/ax-fs-ng/src/root/grow.rs:533:17:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 222 filtered out; finished in 0.19s
```

The `e2fsck` message the Issue and the pull request quote comes from a separate image, not from
the test. `mke2fs -t ext4 -b 4096 -O ^orphan_file,^metadata_csum` made a 64 MiB image, with the
checksum feature off so that an extent block checksum does not report first. `debugfs` wrote a
3,200-block file and punched every odd block, which gives a depth-2 tree whose level-1 index node
sits at block 2070. `e2fsck -fn` passed that image with exit 0. With that node's `eh_entries` set
from 5 to 0, it reported:

```
e2fsck 1.47.2 (1-Jan-2025)
Pass 1: Checking inodes, blocks, and sizes
Failed to iterate extents in inode 12
	(op EXT2_EXTENT_DOWN, blk 2070, lblk 0): No 'down' extent
Clear inode? no

Inode 12 extent tree (at level 1) could be shorter.  Optimize? no

Error while iterating over blocks in inode 12: No 'down' extent

img: ********** WARNING: Filesystem still has errors **********
```

`e2fsck -fn` exited 12 there.
