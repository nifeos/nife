# Item 5: the `unsafe` review of `rsext4`

The draft of upstream item 5 in [milestone 190](../../0190-ext4-read-write.md): a safety argument
at each CRC32C `unsafe` use, and a sound replacement for the two group-descriptor casts. Nothing
here is filed. The architect reviews this exact text first, and filing is a separate go.

- Repository: `rcore-os/tgoskits`, branch `dev`, package `fs/rsext4` (0.9.2).
- Base commit: `6e80b940d46167a59a795e158bb17a7eb28333af`, the head of `dev` when the draft was
  checked; `dev` has no later commit under `fs/rsext4`.
- The change: [5-unsafe-review.patch](5-unsafe-review.patch), two commits, `git format-patch`
  output. It applies with `git am` on the base commit.

## What upstream may refuse

The cast replacement changes public API, so the second commit is marked breaking (`fix(rsext4)!`).
`BlockGroupDescTable::get_desc` returns `Option<Ext4GroupDesc>` by value instead of a reference,
`BlockGroupDescIter` yields values, and `BlockGroupDescTableMut::get_desc_mut` gives way to
`get_desc` and `set_desc`. A reference into the table cannot be sound for a 32-byte record, since
`Ext4GroupDesc` is 64 bytes and a reference to it covers 64 bytes. So the only change that keeps a
reference-returning signature is one that refuses 32-byte tables, which would lose a format Linux
writes. No code in `tgoskits` outside the crate's own tests calls these types, so the break falls
on users of the crates.io release. If upstream prefers to keep the old names through a deprecation
release, the change still stands; only the removal of `get_desc_mut` would wait.

## Pull request title

```text
fix(rsext4)!: decode group descriptors instead of casting table bytes, and state the CRC32C safety arguments
```

## Pull request body (Chinese, as filed)

```markdown
## 问题

`fs/rsext4` 中有两处把字节表直接转换为结构体引用的 `unsafe`，以及九处 CRC32C 相关的 `unsafe`，后者有一部分缺少安全性论证。

1. `BlockGroupDescTable::get_desc` 和 `BlockGroupDescTableMut::get_desc_mut` 把表中某个字节偏移直接转换为 `&Ext4GroupDesc` / `&mut Ext4GroupDesc`。这有三个问题：
   - `Ext4GroupDesc` 要求 4 字节对齐，而表字节（例如 `Vec<u8>` 的某个切片）不保证对齐。未对齐时构造引用本身就是未定义行为；debug 构建下 Rust 的对齐检查会直接中止进程（`misaligned pointer dereference`）。
   - 转换按本机字节序读取字段，而磁盘格式是小端序。
   - 引用总是覆盖 64 字节。描述符大小为 32 字节时（未启用 `64bit` 特性的文件系统，例如 `mke2fs -O ^64bit` 或由 ext3 转换而来的文件系统），读取会把下一条记录当作 `_hi` 高半部分；对恰好按大小分配的表，最后一条记录返回 `None`；通过可变视图做的每一次更新都会把高半部分写进下一条记录。
2. CRC32C 路径中，aarch64 的 `mrs` 读取、aarch64 的硬件循环（整个函数体是一个 `unsafe` 块）、以及 `crc32c_append` 和测试中对 `crc32c_hardware` 的三处调用没有说明它们依赖的 CPU 特性在哪里检测。x86_64 的两处 `asm!` 已有说明，本 PR 不修改它们。

## 修改内容

第一个提交（`docs(rsext4)`）只增加安全性注释，执行的指令不变：

- `crc32c_append` 和测试中的每一处 `unsafe { crc32c_hardware(..) }` 都写明所需的 CPU 特性（ARMv8 CRC32 扩展或 SSE4.2）以及检测它的寄存器（`ID_AA64ISAR0_EL1[19:16]` 或 CPUID leaf 1 ECX bit 20）。
- aarch64 的 `mrs` 说明为什么在 EL0 读取 EL1 的 ID 寄存器是有效的：内核为用户态模拟 ID 寄存器（Linux 设置 `HWCAP_CPUID` 时如此）；不模拟的主机会把它作为非法指令陷入。
- aarch64 的硬件循环从一个覆盖整个函数体的 `unsafe` 块拆成每个指针操作一个最小的 `unsafe` 块，每块写明对齐和边界的论证。

第二个提交（`fix(rsext4)!`）去掉两处转换和它们的 `unsafe`：

- 两个视图都用 `Ext4GroupDesc::decode_checked` 解码该记录自己的 `desc_size` 字节，可变视图用 `encode_checked` 写回，因此 32 字节、64 字节（及更大）的描述符都只触及自己的记录。
- `get_desc` 按值返回 `Option<Ext4GroupDesc>`，`BlockGroupDescIter` 产出 `Ext4GroupDesc`；`BlockGroupDescTableMut` 的 `get_desc_mut` 改为 `get_desc` 和 `set_desc`。`update_free_blocks` 等更新方法改为读取、修改、写回。
- 新增两个测试：`descriptor_table_reads_and_updates_32_byte_records_in_place` 和 `descriptor_table_reads_and_updates_64_byte_records_in_place`。它们把表放在故意不对齐的缓冲区中，按小端序写入每条记录，读取每条记录、更新一条记录，并检查相邻记录的字节没有改变。

## 每一步的理由

- 为什么按值返回而不是保留引用：`Ext4GroupDesc` 是 64 字节，32 字节记录不可能安全地借出 64 字节的引用。保留引用签名的唯一办法是拒绝 32 字节的表，而那会失去 Linux 写出的格式。
- 为什么用 `decode_checked` / `encode_checked`：crate 中已有这两个函数，按记录大小逐字段小端解码和编码，并拒绝不支持的记录大小，不需要新的解码代码。
- 为什么这是破坏性修改：公共签名改变了。本仓库中除 crate 自己的测试外没有代码调用这些类型，所以影响落在 crates.io 发布版的用户身上。如果希望先经过一个弃用版本，可以保留旧名字并标记 `#[deprecated]`，只把删除 `get_desc_mut` 推迟。
- 为什么拆分 aarch64 的 `unsafe` 块：仓库规则要求最小的 `unsafe` 块且每块有证明注释；一个覆盖整个函数的块无法把每次读取的论证放在读取旁边。

