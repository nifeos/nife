# Upstream draft 8: InodeInfo reports whole times

Item 8 of [milestone 190](../../0190-ext4-read-write.md)'s upstream round: an issue, then the pull
request that links it. The slug is provisional. Nobody has filed either; the architect reviews this
text before anyone files it.

- Repository and base: rcore-os/tgoskits, branch `dev`, commit
  `6e80b940d46167a59a795e158bb17a7eb28333af`, still the head of `dev` as of this draft.
- Patch: [8-whole-inode-times.patch](8-whole-inode-times.patch), one commit.
- Issue title: `fix(rsext4): InodeInfo drops nanoseconds and misreads times outside 1970-2106`
- Pull request title: `fix(rsext4)!: report whole inode times in InodeInfo`
- Issue settings, which upstream's contributing guide (section 3.7) requires in the sidebar and
  the filer sets: Assignees `calef`, labels `bug` and `q4`, Type `Bug`, Milestone `2026Q4`.

## Issue to file first (Chinese, as filed)

```markdown
## 当前问题

拥有型接口返回的 `InodeInfo` 把 `atime`、`ctime`、`mtime`、`btime` 存为 inode 中原始的 32 位秒字段，因此丢失纳秒，把 1970 年以前的时间读成 2106 年附近的时间，并让 2106 年以后的时间回绕。

时间的编码以 ext4 格式文档（Linux 内核 Documentation/filesystems/ext4/）“4.1. Index Nodes”页的“4.1.3. Inode Timestamps”一节为准：秒字段是有符号 32 位数；inode 大于 128 字节且 `i_extra_isize` 覆盖相应的 `i_[cma]time_extra` 字段时，该字段的低两位扩展秒数，高 30 位保存纳秒。

crate 已有正确的解码（`Ext4Inode::atime_ts` 等访问器），写入接口也完整写入带纳秒和纪元位的 `Ext4Timestamp`，问题只在读回给调用者的这一层。`ax-fs-ng` 用 `Duration::from_secs(u64::from(inode.atime))` 转换，因此 StarryOS/ArceOS 的 `stat` 同样看不到纳秒。

复现：用 `update_inode_metadata` 写入一个 1970 年以前带纳秒的访问时间，再用 `inode()` 读回。期望结果：读回的值与写入的值相同。实际结果：读回的是 `u32` 秒数 4294880896，没有纳秒。影响的组件：`fs/rsext4`，以及使用这些字段的 `fs/ax-fs-ng`。

## 修改方案

- 交付范围：`InodeInfo` 的四个时间改为 `Ext4Timestamp`（创建时间为 `Option<Ext4Timestamp>`），用现有访问器解码；`ax-fs-ng` 改为保留纳秒的转换。
- 不包含：VFS 时间类型的改动；1970 年以前的时间在 `ax-fs-ng` 中仍读为纪元起点，因为 VFS 的 `Duration` 无法表示负时间。
- 兼容性：这是公开类型的破坏性改动。仓库中只有 `ax-fs-ng` 使用这些字段，本改动一并更新它。
- 负责人：@calef。期望季度：2026Q4。

## 测试验证

- 验收条件：`rsext4` 中新增测试读回 1970 年以前和 2106 年以后带纳秒的时间，结果与写入值相同；`ax-fs-ng` 的持久化测试在设置带纳秒的时间后读回相同的值。两者都在修复前失败、修复后通过。
- 环境：x86_64 Linux，`cargo xtask test`，e2fsprogs。
```

## Issue to file first (English rendering, for review)

