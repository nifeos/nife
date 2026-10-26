# Proving things about `kernel/src`

Milestone 193. The companion to [verification.md](verification.md), which is about proving the pure
crates; this one is about the 64,818 lines the prover could not see until 2026-08-30, and about the
stubs you take on when you point it at them.

*Name provisional: notes are an interface and their names are an architect's call (AGENTS.md).
`kernel-proofs` says what the file is about and matches `verification.md`'s neighborhood; expect it
to change.*

## Why this note exists at all

Milestone 191 asked whether any Kani harness in this tree had ever caught a defect after the day it
was written. None had. The cause was one line in `script/verify`'s own header, which has always been
honest about it.

> `cargo kani -p <crate>` never compiles the kernel, the user programs, or xtask.

So every proof in the tree was aimed at pure logic, and every concurrency, hardware-contract and
resource-accounting defect the corpus recorded lived somewhere else. DECISIONS §14 promises a
verified core and the core was the part not being verified.

The distance turned out to be four changes.

## What it took, exactly

`cargo check -p kernel --target aarch64-apple-darwin` failed with three errors before this
milestone, and `cargo kani -p kernel` with two more once those were gone. All five, and the fix for
each:

| what stopped it | why | the fix |
|---|---|---|
| `unwinding panics are not supported without std` | the workspace deliberately does not set `panic = "abort"` (DECISIONS §7) | nothing: Kani passes `-C panic=abort` itself, so this is a `cargo check` problem and not a `cargo kani` one |
| `invalid Mach-O section specifier`, twice | `#[unsafe(link_section = ".interrupt_stacks")]` and `".secondary_stacks"` are ELF section names, and the dev machine's host target is Mach-O | `#[cfg_attr(target_os = "none", ...)]`, because the linker script that gives those sections meaning exists only for the bare-metal build |
| `found duplicate lang item panic_impl` | Kani links `std` into the crate it proves, and `std` already defines the panic handler | `#[cfg(not(kani))]` on `kernel/src/panic.rs`'s handler |
| `Crate kernel contains global ASM, which is not supported by Kani` | eleven `global_asm!` blocks: the boot entry, the vector table and the context switch, on all three architectures | `-Z unstable-options --ignore-global-asm`, in `script/verify`'s `kernel` case |

That is the whole list. DECISIONS §4 (kernel shape) rule 1 is why it is this short.
Architecture-specific code lives under `kernel/src/arch/`, so `asm!` appears outside it at only
three sites in two files (`cpu.rs` once, `user.rs` twice) and the other ~50,000 lines are ordinary
Rust that a host compiler was always going to accept. A tree that had let `asm!` spread would have
had no front door here.

## The stub boundary, enumerated

A proof with an unexamined stub is worse than no proof, because it reads as coverage. This is the
exhaustive list of what a harness in `kernel/src` cannot see. The same list is in `syscall.rs`'s
`mod proofs`, where the next harness's author will meet it.

1. Global assembly is skipped. Boot entry, vectors, context switch, three architectures. No harness reaches it, or could.
2. `asm!` is an unsupported construct. Every `asm!` site in `kernel/src/arch/`, plus the three
   above. This is a *hard* boundary: if a
   harness ever calls into that code, Kani reports the unsupported construct instead of proving
   past it.

   This was written as "all of `kernel/src/arch/`" and that was too strong (milestone 255,
   2026-09-04). The boundary is `asm!` and MMIO, not the directory: 4,004 of `arch/`'s 16,225 lines
   are in files containing no `asm!` at all, and `arch/aarch64/iommu.rs` is now proved in place. The
   architecture layer is still overwhelmingly unverified, and the timer re-arm drift is still on the
   far side of this line, but "unreachable by construction" was a claim about a directory and the
   truth is a claim about two constructs. (This paragraph named the VisionFive 2 undelivered wake
   until 2026-09-24; BUGS below records why that reading is retracted.)

   A stub can cross it, at a price: [kernel-proofs/stubbing-an-instruction.md](kernel-proofs/stubbing-an-instruction.md).
3. **Two thirds of `kernel/src/arch/` is not compiled at all, and this is a `cfg` rather than a
   construct** (milestone 304, 2026-09-16). `arch/mod.rs` selects its subtree with
   `#[cfg(target_arch = ...)]` and Kani compiles for the **host**, so a run sees exactly one
   architecture's `arch/` and no line of the other two (item 8 is the one exception). Unlike items 1 and 2 it is **silent**: an `asm!` site makes Kani
   report an unsupported construct, and a `cfg`-excluded file produces no diagnostic of any kind.
   The `kernel` row runs on two hosts and, via script/verify-riscv64, a third target; see below.
4. The panic handler is absent. Nothing proved here says anything about what the kernel does
   after a panic.
