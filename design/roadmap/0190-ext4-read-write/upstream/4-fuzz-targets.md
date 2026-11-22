# Upstream draft 4: fuzz targets

Item 4 of [milestone 190](../../0190-ext4-read-write.md)'s upstream round. The slug and the seven
target names are provisional. Nobody has filed it; the architect reviews this text before
anyone files it.

- **Repository and base.** rcore-os/tgoskits, branch `dev`, commit
  `6e80b940d46167a59a795e158bb17a7eb28333af`, still the head of `dev` as of this draft.
- **Patch.** [4-fuzz-targets.patch](4-fuzz-targets.patch), one commit, which adds only files under
  `fs/rsext4/fuzz/`.
- **Pull request title.** `test(rsext4): add fuzz targets for the on-disk parsers, mount and journal replay`

## Pull request body (Chinese, as filed)

### 问题

`rsext4` 读取的每个字节都来自磁盘，而磁盘内容可能损坏，也可能被恶意构造（例如插入的 U 盘或下载的镜像）。crate 已有大量针对具体错误的单元测试和契约测试，但没有用任意输入检验解析和挂载路径的手段：超级块、块组描述符、extent 树、目录块、HTree 索引和日志重放遇到不一致的字段时，应当返回错误，而不是越界、溢出、死循环或耗尽内存。仓库中目前没有任何 `cargo fuzz` 目标。

### 改动

新增 `fs/rsext4/fuzz/`，一个 `cargo-fuzz` crate，有七个目标：

| 目标 | 输入 | 覆盖的路径 |
|---|---|---|
| `superblock` | 1024 字节超级块 | 解码、校验和、块大小、块组数、几何检查 |
| `group_descriptor_table` | 超级块后接描述符表 | 按超级块的描述符大小和块组数解码每个描述符，以及 `BlockGroupDescTable` |
| `extent_tree` | 一个 extent 树节点 | `ExtentTree::parse_node` |
| `directory_block` | 一个目录块 | `DirEntryIterator`、`classic_dir::list_entries`、目录块和 HTree 校验尾 |
| `htree_lookup` | 整个镜像 | 只读挂载后对每个目录逐名 `lookup_child`，索引目录走 HTree 查找 |
| `mount_and_walk` | 整个镜像 | 只读挂载，有界遍历目录、元数据、extent、扩展属性和文件内容，然后卸载 |
| `journal_replay` | 整个镜像 | 读写挂载（需要恢复时重放日志），读根目录后卸载 |

- `src/lib.rs` 提供共用部分：以输入为内容的内存块设备（512 字节扇区，因此任何块大小都能挂载）、固定时钟，以及有上限的遍历（最多 512 个 inode，每个文件最多读 256 KiB）。
- `make-seeds.sh` 用 `mke2fs` 和固定时间生成种子语料：1 KiB 和 4 KiB 块、有无 `metadata_csum`、一个由 `e2fsck -D` 建立索引的 150 项目录，以及一个带一条已提交日志事务的镜像。解析器目标的种子从这些镜像中截取。
- `README.md` 说明目标、语料和运行方法。

### 每一步的理由

- 目标只调用 crate 的公开接口，不修改 `rsext4` 本身，因此不改变任何行为，只增加检查手段。
- fuzz crate 有自己的 `[workspace]`，不加入仓库工作区：`cargo fuzz` 需要夜间工具链和 sanitizer 编译选项，加入工作区会让 `cargo xtask` 的构建和检查都去编译 `libfuzzer-sys`。`libfuzzer-sys` 是 `cargo fuzz` 的标准依赖，只在这个不发布的 crate 中使用（`publish = false`）；`rsext4` 的依赖不变。按 `fs/rsext4/.gitignore` 的现有约定，不提交 `Cargo.lock`。
- 整镜像目标用一个内存设备而不是文件，使每次迭代不触碰磁盘；512 字节扇区让 1 KiB 块的镜像也能挂载，否则大部分输入会在块大小一步被拒绝。
- 遍历设上限，是因为损坏的镜像可以让目录成环或声称文件极大；没有上限时一次迭代可能超时，掩盖真正的问题。上限只限制 fuzz 目标的工作量，不改变 crate 的行为。
- 日志种子由脚本自己写入一条已提交事务（描述块、两个数据块、提交块），并标记文件系统需要恢复，因为 `debugfs` 的日志写入命令在这个版本上不留下已提交的事务。脚本用 `debugfs logdump` 确认事务存在，并用 `e2fsck` 重放一份副本、检查目标块确实被改写，任何一步不符都会失败，所以种子总是一个 Linux 工具认可的需要重放的镜像。
- 每个解析器目标都从真实镜像截取种子，使变异从合法结构开始，而不是从随机字节开始。

### 验证

