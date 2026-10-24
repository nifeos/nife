# What Kani actually stops at, measured rather than described

*Name: provisional, after the parent's "What Kani actually stops at".*

An appendix to [`notes/verus.md`](../verus.md). It holds the three throwaway harnesses run against
`kernel/src` on 2026-09-20 with their verbatim output, and the measurement of how large the
residue is.

Three throwaway harnesses, added to `kernel/src`, run, and reverted. All three outputs are verbatim.

## 1. `asm!` is a loud refusal, and it reaches through a dependency

A harness calling `arch::aarch64::timer::rearm`, the re-arm drift from milestone 6 (threads, the
context switch, and preemption) that `notes/kernel-proofs.md` nominates as the first property worth
trying:

```
Failed Checks: TerminatorKind::InlineAsm is not currently supported by Kani. Please post your
example at https://github.com/model-checking/kani/issues/2
 File: ".../aarch64-cpu-11.2.0/src/registers/macros.rs", line 18, in
 <aarch64_cpu::registers::cntvct_el0::Reg as aarch64_cpu::registers::Readable>::get

VERIFICATION:- FAILED
 WARNING: A Rust construct that is not currently supported by Kani was found to be reachable.
```

Two things worth having in writing. The `asm!` Kani tripped over is in a dependency
(`aarch64-cpu`), not in this tree, so the boundary is the call graph and not the repository. And the
run reported 1 of 42 failed (41 undetermined): everything downstream of the unsupported construct
goes `UNDETERMINED` rather than silently `SUCCESS`. This is the good direction, and
`notes/kernel-proofs.md` already says so.

## 2. MMIO is also a loud refusal, which that note words more softly than the tool behaves

A harness doing a `read_volatile` at `0x0800_0000` (QEMU `virt`'s GICv2 distributor):

```
Failed Checks: dereference failure: pointer NULL
Failed Checks: dereference failure: pointer outside object bounds
Failed Checks: dereference failure: invalid integer address
 File: ".../library/core/src/ptr/mod.rs", line 2082, in core::ptr::read_volatile::<u32>
VERIFICATION:- FAILED
```

`notes/kernel-proofs.md` stub item 6 says MMIO addresses "are raw pointers to nothing under a model
checker", which reads as though a harness might quietly reason about a fiction. It does not: CBMC has
no object at that address and says `invalid integer address` three ways. Worth correcting there.

## 3. The silent one is the architecture `cfg`, and it is silent exactly as documented

A deliberately false harness (`assert!(1 + 1 == 3)`) added under `arch/riscv64/`, on this aarch64 Mac:

```
error: no harnesses matched the harness filter: `the_riscv_scratch_probe`
```

Nothing compiled it, so nothing could fail. That is stub item 3, reproduced. One correction to that
note's `BUGS`: `--ignore-global-asm` is not entirely undetected. Every run prints

```
warning: Ignoring global ASM in crate kernel. Verification results may be impacted.
```

which is per-run rather than per-item, so the `BUGS` entry's substance stands and its wording
("nothing detects that") is a little too strong.

## How large the residue actually is

Measured in this worktree on 2026-09-20, by walking `kernel/src` in Python:

| | lines |
|---|---|
| `kernel/src`, all `.rs` | 81,413 |
| of those, non-blank non-comment ("code") | 40,953 |
| `kernel/src/arch/` | 20,907 |
| lines in files containing an `asm!` call | **15,001**, and **every one of them is under `arch/`** |
| `arch/aarch64` / `arch/riscv64` / `arch/x86_64` | 6,052 / 5,766 / 8,953, of which 4,688 / 4,974 / 5,339 are in `asm!`-bearing files |

So DECISIONS §4 (kernel shape: monolithic, deferred, with two cheap rules) rule 1,
architecture-specific code under `kernel/src/arch/`, has kept the construct-level residue to at most
18% of the kernel, and it is exactly the 18% you would expect.
File granularity overstates it, because Kani refuses on the call graph rather than on the file. It is
still the right order of magnitude, and it is still where the timer re-arm drift lives. (This
sentence used to add "and the VisionFive 2 undelivered wake." That reading is retracted,
`notes/visionfive2.md`'s fifth bench stop, 2026-08-15: it was a completed tour's terminal state, not
a stranded receiver, and never happened. Found still repeating it here 2026-09-23.)

The 64,818 in risk 2 is also stale as a size: `kernel/src` is 81,413 lines today.