5. The link sections are absent. Under `cfg(kani)` the host target is not `target_os = "none"`,
   so `STACKS` and `SECONDARY_STACKS` are ordinary statics rather than statics in their own regions.
   A harness reasoning about guard-page layout would therefore be reasoning about a fiction. Do not
   write one; the region-size claim is held by
   `the_secondary_stack_region_is_the_size_the_linker_reserved` in a real build instead.
6. MMIO and fixed physical addresses are raw pointers to nothing under a model checker. The same
   argument applies: stub the boundary, do not pretend.

   Measured 2026-09-20, it is louder than that (the `maintainer/verus-versus-kani` lane,
   `notes/verus.md`). A harness doing `read_volatile` on `0x0800_0000`, QEMU `virt`'s GICv2
   distributor, does not quietly reason about a fiction: CBMC has no object at that address and
   reports `dereference failure: pointer NULL`, `pointer outside object bounds` and
   `invalid integer address`. So this is a hard boundary like item 2, the good direction. Stub it anyway; the point of the item stands.
7. `script/lint`'s harness-clippy pass excludes `kernel`, so clippy lints do not fire inside
   these harnesses. That pass's own comment carries the two tooling reasons and what still covers
   them. **Keep kernel harnesses free of `unsafe`.**
8. Where an aarch64 host proves `arch/riscv64/iommu.rs`, `crate::arch` is aarch64's:
   [kernel-proofs/riscv64-from-an-aarch64-host.md](kernel-proofs/riscv64-from-an-aarch64-host.md).

## What is proved today

Two properties, both in `kernel/src/syscall.rs`, both about `run_end_va` and the map loops that
guard on it. Milestone 193 (put `kernel/src` within reach of the prover) predicted this code would never become a
crate, rightly: syscall dispatch is
a boundary, not a library.

`run_end_va` computes the last virtual address a `count`-page run starting at `va` covers.

- **`the_run_end_is_exact_and_refuses_exactly_what_does_not_fit`.** For every `va` and every
  `count >= 1`: a `Some(last)` is exactly `va + (count - 1) * PAGE_SIZE` computed without wrapping,
  and a `None` is exactly a run that does not fit in a `u64`. The claim is stated in `u128`, so the
  harness does not echo the implementation's expression.
- **`every_page_between_the_checked_ends_is_itself_a_user_page`.** Both mapping paths check `va` and
  the run's last address and then walk `va + k * PAGE_SIZE` for `k` in `0..count` with nothing
  re-checking the pages between. Given the two endpoints pass `is_user_page_va`, every page in the
  run does too, and no page is computed past the checked end. `k` is symbolic rather than iterated,
  so this covers every page of every run with no unwind bound.

### And two in `kernel/src/arch/aarch64/iommu.rs` (milestone 255)

The SMMUv3 driver, aarch64's IOMMU, has no `asm!` in it, and what it computes is which physical
addresses a device may touch, which `design/fatal-risks/README.md` risk 7 rests on. Its word
arithmetic now sits in three functions beside `attach`, and nothing moved out of `arch/`.
`the_smmu_is_handed_exactly_the_tables_the_kernel_built` proves that each physical address split
across two words reads back exactly, and that no address bit lands on a control field.
`no_stream_can_reach_another_streams_tables` proves that every entry lies inside its table's frame
and no two share a byte. Both were falsified with patches that leave every test green. The register
constants are unproved, and a property proved of the SMMUv3 is not proved of the other two IOMMUs.
The detail is in [kernel-proofs/smmu-proofs.md](kernel-proofs/smmu-proofs.md).

### And two in `kernel/src/arch/x86_64/irq.rs` (milestone 304)

The first properties this tree has checked on `arch/x86_64/`, which no Kani job compiled before
milestone 304 added an x86_64 runner. `no_vector_belongs_to_two_bands` is `MSI_VECTOR_BASE`'s claim
that the three vector bands are "disjoint by construction rather than by anybody remembering".
`an_owned_gsi_routes_inside_the_io_apic_band` found a defect, which the harness carries as an
assumption: a second IO APIC owning GSIs from 200 lets `gsi_vector(210)` wrap onto vector 2, the
NMI. It is not fixed here; see `irq.rs`'s module `BUGS` and
`design/roadmap/proposals/the-gsi-vector-map-wraps-on-a-second-io-apic.md`. Both were falsified by
hand and are recorded `attested`, because the sweep cannot run an x86_64 harness on an aarch64 host.
The detail is in [kernel-proofs/x86-irq-proofs.md](kernel-proofs/x86-irq-proofs.md).

## Which architecture the prover actually sees

One, and it is the host's (milestone 304). This is stub-list item 3, as a table because it is most often read
as more coverage than it is.