- 每个目标在本地运行 600 秒（`cargo fuzz run -O <目标> corpus/<目标> -- -max_total_time=600 -timeout=10`，rustc nightly 2026-11-12，cargo-fuzz 0.13.2，x86_64 Linux，e2fsprogs 1.47.2），全部没有崩溃、超时或内存超限。运行次数见下文。
- 用一个不提交的小程序确认种子覆盖预期路径：三个遍历镜像和索引目录镜像都能通过 fuzz crate 的设备挂载，遍历到的每个名字都能用 `lookup_child` 找到；日志镜像以读写方式挂载时，重放把事务中的两个块写回文件系统。
- `cargo xtask clippy --package rsext4` 通过；fuzz crate 上 `cargo clippy --all-targets -- -D warnings` 通过；`cargo fmt --check` 无改动。
- 未运行 `cargo xtask test`：本改动不修改任何被测软件包的源代码，fuzz crate 不在工作区中，`xtask` 不会构建它。
- 未验证：持续集成中的定期运行。本 PR 不增加持续集成任务；是否以及如何在持续集成中运行 fuzz 目标由维护者决定。

### 阅读代码时发现的问题（未由 fuzz 发现）

`BlockGroupDescTable::get_desc` 把 `&self.data[offset..]` 的指针转换为 `*const Ext4GroupDesc` 并在 `unsafe` 块中返回引用。`Ext4GroupDesc` 是 `#[repr(C)]`、含 `u32` 字段，对齐为 4，而函数只检查了长度，没有检查对齐；安全的公开接口 `BlockGroupDescTable::new` 接受任意 `&[u8]`，因此调用者传入未对齐的切片（例如 `&bytes[1..]`）时会产生未对齐的引用，这是未定义行为。另外，没有 `64bit` 特性时描述符只有 32 字节，而 `Ext4GroupDesc` 有 64 字节，非最后一个描述符的高半部字段会读到下一个描述符的字节。fuzz 目标传入的切片总是对齐的，所以没有触发它，本 PR 也不修改它；crate 内部只在 `test_contracts.rs` 中使用 `get_desc`。可以改为按值解码（`Ext4GroupDesc::decode_checked`），如有需要我们可以另提 PR。

## Pull request body (English rendering, for review)

### Problem

Every byte `rsext4` reads comes from a disk, and a disk can arrive damaged or built to attack (a USB
stick someone plugs in, an image someone downloads). The crate has many unit and contract tests for
particular errors, but nothing that tries the parse and mount paths with arbitrary input. When the
superblock, group descriptors, extent tree, directory blocks, hash-tree index or journal replay
meet fields that do not agree, they should return an error, not index out of bounds, overflow,
loop forever or run out of memory. The repository has no `cargo fuzz` target yet.

### Change

A new `fs/rsext4/fuzz/`, a `cargo-fuzz` crate with seven targets:

| Target | Input | Path covered |
|---|---|---|
| `superblock` | a 1,024-byte superblock | decoding, checksum, block size, group count, geometry checks |
| `group_descriptor_table` | a superblock followed by the descriptor table | each descriptor decoded with the superblock's descriptor size and group count, and `BlockGroupDescTable` |
| `extent_tree` | one extent-tree node | `ExtentTree::parse_node` |
| `directory_block` | one directory block | `DirEntryIterator`, `classic_dir::list_entries`, the directory-block and hash-tree checksum tails |
| `htree_lookup` | a whole image | a read-only mount, then `lookup_child` of each name in each directory; an indexed directory takes the hash-tree lookup |
| `mount_and_walk` | a whole image | a read-only mount, a bounded walk of directories, metadata, extents, extended attributes and file contents, then unmount |
| `journal_replay` | a whole image | a read-write mount (replaying the journal when recovery is needed), a read of the root directory, unmount |

- `src/lib.rs` holds the shared pieces: an in-memory block device holding the input (512-byte
  sectors, so every block size mounts), a fixed clock, and a bounded walk (at most 512 inodes, and
  at most 256 KiB read from each file).
- `make-seeds.sh` builds the seed corpus with `mke2fs` and a fixed time: 1 KiB and 4 KiB blocks,
  with and without `metadata_csum`, a directory of 150 entries indexed by `e2fsck -D`, and an image
  with one committed journal transaction. The script cuts the parser targets' seeds from these images.
- `README.md` describes the targets, the corpus and how to run them.

### Reasoning of each step

- The targets call only the crate's public interface and do not change `rsext4`, so no behavior
  changes; this adds a way to check it.
- The fuzz crate has its own `[workspace]` and stays out of the repository workspace. `cargo fuzz`
  needs the nightly toolchain and sanitizer flags, and joining the workspace would make every
  `cargo xtask` build and check compile `libfuzzer-sys`. `libfuzzer-sys` is `cargo fuzz`'s standard
  dependency, and only this unpublished crate uses it (`publish = false`); `rsext4`'s own
  dependencies do not change. Following the existing rule in `fs/rsext4/.gitignore`, the
  change commits no `Cargo.lock`.
- The whole-image targets use an in-memory device rather than a file, so an iteration never touches
  the disk. 512-byte sectors let a 1 KiB block image mount; otherwise mount would refuse most
  inputs at the block-size step.
- The walk has a bound because a damaged image can make a directory cycle or claim a huge file, and
  without a bound one iteration could time out and hide the real problem. The bound limits only
  the target's work, not the crate's behavior.
