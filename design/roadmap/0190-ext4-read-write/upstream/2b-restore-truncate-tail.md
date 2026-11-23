# Upstream item 2b: restore the zeroed tail when a truncate fails

Appendix to [milestone 190](../../0190-ext4-read-write.md), upstream item 2, second of its two
submissions (bug 2's lost tail). Name provisional.

This is a draft. Nobody has filed anything here. The architect reviews the exact text below, and
filing it is a separate step. The Issue goes first, since upstream does not process a pull request
with no linked Issue; its number then replaces `#<to be filled>` in the pull request body.
`#<to be filled: 2a>` takes item 2a's Issue number in the Issue and its pull request number in
the pull request.

- Repository and branch: `rcore-os/tgoskits`, `dev`.
- Base commit: `6e80b940d46167a59a795e158bb17a7eb28333af` ("feat(ivc): add fragmented Message V1
  full-duplex transport (#2068)"). No commit after it on `dev` touches `fs/rsext4`.
- The change: [`2b-restore-truncate-tail.patch`](2b-restore-truncate-tail.patch), one commit,
  `git format-patch` output against that base. It applies to the base alone, but
  [item 2a](2a-free-empty-extent-index.md) adds its test at the same place in
  `fs/rsext4/tests/file_operations.rs`. So 2b is filed after 2a, and rebased on 2a once 2a lands;
  its bodies say so.
- Issue settings, which upstream's contributing guide (section 3.7) requires in the sidebar and
  the filer sets: Assignees `calef`, labels `bug` and `q4`, Type `Bug`, Milestone `2026Q4`.

## Issue to file first

### Title

```
rsext4: a failed truncate leaves the file's tail zeroed
```

### Body (Chinese, as filed)

```markdown
## 当前问题

`truncate` 缩小文件失败时，文件内容被改动。缩小操作先把保留下来的末尾块中新文件末尾之后的字节清零，
再提交元数据事务。事务失败时元数据回滚，文件保持原来的大小，但被清零的字节不在回滚范围内，
因此一次失败的 truncate 会丢失从新文件末尾到该块末尾的数据，而调用方看到的是“操作失败、文件未变”。

- 代码版本：`dev` 分支 `6e80b940d`；crates.io 上的 rsext4 0.9.2 同样复现。
- 环境：x86_64 Linux 宿主，`rust-toolchain.toml` 指定的 nightly，`cargo test -p rsext4 --features host-test`
  使用的、可以在指定扇区注入写入失败的内存块设备。
- 复现步骤：创建一个两块长、内容非零的文件并卸载提交；关闭日志后让 inode 表所在扇区的写入失败；
  调用 `truncate` 缩小到 100 字节，然后重新读取文件。
- 实际结果：`truncate` 返回 `Io`，文件长度不变，但从第 100 个字节到第一个块末尾全部变成 0。
  期望结果：`truncate` 返回 `Io`，文件的每个字节都与原来相同。

## 修改方案

交付范围：清零时保存被替换的字节，单事务提交失败时把它们写回。具体为在 `fs/rsext4/src/file/io/zero.rs` 中新增
`zero_mapped_inode_tail_with_undo`，返回记录原字节的 `ZeroedTail`；`fs/rsext4/src/file/io/resize.rs` 的盘区路径和
传统间接块路径在单事务提交失败时，通过 `restore_tail_on_error` 把这些字节写回数据块缓存。原来的
`zero_mapped_inode_tail` 保留为不需要撤销的调用方的包装。

理由：保留“先清零再解除映射”的顺序，用撤销副本实现原子性，而不是改成提交之后再清零。ext4 格式文档
（4.1 Index Nodes、4.2 The Contents of inode.i_block）只描述磁盘上的字段，没有规定截断各步骤的顺序，`e2fsck` 也不检查文件末尾之后的字节。
因此这个顺序的依据是原有注释给出的理由：损坏的隐藏分支在修改任何数据之前就在预检阶段失败，改成提交之后再清零会失去这一点。
Linux 的 `ext4_truncate`（`fs/ext4/inode.c`）同样在 `ext4_ext_truncate` 之前清零末尾，这里只作为实现上的参照。代价是每次缩小多复制最多一个块的字节。

不包含：需要重新启动、分多个事务提交的缩小。较早的事务可能已经持久化，文件大小已经变化，恢复旧字节反而会把已经超出文件末尾的数据写回，
所以这条路径保持原有行为，代码中加注释说明。深度为 2 的盘区树缩小失败（`extent:empty_index`）是另一个 Issue。

依赖与兼容性：无新依赖，不改变磁盘格式和公开接口。合入顺序：`extent:empty_index` 的修复（#<to be filled: 2a>）与本任务在 `fs/rsext4/tests/file_operations.rs` 的同一位置新增测试，本任务的 PR 在它之后提交，并在它合入后变基到其上。负责人：@calef。期望季度：2026Q4。

## 测试验证

验收条件：在 `fs/rsext4/tests/file_operations.rs` 新增一个测试，按上面的步骤让盘区文件的 truncate 在写 inode 时失败，
要求返回 `Io`，并且文件的每个字节都与原来相同。去掉修复时该测试在第 100 个字节处发现差异，加上修复后通过。
`cargo xtask clippy --package rsext4`、`cargo fmt` 与 `cargo xtask test --since <基准>` 中的 rsext4 测试通过。
```

### Body (English rendering, for review)

```markdown
## Current problem

A failed `truncate` changes the file's contents. The shrink first zeroes the bytes after the new
end of file in the block it keeps, then commits the metadata
transaction. When the transaction fails, the metadata rolls back and the file keeps its old size,
but the zeroed bytes are outside the rollback, so a failed truncate loses the data from the new
end of file to the end of that block, while the caller sees "the operation failed and the file is
unchanged".

- Code version: `dev` at `6e80b940d`; rsext4 0.9.2 on crates.io reproduces it too.
- Environment: an x86_64 Linux host, the nightly `rust-toolchain.toml` names, and the in-memory
  block device `cargo test -p rsext4 --features host-test` uses, which can fail writes at a
  chosen sector.
- Steps: create a two-block file of nonzero bytes and commit it with an unmount; turn the journal
  off and make writes to the inode table's sector fail; call `truncate` to shrink the file to 100
  bytes, then read the file again.
- Actual result: `truncate` returns `Io` and the length is unchanged, but every byte from byte 100
  to the end of the first block is 0. Expected: `truncate` returns `Io` and every byte of the file
  is what it was.

## Proposed change

Scope: save the bytes the zeroing replaces, and write them back when the single-transaction commit
fails. Concretely, `fs/rsext4/src/file/io/zero.rs` gains `zero_mapped_inode_tail_with_undo`, which
returns a `ZeroedTail` holding the original bytes; when the single-transaction commit fails, the
extent path and the legacy indirect-block path in `fs/rsext4/src/file/io/resize.rs` write those
bytes back into the data block cache through `restore_tail_on_error`. The original
`zero_mapped_inode_tail` stays, as a wrapper for callers that need no undo.

Reasoning: keep "zero, then detach mappings" and get atomicity from an undo copy, rather than
zeroing after the commit. The ext4 format documentation (4.1 Index Nodes and 4.2 The Contents of
inode.i_block) describes on-disk fields only and is silent on the order of a truncate's steps, and
`e2fsck` does not check the bytes past end of file. The order therefore rests on the reason the
existing comment gives: a corrupt hidden branch fails in the preflight before any data is changed,
which zeroing after the commit would lose. Linux's `ext4_truncate` (`fs/ext4/inode.c`) implements
the same order, zeroing the tail before `ext4_ext_truncate`; it is cited only as an
implementation. The cost is copying at most one block's bytes per shrink.

Out of scope: a restarted shrink, which commits in several transactions. An earlier one may
already be durable and the file's size has then changed, so restoring the old bytes would write
back data that already lies past the end of file; that path keeps its existing behavior, with a
comment in the code saying why. A depth-2 extent tree that fails to shrink
(`extent:empty_index`) is a separate Issue.

Dependencies and compatibility: no new dependency; the on-disk format and the public interface do
not change. Merge order: the `extent:empty_index` fix (#<to be filled: 2a>) adds its test at the
same place in `fs/rsext4/tests/file_operations.rs`, so this task's pull request is filed after
that one and rebased on it once it lands. Owner: @calef. Target quarter: 2026Q4.

## Test plan

Acceptance: a new test in `fs/rsext4/tests/file_operations.rs` fails an extent file's truncate at
its inode write, as above, and requires `Io` and every byte of the file to equal what it was.
Without the fix the test finds a difference at byte 100; with it the test passes.
`cargo xtask clippy --package rsext4`, `cargo fmt` and the rsext4 tests in
`cargo xtask test --since <base>` pass.
```

## Pull request

### Title

```
fix(rsext4): restore the zeroed EOF tail when a single-transaction truncate fails
```

### Body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#<to be filled>

缩小文件时，`truncate` 先把保留下来的末尾块中新文件末尾之后的字节清零，再提交元数据事务。事务失败时元数据回滚、
文件大小不变，但被清零的字节不在回滚范围内，于是一次失败的 truncate 会丢失从新文件末尾到该块末尾的数据。
本 PR 只修复这一点；深度为 2 的盘区树缩小失败由另一个 PR（#<to be filled: 2a>）处理。该 PR 与本 PR 在 `fs/rsext4/tests/file_operations.rs` 的同一位置新增测试，所以本 PR 在它之后提交，并在它合入后变基到其上。

## 修改方案

一个提交，改动 `fs/rsext4/src/file/io/zero.rs` 和 `fs/rsext4/src/file/io/resize.rs`：

- `zero_mapped_inode_tail_with_undo` 在清零时保存被替换的字节，返回 `ZeroedTail`；原来的
  `zero_mapped_inode_tail` 保留为不需要撤销的调用方的包装。
- 盘区路径和传统间接块路径在单事务提交失败时，通过 `restore_tail_on_error` 把这些字节写回数据块缓存。
- 为什么用撤销副本而不是改成提交之后再清零：ext4 格式文档（4.1 Index Nodes、4.2 The Contents of inode.i_block）
  没有规定截断各步骤的顺序，`e2fsck` 也不检查文件末尾之后的字节。依据是原有注释给出的理由：“先清零再解除映射”让损坏的隐藏分支
  在修改任何数据之前就在预检阶段失败，改变顺序会失去这一点，所以选择在失败时恢复。Linux 的 `ext4_truncate`
  （`fs/ext4/inode.c`）同样在 `ext4_ext_truncate` 之前清零末尾，这里只作为实现上的参照。代价是每次缩小多复制最多一个块的字节。
- 为什么只在单事务路径恢复：需要重新启动的缩小操作分多个事务提交，较早的事务可能已经持久化，此时文件大小已经变化，
  恢复旧字节反而会把已经超出文件末尾的数据写回。这条路径保持原有行为，代码中有注释说明。
- 恢复本身失败时返回缩小操作自己的错误：用恢复的错误替换它会隐藏缩小失败的原因。这一点是取舍，
  如果维护者希望两种错误都能看到，可以改成别的报告方式。

兼容性：不改变磁盘格式和公开接口；只有单事务缩小失败时的行为变化，从“尾部被清零”变为“文件保持原样”。

## 测试验证

环境：x86_64 Linux 宿主，`rust-toolchain.toml` 指定的 nightly，e2fsprogs 1.47.2。

- 新增测试 `failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block`
  （`fs/rsext4/tests/file_operations.rs`）：让 inode 写入失败，要求 truncate 返回 `Io`，并且文件的每个字节都与原来相同。
- `cargo test -p rsext4 --features host-test --test file_operations failed_extent_truncate_restores`：
  去掉本修复时失败，`a failed truncate must leave every byte of the file as it was`，`left: Some(100)`、`right: None`；
  加上修复后通过。
- `cargo fmt`、`cargo xtask clippy --package rsext4` 通过。
- `cargo xtask test --since 6e80b940d`：rsext4 只有 `tests/filesystem_growth.rs` 中的两个测试失败，ax-fs-ng 的
  `root_growth_preserves_other_partitions_and_ext4_contents` 失败，其余全部通过。这三个测试在基准提交上同样失败，
  原因是宿主的 `mke2fs` 默认启用 `orphan_file`，而扩容拒绝该特性（`fs/rsext4/src/ext4/grow/layout.rs` 的特性检查），
  与本改动无关。

未验证：

- 传统间接块路径的恢复没有单独的测试：它与盘区路径使用同一个辅助函数，失败注入方式相同。如果需要，可以补一个对应测试。
- 需要重新启动的多事务缩小在失败时仍会留下被清零的尾部，原因见上文。
- 没有运行 QEMU 或板卡测试：改动只影响宿主测试可以覆盖的 rsext4 行为，不涉及启动路径。
```

### Body (English rendering, for review)

```markdown
## Current problem

Linked Issue: #<to be filled>

When it shrinks a file, `truncate` first zeroes the bytes after the new end of file in the block it
keeps, then commits the metadata transaction. When the transaction fails, the metadata rolls back
and the size stays the same, but the zeroed bytes are outside the rollback, so a failed truncate
loses the data from the new end of file to the end of that block. This pull request fixes only
that; another pull request (#<to be filled: 2a>) handles a depth-2 extent tree that fails to
shrink. That pull request adds its test at the same place in `fs/rsext4/tests/file_operations.rs`,
so this one is filed after it and rebased on it once it lands.

## Change

One commit, in `fs/rsext4/src/file/io/zero.rs` and `fs/rsext4/src/file/io/resize.rs`:

- `zero_mapped_inode_tail_with_undo` saves the bytes it replaces while zeroing and returns a
  `ZeroedTail`; the original `zero_mapped_inode_tail` stays, as a wrapper for callers that need no
  undo.
- When the single-transaction commit fails, the extent path and the legacy indirect-block path
  write those bytes back into the data block cache through `restore_tail_on_error`.
- Why an undo copy rather than zeroing after the commit: the ext4 format documentation (4.1
  Index Nodes and 4.2 The Contents of inode.i_block) is silent on the order of a truncate's
  steps, and `e2fsck` does not check the bytes past end of file. The authority is the reason the
  existing comment gives: "zero, then detach mappings" lets a corrupt hidden branch fail in the
  preflight before any data is changed. Changing the order would lose that, so the fix restores on
  failure instead. Linux's `ext4_truncate` (`fs/ext4/inode.c`) implements the same order, zeroing
  the tail before `ext4_ext_truncate`; it is cited only as an implementation. The cost is copying at most one block's bytes per shrink.
- Why restore only on the single-transaction path: a restarted shrink commits in several
  transactions, and an earlier one may already be durable. The file's size has then changed, and
  restoring the old bytes would write back data that already lies past the end of file. That path
  keeps its existing behavior, with a comment in the code saying why.
- When the restore itself fails, the shrink's own error is returned: replacing it with the
  restore's error would hide why the shrink failed. This is a trade-off; if the maintainers want
  both errors visible, the reporting can change.

Compatibility: the on-disk format and the public interface stay the same; only a failed
single-transaction shrink behaves differently, leaving the file as it was instead of its tail
zeroed.

## Test results

Environment: an x86_64 Linux host, the nightly `rust-toolchain.toml` names, e2fsprogs 1.47.2.

- New test `failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block`
  (`fs/rsext4/tests/file_operations.rs`): it makes the inode write fail, and requires the
  truncate to return `Io` and every byte of the file to equal what it was.
- `cargo test -p rsext4 --features host-test --test file_operations failed_extent_truncate_restores`:
  without this fix it fails with `a failed truncate must leave every byte of the file as it was`,
  `left: Some(100)`, `right: None`; with it, it passes.
- `cargo fmt` and `cargo xtask clippy --package rsext4` pass.
- `cargo xtask test --since 6e80b940d`: in rsext4 only two tests in `tests/filesystem_growth.rs`
  fail, plus ax-fs-ng's `root_growth_preserves_other_partitions_and_ext4_contents`; everything
  else passes. Those three fail the same way at the base commit, because the host's `mke2fs`
  enables `orphan_file` by default and growth refuses that feature (the feature check in
  `fs/rsext4/src/ext4/grow/layout.rs`), so they are unrelated to this change.

Not verified:

- The restore on the legacy indirect-block path has no test of its own: it uses the same helper as
  the extent path, with the same failure injection. A matching test can be added if wanted.
- A restarted, multi-transaction shrink still leaves its tail zeroed when it fails, for the reason
  above.
- No QEMU or board test ran: the change affects only rsext4 behavior host tests reach, and no boot
  path.
```

## Test evidence

Run on the base commit above with the patch applied, on an x86_64 Linux host with e2fsprogs
1.47.2. The test ran with the commit's source change reverted (red), then as committed (green),
through `cargo test -q -p rsext4 --features host-test --test file_operations <test>`, the profile
`cargo xtask test` runs for rsext4. Then `cargo xtask clippy --package rsext4` and
`cargo fmt --check -p rsext4`:

```
== 2c004f2c4 with zero.rs and resize.rs from the base
thread 'file_functional_tests::failed_extent_truncate_restores_the_zeroed_tail_of_the_retained_block' panicked at fs/rsext4/tests/file_operations.rs:2050:9:
assertion `left == right` failed: a failed truncate must leave every byte of the file as it was
  left: Some(100)
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 49 filtered out; finished in 0.10s
== 2c004f2c4 as committed
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out; finished in 0.08s
passed checks: 3, failed checks: 0, skipped checks: 0
cargo fmt --check -p rsext4: clean
```

`cargo xtask test --since 6e80b940d`, run before the last edit to this commit, which changed only
comments, selected 20 packages and exited 1. rsext4 passed 543 tests and
failed 2, both in `tests/filesystem_growth.rs`; ax-fs-ng failed
`root_growth_preserves_other_partitions_and_ext4_contents` in two feature profiles. Every
other selected package passed. The growth failures come from this host's `mke2fs.conf`, which
enables `orphan_file`; `layout.rs`'s feature check refuses any compatible feature outside its
list. At the base commit the same three tests fail the same way. With `MKE2FS_CONFIG` pointing at a copy of `/etc/mke2fs.conf` without `orphan_file`, they
pass:

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
