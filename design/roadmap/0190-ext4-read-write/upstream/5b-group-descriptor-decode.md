# Item 5b: decode group descriptors instead of casting the table bytes

The second half of upstream item 5 in [milestone 190](../../0190-ext4-read-write.md): a sound
replacement for the two group-descriptor casts in `rsext4`, tested against 32-byte and 64-byte
descriptors. The CRC32C safety arguments are [item 5a](5a-crc32c-safety-arguments.md), a pull
request of their own. Both submissions are upstream, each with the title and Chinese body below: the
issue as [rcore-os/tgoskits#2832](https://github.com/rcore-os/tgoskits/issues/2832) and the pull
request as [rcore-os/tgoskits#2833](https://github.com/rcore-os/tgoskits/pull/2833), whose one
commit is the patch beside this file.

- Repository: `rcore-os/tgoskits`, branch `dev`, package `fs/rsext4` (0.9.2).
- Base commit: `0353572d1e2cca79e25a2805deeff4521449a5c1` ("fix(x86_vlapic): stop the APIC timer
  without yielding in the vCPU entry window (#2819)"), the head of `dev` at filing. The patch beside this file dates from the earlier base
  `6e80b940d` and applies to this one unchanged.
- The change: [5b-group-descriptor-decode.patch](5b-group-descriptor-decode.patch), one commit,
  `git format-patch` output. It applies with `git am` on the base commit, with or without 5a.
- Upstream's contributing guide (section 4.3) processes no pull request without a linked issue, so
  the issue below goes first, and the pull request body names its number.
- Issue settings, which the guide (section 3.7) requires in the sidebar:
  Assignees `calef`, labels `bug` and `q4`, Type `Bug`, Milestone `2026Q4`.
  GitHub dropped these at creation, because the filing account has no triage
  permission on the repository; the guide asks a maintainer to fill them in.

## What upstream may refuse

The fix changes public API, so the commit carries the breaking marker (`fix(rsext4)!`).
`BlockGroupDescTable::get_desc` returns `Option<Ext4GroupDesc>` by value instead of a reference,
`BlockGroupDescIter` yields values, and `BlockGroupDescTableMut::get_desc_mut` gives way to
`get_desc` and `set_desc`. A reference into the table cannot be sound for a 32-byte record, since
`Ext4GroupDesc` is 64 bytes and a reference to it covers 64 bytes. So the only change that keeps a
reference-returning signature is one that refuses 32-byte tables, which would drop the layout that the ext4 format documentation (3.2 Block
Group Descriptors) gives a filesystem without the `64bit` feature.

The defect does not reach a mounted filesystem. The mount path in `fs/rsext4/src/ext4/mount.rs`
decodes each descriptor with `Ext4GroupDesc::decode_checked` and never builds these views; only the
crate's tests (`blockgroup_description/tests.rs` and `test_contracts.rs`) use them. The break, and
the bug, fall on code outside `tgoskits` that uses the crates.io release's public table types. If
upstream prefers to keep the old names through a deprecation release, the change still stands; only
the removal of `get_desc_mut` would wait.

## Issue (filed)

### Issue title

```text
fix(rsext4): BlockGroupDescTable casts unaligned table bytes to &Ext4GroupDesc
```

### Issue body (Chinese, as filed)

```markdown
## 当前问题

`BlockGroupDescTable::get_desc` 和 `BlockGroupDescTableMut::get_desc_mut`（`fs/rsext4/src/blockgroup_description/table.rs:36` 和 `:158`）把表中某个字节偏移直接转换为 `&Ext4GroupDesc` / `&mut Ext4GroupDesc`。这有三个问题：

- `Ext4GroupDesc` 要求 4 字节对齐，而表字节（例如 `Vec<u8>` 的某个切片）不保证对齐。未对齐时构造引用本身就是未定义行为；debug 构建下 Rust 的对齐检查会直接中止进程（`misaligned pointer dereference`）。
- 转换按本机字节序读取字段，而磁盘格式是小端序（ext4 格式文档，2 High Level Design：“All fields in ext4 are written to disk in little-endian order”）。
- 引用总是覆盖 64 字节。未启用 `64bit` 特性时描述符只有 32 字节，`_hi` 字段只在启用 `64bit` 且 `s_desc_size > 32` 时存在（ext4 格式文档，3.2 Block Group Descriptors）。描述符大小为 32 字节时（例如 `mke2fs -O ^64bit` 或由 ext3 转换而来的文件系统），读取会把下一条记录当作 `_hi` 高半部分；对恰好按大小分配的表，最后一条记录返回 `None`；通过可变视图做的每一次更新都会把高半部分写进下一条记录。

影响范围：挂载路径（`fs/rsext4/src/ext4/mount.rs`）直接用 `Ext4GroupDesc::decode_checked` 解码描述符，不使用这两个视图；仓库中只有 crate 自己的测试使用它们。受影响的是通过 crates.io 发布版的公共接口使用这两个类型的代码。

## 修改方案

两个视图都用 `Ext4GroupDesc::decode_checked` 解码该记录自己的 `desc_size` 字节，可变视图用 `encode_checked` 写回，去掉两处 `unsafe` 转换。

兼容性：这是破坏性修改。`get_desc` 改为按值返回 `Option<Ext4GroupDesc>`，`BlockGroupDescIter` 产出 `Ext4GroupDesc`，`BlockGroupDescTableMut` 的 `get_desc_mut` 改为 `get_desc` 和 `set_desc`。`Ext4GroupDesc` 是 64 字节，32 字节记录不可能安全地借出 64 字节的引用，所以保留引用签名的唯一办法是拒绝 32 字节的表，而那会丢掉 ext4 格式文档（3.2 Block Group Descriptors）为未启用 `64bit` 特性的文件系统规定的布局。如果希望先经过一个弃用版本，可以保留旧名字并标记 `#[deprecated]`。

不包含：CRC32C 路径的安全性注释（另开 Issue）。

负责人：@calef。期望季度：2026Q4。

## 测试验证

新增两个测试，把 32 字节和 64 字节描述符的表放在故意不对齐的缓冲区中，读取每条记录、更新一条记录，并检查相邻记录的字节没有改变。验收条件：修复前这两个测试失败（debug 构建中止，release 构建下 32 字节测试读到错误的值），修复后通过；`cargo test -p rsext4 --lib` 和 `cargo xtask clippy --package rsext4` 全部通过。
```

### English rendering of the issue

```markdown
## Current problem

`BlockGroupDescTable::get_desc` and `BlockGroupDescTableMut::get_desc_mut` (`fs/rsext4/src/blockgroup_description/table.rs:36` and `:158`) cast a byte offset in the table straight to `&Ext4GroupDesc` / `&mut Ext4GroupDesc`. That has three problems:

- `Ext4GroupDesc` needs 4-byte alignment, and the table bytes (a slice of a `Vec<u8>`, say) do not guarantee it. Making the reference when they are not aligned is undefined behavior in itself; a debug build's alignment check aborts the process (`misaligned pointer dereference`).
- The cast reads the fields in native byte order, and the on-disk format is little-endian (ext4 format documentation, 2 High Level Design: "All fields in ext4 are written to disk in little-endian order").
- The reference always spans 64 bytes. Without the `64bit` feature a descriptor is 32 bytes, and the `_hi` fields exist only with `64bit` and `s_desc_size > 32` (ext4 format documentation, 3.2 Block Group Descriptors). With 32-byte descriptors (such as a filesystem made with `mke2fs -O ^64bit` or converted from ext3), a read takes the next record as the `_hi` halves, the last record of an exactly sized table returns `None`, and every update through the mutable view writes high halves into the next record.

Scope: the mount path (`fs/rsext4/src/ext4/mount.rs`) decodes descriptors directly with `Ext4GroupDesc::decode_checked` and does not use these two views; in this repository only the crate's own tests use them. The code affected is code that uses these two types through the public API of the crates.io release.

## Proposed change

Both views decode the record's own `desc_size` bytes with `Ext4GroupDesc::decode_checked`, and the mutable view writes them back with `encode_checked`, removing the two `unsafe` casts.

Compatibility: this is a breaking change. `get_desc` returns `Option<Ext4GroupDesc>` by value, `BlockGroupDescIter` yields `Ext4GroupDesc`, and on `BlockGroupDescTableMut`, `get_desc_mut` becomes `get_desc` and `set_desc`. `Ext4GroupDesc` is 64 bytes, and a 32-byte record cannot soundly lend a 64-byte reference, so the only way to keep a reference-returning signature is to refuse 32-byte tables, which would drop the layout that the ext4 format documentation (3.2 Block Group Descriptors) gives a filesystem without the `64bit` feature. If a deprecation release is wanted first, the old names can stay under `#[deprecated]`.

Not included: the safety comments on the CRC32C paths (a separate issue).

Owner: @calef. Target quarter: 2026Q4.

## Test and verification

Two new tests put a table of 32-byte and one of 64-byte descriptors in a deliberately unaligned buffer, read every record, update one, and check that the bytes of its neighbors did not change. Acceptance: both tests fail before the fix (a debug build aborts, and in a release build the 32-byte test reads the wrong value) and pass after it; `cargo test -p rsext4 --lib` and `cargo xtask clippy --package rsext4` pass.
```

## Pull request title

```text
fix(rsext4)!: decode group descriptors instead of casting
```

## Pull request body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#2832

`BlockGroupDescTable::get_desc` 和 `BlockGroupDescTableMut::get_desc_mut` 把表中某个字节偏移直接转换为 `&Ext4GroupDesc` / `&mut Ext4GroupDesc`：未对齐时这是未定义行为，字段按本机字节序读取而磁盘格式是小端序（ext4 格式文档，2 High Level Design），而且引用总是覆盖 64 字节，所以 32 字节描述符（ext4 格式文档，3.2 Block Group Descriptors：未启用 `64bit` 时描述符为 32 字节）的读取会把下一条记录当作高半部分，更新会写进下一条记录。挂载路径直接用 `Ext4GroupDesc::decode_checked` 解码描述符，不使用这两个视图；仓库中只有 crate 自己的测试使用它们，受影响的是 crates.io 发布版公共接口的使用者。

## 修改方案

- 两个视图都用 `Ext4GroupDesc::decode_checked` 解码该记录自己的 `desc_size` 字节，可变视图用 `encode_checked` 写回，因此 32 字节、64 字节（及更大）的描述符都只触及自己的记录。理由：crate 中已有这两个函数，按记录大小逐字段小端解码和编码，并拒绝不支持的记录大小，不需要新的解码代码。
- `get_desc` 按值返回 `Option<Ext4GroupDesc>`，`BlockGroupDescIter` 产出 `Ext4GroupDesc`；`BlockGroupDescTableMut` 的 `get_desc_mut` 改为 `get_desc` 和 `set_desc`。`update_free_blocks` 等更新方法改为读取、修改、写回。理由：`Ext4GroupDesc` 是 64 字节，32 字节记录不可能安全地借出 64 字节的引用。保留引用签名的唯一办法是拒绝 32 字节的表，而那会丢掉 ext4 格式文档（3.2 Block Group Descriptors）为未启用 `64bit` 特性的文件系统规定的布局。
- 新增两个测试：`descriptor_table_reads_and_updates_32_byte_records_in_place` 和 `descriptor_table_reads_and_updates_64_byte_records_in_place`。它们把表放在故意不对齐的缓冲区中，按小端序写入每条记录，读取每条记录、更新一条记录，并检查相邻记录的字节没有改变。

兼容性：公共签名改变，所以这是破坏性修改。本仓库中除 crate 自己的测试外没有代码调用这些类型。如果希望先经过一个弃用版本，可以保留旧名字并标记 `#[deprecated]`，只把删除 `get_desc_mut` 推迟。

## 测试验证

- 修复前（基线 `0353572d1`，只换入新测试文件）：debug 构建下两个新测试都在 `table.rs:36:23` 以 `misaligned pointer dereference` 中止；release 构建下 32 字节测试在 `tests.rs:133` 失败（`left: Some(18446744069414584577)`，`right: Some(257)`，即读到了下一条记录的字节），64 字节测试通过（release 构建不检查对齐，所以未对齐引用的未定义行为在这里没有表现出来）。
- 修复后：两个新测试通过，`cargo test -p rsext4 --lib` 430 个测试全部通过。
- `cargo xtask clippy --package rsext4`：3 项检查全部通过。
- `cargo fmt --check` 通过。
- `cargo xtask test --since 0353572d1`：除 `rsext4` 的 `tests/filesystem_growth.rs` 中两个测试和 `ax-fs-ng` 的 `root_growth_preserves_other_partitions_and_ext4_contents` 外全部通过。这三个测试在基线提交上以同样方式失败，原因相同：测试机的 `mke2fs` 1.47.2 默认启用 `orphan_file`，扩容在 `fs/rsext4/src/ext4/grow/layout.rs:93` 以 `grow:features_or_durability` 拒绝它。
- 没有运行 QEMU：挂载路径不使用这两个视图，运行时行为没有变化；改变的只有公共表类型，宿主机单元测试已覆盖。
```

## English rendering of the pull request body

```markdown
## Current problem

Linked issue: #<to be filled>

`BlockGroupDescTable::get_desc` and `BlockGroupDescTableMut::get_desc_mut` cast a byte offset in the table straight to `&Ext4GroupDesc` / `&mut Ext4GroupDesc`: that is undefined behavior when the bytes are not aligned, it reads the fields in native byte order where the format is little-endian (ext4 format documentation, 2 High Level Design), and the reference always spans 64 bytes, so a read of a 32-byte descriptor (ext4 format documentation, 3.2 Block Group Descriptors: 32 bytes without the `64bit` feature) takes the next record as the high halves and an update writes into the next record. The mount path decodes descriptors directly with `Ext4GroupDesc::decode_checked` and does not use these two views; in this repository only the crate's own tests use them, and the code affected is code that uses the crates.io release's public API.

## Change

- Both views decode the record's own `desc_size` bytes with `Ext4GroupDesc::decode_checked`, and the mutable view writes them back with `encode_checked`, so 32-byte and 64-byte (and larger) descriptors each touch only their own record. Why: the crate already has both functions. They decode and encode field by field, little-endian, by record size, and refuse an unsupported record size, so no new decoding code is needed.
- `get_desc` returns `Option<Ext4GroupDesc>` by value and `BlockGroupDescIter` yields `Ext4GroupDesc`; on `BlockGroupDescTableMut`, `get_desc_mut` becomes `get_desc` and `set_desc`. The update methods such as `update_free_blocks` now read, change and write back. Why: `Ext4GroupDesc` is 64 bytes, and a 32-byte record cannot soundly lend a 64-byte reference. The only way to keep a reference-returning signature is to refuse 32-byte tables, which would drop the layout that the ext4 format documentation (3.2 Block Group Descriptors) gives a filesystem without the `64bit` feature.
- Two new tests, `descriptor_table_reads_and_updates_32_byte_records_in_place` and `descriptor_table_reads_and_updates_64_byte_records_in_place`, put the table in a deliberately unaligned buffer, write each record little-endian, read every record, update one, and check that the bytes of its neighbors did not change.

Compatibility: the public signatures change, so this is a breaking change. Nothing in this repository outside the crate's own tests calls these types. If a deprecation release is wanted first, the old names can stay under `#[deprecated]`, and only the removal of `get_desc_mut` waits.

## Test and verification

- Before the fix (the base `0353572d1`, with only the new test file swapped in): in a debug build both new tests abort at `table.rs:36:23` with `misaligned pointer dereference`. In a release build the 32-byte test fails at `tests.rs:133` (`left: Some(18446744069414584577)`, `right: Some(257)`, the bytes of the next record) and the 64-byte test passes (a release build does not check alignment, so the undefined behavior of the unaligned reference does not show there).
- After the fix: both new tests pass, and all 430 tests of `cargo test -p rsext4 --lib` pass.
- `cargo xtask clippy --package rsext4`: all 3 checks pass.
- `cargo fmt --check` passes.
- `cargo xtask test --since 0353572d1`: everything passes except two tests in `rsext4`'s `tests/filesystem_growth.rs` and `ax-fs-ng`'s `root_growth_preserves_other_partitions_and_ext4_contents`. The three fail the same way on the base commit, for one reason: the test host's `mke2fs` 1.47.2 enables `orphan_file` by default, and growth refuses it with `grow:features_or_durability` at `fs/rsext4/src/ext4/grow/layout.rs:93`.
- QEMU was not run: the mount path does not use these views, so runtime behavior does not change; only the public table types change, and the host unit tests cover them.
```

## Test evidence

Before filing, every check the pull request cites ran again on `0353572d1`, the head of `dev` then.
With only the new test file on that base, the debug build aborted with `misaligned pointer
dereference` in `get_desc`. The release build failed the 32-byte test with the same values and
passed the 64-byte one. With the patch, `cargo test -p rsext4 --lib` passed 430 tests. `cargo xtask
clippy --package rsext4` passed its 3 checks, and `cargo fmt --check` was clean. `cargo xtask test
--since 0353572d1` failed only the three growth tests, and they fail the same way on unchanged
`dev`. The runs below are the earlier ones, on `6e80b940d`.

Each command ran in the clone on the base commit or on the patch applied to it.

The new tests before the fix, debug build (`cargo test -p rsext4 --lib blockgroup_description`):

```text
thread 'blockgroup_description::tests::descriptor_table_reads_and_updates_32_byte_records_in_place' panicked at fs/rsext4/src/blockgroup_description/table.rs:36:23:
misaligned pointer dereference: address must be a multiple of 0x4 but is 0x5c68df47c711
thread caused non-unwinding panic. aborting.
```

The 64-byte test alone, before the fix, aborts at the same line. The same tests before the fix,
release build (`cargo test --release -p rsext4 --lib blockgroup_description`):

```text
test blockgroup_description::tests::descriptor_table_reads_and_updates_64_byte_records_in_place ... ok
test blockgroup_description::tests::descriptor_table_reads_and_updates_32_byte_records_in_place ... FAILED
thread 'blockgroup_description::tests::descriptor_table_reads_and_updates_32_byte_records_in_place' panicked at fs/rsext4/src/blockgroup_description/tests.rs:133:5:
assertion `left == right` failed
  left: Some(18446744069414584577)
 right: Some(257)
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 424 filtered out
```

After the fix:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 424 filtered out

$ cargo test -p rsext4 --lib
test result: ok. 430 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo xtask clippy --package rsext4
ok: rsext4 (base)
ok: rsext4 (feature: USE_MULTILEVEL_CACHE)
ok: rsext4 (feature: host-test)
passed checks: 3, failed checks: 0, skipped checks: 0

$ cargo xtask test --since 6e80b940d
std tests failed for 2 package(s): ax-fs-ng, rsext4
```

This draft does not run nife's read oracle (`tools/ext4_oracle`) against the patch: it mounts
through `Ext4FileSystem`, and the mount path never builds `BlockGroupDescTable`, so it cannot
observe this change.
