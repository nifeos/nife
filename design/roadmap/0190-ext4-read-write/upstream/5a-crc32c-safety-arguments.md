# Item 5a: the safety arguments at the CRC32C `unsafe` uses

The first half of upstream item 5 in [milestone 190](../../0190-ext4-read-write.md): a safety
argument at each CRC32C `unsafe` use in `rsext4`, with no change to what runs. The other half, the
group-descriptor casts, is [item 5b](5b-group-descriptor-decode.md), because upstream takes one
task per pull request and keeps a documentation change apart from a fix. Nobody files either until
the architect reviews this exact text and gives a separate go.

- Repository: `rcore-os/tgoskits`, branch `dev`, package `fs/rsext4` (0.9.2).
- Base commit: `6e80b940d46167a59a795e158bb17a7eb28333af`, the head of `dev` when this draft
  last checked it; `dev` has no later commit under `fs/rsext4`.
- The change: [5a-crc32c-safety-arguments.patch](5a-crc32c-safety-arguments.patch), one commit,
  `git format-patch` output. It applies with `git am` on the base commit.
- Upstream's contributing guide (section 4.3) processes no pull request without a linked issue, so
  the issue below goes first, and the pull request body names its number.

## Issue to file first

### Issue title

```text
docs(rsext4): state the safety argument at each CRC32C unsafe use
```

### Issue body (Chinese, as filed)

```markdown
## 当前问题

`fs/rsext4` 的 CRC32C 路径中有九处 `unsafe`。x86_64 的两处 `asm!` 已说明它们依赖的 CPU 特性，其余几处没有安全性论证：

- aarch64 的 `mrs` 读取 `ID_AA64ISAR0_EL1`，没有说明为什么在 EL0 读取 EL1 的 ID 寄存器是有效的。
- aarch64 的硬件循环整个函数体是一个 `unsafe` 块，每次指针读取的对齐和边界论证无法放在读取旁边。
- `crc32c_append` 中两处和测试中一处 `unsafe { crc32c_hardware(..) }` 没有说明所需的 CPU 特性在哪里检测。

仓库规则要求最小的 `unsafe` 块，且每块都有证明注释。

## 修改方案

只增加安全性注释并拆小 `unsafe` 块，执行的指令不变：

- 每处 `crc32c_hardware` 调用写明所需的 CPU 特性（ARMv8 CRC32 扩展或 SSE4.2）以及检测它的寄存器（`ID_AA64ISAR0_EL1[19:16]` 或 CPUID leaf 1 ECX bit 20）。
- aarch64 的 `mrs` 说明：内核为用户态模拟 ID 寄存器（Linux 设置 `HWCAP_CPUID` 时如此）；不模拟的主机会把它作为非法指令陷入。
- aarch64 的硬件循环改为每个指针操作一个最小的 `unsafe` 块，每块写明对齐和边界的论证。

不包含：x86_64 的两处 `asm!`（已有说明），以及组描述符表的两处转换（另开 Issue）。没有兼容性影响。

## 测试验证

- `cargo test -p rsext4 --lib` 全部通过。
- `cargo xtask clippy --package rsext4` 全部通过。
- `rsext4` 没有声明 docs.rs 目标，所以 `cargo xtask clippy` 只在宿主机目标上检查它，而修改的 `arm64.rs` 只在 aarch64 上编译；因此另外运行 `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` 并确认通过。
- `cargo fmt --check` 通过。
```

### English rendering of the issue

