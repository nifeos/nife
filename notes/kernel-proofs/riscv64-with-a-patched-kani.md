# Proving `kernel/src` for riscv64 with a patched Kani

*Name: provisional, minted 2026-09-25 by milestone 589 (Kani can prove riscv64 from the hosts we
already have). The native riscv64 route, beside the aarch64-host one.*

An appendix to [`notes/kernel-proofs.md`](../kernel-proofs.md).

Stock Kani compiles for the machine it runs on. `kernel/src/arch/mod.rs` picks its subtree by
`target_arch`, so until 2026-09-25 no job anywhere compiled a line of `arch/riscv64/`. No riscv64 host
is needed to fix that. CBMC checks a goto program and does not care where it runs, and it has known
riscv64's machine model for years. What Kani lacked was a riscv64 target.

DECISIONS §218 (carry a Kani patch so riscv64 is proved) is the ruling. The tree carries
`patches/kani-0.67.0-riscv64-target.patch`, builds Kani from the release tag plus that file, and never
builds from a fork. The patch reads the target from `KANI_TARGET` (a provisional name). The
`prove the kernel on riscv64` job in `.github/workflows/verify.yml` runs it on the arm64 runner.

## EXAMPLES

The kernel row for riscv64, from an aarch64 or x86_64 machine, macOS or Linux:

```console
$ script/verify-riscv64
==> patched Kani 0.67.0 already built at /Users/calef/.cache/nife-kani-riscv64/kani-0.67.0 (cache hit)
==> kani: kernel (...)
Complete - 7 successfully verified harnesses, 0 failures, 7 total.
==> the kernel proved for riscv64gc-unknown-linux-gnu
```

The first run clones Kani at the tag the patch's file name pins and applies the patch. It fetches that
release's stock CBMC and builds Kani into `~/.cache/nife-kani-riscv64/`. Measured cold: 145 s on the
`ubuntu-24.04-arm` runner, and 244 to 621 s on patagonia depending on load, under 1 GB resident.
Later runs reuse the build until the patch's bytes change. A warm run, compiling the kernel from
scratch, took 16 s on patagonia. Nothing in `~/.kani` or `~/.cargo/bin` is touched.

One harness: arguments after the script name go to `cargo kani`.

```console
$ script/verify-riscv64 --harness the_run_end_is_exact_and_refuses_exactly_what_does_not_fit
```

By hand, with CBMC first on PATH and the target named:

```console
$ K=~/.cache/nife-kani-riscv64/kani-0.67.0
$ PATH="$K/cbmc/bin:$PATH" KANI_TARGET=riscv64gc-unknown-linux-gnu \
      "$K/src/scripts/cargo-kani" -p kernel -Z unstable-options --ignore-global-asm
```

Call `cargo-kani` by path, never as `cargo kani`. Cargo looks in `~/.cargo/bin` before PATH for a
subcommand, so a stock install there would win and prove the host's `arch/` instead. The script
guards against the same mistake twice: it hands `script/verify` the patched launcher through
`VERIFY_CARGO_KANI`, which also stops that script installing stock Kani. After the run it requires
goto output from this run under `target/kani/riscv64gc-unknown-linux-gnu/`. Tested against a stand-in
that ran stock Kani: four aarch64 harnesses passed, and the script exited 1.

## What riscv64 support does not reach

The target moves the `cfg` boundary, kernel-proofs.md's stub-list item 3. The other two boundaries
hold on riscv64 exactly as elsewhere. Counts are from the milestone 589 (Kani can prove riscv64 from the hosts we already have) proposal, at base `334804c8e`.

- `asm!`: 56 inline sites in 9 of `arch/riscv64/`'s 13 files. A harness that reaches one fails with
  `TerminatorKind::InlineAsm is not currently supported by Kani`, loudly. Most sites are one-instruction
  wrappers (`csrr`, `sfence.vma`, an SBI `ecall`) that `kani::stub` could replace with a model. [stubbing-an-instruction.md](stubbing-an-instruction.md)
  says how, and what a stub assumes.
- Fixed-address MMIO: 22 volatile sites (`iommu.rs` 13, `mmu.rs` 6, `exceptions.rs` 2,
  `semihosting.rs` 1). Kani reads each as a dereference of an invalid pointer.
  model-checking/kani#1304 is still open, waiting on CBMC's MMIO regions.
- The four `.s` files: boot, context switch, trap entry and FP save, 790 lines. No Kani on any host
  reads them, and `--ignore-global-asm` drops them.

So the reach without stubs is the asm-free, MMIO-light logic. That means `context.rs`, `irq.rs`, the
logic of `iommu.rs`, and pure functions inside asm-bearing files, such as composing `satp`.

## BUGS

- Seven harnesses run here, all passing on 2026-09-25: the two portable `syscall.rs` ones,
  milestone 432 (the RISC-V IOMMU driver has no counterpart to the SMMU's proofs)'s two in
  `iommu.rs`, and three `satp` proofs in `mmu.rs` that stub their `csrr`/`csrw` wrappers. The
  `mmu.rs` three run nowhere else. The IOMMU pair also runs on the aarch64 host, under aarch64's
  `crate::arch`.
- The patch costs a rebase and a rename whenever the Kani pin moves, until a Kani release carries
  riscv64 target support (model-checking/kani#2402).
- The other Kani jobs install the stock version pinned in `.kani-version` (`script/kani-install`), so
  this row runs a different Kani from its siblings: the patch's file name pins 0.67.0 here while
  the stock pin is whatever `.kani-version` says. The skew ends when a released Kani carries
  `--target` (kani#4913).
- A build made with `KANI_TARGET` set has a riscv64-only sysroot, because Kani's sysroot has one slot
  for `std`. That is why the script builds into its own prefix rather than `~/.kani`.
- `script/falsifications --sweep` runs stock Kani for the host. It cannot replay a riscv64-only
  harness, so such a record can only be `attested` until that script learns to use this Kani.
