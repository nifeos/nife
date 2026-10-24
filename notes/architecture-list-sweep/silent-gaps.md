# The eleven silent gaps, as the sweep found them

*Name: provisional. After the main page's own heading.*

An appendix to [`notes/architecture-list-sweep.md`](../architecture-list-sweep.md), which states the
result.

| # | Where | What enumerates | Missing | Evidence that the premise holds |
|---|---|---|---|---|
| 1 | `script/stack-depth-check:89`, and its `TRAP_FRAME`, `DISPATCH`, `BODY` dicts at `:156`, `:161`, `:170` | `arches="aarch64 riscv64"` plus three two-key dicts | x86_64 | `kernel/src/arch/x86_64/` has `exceptions.rs` and `trap.s`, so both a trap-frame size and a dispatch symbol exist to name. Its `BUGS` section carries six entries and none is this one. |
| 2 | `script/bootstrap:32` | `rustup target add --toolchain "$channel" aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf` | `x86_64-unknown-none` | The next paragraph in the same file checks for all three QEMU emulators and says "`cargo xtask test` now needs the full set". The bootstrap installs two of the three targets that sentence requires. |
| 3 | `script/drift:33`, and `:44` | `rustup target add` list; and a bare-metal build that is aarch64 only | `x86_64-unknown-none` in the list; riscv64 and x86_64 in the build | The job's whole purpose is "does a toolchain still build us". A nightly that breaks only the x86_64 kernel is invisible to it. Note the second half independently: riscv64 is installed on line 34 and never built. |
| 4 | `script/toolchain-bump:38` | `rustup target add --toolchain "$new"` list | `x86_64-unknown-none` | A bump installs two of three targets against the new pin, so the first x86_64 build after a bump is the one that discovers it. |
| 5 | `deny.toml`, the `targets` array | two bare-metal targets and three hosts | `x86_64-unknown-none` | `cargo-deny` resolves the dependency graph per target, so a crate reached only under `cfg(target_arch = "x86_64")` gets no advisory, license, or ban verdict. **This one had a scope note and the note went stale**, which is the worse variant: it said x86_64 bare metal "is declared in §19 but not started; it joins this list when it does", and it started. A reader who checked was told the gap was tracked. |
| 6 | `.github/workflows/ci.yml:390` | the bench job's two legs, `script/bench --check` and `script/bench --riscv --check` | an x86_64 leg | `cargo xtask bench --x86 --check` exists (`xtask/src/bench.rs`, `bench_x86`) and `bench/baseline-x86_64.txt` is committed. The tripwire is built, the baseline is recorded, and nothing pulls it. |
| 7 | `script/bench:5` | the header's `EXAMPLES`, which names only `bench/baseline-aarch64.txt` | `--riscv` and `--x86` | CI runs `--riscv` and the script's own documentation does not mention it exists. Two of three architectures are undiscoverable from the front door. |
| 8 | `notes/arch-audit.md`, "What was audited" | "Everything under `kernel/src/arch/`, read in full, both ISAs", and a two-column table | x86_64 | The note contains zero occurrences of "x86". `kernel/src/arch/x86_64/` is 18 files and 6,797 lines, which is **larger than the 6,200-line two-ISA tree that audit read in full**. The audit's own argument is that hand-written arch assembly is the least-verified code in the trusted computing base; a third of it is now unread. |
| 9 | `crates/virtio/src/lib.rs` `barrier`, `components/src/gpu_driver.rs` `barrier`, `components/src/keyboard_driver.rs` `barrier`, `components/src/virtio_net_transport.rs` `barrier` | two `#[cfg(target_arch)]` arms with no third arm and no fallback | x86_64 | All four compile for x86_64 (`initial_ramdisk_x86` builds `-p user` for the target; `script/lint` clippies `-p user -p user_mode_runtime --target x86_64-unknown-none --all-targets -- -D warnings` and passes). On x86_64 both arms compile out and the function body is empty, so it emits no compiler fence at all. TSO covers the machine half and nothing covers the compiler half. |
| 10 | `components/src/pgrep.rs`, its `#[panic_handler]` | the same two-arm shape, for a deliberate trap (`brk #0` / `ebreak`) | x86_64 | On x86_64 neither arm compiles, control reaches the spin loop, and a panic becomes a thread burning CPU forever instead of the kill the comment above it promises. `pgrep` is the **only** program in `user/` with a hand-written panic handler of this shape; every other one goes through `crates/user_mode_runtime`, whose arms cover all three ISAs. **Fixed on this branch**, since it is a live defect rather than a coverage gap. |
| 11 | `script/fastpath-footprint`, its `arches` and its `ROOTS` table | `arches="aarch64 riscv64"` plus a two-key `ROOTS` dict | x86_64 | The file has no occurrence of "x86" at all. The IPC fastpath is exactly the kind of thing §19 says must not fit on one ISA and silently not another. Not a one-word widening: it needs `bench/fastpath-x86_64.txt` and `kernel/src/arch/x86_64/fastpath_pad.rs`, neither of which exists, plus an x86_64 `ROOTS` row. |