```markdown
## Current problem

The `InodeInfo` the owned interface returns stores `atime`, `ctime`, `mtime` and `btime` as the
inode's raw 32-bit seconds fields. So it drops the nanoseconds, reads a time before 1970 as one
near 2106, and wraps a time past 2106.

The authority for the encoding is the ext4 format documentation (the Linux kernel's
Documentation/filesystems/ext4/), page "4.1. Index Nodes", section "4.1.3. Inode Timestamps". The
seconds fields hold signed 32-bit numbers. When the inode is larger than 128 bytes and
`i_extra_isize` covers the matching `i_[cma]time_extra` field, that field's low two bits extend the
seconds and its upper 30 bits hold the nanoseconds.

The crate already decodes these correctly (the `Ext4Inode::atime_ts` accessors and their siblings), and its write interface writes an
`Ext4Timestamp` whole, nanoseconds and epoch bits included. The problem is only in the layer that
reads them back to the caller. `ax-fs-ng` converts with `Duration::from_secs(u64::from(inode.atime))`,
so `stat` on StarryOS/ArceOS sees no nanoseconds either.

To reproduce, write an access time before 1970 with nanoseconds through `update_inode_metadata`,
then read it back with `inode()`. The expected result is the value written. The actual result is
the `u32` seconds 4294880896, with no nanoseconds. The components affected are `fs/rsext4`, and
`fs/ax-fs-ng`, which uses these fields.

## Proposed change

- Scope: `InodeInfo`'s four times become `Ext4Timestamp` (the creation time
  `Option<Ext4Timestamp>`), decoded with the existing accessors, and `ax-fs-ng` converts them
  keeping the nanoseconds.
- Not included: any change to the VFS time type. In `ax-fs-ng` a time before 1970 still reads as
  the epoch, because the VFS `Duration` cannot hold a negative time.
- Compatibility: this breaks a public type. Inside the repository only `ax-fs-ng` uses these
  fields, and this change updates it too.
- Owner: @calef. Target quarter: 2026Q4.

## Testing and verification

- Acceptance: a new `rsext4` test reads back times before 1970 and past 2106, with nanoseconds, as
  the values written, and `ax-fs-ng`'s persistence test reads back the nanosecond times it set.
  Both fail before the fix and pass after it.
- Environment: x86_64 Linux, `cargo xtask test`, e2fsprogs.
```

## Pull request body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#<to be filled>

拥有型接口 `Ext4::inode`、`lookup_child`、`update_inode_metadata` 等返回的 `InodeInfo` 把 `atime`、`ctime`、`mtime`、`btime` 存为 inode 中原始的 32 位秒字段（`i_atime` 等）。按 ext4 格式文档（Linux 内核 Documentation/filesystems/ext4/）“4.1. Index Nodes”页的“4.1.3. Inode Timestamps”一节，这样做有三个后果：

- 纳秒丢失。inode 大于 128 字节且 `i_extra_isize` 覆盖相应字段时，`i_[cma]time_extra` 的高 30 位保存纳秒，`InodeInfo` 不读它。
- 1970 年以前的时间读成 2106 年附近的时间。ext4 的秒字段是有符号 32 位数，`u32` 把 1969-12-31 读成 4294880896。
- 2106 年以后的时间回绕。`i_[cma]time_extra` 的低两位是纪元扩展位，`InodeInfo` 不读它。

crate 本身已经能正确处理这些时间：`Ext4Inode::atime_ts` 等访问器按 inode 大小解码扩展字段，`update_inode_metadata` 也会把带纳秒和纪元位的 `Ext4Timestamp` 完整写入磁盘。问题只在读回给调用者的这一层。`ax-fs-ng` 用 `Duration::from_secs(u64::from(inode.atime))` 转换，因此 VFS 看到的时间同样没有纳秒。

## 修改方案

- `InodeInfo::{atime, ctime, mtime}` 的类型由 `u32` 改为 `Ext4Timestamp`，`InodeInfo::btime` 改为 `Option<Ext4Timestamp>`。选择 `Ext4Timestamp` 而不是 `Duration` 或 `(i64, u32)`，是因为它就是 crate 写入接口 `InodeMetadataUpdate` 使用的类型，读写两侧对称，也能表示 1970 年以前的时间。`btime` 用 `Option`，是因为 128 字节 inode 没有创建时间字段，此时返回 0 会和真实的 1970-01-01 混淆；`crtime_ts` 本身已经返回 `Option`。
  - 依据：同一页的 inode 表中 `i_crtime` 位于偏移 0x90，在原始的 128 字节 ext2 inode 之外；“4.1.1. Inode Size”一节要求访问这类字段前先确认它在 `i_extra_isize` 范围内。