```markdown
## Current problem

The CRC32C paths of `fs/rsext4` hold nine `unsafe` uses. The two x86_64 `asm!` blocks already state the CPU feature they rely on; the others state no safety argument:

- The aarch64 `mrs` reads `ID_AA64ISAR0_EL1` and does not say why reading an EL1 ID register at EL0 is valid.
- The aarch64 hardware loop is one `unsafe` block over the whole function body, so the alignment and bounds argument of each pointer read cannot sit beside that read.
- Two `unsafe { crc32c_hardware(..) }` calls in `crc32c_append` and one in a test do not say where the CPU feature they need is detected.

The repository's rules ask for minimal `unsafe` blocks, each with a proof comment.

## Proposed change

Add safety comments and make the `unsafe` blocks smaller; the instructions executed stay the same:

- Each `crc32c_hardware` call names the CPU feature it needs (the ARMv8 CRC32 extension or SSE4.2) and the register it was detected from (`ID_AA64ISAR0_EL1[19:16]` or CPUID leaf 1 ECX bit 20).
- The aarch64 `mrs` says that the kernel emulates the ID registers for user space (Linux does when it sets `HWCAP_CPUID`), and that a host that does not delivers the trap as an illegal instruction.
- The aarch64 hardware loop gets one minimal `unsafe` block per pointer operation, each stating its alignment and bounds argument.

Not included: the two x86_64 `asm!` blocks (already stated), and the two casts in the group descriptor table (a separate issue). No compatibility impact.

## Test and verification

- `cargo test -p rsext4 --lib` passes.
- `cargo xtask clippy --package rsext4` passes.
- `rsext4` declares no docs.rs targets, so `cargo xtask clippy` checks it on the host target only, and the changed `arm64.rs` compiles only on aarch64. So `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` also runs, and passes.
- `cargo fmt --check` passes.
```

## Pull request title

```text
docs(rsext4): state the safety argument at each CRC32C unsafe use
```

## Pull request body (Chinese, as filed)

```markdown
## 当前问题

关联 Issue：#<to be filled>

CRC32C 路径中，aarch64 的 `mrs` 读取、aarch64 的硬件循环（整个函数体是一个 `unsafe` 块）、以及 `crc32c_append` 和测试中对 `crc32c_hardware` 的三处调用没有说明它们的安全性依据。x86_64 的两处 `asm!` 已有说明，本 PR 不修改它们。

## 修改方案

本 PR 只增加安全性注释并拆小 `unsafe` 块，执行的指令不变：

- `crc32c_append` 和测试中的每一处 `unsafe { crc32c_hardware(..) }` 都写明所需的 CPU 特性（ARMv8 CRC32 扩展或 SSE4.2）以及检测它的寄存器（`ID_AA64ISAR0_EL1[19:16]` 或 CPUID leaf 1 ECX bit 20）。
- aarch64 的 `mrs` 说明为什么在 EL0 读取 EL1 的 ID 寄存器是有效的：内核为用户态模拟 ID 寄存器（Linux 设置 `HWCAP_CPUID` 时如此）；不模拟的主机会把它作为非法指令陷入。
- aarch64 的硬件循环从一个覆盖整个函数体的 `unsafe` 块拆成每个指针操作一个最小的 `unsafe` 块，每块写明对齐和边界的论证。拆分的理由：仓库规则要求最小的 `unsafe` 块且每块有证明注释，一个覆盖整个函数的块无法把每次读取的论证放在读取旁边。

兼容性：没有公共接口或行为变化。

## 测试验证

- `cargo test -p rsext4 --lib`：428 个测试全部通过。
- `cargo xtask clippy --package rsext4`：3 项检查全部通过（base、`USE_MULTILEVEL_CACHE`、`host-test`，都在宿主机目标上）。
- `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` 通过。`rsext4` 没有声明 docs.rs 目标，`cargo xtask clippy` 只检查宿主机目标，而修改的 `arm64.rs` 只在 aarch64 上编译，所以补充这一项，不是替代 `cargo xtask clippy`。
- `cargo fmt --check` 通过。
- `cargo xtask test --since 6e80b940d`：除 `rsext4` 的 `tests/filesystem_growth.rs` 中两个测试和 `ax-fs-ng` 的 `root_growth_preserves_other_partitions_and_ext4_contents` 外全部通过。这三个测试在基线提交上以同样方式失败，原因相同：测试机的 `mke2fs` 1.47.2 默认启用 `orphan_file`，扩容在 `fs/rsext4/src/ext4/grow/layout.rs:93` 以 `grow:features_or_durability` 拒绝它。第一次运行中 `ax-block` 的六个缓存测试也失败过一次（`fs/ax-block/src/block/cache/tests.rs:890`，随后是锁中毒）；本 PR 只改 `rsext4` 的注释，不影响 `ax-block`，在同一分支上重跑 `ax-block` 的 host-test 测试五次（其中三次加 `write-zeroes,reclaim` 特性）均通过，所以这是一次偶发失败。

未验证：aarch64 的硬件 CRC32C 路径只做了编译和 clippy 检查，没有在 aarch64 硬件上运行；本 PR 不改变执行的指令。没有运行 QEMU，因为没有运行时行为变化。
```

