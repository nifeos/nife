# Upstream draft 4: fuzz targets

Item 4 of [milestone 190](../../0190-ext4-read-write.md)'s upstream round: an issue, then the pull
request that links it. The slug and the seven target names are provisional. Nobody has filed
either; the architect reviews this text before anyone files it.

- Repository and base: rcore-os/tgoskits, branch `dev`, commit
  `6e80b940d46167a59a795e158bb17a7eb28333af`, still the head of `dev` as of this draft.
- Patch: [4-fuzz-targets.patch](4-fuzz-targets.patch), one commit, which adds only files under
  `fs/rsext4/fuzz/`.
- Issue title: `test(rsext4): fuzz the on-disk parsers, mount and journal replay`
- Pull request title: `test(rsext4): add fuzz targets for the on-disk parsers, mount and journal replay`
- Filing order: after item 5, the group-descriptor fix on its own pull request.

## Issue to file first (Chinese, as filed)

### 当前问题

`rsext4` 读取的每个字节都来自磁盘，而磁盘内容可能损坏，也可能被恶意构造（例如插入的 U 盘或下载的镜像）。crate 已有大量针对具体错误的单元测试和契约测试，但没有用任意输入检验解析和挂载路径的手段。仓库中目前没有任何 `cargo fuzz` 目标。期望结果：超级块、块组描述符、extent 树、目录块、HTree 索引和日志重放遇到不一致的字段时返回错误，而不是越界、溢出、死循环或耗尽内存。影响的组件：`fs/rsext4`。

### 修改方案

- 交付范围：在 `fs/rsext4/fuzz/` 新增一个 `cargo-fuzz` crate，覆盖上述解析器、只读挂载与遍历、HTree 查找和日志重放，并附一个用 `mke2fs` 生成种子语料的脚本。
- 不包含：对 `rsext4` 源代码的任何修改，以及持续集成任务。fuzz 发现的问题各自另开 Issue。
- 依赖和兼容性：fuzz crate 不加入仓库工作区，依赖 `libfuzzer-sys` 和夜间工具链；`rsext4` 本身的依赖和行为不变。

### 测试验证

- 验收条件：每个目标都能用 `cargo fuzz build -O` 编译；在种子语料上各运行 600 秒，没有崩溃、超时或内存超限；种子确实到达预期路径（整镜像种子可以挂载，日志种子在读写挂载时被重放）。
- 环境：x86_64 Linux，夜间 Rust 工具链，`cargo-fuzz`，e2fsprogs。

## Issue to file first (English rendering, for review)

### Current problem

Every byte `rsext4` reads comes from a disk, and a disk can arrive damaged or built to attack (a USB
stick someone plugs in, an image someone downloads). The crate has many unit and contract tests for
particular errors, but nothing that tries the parse and mount paths with arbitrary input. The
repository has no `cargo fuzz` target yet. The expected result: when the superblock, group
descriptors, extent tree, directory blocks, hash-tree index or journal replay meet fields that do
not agree, they return an error, not index out of bounds, overflow, loop forever or run out of
memory. The component affected is `fs/rsext4`.

### Proposed change

- Scope: a new `cargo-fuzz` crate in `fs/rsext4/fuzz/` covering those parsers, a read-only mount
  and walk, hash-tree lookup and journal replay, with a script that builds a seed corpus with
  `mke2fs`.
- Not included: any change to `rsext4`'s source, and any CI job. Each problem the fuzzing finds
  gets its own issue.
- Dependencies and compatibility: the fuzz crate stays out of the repository workspace and depends
  on `libfuzzer-sys` and the nightly toolchain; `rsext4`'s own dependencies and behavior do not
  change.

### Testing and verification

- Acceptance: every target builds with `cargo fuzz build -O`; each runs 600 seconds on its seed
  corpus with no crash, timeout or out-of-memory; the seeds reach the intended paths (the
  whole-image seeds mount, and a read-write mount replays the journal seed).
- Environment: x86_64 Linux, the nightly Rust toolchain, `cargo-fuzz`, e2fsprogs.

## Pull request body (Chinese, as filed)

### 当前问题

关联 Issue：#<to be filled>

`rsext4` 没有用任意输入检验磁盘解析和挂载路径的手段，仓库中也没有任何 `cargo fuzz` 目标（详见关联 Issue）。本 PR 交付该 Issue 的全部范围：七个 fuzz 目标和种子语料脚本，不修改 `rsext4` 本身。

### 修改方案

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

- `src/lib.rs` 提供共用部分：以输入为内容的内存块设备（512 字节扇区，因此任何块大小都能挂载）、固定时钟，以及有上限的遍历（最多 512 个 inode，每个文件最多读 256 KiB）。整镜像目标用内存设备而不是文件，使每次迭代不触碰磁盘；512 字节扇区让 1 KiB 块的镜像也能挂载，否则大部分输入会在块大小一步被拒绝。遍历设上限，是因为损坏的镜像可以让目录成环或声称文件极大，没有上限时一次迭代可能超时，掩盖真正的问题；上限只限制 fuzz 目标的工作量，不改变 crate 的行为。
- `make-seeds.sh` 用 `mke2fs` 和固定时间生成种子语料：1 KiB 和 4 KiB 块、有无 `metadata_csum`、一个由 `e2fsck -D` 建立索引的 150 项目录，以及一个带一条已提交日志事务的镜像。解析器目标的种子从这些镜像中截取，使变异从合法结构开始，而不是从随机字节开始。日志种子由脚本自己写入一条已提交事务（描述块、两个数据块、提交块）并标记文件系统需要恢复，因为 `debugfs` 的日志写入命令在这个版本上不留下已提交的事务。脚本用 `debugfs logdump` 确认事务存在，并用 `e2fsck` 重放一份副本、检查目标块确实被改写，任何一步不符都会失败，所以种子总是一个 Linux 工具认可的需要重放的镜像。
- `README.md` 说明目标、语料和运行方法。
- 兼容性和受影响范围：目标只调用 crate 的公开接口，不修改 `rsext4`，因此不改变任何行为。fuzz crate 有自己的 `[workspace]`，不加入仓库工作区：`cargo fuzz` 需要夜间工具链和 sanitizer 编译选项，加入工作区会让 `cargo xtask` 的构建和检查都去编译 `libfuzzer-sys`。`libfuzzer-sys` 是 `cargo fuzz` 的标准依赖，只在这个不发布的 crate 中使用（`publish = false`）；`rsext4` 的依赖不变。按 `fs/rsext4/.gitignore` 的现有约定，不提交 `Cargo.lock`。