## 验证情况

已验证：

- 修复前（基线 `6e80b940d`，只换入新测试文件）：debug 构建下两个新测试都在 `table.rs:36:23` 以 `misaligned pointer dereference` 中止；release 构建下 32 字节测试在 `tests.rs:133` 失败（`left: Some(18446744069414584577)`，`right: Some(257)`，即读到了下一条记录的字节），64 字节测试通过（release 构建不检查对齐，所以未对齐引用的未定义行为在这里没有表现出来）。
- 修复后：两个新测试通过，`cargo test -p rsext4 --lib` 430 个测试全部通过。
- `cargo xtask clippy --package rsext4`：3 项检查全部通过。
- `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` 通过（修改的 `arm64.rs` 只在 aarch64 上编译）。
- `cargo fmt --check` 通过。
- `cargo xtask test --since 6e80b940d`：除下面两项外全部通过。

未验证，以及原因：

- `rsext4` 的 `tests/filesystem_growth.rs` 中两个测试和 `ax-fs-ng` 的 `root_growth_preserves_other_partitions_and_ext4_contents` 失败，错误是 `grow:features_or_durability` 返回 `Unsupported`。它们在基线提交上以同样方式失败，与本修改无关：测试机的 `mke2fs` 1.47.2 默认启用 `orphan_file` 兼容特性，而扩容代码拒绝该特性。
- aarch64 的硬件 CRC32C 路径只做了编译和 clippy 检查，没有在 aarch64 硬件上运行。第一个提交不改变执行的指令，只把 `unsafe` 块拆小。
- 没有运行 QEMU：运行时行为只在描述符表读写上改变，宿主机单元测试已覆盖。
```

## English rendering of the body

```markdown
## Problem

`fs/rsext4` has two `unsafe` casts from table bytes to a struct reference, and nine CRC32C `unsafe` uses, some of which state no safety argument.

1. `BlockGroupDescTable::get_desc` and `BlockGroupDescTableMut::get_desc_mut` cast a byte offset in the table straight to `&Ext4GroupDesc` / `&mut Ext4GroupDesc`. That has three problems:
   - `Ext4GroupDesc` needs 4-byte alignment, and the table bytes (a slice of a `Vec<u8>`, say) do not guarantee it. Making the reference when they are not aligned is undefined behaviour in itself; a debug build's alignment check aborts the process (`misaligned pointer dereference`).
   - The cast reads the fields in native byte order, and the on-disk format is little-endian.
   - The reference always spans 64 bytes. With 32-byte descriptors (a filesystem without the `64bit` feature, such as one made with `mke2fs -O ^64bit` or converted from ext3), a read takes the next record as the `_hi` halves, the last record of an exactly sized table returns `None`, and every update through the mutable view writes high halves into the next record.
2. On the CRC32C paths, the aarch64 `mrs` read, the aarch64 hardware loop (its whole body is one `unsafe` block), and the three calls to `crc32c_hardware` in `crc32c_append` and in a test do not say where the CPU feature they rely on is detected. The two x86_64 `asm!` blocks already say so, and this PR leaves them alone.

## Change

The first commit (`docs(rsext4)`) adds safety comments only; the instructions executed are unchanged:

