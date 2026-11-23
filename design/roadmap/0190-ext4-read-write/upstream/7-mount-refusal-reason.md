# Upstream draft 7: mount reports why it refused the superblock

Item 7 of [milestone 190](../../0190-ext4-read-write.md)'s upstream round: an issue, then the pull
request that links it. The slug is provisional. Nobody has filed either; the architect reviews this
text before anyone files it.

- Repository and base: rcore-os/tgoskits, branch `dev`, commit
  `6e80b940d46167a59a795e158bb17a7eb28333af`, still the head of `dev` as of this draft.
- Patch: [7-mount-refusal-reason.patch](7-mount-refusal-reason.patch), one commit.
- Issue title: `fix(rsext4): mount reports every superblock refusal as a bare Io`
- Pull request title: `fix(rsext4): report why mount refused the superblock`
- Issue settings, which upstream's contributing guide (section 3.7) requires in the sidebar and
  the filer sets: Assignees `calef`, labels `bug` and `q4`, Type `Bug`, Milestone `2026Q4`.

## Issue to file first (Chinese, as filed)

```markdown
## 当前问题

`Ext4FileSystem::mount`、`device_has_error_state` 和日志重放后的 `reload_after_journal_replay` 都把 `read_superblock` 的任何错误映射成不带上下文的 `Ext4Error::io()`。因此，当文件系统块小于设备逻辑扇区时（例如 1 KiB 块的镜像放在 4096 字节扇区的设备上），或者设备上根本没有 ext4 文件系统时，挂载都只报 `Ext4Error { kind: Io, context: None }`，与一次真正失败的设备读取无法区分。经 `ax-fs-ng` 映射后，StarryOS/ArceOS 上的挂载对这些情况都返回 `EIO`。

复现：在 4096 字节扇区的设备上挂载一个全零设备，或一个 1 KiB 块的 `mke2fs` 镜像。期望结果：分别报 `InvalidMagic` 和带 `device:filesystem_geometry` 上下文的 `BadSuperblock`。实际结果：两者都报 `Io`。影响的组件：`fs/rsext4`，以及经由它的 `fs/ax-fs-ng`。

## 修改方案

- 交付范围：挂载入口原样传播 `read_superblock` 的错误；魔数检查移入 `read_superblock`，放在块大小检查之前，使非 ext4 设备总是报 `InvalidMagic`。
- 不包含：其他错误路径的分类，以及 `ax-fs-ng` 的错误映射。
- 兼容性：调用者看到的错误类型改变（`Io` 变为 `BadSuperblock` 或 `InvalidMagic`）。`ax-fs-ng` 把这两种映射为 `VfsError::InvalidInput`，所以这些拒绝在 StarryOS/ArceOS 上从 `EIO` 变为 `EINVAL`：这些拒绝针对的是超级块的内容，而不是设备读取失败。
- 负责人：@calef。期望季度：2026Q4。

## 测试验证

- 验收条件：新增测试在修复前失败、修复后通过：全零设备报 `InvalidMagic`；4096 字节扇区设备上只写入魔数的 1 KiB 块超级块报带 `device:filesystem_geometry` 上下文的 `BadSuperblock`。
- 环境：x86_64 Linux，`cargo xtask test`，e2fsprogs。
```

## Issue to file first (English rendering, for review)

```markdown
## Current problem

`Ext4FileSystem::mount`, `device_has_error_state` and `reload_after_journal_replay` (run after a
journal replay) all map any error from `read_superblock` to a bare `Ext4Error::io()` with no
context. So when the filesystem block is smaller than the device's logical sector (a 1 KiB block
image on a device of 4096-byte sectors, for example), or when the device holds no ext4
filesystem at all, mount reports only `Ext4Error { kind: Io, context: None }`. Nobody can tell
that apart from a device read that really failed. After `ax-fs-ng`'s mapping, a mount on
StarryOS/ArceOS returns `EIO` for all of these.

To reproduce, mount a zeroed device, or a 1 KiB block `mke2fs` image, on a device of 4096-byte
sectors. The expected result is `InvalidMagic` and `BadSuperblock` with the context
`device:filesystem_geometry` respectively; the actual result is `Io` for both. The components
affected are `fs/rsext4`, and `fs/ax-fs-ng` through it.

## Proposed change

- Scope: the mount entry points propagate `read_superblock`'s error as it is, and the magic check
  moves into `read_superblock` ahead of the block-size check, so a non-ext4 device always reports
  `InvalidMagic`.
- Not included: how other error paths classify errors, and `ax-fs-ng`'s error mapping.
- Compatibility: the error kind a caller sees changes (`Io` becomes `BadSuperblock` or
  `InvalidMagic`). `ax-fs-ng` maps both to `VfsError::InvalidInput`, so on StarryOS/ArceOS these
  refusals change from `EIO` to `EINVAL`: they concern the superblock's contents, not a failed
  device read.
- Owner: @calef. Target quarter: 2026Q4.

## Testing and verification

- Acceptance: a new test fails before the fix and passes after it. A zeroed device reports
  `InvalidMagic`; a 1 KiB block superblock holding only the magic, on a device of 4096-byte
  sectors, reports `BadSuperblock` with the context `device:filesystem_geometry`.
- Environment: x86_64 Linux, `cargo xtask test`, e2fsprogs.
```