## English rendering of the pull request body

```markdown
## Current problem

Linked issue: #<to be filled>

On the CRC32C paths, the aarch64 `mrs` read, the aarch64 hardware loop (its whole body is one `unsafe` block), and the three calls to `crc32c_hardware` in `crc32c_append` and in a test state no safety argument. The two x86_64 `asm!` blocks already do, and this PR leaves them alone.

## Change

This PR adds safety comments and makes the `unsafe` blocks smaller; the instructions executed are unchanged:

- Each `unsafe { crc32c_hardware(..) }` in `crc32c_append` and in the test names the CPU feature it needs (the ARMv8 CRC32 extension or SSE4.2) and the register it was detected from (`ID_AA64ISAR0_EL1[19:16]` or CPUID leaf 1 ECX bit 20).
- The aarch64 `mrs` says why reading an EL1 ID register at EL0 is valid: the kernel emulates the ID registers for user space (Linux does when it sets `HWCAP_CPUID`), and a host that does not delivers the trap as an illegal instruction.
- The aarch64 hardware loop goes from one `unsafe` block over the whole body to one minimal `unsafe` block per pointer operation, each stating its alignment and bounds argument. Why split it: the repository's rules ask for minimal `unsafe` blocks, each with a proof comment, and one block over the whole function cannot keep each read's argument beside that read.

Compatibility: no change to the public API or to behavior.

## Test and verification

- `cargo test -p rsext4 --lib`: all 428 tests pass.
- `cargo xtask clippy --package rsext4`: all 3 checks pass (base, `USE_MULTILEVEL_CACHE` and `host-test`, all on the host target).
- `cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings` passes. `rsext4` declares no docs.rs targets, so `cargo xtask clippy` checks only the host target, and the changed `arm64.rs` compiles only on aarch64. This check adds to `cargo xtask clippy` and does not replace it.
- `cargo fmt --check` passes.
- `cargo xtask test --since 6e80b940d`: everything passes except two tests in `rsext4`'s `tests/filesystem_growth.rs` and `ax-fs-ng`'s `root_growth_preserves_other_partitions_and_ext4_contents`. The three fail the same way on the base commit, for one reason: the test host's `mke2fs` 1.47.2 enables `orphan_file` by default, and growth refuses it with `grow:features_or_durability` at `fs/rsext4/src/ext4/grow/layout.rs:93`. On the first run six `ax-block` cache tests also failed once (`fs/ax-block/src/block/cache/tests.rs:890`, then poisoned locks). This PR changes only comments in `rsext4` and cannot reach `ax-block`; five reruns of the `ax-block` host tests on the same branch (three with the `write-zeroes,reclaim` features) all pass, so that failure is intermittent.

Not verified: the aarch64 hardware CRC32C path was only compiled and checked with clippy, not run on aarch64 hardware; this PR does not change the instructions executed. QEMU was not run, because no runtime behavior changes.
```

## Test evidence

Each command ran in the clone on the patch applied to the base commit.

```text
$ cargo test -p rsext4 --lib
test result: ok. 428 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo xtask clippy --package rsext4
ok: rsext4 (base)
ok: rsext4 (feature: USE_MULTILEVEL_CACHE)
ok: rsext4 (feature: host-test)
passed checks: 3, failed checks: 0, skipped checks: 0

$ cargo clippy --no-deps -p rsext4 --target aarch64-unknown-none-softfloat -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo xtask test --since 6e80b940d
std tests failed for 3 package(s): ax-fs-ng, ax-block, rsext4

$ cargo test -p ax-block --features host-test   # rerun twice on this branch, plus three runs with write-zeroes,reclaim
test result: ok. 144 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The three `xtask clippy` checks pass no `--target`: the configuration code in
`scripts/axbuild/src/checks/clippy/expand.rs` builds a host-only check for a package whose
`Cargo.toml` names no docs.rs targets, and `fs/rsext4/Cargo.toml` names none.