- `owned/metadata.rs` 用现有的 `atime_ts`、`ctime_ts`、`mtime_ts`、`crtime_ts` 访问器按 inode 大小解码这四个时间。解码逻辑已经存在且经过测试，`InodeInfo` 只需要调用它，不新增解码代码。
- `ax-fs-ng` 新增 `Inode::duration`，把 `Ext4Timestamp` 转换为 VFS 的 `Duration`，保留纳秒；1970 年以前的时间无法用 `Duration` 表示，读为纪元起点。这里不报错，是因为 `stat` 一个文件不应因为时间早于 1970 而失败；VFS 的 `Duration` 无法表示负时间，这是该层现有的限制。
- 新增测试 `owned_metadata_update_reports_whole_times`（`fs/rsext4/tests/integration_test.rs`）：写入一个 1970 年以前带纳秒的访问时间和一个 2106 年以后带纳秒的修改时间，检查 `update_inode_metadata` 的返回值和只读重新挂载后 `inode()` 的结果都与写入的值相同，并检查新文件的创建时间存在且等于其状态改变时间。`rsext4` 中没有检查拥有型接口读回时间的测试，因此新增一个。
- 加强 `ax-fs-ng` 现有测试 `metadata_owner_and_times_remain_durable_after_explicit_sync`：设置的时间改为带纳秒，修改时间改为 2106 年以后。修复错误优先加强现有测试：该测试已经覆盖设置时间后同步再读回，只需让它的取值覆盖纳秒和纪元位。
- 兼容性和受影响范围：这是公开接口的破坏性改动，标题用 `fix(rsext4)!` 并在提交说明中写明 `BREAKING CHANGE`，由 release-plz 生成版本号和变更日志。仓库中只有 `ax-fs-ng` 使用 `InodeInfo` 的时间字段，本 PR 一并更新了它；仓库外的使用者需要改用 `Ext4Timestamp`。

## 测试验证

- 先把 `owned/metadata.rs` 的解码临时恢复为只读 32 位秒字段，新测试失败：`left: Ext4Timestamp { sec: 4294880896, nsec: 0 }, right: Ext4Timestamp { sec: -86400, nsec: 123456789 }`；`ax-fs-ng` 的持久化测试也失败：`left: 123s, right: 123.456789012s`。恢复修复后两者都通过。
- `cargo xtask clippy --package rsext4`（3 项检查）和 `cargo xtask clippy --package ax-fs-ng`（6 项检查）通过，`cargo fmt` 无改动。
- `cargo xtask test --since 6e80b940d` 中，除下面三项外所有被选中的软件包都通过。`rsext4` 的 `tests/filesystem_growth.rs` 中 `growth_recovers_commit_checkpoint_and_flush_interruptions` 和 `growth_preserves_files_and_exposes_new_groups_to_linux`，以及 `ax-fs-ng` 的 `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents` 失败；这三项在未改动的基准提交上以同样的方式失败，与本改动无关。原因是本机 `mke2fs`（e2fsprogs 1.47.2）默认启用 `orphan_file`，而扩容在 `GrowthLayout::check`（`fs/rsext4/src/ext4/grow/layout.rs`）拒绝该特性（`grow:features_or_durability`）。`orphan_file` 是兼容特性：ext4 格式文档（Linux 内核 Documentation/filesystems/ext4/）“3.1. Super Block”一节规定，`s_feature_compat` 中的特性即使不被理解，也仍可读写该文件系统，因此这一拒绝是 `rsext4` 扩容自身的限制，不是格式规则的要求。
- 另用一个外部读取比对工具（用 `mke2fs` 和 `debugfs` 生成包含带纳秒、1970 年以前和 2106 年以后时间的文件的镜像，与 `debugfs` 读出的时间比较）验证：修复前所有镜像和扇区组合中这三类文件的时间都与 `debugfs` 不同，修复后全部相同。
- 未验证：QEMU 和实体板卡上的 StarryOS 和 ArceOS `stat` 结果。改动只影响 inode 时间的读取和转换，`ax-fs-ng` 的主机测试已覆盖该转换。
```

## Pull request body (English rendering, for review)

```markdown
## Current problem

Linked issue: #<to be filled>