## Pull request body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#<to be filled>

`Ext4FileSystem::mount`、`device_has_error_state` 和日志重放后的 `reload_after_journal_replay` 都把 `read_superblock` 的任何错误映射成不带上下文的 `Ext4Error::io()`。`read_superblock` 读出超级块后会按其中的块大小调用 `set_filesystem_block_size`；当文件系统块小于设备逻辑扇区时（例如 1 KiB 块的镜像放在 4096 字节扇区的设备上），这一步返回带有 `device:filesystem_geometry` 上下文的 `BadSuperblock`。映射之后，调用者只看到 `Ext4Error { kind: Io, context: None }`，与一次真正失败的设备读取无法区分，也无从得知该换一个扇区大小的设备还是更换介质。

同样的映射还掩盖了没有 ext4 文件系统的设备：全零设备的块大小字段为 0，即 1 KiB 块（ext4 格式文档，即 Linux 内核 Documentation/filesystems/ext4/，“3.1. Super Block”一节给出块大小为 2 ^ (10 + `s_log_block_size`)），在 4096 字节扇区的设备上同样先在块大小一步失败，于是报 `Io` 而不是 `InvalidMagic`。

## 修改方案

- `read_superblock`（`fs/rsext4/src/ext4/mkfs.rs`）在解码后、检查块大小之前先检查魔数，魔数不符（同一节规定 `s_magic` 为 0xEF53）时返回 `Ext4Error::invalid_magic()`。这样做是因为传播错误后，块大小一步会先于调用者的魔数检查失败，非 ext4 设备就会被报成 `BadSuperblock`；先检查魔数让“这不是 ext4”始终优先于“这个 ext4 的几何不合适”，也让三个调用点不必各自重复同一检查。
- `mount.rs` 中三处 `read_superblock(...).map_err(|_| Ext4Error::io())` 改为直接用 `?` 传播错误，并删除其后已经重复的魔数检查。错误已经由 `read_superblock` 及其调用的函数准确分类，并带有操作上下文；在挂载入口再统一改写成 `Io` 只会丢失信息。原来的映射没有注释说明意图。
- `mkfs_with_options` 末尾的回读检查改为直接返回 `read_superblock` 的结果；它原来在魔数不符时返回 `corrupted()`，现在返回 `invalid_magic()`。这是魔数检查移入 `read_superblock` 的直接结果。
- 在 `fs/rsext4/tests/error_handling.rs` 中新增测试 `mount_reports_why_it_refused_the_superblock`：全零设备必须报 `InvalidMagic`；只写入魔数的 1 KiB 块超级块放在 4096 字节扇区的设备上必须报 `BadSuperblock`，且上下文为 `device:filesystem_geometry`。测试复用现有的 `ErrorMockDevice`（4096 字节扇区），两种情况放在同一个测试里，因为它们共同证明一个行为：挂载拒绝时报告真实原因。
- 兼容性和受影响范围：调用者看到的错误类型改变，`Io` 变为 `BadSuperblock` 或 `InvalidMagic`，mkfs 的回读在魔数不符时报 `InvalidMagic` 而不是 `Corrupted`。仓库中经由挂载接收这些错误的是 `ax-fs-ng`：它的 `into_vfs_err`（`fs/ax-fs-ng/src/fs/ext4/rsext4/util.rs`）把 `BadSuperblock` 和 `InvalidMagic` 映射为 `VfsError::InvalidInput`，把 `Io` 映射为 `VfsError::Io`。因此在 StarryOS/ArceOS 上，因这些原因被拒绝的挂载从返回 `EIO` 变为返回 `EINVAL`，因为拒绝的原因是超级块的内容，而不是设备读取失败。成功挂载的路径不变。

## 测试验证

