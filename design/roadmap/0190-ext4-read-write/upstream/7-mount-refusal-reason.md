# Upstream draft 7: mount reports why it refused the superblock

Item 7 of [milestone 190](../../0190-ext4-read-write.md)'s upstream round. The slug is
provisional. Nobody has filed it; the architect reviews this text before anyone files it.

- **Repository and base.** rcore-os/tgoskits, branch `dev`, commit
  `6e80b940d46167a59a795e158bb17a7eb28333af`, still the head of `dev` as of this draft.
- **Patch.** [7-mount-refusal-reason.patch](7-mount-refusal-reason.patch), one commit.
- **Pull request title.** `fix(rsext4): report why mount refused the superblock`

## Pull request body (Chinese, as filed)

### 问题

`Ext4FileSystem::mount`、`device_has_error_state` 和日志重放后的 `reload_after_journal_replay` 都把 `read_superblock` 的任何错误映射成不带上下文的 `Ext4Error::io()`。`read_superblock` 读出超级块后会按其中的块大小调用 `set_filesystem_block_size`；当文件系统块小于设备逻辑扇区时（例如 1 KiB 块的镜像放在 4096 字节扇区的设备上），这一步返回带有 `device:filesystem_geometry` 上下文的 `BadSuperblock`。映射之后，调用者只看到 `Ext4Error { kind: Io, context: None }`，与一次真正失败的设备读取无法区分，也无从得知该换一个扇区大小的设备还是更换介质。

同样的映射还掩盖了没有 ext4 文件系统的设备：全零设备的块大小字段为 0，即 1 KiB 块，在 4096 字节扇区的设备上同样先在块大小一步失败，于是报 `Io` 而不是 `InvalidMagic`。

### 改动

- `read_superblock`（`fs/rsext4/src/ext4/mkfs.rs`）在解码后、检查块大小之前先检查魔数，魔数不符时返回 `Ext4Error::invalid_magic()`。
- `mount.rs` 中三处 `read_superblock(...).map_err(|_| Ext4Error::io())` 改为直接用 `?` 传播错误，并删除其后已经重复的魔数检查。
- `mkfs_with_options` 末尾的回读检查改为直接返回 `read_superblock` 的结果；它原来在魔数不符时返回 `corrupted()`，现在返回 `invalid_magic()`。
- 在 `fs/rsext4/tests/error_handling.rs` 中新增测试 `mount_reports_why_it_refused_the_superblock`：全零设备必须报 `InvalidMagic`；只写入魔数的 1 KiB 块超级块放在 4096 字节扇区的设备上必须报 `BadSuperblock`，且上下文为 `device:filesystem_geometry`。

### 每一步的理由

- 错误已经由 `read_superblock` 及其调用的函数准确分类，并带有操作上下文；在挂载入口再统一改写成 `Io` 只会丢失信息，因此改为原样传播。原来的映射没有注释说明意图，我们也没有找到依赖 `Io` 的调用者（仓库内只有测试检查这些错误类型）。
- 魔数检查移入 `read_superblock`，是因为传播错误后，块大小一步会先于调用者的魔数检查失败，非 ext4 设备就会被报成 `BadSuperblock`。先检查魔数让“这不是 ext4”始终优先于“这个 ext4 的几何不合适”，也让三个调用点不必各自重复同一检查。
- 测试复用现有的 `ErrorMockDevice`（4096 字节扇区），两种情况放在同一个测试里，因为它们共同证明一个行为：挂载拒绝时报告真实原因。

### 验证

- 先在未修复的代码上运行新测试，它失败：`left: Io, right: InvalidMagic`，错误为 `Ext4Error { kind: Io, context: None }`。修复后通过。
- `cargo xtask clippy --package rsext4` 通过（3 项检查），`cargo fmt` 无改动。
- `cargo xtask test --since 6e80b940d` 中，除下面三项外所有被选中的软件包都通过。`rsext4` 的 `tests/filesystem_growth.rs` 中 `growth_recovers_commit_checkpoint_and_flush_interruptions` 和 `growth_preserves_files_and_exposes_new_groups_to_linux`，以及 `ax-fs-ng` 的 `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents` 失败；这三项在未改动的基准提交上以同样的方式失败（e2fsprogs 1.47.2，x86_64 Linux），与本改动无关。
- 另用一个外部读取比对工具（用 `mke2fs` 生成 1 KiB、2 KiB、4 KiB 块的镜像，分别放在 512 和 4096 字节扇区的设备上挂载）验证：修复前 1 KiB 和 2 KiB 块镜像在 4096 字节扇区上被拒绝为 `Ext4Error { kind: Io, context: None }`，修复后为 `BadSuperblock` 及 `device:filesystem_geometry` 上下文；其余镜像与扇区组合的结果不变。
- 未验证：QEMU 和实体板卡。本改动只影响挂载失败时返回的错误类型，不改变任何成功挂载的路径。

