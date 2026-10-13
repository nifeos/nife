---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: riscv64-code-the-prover-can-already-compile
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 650. Some of `arch/riscv64/` compiles under Kani on an aarch64 host today, and the tree says none can

*Title and slug are drafts.*
Raised while pricing `design/fatal-risks/README.md` risk 2 (the proofs prove trivia) (pull request
#1276). `cargo kani` does compile parts of the kernel, and the wall it hits is lower than the tree
records. *(Slug provisional; naming is
calef's.)*

Option 1 below is reversible code and one harness. Risk 2's color is not asked
about here: that is `design/fatal-risks/README.md`, calef's file, and milestone 536 (two records still say
the prover cannot see `kernel/src`) already holds that decision.

## The premise, checked first

The brief said risk 2 is amber because `cargo kani` never compiled the kernel. That was true until
2026-08-30. Milestone 193 (put `kernel/src` within reach of the prover) made `kernel` a harness
crate, and it proves today, on this machine (patagonia, aarch64 macOS, Kani 0.67.0, base
`9e879f1e7`):

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm --output-format=terse
Complete - 4 successfully verified harnesses, 0 failures, 4 total.
        7.25 real         7.45 user         2.08 sys
           446873600  maximum resident set size
```

No compile failure to classify. Milestone 536 owns the *"never compiles the kernel"* copies in `design/fatal-risks/README.md`, its appendix
and `notes/proof-retrospective.md`.

So what keeps risk 2 amber today is two things, per its own 2026-09-24 correction: **`arch/riscv64/`
is compiled by nothing**, and the survivorship caveat (every defect a proof caught was caught while
its harness was being written). Only the first is buildable. This proposal prices it.

## What was measured

`notes/kernel-proofs.md` and the block of milestone 304 (`cargo kani -p kernel` only ever
compiled one architecture) both say *"riscv64 is unreachable and no one here
can fix it"*, because Kani compiles for the host and no host here is riscv64. That is true of the
`cfg` dispatch in `arch/mod.rs`. It is not true of the files. A probe module, appended to
`arch/mod.rs` and not committed:

```rust
#[cfg(all(kani, not(target_arch = "riscv64")))]
#[path = "riscv64/iommu.rs"]
mod riscv64_iommu_probe;
```

plus a harness asserting `false` appended to `riscv64/iommu.rs`:

```console
Checking harness arch::riscv64_iommu_probe::reach_probe_riscv64_iommu...
VERIFICATION:- FAILED
Complete - 4 successfully verified harnesses, 1 failures, 5 total.
```

The file compiled unchanged, and the one failure is the probe's. Then each other candidate, with
`--only-codegen`, recording the first errors verbatim:

| file | lines | `asm!` sites | result on the aarch64 host | what blocks it |
|---|---:|---:|---|---|
| `iommu.rs` | 379 | 0 | compiles, harness runs | nothing |
| `context.rs` | 105 | 0 | compiles | nothing |
| `irq.rs` | 252 | 0 | 8 errors | **siblings**: `error[E0433]: could not find \`plic\` in \`drivers\`` (the PLIC driver is gated to riscv64 in `drivers/mod.rs`); `cannot find function \`boot_hartid\` in module \`super\`` |
| `isa.rs` | 356 | 1 | 6 errors | **target**: `error: invalid register \`a7\`: unknown register` at `isa.rs:222` |
| `pmu.rs` | 616 | 2 | 10 errors | **target**: the same, at `pmu.rs:417` |

The classes, against the brief's list:

- `asm!`, as a target error rather than a Kani one. rustc validates register names against the
  compilation target before Kani sees anything, so a riscv64 `asm!` is a hard front-end error on an
  aarch64 host even where no harness could reach it. On its native target `asm!` is only Kani's
  "unsupported construct", which fails at reach. 61 sites across 10 of the 17 files.
- Siblings and drivers. An asm-free file that calls `super::` or a riscv64-gated driver fails to
  resolve. Local and countable per file.
- **Not blocking:** `no_std`, the linker and features. Milestone 193 already cleared those for the
  whole crate.

Outside `arch/`, 98 `#[cfg(target_arch = "riscv64")]` sites in 15 files (30 in `bench.rs`, 12 in
`user.rs`, 10 in `console.rs`) are compiled by no host at all. None of the options below reaches
them except option 3.

## Prior art, read

- Kani 0.67.0's `kani-compiler/src/codegen_cprover_gotoc/compiler_interface.rs`, read at the
  tag through `gh api`. `new_machine_model` matches `Arch::X86_64` and `Arch::AArch64` and otherwise
  hits `panic!("Unsupported architecture: {architecture}")` (lines 817 and 888). So a riscv64 host
  running stock Kani does not get a proof; it gets a compiler panic. That makes the tree's reason
  ("no riscv64 runner") incomplete: a runner would not be enough.
- model-checking/kani#2086 (closed 2023-01-13). A user verifying a non-host target
  (`armv7-unknown-linux-gnueabi`) needed three changes to Kani itself: a machine model, a `--target`
  passed to cargo, and Kani's library sysroot rebuilt for that target. They got a trivial harness
  through, then hit a CBMC invariant violation on the first reference, because the pointer width
  disagreed with the host's. riscv64 is 64-bit little-endian like both supported hosts, so that
  particular failure should not recur, but it is the only field report and it is a warning.
- model-checking/kani#2402, "Command-line flag to change model target or environment", open
  since 2023-04-23, last touched 2024-10-02, filed for zerocopy's endianness proofs. Upstream has not
  prioritized it in three years.

## The options

Option 1: compile riscv64's self-contained, asm-free files as proof-only modules on the aarch64
host. The probe above, made permanent for `iommu.rs` (and `context.rs` if a property is worth
writing there). It adds the property that milestone 255 (a quarter of `kernel/src/arch/` has no
assembly in it, and none of it is proved) said "can be written and cannot be run": the device
context's address fields round-trip, and no device id reaches another's entry. Measured cost:
five lines in `arch/mod.rs` and one harness. The probe run's wall time was not taken separately;
the row it joins is 7.25 seconds.
Extending to `irq.rs` costs its 8 resolve errors, most likely by compiling the PLIC driver under
the same `cfg` (unmeasured). The ceiling was 736 lines, 12.5% of `arch/riscv64/`'s 5,879 Rust
lines by `wc -l`. This said 6% of 11,746, a total that counted every file twice. Both predate
milestone 432 (the RISC-V IOMMU driver has no counterpart to the SMMU's proofs), which grew
`iommu.rs`.
It carries **a new stub-list item**, and it is the one that matters: inside the probe module,
`crate::arch::...` resolves to the *host's* architecture. `iommu.rs` calls
`crate::arch::mmu::phys_to_virt` and gets aarch64's. For word arithmetic that is irrelevant; for
anything that crosses into `crate::arch` it is a fiction, and `notes/kernel-proofs.md` has to say
so where the next harness author reads.
Two details: the `cfg` should be `all(kani, target_arch = "aarch64")` rather than
`not(riscv64)`, so exactly one verify host runs it (whether the x86_64 host resolves the same names
is unmeasured). And `script/lint` check 5 counts `#[path]` targets per consumer. One consumer is
within its rule, but that is read from the rule rather than run.

Option 2: isolate every riscv64 `asm!` behind a function with a host stub, then compile all of
`arch/riscv64/` as a proof-only module. Reaches the remaining 10 files, including `mmu.rs`
(1,484 lines) and `timer.rs` (1,034), the two largest. Cost, estimated rather
than measured: 61 sites in 10 files, one to two lane-days, and a standing tax (every new riscv64
`asm!` goes through the wrapper, which wants a lint). And option 1's fiction becomes structural,
because `mmu.rs` and `timer.rs` call `crate::arch::mmu` and `crate::arch::timer` and would get
aarch64's. A proof of riscv64's page-table walk that silently calls aarch64's `mmu` is the "stub
that reads as coverage" `notes/kernel-proofs.md` exists to prevent. Refused on that, not on cost.

Option 3: run Kani natively for riscv64. A patched Kani (a riscv64 arm in `new_machine_model`,
modeled on the aarch64 one), built with its own pinned nightly and a CBMC for a riscv64 host, run
on `radon` or a riscv64 QEMU guest on cordoba. Reaches everything, including the 98 gated sites
outside `arch/`, with the real `cfg`s and the real `crate::arch`. Cost, estimated: a Kani fork this
tree maintains through every Kani toolchain bump, which DECISIONS §46 (thin primitives or whole
subsystems) treats as a dependency decision rather than a build task. It also needs a verify host
that is either a lab board or an emulator. Days to weeks, and not measured here. Upstreaming the machine
model is the version of this that does not leave a fork behind; #2402's age says not to wait on it.

**Option 4: nothing.** Record the corrected wall in `notes/kernel-proofs.md` and move on.

## Recommendation

Option 1, for `iommu.rs` only, in one lane, with the stub item written before the harness.

## The seven questions

1. Considered and lost: option 2 lost on correctness, because the host `crate::arch` makes its
   biggest proofs fictions. Option 3 lost on dependency cost and an unowned runner, not on merit.
   Option 4 lost because the one file milestone 255 already wanted proved costs five lines.
2. What the tree does in the analogous case: milestone 255 proved `arch/aarch64/iommu.rs` in
   place rather than lifting it into a crate, and milestone 193 refused lifting as its option B.
   Option 1 is the same move one architecture over.
3. Prior art: above, read at the source, not recalled.
4. Is the premise true? The brief's premise is not. The tree's premise ("riscv64: nothing can")
   is also not, for asm-free files.
5. Cost: option 1 measured, options 2 and 3 estimated and marked so.
6. Reversibility: option 1 is five lines and a harness, and nobody downstream acts on a proof
   module. Option 3 is a dependency, on the irreversible list.
7. Same cost, same choice? No. If option 3 cost what option 1 does, option 3 wins: real `cfg`s,
   real `crate::arch`, the whole subsystem. **So this recommendation is partly about effort**, and
   says so. The rest of it is §46: option 3 is a fork of the prover, which is a decision and not a
   build task, and nobody has made that decision.

## What it does not change

**This does not turn risk 2 green, and nothing buildable does.** Reach was the half of the amber a
lane could build; the survivorship half closes only when a standing harness goes red on a regression
somebody else introduced. Option 1 adds a riscv64 proof, which is reach. The honest outcome is a
narrower sentence in risk 2: riscv64's asm-free files are provable from an aarch64 host, and 87.5%
of `arch/riscv64/` still is not. That sentence is an architect's to write, through milestone 536.

## Index row

Part of `arch/riscv64/` compiles under Kani on an aarch64 host today, and the tree says none can. Proposed: price and build the harnesses that use it, toward closing fatal risk 2 (the proofs prove trivia).