- 先在未修复的代码上运行新测试，它失败：`left: Io, right: InvalidMagic`，错误为 `Ext4Error { kind: Io, context: None }`。修复后通过。
- `cargo xtask clippy --package rsext4` 通过（3 项检查），`cargo fmt` 无改动。
- `cargo xtask test --since 6e80b940d` 中，除下面三项外所有被选中的软件包都通过。`rsext4` 的 `tests/filesystem_growth.rs` 中 `growth_recovers_commit_checkpoint_and_flush_interruptions` 和 `growth_preserves_files_and_exposes_new_groups_to_linux`，以及 `ax-fs-ng` 的 `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents` 失败；这三项在未改动的基准提交上以同样的方式失败，与本改动无关。原因是本机 `mke2fs`（e2fsprogs 1.47.2）默认启用 `orphan_file`，而扩容在 `GrowthLayout::check`（`fs/rsext4/src/ext4/grow/layout.rs`）拒绝该特性（`grow:features_or_durability`）。`orphan_file` 是兼容特性：ext4 格式文档（Linux 内核 Documentation/filesystems/ext4/）“3.1. Super Block”一节规定，`s_feature_compat` 中的特性即使不被理解，也仍可读写该文件系统，因此这一拒绝是 `rsext4` 扩容自身的限制，不是格式规则的要求。
- 另用一个外部读取比对工具（用 `mke2fs` 生成 1 KiB、2 KiB、4 KiB 块的镜像，分别放在 512 和 4096 字节扇区的设备上挂载）验证：修复前 1 KiB 和 2 KiB 块镜像在 4096 字节扇区上被拒绝为 `Ext4Error { kind: Io, context: None }`，修复后为 `BadSuperblock` 及 `device:filesystem_geometry` 上下文；其余镜像与扇区组合的结果不变。
- 未验证：QEMU 和实体板卡。本改动只影响挂载失败时返回的错误类型，不改变任何成功挂载的路径。
```

## Pull request body (English rendering, for review)

```markdown
## Current problem

Linked issue: #<to be filled>

`Ext4FileSystem::mount`, `device_has_error_state` and `reload_after_journal_replay` (run after a
journal replay) all map any error from `read_superblock` to a bare `Ext4Error::io()` with no
context. After decoding the superblock, `read_superblock` calls `set_filesystem_block_size` with
its block size. When the filesystem block is smaller than the device's logical sector (a 1 KiB
block image on a device of 4096-byte sectors, for example), that step returns `BadSuperblock`
with the context `device:filesystem_geometry`. After the mapping, the caller sees only
`Ext4Error { kind: Io, context: None }`, which nobody can tell apart from a device read that
really failed, and gives no hint whether to use a device with another sector size or to replace the
medium.

The same mapping hides a device that holds no ext4 filesystem. A zeroed device's block-size field
is 0, which means 1 KiB blocks. The ext4 format documentation (the Linux kernel's
Documentation/filesystems/ext4/), section "3.1. Super Block", gives the block size as
2 ^ (10 + `s_log_block_size`). So on a device of 4096-byte sectors a zeroed device also fails
first at the block size, and mount reports `Io` rather than `InvalidMagic`.

## Proposed change

- `read_superblock` (`fs/rsext4/src/ext4/mkfs.rs`) checks the magic after decoding and before the
  block size, and returns `Ext4Error::invalid_magic()` when it is not 0xEF53, the `s_magic` value the same
  section gives. Once errors
  propagate, the block-size step would fail before the callers' magic checks, and mount would
  report a non-ext4 device as `BadSuperblock`. Checking the magic first keeps "this is not ext4"
  ahead of "this ext4's geometry does not fit", and spares the three call sites from each
  repeating the same check.
- The three `read_superblock(...).map_err(|_| Ext4Error::io())` in `mount.rs` propagate the error
  with `?`, and the change deletes the magic checks after them, now duplicates. `read_superblock`
  and the functions it calls already classify the error and attach the operation, so rewriting it
  to `Io` at the mount entry only loses information. The old mapping carries no comment stating an
  intent.
- The read-back check at the end of `mkfs_with_options` returns `read_superblock`'s result
  directly. It used to return `corrupted()` on a bad magic and now returns `invalid_magic()`, a
  direct result of moving the magic check into `read_superblock`.
- A new test, `mount_reports_why_it_refused_the_superblock` in `fs/rsext4/tests/error_handling.rs`:
  mount must refuse a zeroed device as `InvalidMagic`, and must refuse a 1 KiB block superblock
  holding only the magic, on a device of 4096-byte sectors, as `BadSuperblock` with the context
  `device:filesystem_geometry`. The test reuses the existing `ErrorMockDevice` (4096-byte
  sectors). The two cases share one test because together they prove one behavior: a refused
  mount reports its real reason.