## Pull request body (English rendering, for review)

### Problem

`Ext4FileSystem::mount`, `device_has_error_state` and `reload_after_journal_replay` (run after a
journal replay) all map any error from `read_superblock` to a bare `Ext4Error::io()` with no
context. After decoding the superblock, `read_superblock` calls `set_filesystem_block_size` with
its block size. When the filesystem block is smaller than the device's logical sector (a 1 KiB
block image on a device of 4,096-byte sectors, for example), that step returns `BadSuperblock`
with the context `device:filesystem_geometry`. After the mapping, the caller sees only
`Ext4Error { kind: Io, context: None }`, which nobody can tell apart from a device read that
really failed, and gives no hint whether to use a device with another sector size or to replace the
medium.

The same mapping hides a device that holds no ext4 filesystem. A zeroed device's block-size field
is 0, meaning 1 KiB blocks, so on a device of 4,096-byte sectors it also fails first at the block
size, and mount reports `Io` rather than `InvalidMagic`.

### Change

- `read_superblock` (`fs/rsext4/src/ext4/mkfs.rs`) checks the magic after decoding and before the
  block size, and returns `Ext4Error::invalid_magic()` when it does not match.
- The three `read_superblock(...).map_err(|_| Ext4Error::io())` in `mount.rs` propagate the error
  with `?`, and the change deletes the magic checks after them, now duplicates.
- The read-back check at the end of `mkfs_with_options` returns `read_superblock`'s result
  directly. It used to return `corrupted()` on a bad magic and now returns `invalid_magic()`.
- A new test, `mount_reports_why_it_refused_the_superblock` in `fs/rsext4/tests/error_handling.rs`:
  mount must refuse a zeroed device as `InvalidMagic`, and must refuse a 1 KiB block superblock
  holding only the magic, on a device of 4,096-byte sectors, as `BadSuperblock` with the context
  `device:filesystem_geometry`.

### Reasoning of each step

- `read_superblock` and the functions it calls already classify the error and attach the
  operation. Rewriting it to `Io` at the mount entry only loses information, so mount now
  propagates the error as it is. The old mapping carries no comment stating an intent, and we found no
  caller that depends on `Io` (inside the repository only tests examine these error kinds).
- The magic check moves into `read_superblock` because, once errors propagate, the block-size step
  would fail before the callers' magic checks and mount would report a non-ext4 device as
  `BadSuperblock`. Checking the magic first keeps "this is not ext4" ahead of "this ext4's
  geometry does not fit", and spares the three call sites from each repeating the same check.
- The test reuses the existing `ErrorMockDevice` (4,096-byte sectors). The two cases share one
  test because together they prove one behaviour: a refused mount reports its real reason.

### Verification

- We ran the new test first on the unfixed code, and it failed (`left: Io, right: InvalidMagic`, the
  error `Ext4Error { kind: Io, context: None }`). It passes with the fix.
- `cargo xtask clippy --package rsext4` passes (3 checks), and `cargo fmt` changes nothing.
- In `cargo xtask test --since 6e80b940d`, every selected package passes except three tests:
  `growth_recovers_commit_checkpoint_and_flush_interruptions` and
  `growth_preserves_files_and_exposes_new_groups_to_linux` in `rsext4`'s
  `tests/filesystem_growth.rs`, and `ax-fs-ng`'s
  `root::grow::tests::root_growth_preserves_other_partitions_and_ext4_contents`. These three fail
  the same way on the unchanged base commit (e2fsprogs 1.47.2, x86_64 Linux), so this change does
  not cause them.
- Also checked with an external read comparison tool, which mounts `mke2fs` images with 1 KiB,
  2 KiB and 4 KiB blocks on devices of 512-byte and 4,096-byte sectors. Before the fix, mount refuses the
  1 KiB and 2 KiB block images on 4,096-byte sectors as `Ext4Error { kind: Io, context: None }`;
  after it, as `BadSuperblock` with the context `device:filesystem_geometry`. Every other image
  and sector pairing gives the same result as before.
- Not verified: QEMU and physical boards. The change alters only the error a failed mount returns,
  and no path of a mount that succeeds.

## Notes for the reviewer (not filed)

- **Design choice upstream might refuse.** The error kind a caller sees changes: `Io` becomes
  `BadSuperblock` or `InvalidMagic`, and mkfs's read-back reports `InvalidMagic` where it reported
  `Corrupted`. A caller that matched on `Io` to mean "this device cannot hold this filesystem"
  would change behaviour. We found none in the repository.
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