| host | `arch/` subtree compiled | harnesses that run |
|---|---|---|
| aarch64 (dev Mac; `ubuntu-24.04-arm` runners) | `arch/aarch64/`, and `arch/riscv64/iommu.rs` | 7: two in `syscall.rs`, two in each `iommu.rs`, one in `fp.rs` |
| x86_64 (cordoba; the `ubuntu-24.04` runner) | `arch/x86_64/` | 5: the same two in `syscall.rs`, two in `irq.rs`, one in `fp.rs` |
| either, via script/verify-riscv64 | `arch/riscv64/`, natively | 7: two in `syscall.rs`, two in `iommu.rs`, three in `mmu.rs` |

Ten distinct harnesses, not twelve: `syscall.rs`'s two pass identically on both hosts.

**riscv64 needed a target, not a host**: milestone 589 (Kani can prove riscv64 from the hosts we
already have) patches Kani, since CBMC runs anywhere. See
[kernel-proofs/riscv64-with-a-patched-kani.md](kernel-proofs/riscv64-with-a-patched-kani.md).

One thing had to change before an x86_64 host could compile the kernel at all. `core::arch::x86_64::__cpuid` is a *safe* function on the toolchain this tree pins
and was an `unsafe fn` until upstream changed it; Kani bundles its own rustc, `kani-0.67.0`
pinning `nightly-2025-11-21`. So four bare `__cpuid` calls in `arch/x86_64/` were four `E0133`s
under the prover and nowhere else. They are wrapped in `isa::cpuid` and `isa::cpuid_count`, whose
`#[allow(unused_unsafe)]` is a labeled exception that comes out when Kani's pin catches up.

### Why these two and not the timer

Milestone 193's block nominates the milestone 6 timer re-arm drift as the first property worth
trying, on the grounds that its property is already proved in `crates/timetable`'s `next_after` and
the timer does not call it. That is still a good future lane, but the re-arm is in `kernel/src/arch/aarch64/timer.rs` and `kernel/src/arch/riscv64/timer.rs`, which item 2 above
puts out of reach: `rearm` reads `CNTVCT_EL0` and writes `CNTV_CVAL_EL0`, both `asm!`. Both are
accessors in `arch/aarch64/instructions.rs`, which a harness can stub. Proving it means first lifting the arithmetic out of the register accesses, and where
that seam goes is a design question.

`run_end_va` was chosen instead on the criterion that matters most given milestone 191's finding:
it has a real defect on record. Milestone 142's review, MAJOR 4, found that the old
implementation checked only the addition in `va.checked_add((count - 1) * PAGE_SIZE)`, leaving the
multiply free to wrap. So a large `count` produced a `last_va` a few pages above `va`, admitting a run that
spanned the address space.

## EXAMPLES

Prove just the kernel's harnesses, with a counterexample trace on failure:

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm
...
Complete - 6 successfully verified harnesses, 0 failures, 6 total.
```

One harness on its own:

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm \
      --harness the_run_end_is_exact_and_refuses_exactly_what_does_not_fit
```

The whole suite, kernel included, the way CI runs it:

```console
$ script/verify
```

Just the kernel row, which is what the x86_64 CI job runs and the only row whose coverage depends on
the machine it runs on:

```console
$ script/verify --only kernel
```

Falsify a new harness before you believe it. This is the tree's standing discipline for proofs
(verification.md says the same) and milestone 191 is the reason it is not optional. Re-introduce the
historical defect and check that both harnesses go red:

```console
$ # in run_end_va, replace the checked arithmetic with milestone 142's bug:
$ #   va.checked_add((count - 1).wrapping_mul(paging::PAGE_SIZE))
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm --output-format=terse
Verification failed for - syscall::proofs::every_page_between_the_checked_ends_is_itself_a_user_page
Verification failed for - syscall::proofs::the_run_end_is_exact_and_refuses_exactly_what_does_not_fit
Complete - 0 successfully verified harnesses, 2 failures, 2 total.
```

Both catch it. That was run on 2026-08-30 and is the reason these two harnesses are worth their
place rather than an assertion that they are.

## Adding a harness to `kernel/src`

1. Put it beside the code it proves, in a `#[cfg(kani)] mod proofs`, not in a separate file. The
   stub list above is the reason: a reader has to meet the caveats where they meet the harness.
2. Check the call graph against the stub list. If it reaches `asm!` or MMIO, stop; you are proving
   nothing, and Kani will say so rather than lie. `crate::arch` itself is no longer the boundary
   (milestones 255 and 304 put harnesses inside two of its three subtrees), but ask which
   architecture you are in. A harness under `arch/<isa>/` runs only on an `<isa>` host, and a
   `riscv64` one only under script/verify-riscv64. That failure is silent, unlike the `asm!` one.