The `InodeInfo` that the owned interface returns (from `Ext4::inode`, `lookup_child`,
`update_inode_metadata` and others) stores `atime`, `ctime`, `mtime` and `btime` as the inode's
raw 32-bit seconds fields (`i_atime` and so on). Under the ext4 format documentation (the Linux
kernel's Documentation/filesystems/ext4/), page "4.1. Index Nodes", section "4.1.3. Inode
Timestamps", that has three consequences:

- The nanoseconds go missing. When the inode is larger than 128 bytes and `i_extra_isize` covers
  the field, the upper 30 bits of `i_[cma]time_extra` hold them, and `InodeInfo` does not read them.
- A time before 1970 reads as one near 2106. ext4's seconds field is a signed 32-bit number, and
  as a `u32` 1969-12-31 reads as 4294880896.
- A time past 2106 wraps. The low two bits of `i_[cma]time_extra` extend the epoch, and
  `InodeInfo` does not read them.

The crate already handles these times correctly elsewhere. The `Ext4Inode::atime_ts` accessors
and their siblings decode the extra fields according to the inode size, and
`update_inode_metadata` writes an `Ext4Timestamp` to disk whole, nanoseconds and epoch bits
included. The problem is only in the layer that reads them back to the caller. `ax-fs-ng`
converts with `Duration::from_secs(u64::from(inode.atime))`, so the VFS sees no nanoseconds either.

## Proposed change

- `InodeInfo::{atime, ctime, mtime}` change from `u32` to `Ext4Timestamp`, and `InodeInfo::btime`
  to `Option<Ext4Timestamp>`. We chose `Ext4Timestamp` over `Duration` or `(i64, u32)` because it
  is the type the crate's own write interface, `InodeMetadataUpdate`, takes. Reading and writing
  then match, and it can hold a time before 1970. `btime` is an `Option` because a 128-byte inode
  has no creation-time field, and a caller would mistake a 0 there for a real 1970-01-01.
  `crtime_ts` already returns an `Option`.
  - The authority: the same page's inode table places `i_crtime` at offset 0x90, past the original
    128-byte ext2 inode, and section "4.1.1. Inode Size" says to check that such a field lies
    within `i_extra_isize` before reading it.
- `owned/metadata.rs` decodes the four times with the existing `atime_ts`, `ctime_ts`, `mtime_ts`
  and `crtime_ts` accessors, according to the inode size. The crate already has the decoding and
  tests it, so `InodeInfo` only needs to call it; the change adds no new decoding code.
- `ax-fs-ng` gains `Inode::duration`, which converts an `Ext4Timestamp` to the VFS `Duration` and
  keeps the nanoseconds. A time before 1970, which a `Duration` cannot hold, reads as the epoch
  rather than failing, because `stat` on a file should not fail for a time before 1970. The VFS
  `Duration` cannot hold a negative time, which is an existing limit of that layer.
- A new test, `owned_metadata_update_reports_whole_times` in
  `fs/rsext4/tests/integration_test.rs`, writes an access time before 1970 and a modification time
  past 2106, both with nanoseconds. It checks that `update_inode_metadata`'s return value, and
  `inode()` after a read-only remount, both give back the values written, and that a new file has
  a creation time equal to its status change time. `rsext4` has no test of the times the owned
  interface reads back, so the change adds one.
- The change strengthens the existing `ax-fs-ng` test
  `metadata_owner_and_times_remain_durable_after_explicit_sync`: the times it sets carry
  nanoseconds, and its modification time is past 2106. A bug fix strengthens an existing test
  first, and this test already sets times, syncs and reads them back; it only needed values that
  reach the nanoseconds and the epoch bits.
- Compatibility and scope: this breaks a public interface, so the title uses `fix(rsext4)!` and
  the commit message carries `BREAKING CHANGE`; release-plz derives the version and the changelog
  from them. Inside the repository only `ax-fs-ng` uses `InodeInfo`'s time fields, and this pull
  request updates it. Users outside the repository have to move to `Ext4Timestamp`.

## Testing and verification

- With the decoding in `owned/metadata.rs` temporarily put back to the 32-bit seconds fields, the
  new test fails (`left: Ext4Timestamp { sec: 4294880896, nsec: 0 }, right: Ext4Timestamp { sec:
  -86400, nsec: 123456789 }`), and so does `ax-fs-ng`'s persistence test (`left: 123s, right:
  123.456789012s`). Both pass with the fix restored.
- `cargo xtask clippy --package rsext4` (3 checks) and `cargo xtask clippy --package ax-fs-ng`
  (6 checks) pass, and `cargo fmt` changes nothing.
- In `cargo xtask test --since 6e80b940d`, every selected package passes except three tests:
  `growth_recovers_commit_checkpoint_and_flush_interruptions` and
  `growth_preserves_files_and_exposes_new_groups_to_linux` in `rsext4`'s
  `tests/filesystem_growth.rs`, and `ax-fs-ng`'s
  `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents`. These three fail
  the same way on the unchanged base commit, so this change does not cause them. The cause: the
  host's `mke2fs` (e2fsprogs 1.47.2) enables `orphan_file`, and growth refuses that feature in
  `GrowthLayout::check` (`fs/rsext4/src/ext4/grow/layout.rs`, `grow:features_or_durability`).
  `orphan_file` is a compatible feature. The ext4 format documentation (the Linux kernel's
  Documentation/filesystems/ext4/), section "3.1. Super Block", says a filesystem can still be
  read and written by code that does not understand a flag in `s_feature_compat`. So this refusal
  is a limit of `rsext4`'s growth, not a rule of the format.
- Also checked with an external read comparison tool, which uses `mke2fs` and `debugfs` to build
  images holding files with nanosecond times, times before 1970 and times past 2106, and compares
  the times read against `debugfs`'s. Before the fix, those files' times differ from `debugfs`'s
  on every image and sector pairing; after it, all of them match.
- Not verified: `stat` results from StarryOS and ArceOS under QEMU or on physical boards. The
  change touches only how the crate reads and converts inode times, and `ax-fs-ng`'s host test covers
  the conversion.
```

## Notes for the reviewer (not filed)

- **Design choices upstream might refuse.** It is a breaking change to a public type, so the next
  release is 0.10 and every user of `InodeInfo`'s times has to change. Upstream might prefer to
  keep the `u32` fields and add new ones beside them. And `ax-fs-ng` clamps a time before 1970 to
  the epoch; upstream might prefer an error or a wider VFS time type.
- "An external read comparison tool" is our `tools/ext4_oracle`, run through a local, uncommitted
  `[patch]` of `rsext4` and a local formatter for the new type. When we move to a release with the
  fix, the oracle's `/timestamps/` outcomes in `expected.txt` turn from `differs` to `equal`.

## Test evidence

Red, `rsext4`, decoding put back to the 32-bit seconds fields (`cargo test -p rsext4 --features
host-test --test integration_test owned_metadata_update_reports_whole_times`):

```text
test owned_metadata_update_reports_whole_times ... FAILED
thread 'owned_metadata_update_reports_whole_times' panicked at fs/rsext4/tests/integration_test.rs:1226:5:
assertion `left == right` failed
  left: Ext4Timestamp { sec: 4294880896, nsec: 0 }
 right: Ext4Timestamp { sec: -86400, nsec: 123456789 }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.08s
```

Red, `ax-fs-ng` (`cargo test -p ax-fs-ng --features host-test,ext4 --lib
metadata_owner_and_times_remain_durable`):

```text
test fs::ext4::rsext4::fs::tests::persistence::metadata_owner_and_times_remain_durable_after_explicit_sync ... FAILED
thread '...metadata_owner_and_times_remain_durable_after_explicit_sync' panicked at fs/ax-fs-ng/src/fs/ext4/rsext4/fs/tests/persistence.rs:92:5:
assertion `left == right` failed
  left: 123s
 right: 123.456789012s
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 222 filtered out; finished in 0.07s
```

Green, with the fix:

```text
test owned_metadata_update_reports_whole_times ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.07s
test fs::ext4::rsext4::fs::tests::persistence::metadata_owner_and_times_remain_durable_after_explicit_sync ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 222 filtered out; finished in 0.34s
```

`cargo xtask clippy`:

```text
rsext4:    passed checks: 3, failed checks: 0, skipped checks: 0; all clippy checks passed
ax-fs-ng:  passed checks: 6, failed checks: 0, skipped checks: 0; all clippy checks passed
```

`cargo xtask test --since 6e80b940d`, on the commit and on the unchanged base (the same three
failures on both; the other 18 selected packages print `ok`):

```text
std tests failed for 2 package(s): ax-fs-ng, rsext4
test growth_recovers_commit_checkpoint_and_flush_interruptions ... FAILED
test growth_preserves_files_and_exposes_new_groups_to_linux ... FAILED
test root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents ... FAILED
```

The oracle's `compare.py` against its recorded outcomes, with the fix: every `/timestamps/` line
recorded as `differs` is now `equal`, on all nine image and sector pairings, and nothing else
changes. One pairing as printed:

```text
expected, not seen: default-1k	512	differs	/timestamps/after-2106:modification,birth /timestamps/before-1970:access,modification /timestamps/nanoseconds:access,modification,birth
seen, not expected: default-1k	512	equal
```
