# Upstream item 9: the inode bitmap padding

Appendix to [milestone 190](../../0190-ext4-read-write.md), upstream item 9. Name provisional.

This is a draft. Nobody has filed anything here. The architect reviews the exact text below, and
filing it is a separate step.

- Repository and branch: `rcore-os/tgoskits`, `dev`.
- Base commit: `6e80b940d46167a59a795e158bb17a7eb28333af` ("feat(ivc): add fragmented Message V1
  full-duplex transport (#2068)").
- The change: [`9-inode-bitmap-padding.patch`](9-inode-bitmap-padding.patch), one commit,
  `git format-patch` output against that base. It changes `fs/rsext4/src/ext4/alloc.rs` only.

## Title

```
fix(rsext4): set the inode bitmap padding when initializing an uninit group
```

## Body (Chinese, as filed)

```markdown
## 问题

块组带有 `EXT4_BG_INODE_UNINIT` 标志时，磁盘上的 inode 位图尚未初始化。`alloc_inodes` 在该组分配第一个
inode 时，会在内存中把位图整块清零，然后置位新分配的 inode。

Linux 的 `ext4_init_inode_bitmap` 只清零属于该组 inode 的位，并用 `ext4_mark_bitmap_end` 把最后一个
inode 之后直到块末尾的位全部置 1。`e2fsck` 要求这些填充位为 1。每组 inode 数小于一个块的位数时（例如
`mke2fs` 以 1 KiB 块创建的文件系统，每组 1,024 个 inode，而位图块有 8,192 位），只要 rsext4 在这样的
组里分配过 inode，`e2fsck -fn` 就报告 "Padding at end of inode bitmap is not set" 并以状态 4 退出。

## 改动

- `alloc_inodes` 为 `INODE_UNINIT` 组合成位图时，清零之后调用新的 `mark_inode_bitmap_padding`，把
  第 `s_inodes_per_group` 位到块末尾的位全部置 1，与 `ext4_mark_bitmap_end` 相同。
- 增强已有测试 `alloc_inode_treats_uninit_bitmap_as_all_free`：分配之后，要求该组最后一个 inode 之后的
  每一位都为 1。

## 每一步的理由

- 只改合成位图这一处。`mkfs` 为第 0 组写位图时、`grow` 为新组写位图时都已经设置了填充位，
  只有 `INODE_UNINIT` 组的首次分配漏掉了。
- inode 位图校验和（`ext4_inode_bitmap_csum32`）只覆盖前 `s_inodes_per_group / 8` 字节，
  填充位不改变校验和，所以不需要改动校验和的计算。
- 填充位在分配器的搜索范围之外（搜索只到 `s_inodes_per_group`），把它们置 1 不会改变分配结果。
- 按仓库的测试规则，修复复用并增强已有的通用测试，而不是新增测试。增强后的断言在未修复的代码上失败，
  修复后通过。

## 验证

- 只应用测试改动时，`alloc_inode_treats_uninit_bitmap_as_all_free` 失败于新增断言
  （`bitmap.data[1..]` 不全为 `0xff`）；应用修复后通过。
- `cargo fmt --check -p rsext4` 无差异；`cargo xtask clippy --package rsext4` 通过。
- `cargo xtask test --since 6e80b940d` 选中 20 个软件包。rsext4 的单元测试 428 个全部通过。
  失败的只有 `fs/rsext4/tests/filesystem_growth.rs` 的两个测试和 ax-fs-ng 的
  `root_growth_preserves_other_partitions_and_ext4_contents`；它们在基准提交上不带本改动时以同样方式失败，
  与本改动无关（本改动不涉及文件系统扩容）。
- 在 `mke2fs` 默认参数、1 KiB 块创建的镜像上，通过 rsext4 创建文件、创建并删除目录、扩展 htree 目录之后，
  修复前 `e2fsck -fn` 以状态 4 退出并报告填充位未设置；修复后这些情况全部干净通过。

## 未验证的内容

- 没有运行 QEMU 或板卡测试：本改动只影响主机测试能覆盖的 rsext4 行为，不改变任何运行时启动路径。
- 上面提到的三个预先存在的失败没有在本改动中调查。
```

## Body (English rendering, for review)

```markdown
## Problem

When a block group carries `EXT4_BG_INODE_UNINIT`, its inode bitmap on disk is not initialized yet.
When `alloc_inodes` allocates the first inode in such a group, it zeroes the whole bitmap in memory
and then sets the bit of the new inode.

Linux's `ext4_init_inode_bitmap` zeroes only the bits of the group's inodes, and with
`ext4_mark_bitmap_end` sets every bit past the last inode to the end of the block. `e2fsck` requires
those padding bits set. When a group has fewer inodes than one block has bits (for example a file
system `mke2fs` made with 1 KiB blocks, 1,024 inodes per group against an 8,192-bit bitmap block),
once rsext4 has allocated an inode in such a group, `e2fsck -fn` reports "Padding at end of inode
bitmap is not set" and exits with status 4.

## Change

- When `alloc_inodes` synthesizes the bitmap of an `INODE_UNINIT` group, after zeroing it calls the
  new `mark_inode_bitmap_padding`, which sets every bit from bit `s_inodes_per_group` to the end of
  the block, as `ext4_mark_bitmap_end` does.
- The existing test `alloc_inode_treats_uninit_bitmap_as_all_free` is strengthened: after the
  allocation, every bit past the group's last inode must be set.

## Reasoning of each step

- Only the synthesized bitmap changes. `mkfs` writing group 0's bitmap and `grow` writing a new
  group's bitmap already set the padding; only the first allocation in an `INODE_UNINIT` group
  missed it.
- The inode bitmap checksum (`ext4_inode_bitmap_csum32`) covers only the first
  `s_inodes_per_group / 8` bytes, so the padding does not change it, and the checksum computation
  needs no change.
- The padding lies outside the allocator's search range (the search stops at
  `s_inodes_per_group`), so setting it does not change which inode is allocated.
- Under the repository's test rules, the fix reuses and strengthens the existing general test
  rather than adding a new one. The strengthened assertion fails on the unfixed code and passes with
  the fix.

## Verification

- With only the test change applied, `alloc_inode_treats_uninit_bitmap_as_all_free` fails on the new
  assertion (`bitmap.data[1..]` is not all `0xff`); with the fix it passes.
- `cargo fmt --check -p rsext4` shows no difference; `cargo xtask clippy --package rsext4` passes.
- `cargo xtask test --since 6e80b940d` selected 20 packages. All 428 rsext4 unit tests pass. The only
  failures are two tests in `fs/rsext4/tests/filesystem_growth.rs` and ax-fs-ng's
  `root_growth_preserves_other_partitions_and_ext4_contents`; they fail the same way at the base
  commit without this change, and are unrelated to it (it does not touch filesystem growth).
- On images `mke2fs` made with its defaults and 1 KiB blocks, after creating files, creating and
  removing a directory, and growing an htree directory through rsext4, `e2fsck -fn` exited with
  status 4 reporting the padding before the fix; with the fix every one of these cases is clean.

## Not verified

- No QEMU or board test was run: the change affects only rsext4 behavior that host tests reach, and
  changes no runtime boot path.
- The three pre-existing failures above were not investigated as part of this change.
```

## Test evidence

Run on the base commit above, on an x86_64 Linux host with e2fsprogs 1.47.2.

The strengthened test with the source change left out (the test hunk of the patch alone), through
`cargo xtask test --since 6e80b940d`:

```
thread 'ext4::alloc::tests::alloc_inode_treats_uninit_bitmap_as_all_free' panicked at fs/rsext4/src/ext4/alloc.rs:745:9:
assertion failed: bitmap.data[1..].iter().all(|byte| *byte == 0xff)
test result: FAILED. 427 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
```

The patch as committed, through the same command:

```
test ext4::alloc::tests::alloc_inode_treats_uninit_bitmap_as_all_free ... ok
test result: ok. 428 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.99s
```

`cargo xtask clippy --package rsext4`: `passed checks: 3, failed checks: 0, skipped checks: 0`.
`cargo fmt --check -p rsext4`: clean.

`cargo xtask test --since 6e80b940d` selected 20 packages and exited 1: rsext4 failed
`growth_preserves_files_and_exposes_new_groups_to_linux` and
`growth_recovers_commit_checkpoint_and_flush_interruptions` in `tests/filesystem_growth.rs`, and
ax-fs-ng failed `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents`.
`cargo test -p rsext4 --no-fail-fast` at the base commit and with the patch gives the same result
in every test binary: 428 unit tests pass, and the same two `filesystem_growth` tests fail with the
same messages. The ax-fs-ng test failed the same way with only the test hunk applied, which touches
no ax-fs-ng code.

nife's write oracle (`tools/ext4_oracle`), with `rsext4` taken from the base commit and then from
the patched tree through an uncommitted `[patch.crates-io]`. At the base commit, four cases at
1 KiB blocks fail on the padding alone:

```
create                 default-1k  512  exit4: Padding at end of inode bitmap is not set. Fix? no
mkdir-rmdir            default-1k  512  exit4: Padding at end of inode bitmap is not set. Fix? no
htree-growth           default-1k  512  exit4: Padding at end of inode bitmap is not set. Fix? no
inode-bitmap-padding   default-1k  512  exit4: Padding at end of inode bitmap is not set. Fix? no
```

With the patch, all four are clean, and `directory-past-inline-extents` at 1 KiB no longer reports
the padding. Its remaining note ("extent tree could be narrower", exit 0) is not an error and is
the same at the base commit. Every 4 KiB outcome is the same at the base commit and with the patch.