- The script writes the journal seed's committed transaction itself (a descriptor block, two data
  blocks and a commit block) and marks the filesystem as needing recovery, because `debugfs`'s
  journal-writing commands leave no committed transaction in this version. The script confirms the
  transaction with `debugfs logdump`, replays a copy with `e2fsck` and checks that the target
  blocks really changed. It fails if any step does not match, so the seed is always an image a
  Linux tool agrees needs replay.
- The script cuts each parser target's seeds from real images, so mutation starts from valid
  structures rather than random bytes.

### Verification

- Each target ran locally for 600 seconds (`cargo fuzz run -O <target> corpus/<target> --
  -max_total_time=600 -timeout=10`; rustc nightly 2026-11-12, cargo-fuzz 0.13.2, x86_64 Linux,
  e2fsprogs 1.47.2). None crashed, timed out or ran out of memory. The run counts are below.
- A small program, not committed, confirmed that the seeds reach the intended paths. The three walk
  images and the indexed-directory image all mount through the fuzz crate's device, and
  `lookup_child` finds every name the walk lists. Mounting the journal image read-write replays the
  transaction's two blocks into the filesystem.
- `cargo xtask clippy --package rsext4` passes; `cargo clippy --all-targets -- -D warnings` passes
  on the fuzz crate; `cargo fmt --check` changes nothing.
- We did not run `cargo xtask test`: the change touches no source of a tested package, and the fuzz
  crate is outside the workspace, so `xtask` does not build it.
- Not verified: a scheduled run in CI. This pull request adds no CI job; whether and how CI runs the
  fuzz targets is the maintainers' choice.

### A problem found by reading the code (not by fuzzing)

`BlockGroupDescTable::get_desc` casts the pointer of `&self.data[offset..]` to
`*const Ext4GroupDesc` and returns a reference from an `unsafe` block. `Ext4GroupDesc` is
`#[repr(C)]` with `u32` fields, so its alignment is 4, and the function checks only the length,
not the alignment. The safe public constructor `BlockGroupDescTable::new` accepts any `&[u8]`, so a
caller passing a misaligned slice (`&bytes[1..]`, for example) gets a misaligned reference, and
Rust leaves the behavior of forming one undefined. Also, without the `64bit` feature a descriptor
is 32 bytes while `Ext4GroupDesc` is 64, so for every descriptor but the last the high-half fields read the next
descriptor's bytes. The fuzz target always passes an aligned slice, so it did not trigger this,
and this pull request does not change it; inside the crate only `test_contracts.rs` calls
`get_desc`. It could decode by value instead (`Ext4GroupDesc::decode_checked`); we can send a
separate pull request if wanted.

## Notes for the reviewer (not filed)

- **Design choices upstream might refuse.** It is the first fuzz crate in tgoskits, and it brings
  a new dependency, `libfuzzer-sys`, in a crate outside the workspace and outside CI. Upstream may
  want it elsewhere (a top-level `fuzz/`), inside xtask, or not at all. The standing approval of
  `libfuzzer-sys` covers our own crate repositories' fuzz crates; this one is upstream's call, and
  nife takes on no dependency from it.
- **The `get_desc` paragraph is a finding by reading.** No run demonstrated it: the debug-assertion
  alignment check does not fire when code forms a reference this way, and Miri is not installed
  here. If the reviewer prefers, it can leave this body and become its own issue draft.
- **No run found a crash,** so no crash report draft accompanies this one. Ten minutes per target is
  short, especially for the whole-image targets (33,066 to 273,613 runs); nife's fuzz rotation
  over the hostile-image corpus, in milestone 190's own list, is where longer runs belong.

## Test evidence

Each target's 600-second run (`-print_final_stats=1`; coverage, features and corpus from the last
status line):

```text
target                  runs      seconds  final status                             crashes
superblock              6952314   601      cov: 867 features: 870 corp: 27/26Kb     0
group_descriptor_table  6459995   601      cov: 602 features: 1212 corp: 42/56Kb    0
extent_tree             45019720  601      cov: 136 features: 286 corp: 44/6284b    0
directory_block         32870447  601      cov: 186 features: 508 corp: 90/28Kb     0
mount_and_walk          273613    601      cov: 2853 features: 6287 corp: 230/63Mb  0
htree_lookup            66171     603      cov: 3159 features: 7828 corp: 174/79Mb  0
journal_replay          33066     601      cov: 3657 features: 6068 corp: 37/73Mb   0
```

The seed check, with the uncommitted program:

```text
corpus/mount_and_walk/walk-1k: mounted dirs=3 names=12 found=12
corpus/mount_and_walk/walk-1k-nocsum: mounted dirs=3 names=12 found=12
corpus/mount_and_walk/walk-4k: mounted dirs=3 names=12 found=12
corpus/htree_lookup/htree: mounted dirs=3 names=158 found=158
corpus/journal_replay/journal: mount ok=true sectors of 'j' written to blocks 400-401: 4
```

`debugfs logdump` on the journal seed:

```text
Journal starts at block 1, transaction 1
Found expected sequence 1, type 1 (descriptor block) at block 1
Found expected sequence 1, type 2 (commit block) at block 4
No magic number at block 5: end of journal.
```