- Each `unsafe { crc32c_hardware(..) }` in `crc32c_append` and in the test names the CPU feature it needs (the ARMv8 CRC32 extension or SSE4.2) and the register it was detected from (`ID_AA64ISAR0_EL1[19:16]` or CPUID leaf 1 ECX bit 20).
- The aarch64 `mrs` says why reading an EL1 ID register at EL0 is valid: the kernel emulates the ID registers for user space (Linux does when it sets `HWCAP_CPUID`), and a host that does not delivers the trap as an illegal instruction.
- The aarch64 hardware loop goes from one `unsafe` block over the whole body to one minimal `unsafe` block per pointer operation, each stating its alignment and bounds argument.

The second commit (`fix(rsext4)!`) removes the two casts and their `unsafe`:

- Both views decode the record's own `desc_size` bytes with `Ext4GroupDesc::decode_checked`, and the mutable view writes them back with `encode_checked`, so 32-byte and 64-byte (and larger) descriptors each touch only their own record.
- `get_desc` returns `Option<Ext4GroupDesc>` by value and `BlockGroupDescIter` yields `Ext4GroupDesc`; on `BlockGroupDescTableMut`, `get_desc_mut` becomes `get_desc` and `set_desc`. The update methods such as `update_free_blocks` now read, change and write back.
- Two new tests, `descriptor_table_reads_and_updates_32_byte_records_in_place` and `descriptor_table_reads_and_updates_64_byte_records_in_place`, put the table in a deliberately unaligned buffer, write each record little-endian, read every record, update one, and check that the bytes of its neighbours did not change.

## Reasoning of each step

- Why return a value and not keep the reference: `Ext4GroupDesc` is 64 bytes, and a 32-byte record cannot soundly lend a 64-byte reference. The only way to keep a reference-returning signature is to refuse 32-byte tables, which would lose a format Linux writes.
- Why `decode_checked` / `encode_checked`: the crate already has both. They decode and encode field by field, little-endian, by record size, and refuse an unsupported record size, so no new decoding code is needed.
- Why this is a breaking change: the public signatures change. Nothing in this repository outside the crate's own tests calls these types, so the break falls on users of the crates.io release. If a deprecation release is wanted first, the old names can stay under `#[deprecated]`, and only the removal of `get_desc_mut` waits.
- Why split the aarch64 `unsafe` block: the repository's rules ask for minimal `unsafe` blocks, each with a proof comment, and one block over the whole function cannot keep each read's argument beside that read.

## What was verified

Verified:

- Before the fix (the base `6e80b940d`, with only the new test file swapped in): in a debug build both new tests abort at `table.rs:36:23` with `misaligned pointer dereference`. In a release build the 32-byte test fails at `tests.rs:133` (`left: Some(18446744069414584577)`, `right: Some(257)`, the bytes of the next record) and the 64-byte test passes (a release build does not check alignment, so the undefined behaviour of the unaligned reference does not show there).
- After the fix: both new tests pass, and all 430 tests of `cargo test -p rsext4 --lib` pass.
- `cargo xtask clippy --package rsext4`: all 3 checks pass.
- `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` passes (the changed `arm64.rs` compiles only on aarch64).
- `cargo fmt --check` passes.
- `cargo xtask test --since 6e80b940d`: everything passes except the two items below.

Not verified, and why:

- Two tests in `rsext4`'s `tests/filesystem_growth.rs` and `ax-fs-ng`'s `root_growth_preserves_other_partitions_and_ext4_contents` fail, with `grow:features_or_durability` returning `Unsupported`. They fail the same way on the base commit and are unrelated to this change: the test host's `mke2fs` 1.47.2 enables the `orphan_file` compat feature by default, and the growth code refuses it.
- The aarch64 hardware CRC32C path was only compiled and checked with clippy, not run on aarch64 hardware. The first commit does not change the instructions executed; it only makes the `unsafe` blocks smaller.
- QEMU was not run: runtime behaviour changes only in descriptor table reads and writes, which the host unit tests cover.
```

## Test evidence

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
test blockgroup_description::tests::descriptor_table_reads_and_updates_32_byte_records_in_place ... ok
test blockgroup_description::tests::descriptor_table_reads_and_updates_64_byte_records_in_place ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 424 filtered out

$ cargo test -p rsext4 --lib
test result: ok. 430 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo xtask clippy --package rsext4
passed checks: 3, failed checks: 0, skipped checks: 0

$ cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo xtask test --since 6e80b940d
std tests failed for 2 package(s): ax-fs-ng, rsext4
```

The two packages that fail there fail only in the growth tests named above, on `grow:features_or_
durability`, and fail the same way on the base commit.

nife's read oracle (`tools/ext4_oracle`) was not run against the patch: it mounts through
`Ext4FileSystem` and never reaches `BlockGroupDescTable`, so it cannot observe this change.