## The one to read first, because it names its own trap

`script/lint`'s x86_64 user pass carries this sentence, written to justify its own existence:

> Without it, a lane could add an aarch64/riscv64 pair to a user program and never learn that x86
> fell through to nothing until `cargo xtask test --arch x86_64` failed to pack an archive.

Findings 9 and 10 are exactly that, and that gate does not catch them, because falling through to
nothing compiles clean. An empty function is not a warning. This is the strongest argument in the
sweep for the `compile_error!` arm that [design-options.md](design-options.md) proposes as option B:
the gate written to catch this class cannot, and the author knew what to look for.

## Two notes on severity, because overclaiming would waste the reader's time

Finding 9 is latent; finding 10 is live but rarely reached. The x86_64 QEMU runner attaches no
virtio disk, NIC, GPU, or RNG (`helpers/qemu-runner-x86_64.sh` says so in its own header), so no
x86_64 boot reaches a virtio ring today. The exposure is that the code is compiled, is shipped in
the archive, and is wrong the day a device is attached, which is a day the roadmap plans for.
Finding 10 needs no device: any panic in `pgrep` on x86_64 hangs the thread now. It is small only
because that panic handler is the path the comment above it calls "nothing here should panic".

Findings 2 through 5 are mitigated by `rust-toolchain.toml`, which carries `targets =
["aarch64-unknown-none-softfloat", "riscv64imac-unknown-none-elf", "x86_64-unknown-none"]` and is
the reason CI's clippy job installs only aarch64 explicitly and still lints x86_64 successfully.
rustup reads that array. So the stale `rustup target add` lines are dead weight against the pinned
toolchain rather than a live breakage. They are still findings, for two reasons. `script/drift` and
`script/toolchain-bump` both add targets to a different toolchain than the pin, where the file's
array does not apply. And a list that is wrong and harmless today is the exact shape of
`script/stack-frame-check` on 2026-08-24.

## The two that were another lane's while this ran, and how they ended differently

Both are the same shape as finding 1 and both were held by the lane on pull request #567 while this
sweep ran. That pull request landed 2026-08-27, and it settled one of them and not the other, which
is worth recording because the split is instructive.

`script/stack-frame-check` is now a recorded gap, and is the best example in the tree of the
conversion this note keeps asking for. The lane widened it to accept `--arch x86_64`, ran it, and
found a real offender the first time: `kernel::arch::x86_64::iommu::init` at 12,504 bytes
against a 4,096-byte ceiling, from an `Iommu { ctx: [Option<u64>; 256], .. }`, which is the same
"`[T; MAX]` local sized to a table maximum" shape that gate's own first `BUGS` entry describes. So
x86_64 is reachable but deliberately out of the default set, with a `BUGS` entry saying so, saying
what was found, and saying that whether to fix the offender or except it is not that gate's call.
`main` stays green by naming the finding rather than by not looking. That is exactly the phase-3
outcome milestone 186 plans for, arrived at a day early.

`script/fastpath-footprint` was assessed and left alone, and it is still
`arches="aarch64 riscv64"` with no x86 mention anywhere in the file. So it is not a claimed item
any more; it is finding 11, unclaimed, and it is recorded in its own `BUGS` on this branch. It has
two absent companions worth naming for whoever takes it: `bench/fastpath-x86_64.txt` and
`kernel/src/arch/x86_64/fastpath_pad.rs` do not exist, where both other architectures have both.
The two tripwire file sets diverged: `bench/baseline-<arch>.txt` is complete at three and
`bench/fastpath-<arch>.txt` is at two.

The ten-versus-eleven count. The table above is the sweep as it was run, when
`script/stack-frame-check` was the prompting instance and `script/fastpath-footprint` was somebody
else's. Read forward from today it is eleven silent gaps, one of which (`stack-frame-check`) was
converted to a recorded gap by another lane before this note landed. The table is not renumbered,
because an inventory records what the sweep found rather than what survived it.

`.github/workflows/ci.yml` installs two bare-metal targets for each of those two jobs, under a
comment that reads "Both bare-metal targets, because this gates both (§19 parity: a frame that fits
on one ISA and not the other is exactly the asymmetry that rule exists to catch)". That comment
cites the parity rule as the reason for enumerating two of three architectures. It follows the
scripts and should be fixed with them, not separately.