### 测试验证

- 每个目标在本地运行 600 秒（`cargo fuzz run -O <目标> corpus/<目标> -- -max_total_time=600 -timeout=10`，rustc nightly 2026-11-12，cargo-fuzz 0.13.2，x86_64 Linux，e2fsprogs 1.47.2），全部没有崩溃、超时或内存超限。运行次数见下文。
- 用一个不提交的小程序确认种子覆盖预期路径：三个遍历镜像和索引目录镜像都能通过 fuzz crate 的设备挂载，遍历到的每个名字都能用 `lookup_child` 找到；日志镜像以读写方式挂载时，重放把事务中的两个块写回文件系统。
- `cargo xtask clippy --package rsext4` 通过；fuzz crate 上 `cargo clippy --all-targets -- -D warnings` 通过；`cargo fmt --check` 无改动。
- 未运行 `cargo xtask test`：本改动不修改任何被测软件包的源代码，fuzz crate 不在工作区中，`xtask` 不会构建它。
- 未验证：持续集成中的运行。本 PR 不增加持续集成任务；如果维护者希望，我们可以之后另提一个 PR，在持续集成中至少检查 fuzz crate 能够编译。

## Pull request body (English rendering, for review)

### Current problem

Linked issue: #<to be filled>

`rsext4` has no way to try its disk parsers and mount paths with arbitrary input, and the
repository has no `cargo fuzz` target (see the linked issue). This pull request delivers that
issue's whole scope: seven fuzz targets and the seed corpus script, with no change to `rsext4`
itself.

### Proposed change

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
  at most 256 KiB read from each file). The whole-image targets use an in-memory device rather
  than a file, so an iteration never touches the disk. 512-byte sectors let a 1 KiB block image
  mount; otherwise mount would refuse most inputs at the block-size step. The walk has a bound
  because a damaged image can make a directory cycle or claim a huge file, and without a bound one
  iteration could time out and hide the real problem. The bound limits only the target's work, not
  the crate's behavior.
- `make-seeds.sh` builds the seed corpus with `mke2fs` and a fixed time: 1 KiB and 4 KiB blocks,
  with and without `metadata_csum`, a directory of 150 entries indexed by `e2fsck -D`, and an image
  with one committed journal transaction. The script cuts the parser targets' seeds from these
  images, so mutation starts from valid structures rather than random bytes. The script writes the
  journal seed's committed transaction itself (a descriptor block, two data blocks and a commit
  block) and marks the filesystem as needing recovery, because `debugfs`'s journal-writing
  commands leave no committed transaction in this version. It confirms the transaction with
  `debugfs logdump`, replays a copy with `e2fsck` and checks that the target blocks really
  changed. It fails if any step does not match, so the seed is always an image `e2fsck` agrees
  needs replay.
- `README.md` describes the targets, the corpus and how to run them.
- Compatibility and scope: the targets call only the crate's public interface and do not change
  `rsext4`, so no behavior changes. The fuzz crate has its own `[workspace]` and stays out of the
  repository workspace. `cargo fuzz` needs the nightly toolchain and sanitizer flags, and joining
  the workspace would make every `cargo xtask` build and check compile `libfuzzer-sys`.
  `libfuzzer-sys` is `cargo fuzz`'s standard dependency, and only this unpublished crate uses it
  (`publish = false`); `rsext4`'s own dependencies do not change. Following the existing rule in
  `fs/rsext4/.gitignore`, the change commits no `Cargo.lock`.

### Testing and verification

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
- Not verified: a run in CI. This pull request adds no CI job; if the maintainers want one, we can
  send a later pull request that at least checks in CI that the fuzz crate builds.

## Notes for the reviewer (not filed)

- **Design choices upstream might refuse.** It is the first fuzz crate in tgoskits, and it brings
  a new dependency, `libfuzzer-sys`, in a crate outside the workspace and outside CI. Upstream may
  want it elsewhere (a top-level `fuzz/`), inside xtask, or not at all. The standing approval of
  `libfuzzer-sys` covers our own crate repositories' fuzz crates; this one is upstream's call, and
  nife takes on no dependency from it.
- **The group-descriptor alignment defect is item 5's.** `BlockGroupDescTable::get_desc` casts
  table bytes to a reference without checking alignment. Item 5's evidence shows a debug build
  aborting at `table.rs:36:23` with `misaligned pointer dereference`, so this draft leaves the
  defect to item 5 and files after it. The `group_descriptor_table` target calls
  `let _ = view.get_desc(..)`, which compiles whether `get_desc` returns a reference or item 5's
  value. The target never tripped the abort, because the slice it passes starts 1,024 bytes into
  libFuzzer's buffer and is always aligned.
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