3. Falsify it. Break the code under it and watch the harness fail, then put the code back.
4. `kernel` is already in `script/verify`'s crate table, so nothing needs adding there. If you add a
   harness to a crate that is *not* in that table, `script/lint`'s "every crate with proof harnesses
   is in the verify table" check will fail; that gate exists because two crates
   (`multicast_dns_protocol`, `jh7110_entropy`) each spent months carrying harnesses nothing ran.

## BUGS

- `kernel/src/arch/` is very nearly unproved, and the part that is proved is two files.
  Milestone 255 put two harnesses into `arch/aarch64/iommu.rs` and milestone 304 two into
  `arch/x86_64/irq.rs`; that is 1,279 of 16,225 lines. The corpus's own timer re-arm drift is still
  on the far side of the `asm!` boundary and nothing here touches it. (This entry used to pair it
  with "the VisionFive 2 undelivered wake." That reading is retracted,
  `notes/visionfive2.md`'s fifth bench stop, 2026-08-15: it was a completed tour's terminal state,
  not a stranded receiver, and never happened. Found still repeating it here 2026-09-23.)
- Only one architecture's `arch/` is compiled per run. Milestone 304 (`cargo kani -p kernel` only ever compiled one architecture) added an x86_64 runner to
  the aarch64 one, and milestone 589 a riscv64 job, so every subtree is compiled by some job. Since
  milestone 587 (most CI jobs do not need an arm64 host) the `prove` shards are the x86_64 half and
  `prove the kernel on aarch64` is the other. No single run sees more than one. **`x86_64/machine.rs` is the
  work this unblocked and did not do**: 836 lines of ACPI parsing over lengths, checksums and counts
  that firmware supplied, now reachable for the first time and still carrying no harness. Its
  parsing half already lives in `crates/machine_discovery`, which has no harnesses and no verify
  row; the half in `machine.rs` reads raw pointers into the direct map, which stub-list item 6 says
  to stub rather than pretend. Finding that seam is a lane of its own.
- **Eight harnesses is not coverage of an 85,940-line crate**, and the number to watch is not the count
  but whether the properties are ones a defect would violate. The two here were chosen because a
  defect *did* violate them.
- **`--ignore-global-asm` is a global switch, not a per-item one.** A future `global_asm!` block that
  a harness genuinely needs would be skipped silently rather than refused. The reason it is
  acceptable today is that Kani's `asm!` restriction makes any such harness fail for a second, louder
  reason. "Nothing detects that" was too strong (measured 2026-09-20 by the
  `maintainer/verus-versus-kani` lane). Every run prints
  `warning: Ignoring global ASM in crate kernel. Verification results may be impacted.` That is
  per-run rather than per-item, so it cannot say *which* block was skipped or whether anyone cared,
  which is the substance of the entry. But the run is not silent about having done it.
- `user/` and `xtask` are still out of reach, for exactly the reason the kernel was. `user/`
  holds real parsers over untrusted input and has at least as good a claim on the prover as the
  kernel does. Not attempted here.
- The `kernel` row's 5 seconds in `script/verify`'s table is a dev-Mac number, like
  `jh7110_entropy`'s, and the wrong machine for that column. Replace it from the
  first CI log that carries it. Almost all of it is the crate's own compile rather than solver time,
  so it will grow with the harnesses and not with the kernel.
- The `kernel` row is proved on two hosts and means something different on each, which no single
  number in `script/verify`'s table can say. Its seconds column is one figure for two jobs, and
  the harness count a reader would infer from "the kernel row passed" is the host's rather than the
  tree's. The tree has six kernel harnesses; neither host runs more than four.
- The paragraph below was the old statement of this and is kept as the account. It said the
  `kernel` row *needs* an aarch64 host. Since milestone 304 it needs an aarch64 host and an
  x86_64 one, and the `aarch64-cpu` fix it names as a contingency had already landed on 2026-08-31,
  which is why the second host cost nothing on that axis.
- The `kernel` row needed an aarch64 host, and nothing enforced that. `kernel/Cargo.toml` depends
  on `aarch64-cpu` unconditionally, and `crate::arch::mmu::Format` resolves by the host's
  `target_arch` under Kani, so `cargo kani -p kernel` on an x86_64 box would fail to build and would
  be proving a different `Format` if it did. It works today because every job in
  `.github/workflows/verify.yml` runs on `ubuntu-24.04-arm`, which is aarch64 Linux, and because
  the dev machine is Apple Silicon. Milestone 193's block predicted the opposite ("CI is x86_64
  Linux and it very likely will not [compile]"); the premise was checked and is false, and the two
  ELF-style `link_section` names it worried about are valid ELF anyway, so only the Mach-O host ever
  rejected them. If that runner label ever changes, this row breaks, and the fix is putting
  `aarch64-cpu` behind a target `cfg`.
- **This note does not price the rest of the work.** What was measured is that the front door opens.
