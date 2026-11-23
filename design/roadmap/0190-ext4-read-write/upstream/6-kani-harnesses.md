# Item 6: Kani harnesses for the on-disk parsers of `rsext4`

The draft of upstream item 6 in [milestone 190](../../0190-ext4-read-write.md): the claims nife
proves of its own on-disk parsers, written for `rsext4`'s. It is upstream as
[rcore-os/tgoskits#2836](https://github.com/rcore-os/tgoskits/issues/2836), with the title and
Chinese body below.

- Repository: `rcore-os/tgoskits`, branch `dev`, package `fs/rsext4` (0.9.2).
- Base commit: `0353572d1e2cca79e25a2805deeff4521449a5c1`, the head of `dev` at filing. Its only
  change under `fs/rsext4` since `6e80b940d` is in `src/ext4/alloc.rs`, which no harness reads.
- The harnesses:
  [tools/ext4_oracle/tests/parser_proofs.rs](../../../../tools/ext4_oracle/tests/parser_proofs.rs),
  in nife, against the crate's public API. There is no patch beside this file, because the
  submission is an issue, not a pull request.
- Issue settings, which upstream's contributing guide (section 3.7) requires in the sidebar:
  Assignees `calef`, labels `task` and `q4`, Type `Task`, Milestone `2026Q4`. GitHub dropped these
  at creation, because the filing account has no triage permission on the repository; the guide asks
  a maintainer to fill them in.

## Why an issue and not a pull request

`tgoskits` runs no Kani today: no harness, no `cfg(kani)`, no CI job. Harnesses inside the crate
would compile out of every ordinary build, so they would only hold if upstream also ran them, and
that means adopting a new verification tool in its CI, which is upstream's design choice to make.
A pull request would ask for that decision by surprise. The issue asks it plainly and offers the
harnesses either way.

Until upstream answers, the harnesses stay in nife, where `script/verify` runs them. If
upstream wants them, the follow-up is a pull request that moves them into `fs/rsext4` under
`#[cfg(kani)]` with a CI job; its text goes to the architect for review before filing, like this
one.

## How nife runs them without `rsext4` in a shipping lockfile

The 2026-11-16 ruling on #3639 allows `rsext4` only as a host-only dependency of
`tools/ext4_oracle`, pinned at `=0.9.2`, in a workspace of its own. The harnesses keep to that:

- They are an integration test of that same workspace (`tools/ext4_oracle/tests/parser_proofs.rs`),
  so they add no new package, no new dependency and no new lockfile. The `kani` crate is the one
  Kani provides at verification time, and the file is `#![cfg(kani)]`, so an ordinary `cargo test`
  builds an empty test binary.
- deny.toml's `wrappers = ["ext4_oracle"]` entry for `rsext4` still holds: the only package that
  depends on `rsext4` is `ext4_oracle`.
- `script/verify` runs them as its `ext4_oracle` row, with
  `cargo kani -p ext4_oracle --manifest-path tools/ext4_oracle/Cargo.toml --tests -Z stubbing`.
  `helpers/rust_source.py` lists `tools/ext4_oracle` in `PROVED_WORKSPACES`, so the three
  harness counts (`script/lint`, `script/falsifications --count` and the text count) include the
  workspace, and the lint that requires every harness file to have a `script/verify` row sees it.
- Each harness carries an `attested` falsification record with its by-hand replay: apply the
  named change to a copy of `rsext4` 0.9.2, point a local `[patch]` at it, and run the harness.
  `script/falsifications` sweeps only the nife workspace, so it cannot replay them itself.

## The harnesses

Each bound below is the size of the arbitrary input. A bound marked "exactly" covers that one
length; every other harness covers each length from zero up to its bound.
The two CRC32C harnesses stub `__cpuid` to report no features, so the software path runs: Kani does
not model inline assembly, and the hardware path is one instruction per step, not code a prover
reads.

| harness | the claim | bound | time |
|---|---|---|---|
| `software_crc32c_tail_step_equals_bitwise_definition` | the one-byte step of `crc32c_append` equals the bit-at-a-time definition for every seed, and `crc32c_init` and `crc32c_finalize` are the standard ones | exactly 1 byte | 5 s |
| `software_crc32c_tables_equal_bitwise_definition` | every entry of the eight slicing tables equals the bit-at-a-time definition: a round over a word that is zero but for one byte reads one entry | exactly 8 bytes, one of them arbitrary | 10 s |
| `superblock_decode_reads_only_its_record` | decoding any 1,024 bytes does not panic, so no read falls past the record | exactly 1,024 bytes | 125 s |
| `group_descriptor_decode_is_total_and_stays_in_its_record` | `decode_checked` never panics, accepts exactly 32, 64 and 128 bytes, and a 32-byte record has every high half zero | 128 bytes | 11 s |
| `extent_records_decode_reads_only_twelve_bytes` | the header, extent and index decode without panicking and read only their twelve bytes | 12 bytes against 16 | 25 s |
| `extent_node_parse_is_total` | `ExtentTree::parse_node` never panics, and an accepted node holds no more entries than fit after its header | 24 bytes (a header and one entry) | 76 s |
| `directory_entry_parse_is_total` | `Ext4DirEntryInfo::parse_from_bytes` never panics, and the name lies inside the slice after the eight-byte header | 24 bytes | 6 s |
| `directory_block_walk_ends_inside_its_block` | `DirEntryIterator` ends after at most one step per eight bytes, and each entry, name included, lies inside its record and its record inside the block | 24 bytes | 53 s |

Times are wall clock on one x86_64 Linux host with Kani 0.68.0 and CBMC 6.11.0, build excluded.
Three bounds sit below what the claim would ideally cover, because a larger input did not finish in
fifteen minutes. They are the extent node at 36 bytes (a header and two entries), the directory walk
at 32 bytes, and a whole slicing round of CRC32C with an arbitrary seed. The tables harness covers
the round by its parts, and its doc says what it leaves to linearity.

## Issue title

```text
test(rsext4): Kani harnesses for the on-disk parsers, would you take them?
```

## Issue body (Chinese, as filed)

```markdown
## 当前问题

`fs/rsext4` 目前没有 Kani 证明，也没有运行 Kani 的 CI 任务。我们在自己的项目中用 Kani 模型检查器证明自己的磁盘格式解析器的几条性质：对给定上界以内的每一个输入，解析只读取给定切片内的字节、不会 panic；目录块遍历一定结束并且不越出块。我们为 `rsext4` 0.9.2 的公共接口写了对应的 Kani 证明，目前放在我们自己的仓库中，每次验证都运行。

想问的问题：是否希望把这些证明放进 `fs/rsext4`？CI 中是否引入 Kani 需要你们决定，所以先在这里询问。

## 修改方案

如果希望，我们会提交一个 PR：证明放在 `#[cfg(kani)]` 下，普通构建和测试中不编译；`Cargo.toml` 加上 `cfg(kani)` 的 `check-cfg`，使 clippy 不报警告；再加一个运行 `cargo kani` 的 CI 任务。

如果不需要，我们会继续在自己的仓库中针对已发布版本运行它们，发现的问题会单独报告。

不包含：本 Issue 本身不修改代码；任何 PR 都等你们答复之后再提交。兼容性：证明在 `#[cfg(kani)]` 下，普通构建、测试和公共接口都不受影响。

负责人：@calef。期望季度：2026Q4。

## 测试验证

在 0.9.2 上全部通过，每条都标明输入大小（写“以内”的覆盖其中每一个长度）：

- 软件 CRC32C（`crc32c_append`，按 8 字节切片的查表实现）与逐位定义（RFC 3720 B.4 的反射多项式）比较，分两部分：单字节步骤（短于 8 字节的输入和每个输入的尾部都走这一步）对任意种子和任意字节等于逐位定义；8 张切片表的每一项都等于逐位定义（种子为 0、8 字节中只有任意位置的一个任意字节非 0 时，一轮恰好读取一张表的一项）。`crc32c_init` 和 `crc32c_finalize` 是标准的初值和终值。整轮对任意种子和任意 8 字节相等这一步没有由 Kani 检查：它由 CRC 去掉初值和终值后在 GF(2) 上的线性推出，求解器在 15 分钟内无法直接完成整轮。证明中把 `__cpuid` 替换为不报告任何特性，使软件路径运行；Kani 不建模内联汇编。
- `Ext4Superblock::from_disk_bytes`：任意内容的 1024 字节记录都不会 panic，因此没有读取越过记录末尾（每次读取都对切片做下标访问，越界即 panic）。
- `Ext4GroupDesc::decode_checked`：128 字节以内的任意切片都不会 panic，恰好接受 32、64、128 字节，32 字节记录的所有 `_hi` 字段都为 0。
- `Ext4ExtentHeader`、`Ext4Extent`、`Ext4ExtentIdx` 的 `from_disk_bytes`：只读取自己的 12 字节。
- `ExtentTree::parse_node`：24 字节以内（一个头部加一条记录）不会 panic，接受的节点的记录数不超过头部之后能容纳的数量。
- `Ext4DirEntryInfo::parse_from_bytes`：24 字节以内不会 panic，名字位于头部 8 字节之后、切片之内。
- `DirEntryIterator`：24 字节以内的目录块，遍历最多每 8 字节一步就结束，每个条目（包括名字）都在自己的记录内，记录在块内。

每个证明都做过反证：在 crate 的副本中删掉对应的边界检查（或改错一个偏移），证明随之失败。

验证方法：`cargo kani --tests -Z stubbing`（Kani 0.68.0），每个证明都在 15 分钟内完成。
```

## English rendering of the issue

```markdown
## Current problem

`fs/rsext4` has no Kani proofs today, and no CI job that runs Kani. In our own project we use the Kani model checker to prove a few properties of our on-disk format parsers: for every input up to a stated bound, a parse reads only bytes inside the slice it was given and does not panic, and a directory block walk ends and stays inside its block. We wrote the matching Kani proofs against the public API of `rsext4` 0.9.2. They live in our repository for now, and every verification run runs them.

The question: would you like these proofs in `fs/rsext4`? Bringing Kani into CI is your decision, which is why we ask here first.

## Proposed change

If you would, we would send a PR: the proofs under `#[cfg(kani)]`, so ordinary builds and tests do not compile them; a `check-cfg` for `cfg(kani)` in `Cargo.toml`, so clippy does not warn; and a CI job that runs `cargo kani`.

If not, we will keep running them in our repository against each release, and report anything they find separately.

Not included: this issue changes no code, and any PR waits for your answer. Compatibility: the proofs sit under `#[cfg(kani)]`, so ordinary builds, tests and the public API are unaffected.

Owner: @calef. Target quarter: 2026Q4.

## Test and verification

All pass on 0.9.2. Each states its input size, and where it says "up to", every length up to it is covered:

- The software CRC32C (`crc32c_append`, the slicing-by-eight table implementation) against the bitwise definition (the reflected polynomial of RFC 3720 B.4), in two parts: the one-byte step (which every input shorter than 8 bytes, and the tail of every longer one, goes through) equals the definition for any seed and any byte; and every entry of the eight slicing tables equals the definition (with a zero seed and an 8-byte word that is zero but for one arbitrary byte at an arbitrary position, a round reads exactly one entry of one table). `crc32c_init` and `crc32c_finalize` are the standard initial and final values. Kani does not check that a whole round equals the definition for any seed and any 8 bytes: that follows from the linearity of CRC over GF(2) once the initial and final values are removed, and the solver does not finish the whole round directly within 15 minutes. The proof replaces `__cpuid` with one that reports no features, so the software path runs; Kani does not model inline assembly.
- `Ext4Superblock::from_disk_bytes`: a 1024-byte record of any content does not panic, so no read goes past its end (every read indexes the slice, and an index out of range panics).
- `Ext4GroupDesc::decode_checked`: no slice of up to 128 bytes panics, it accepts exactly 32, 64 and 128 bytes, and a 32-byte record has every `_hi` field zero.
- `from_disk_bytes` of `Ext4ExtentHeader`, `Ext4Extent` and `Ext4ExtentIdx`: each reads only its own 12 bytes.
- `ExtentTree::parse_node`: nothing up to 24 bytes (a header and one entry) panics, and an accepted node holds no more entries than fit after its header.
- `Ext4DirEntryInfo::parse_from_bytes`: nothing up to 24 bytes panics, and the name lies after the 8-byte header and inside the slice.
- `DirEntryIterator`: for a directory block of up to 24 bytes, the walk ends after at most one step per 8 bytes, every entry (name included) lies inside its record, and the record inside the block.

Each proof was falsified: with the matching bounds check removed (or one offset made wrong) in a copy of the crate, the proof fails.

Method: `cargo kani --tests -Z stubbing` (Kani 0.68.0); each proof finishes within 15 minutes.
```

## Test evidence

Each harness passes against `rsext4` 0.9.2 from crates.io. Each fails against a copy with the
change its falsification record names, reached through an uncommitted `[patch.crates-io]` in
`tools/ext4_oracle/Cargo.toml`:

| harness | change in the copy | how it failed |
|---|---|---|
| `software_crc32c_tail_step_equals_bitwise_definition` | tail step `(crc >> 8)` made `(crc >> 7)` | its equality assertion |
| `software_crc32c_tables_equal_bitwise_definition` | eighth table index `word >> 56` made `(word >> 56) ^ 1` | its equality assertion |
| `superblock_decode_reads_only_its_record` | one more `read_u32!()` before `s_checksum` | index out of bounds in `read_u32_le` |
| `group_descriptor_decode_is_total_and_stays_in_its_record` | `is_supported_record_size` accepts any size from 32 | its `accepted` assertion and a slice index |
| `extent_records_decode_reads_only_twelve_bytes` | `ee_start_lo` read from `bytes[9..13]` | a slice index |
| `extent_node_parse_is_total` | `max > capacity` and the `entry_truncated` test both dropped | a slice index |
| `directory_entry_parse_is_total` | `data.len() < 8 + name_len` dropped | a slice index |
| `directory_block_walk_ends_inside_its_block` | `rec_len as usize > remaining.len()` dropped | a slice index |

Outside Kani, `cargo clippy --locked --manifest-path tools/ext4_oracle/Cargo.toml --all-targets --
-D warnings` passes, and the lockfile of `tools/ext4_oracle` is unchanged by this work.