- Compatibility and scope: the error kind a caller sees changes. `Io` becomes `BadSuperblock` or
  `InvalidMagic`, and mkfs's read-back reports `InvalidMagic` where it reported `Corrupted`. In
  the repository, `ax-fs-ng` receives these errors through mount. Its `into_vfs_err`
  (`fs/ax-fs-ng/src/fs/ext4/rsext4/util.rs`) maps `BadSuperblock` and `InvalidMagic` to
  `VfsError::InvalidInput`, and `Io` to `VfsError::Io`. So on StarryOS/ArceOS, a mount refused
  for these reasons changes from returning `EIO` to returning `EINVAL`, because the reason is the
  superblock's contents, not a failed device read. No path of a mount that succeeds changes.

## Testing and verification

- We ran the new test first on the unfixed code, and it failed (`left: Io, right: InvalidMagic`, the
  error `Ext4Error { kind: Io, context: None }`). It passes with the fix.
- `cargo xtask clippy --package rsext4` passes (3 checks), and `cargo fmt` changes nothing.
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
- Also checked with an external read comparison tool, which mounts `mke2fs` images with 1 KiB,
  2 KiB and 4 KiB blocks on devices of 512-byte and 4096-byte sectors. Before the fix, mount refuses the
  1 KiB and 2 KiB block images on 4096-byte sectors as `Ext4Error { kind: Io, context: None }`;
  after it, as `BadSuperblock` with the context `device:filesystem_geometry`. Every other image
  and sector pairing gives the same result as before.
- Not verified: QEMU and physical boards. The change alters only the error a failed mount returns,
  and no path of a mount that succeeds.
```

## Notes for the reviewer (not filed)

- **Design choice upstream might refuse.** The errno a StarryOS/ArceOS mount returns changes from
  `EIO` to `EINVAL` for these refusals. A program on those systems that treated `EIO` from mount
  as "this device cannot hold this filesystem" would change behavior.
- "An external read comparison tool" is our `tools/ext4_oracle`, run through a local, uncommitted
  `[patch]` of `rsext4`. When the fix lands upstream and we move to it, the oracle's
  `expected.txt` lines for those three image and sector pairings change with it.

## Test evidence

Red, with the test on the unfixed code (`cargo test -p rsext4 --features host-test --test
error_handling mount_reports_why_it_refused_the_superblock`):

```text
test error_handling_tests::mount_reports_why_it_refused_the_superblock ... FAILED
thread 'error_handling_tests::mount_reports_why_it_refused_the_superblock' panicked at fs/rsext4/tests/error_handling.rs:167:9:
assertion `left == right` failed: Ext4Error { kind: Io, context: None }
  left: Io
 right: InvalidMagic
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
```

Green, with the fix:

```text
test error_handling_tests::mount_reports_why_it_refused_the_superblock ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
```

`cargo xtask clippy --package rsext4`:

```text
clippy summary: 1 package(s), 3 check(s), 1 package(s) passed, 0 package(s) failed, 0 package(s) skipped
all clippy checks passed
```

`cargo xtask test --since 6e80b940d`, on the commit and on the unchanged base (the same three
failures on both):

```text
std tests failed for 2 package(s): ax-fs-ng, rsext4
test growth_recovers_commit_checkpoint_and_flush_interruptions ... FAILED
test growth_preserves_files_and_exposes_new_groups_to_linux ... FAILED
test root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents ... FAILED
```

The oracle's `compare.py` against its recorded outcomes, with the fix; these are the only
differences:

```text
expected, not seen: default-1k	4096	refused	Ext4Error { kind: Io, context: None }
expected, not seen: default-2k	4096	refused	Ext4Error { kind: Io, context: None }
expected, not seen: no-metadata-csum-1k	4096	refused	Ext4Error { kind: Io, context: None }
seen, not expected: default-1k	4096	refused	Ext4Error { kind: BadSuperblock, context: Some(Operation { op: "device:filesystem_geometry" }) }
seen, not expected: default-2k	4096	refused	Ext4Error { kind: BadSuperblock, context: Some(Operation { op: "device:filesystem_geometry" }) }
seen, not expected: no-metadata-csum-1k	4096	refused	Ext4Error { kind: BadSuperblock, context: Some(Operation { op: "device:filesystem_geometry" }) }
```
