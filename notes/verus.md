# Verus, and whether it reaches the code Kani stops at

*Provisional name (`notes/verus.md`), written 2026-09-20 by the `maintainer/verus-versus-kani` lane.
Verus appears nowhere else in this tree except in one line of `script/verify`'s own header, which
says the script was named so that "swapping Kani for Verus someday would cost nothing". This note
is the first attempt to price that someday.*

**Read this for the evidence, not for a verdict.** Adopting a verifier is a dependency and a
methodology decision, which AGENTS.md puts in calef's hands, and the section that would record it is
the maintainer's to mint. The verbatim tool output and the quoted figures are in
[`verus/`](verus/README.md).

## What was read and run, and when

Two Atmosphere papers, the Verus paper, guide and source, `notes/redleaf.md` for one table
second-hand, and two tools run on this Mac: Verus release `0.2026.09.20.aef82ed` and `cargo kani`
0.67.0. All were read or run on 2026-09-20. Every tool message quoted is output this lane produced,
and nothing is recalled. [`sources.md`](verus/sources.md) lists each with where it came from.

## The premise this lane was given is out of date, and that is the first finding

The brief quoted `design/fatal-risks/README.md` risk 2 (the proofs prove trivia, and the real bugs live
where Kani cannot reach):

> The cause is one line of `script/verify`'s own header, verified rather than inferred:
> *"`cargo kani -p <crate>` never compiles the kernel, the user programs, or xtask."* So **64,818
> lines of `kernel/src` are out of reach by construction**.

That has not been true since 2026-08-30. Milestone 193 (put `kernel/src` within reach of the
prover, because today the proofs cannot see it) opened the door for four changes, three of them
one-line `cfg`s. Milestone 255 (a quarter of `kernel/src/arch/` has no assembly in it, and none of it
is proved) put two harnesses inside `arch/aarch64/iommu.rs`. Milestone 304 (`cargo kani -p kernel`
only ever compiled one architecture, and it was the runner's) put two inside `arch/x86_64/irq.rs` and
added an x86_64 verify job. `notes/kernel-proofs.md` is the record, and this lane confirmed it runs.

So the question is not *"can Kani reach `kernel/src`"*. It is "what is left after milestone 193,
and does Verus reach that residue", and the residue is where Verus turns out
to stop too.

## What Kani actually stops at, measured rather than described

Three throwaway harnesses, added to `kernel/src`, run and reverted on 2026-09-20. `asm!` is a loud
refusal, and it reaches through a dependency: the timer re-arm stopped at `aarch64-cpu`'s
`asm!`, with everything downstream `UNDETERMINED`. An MMIO `read_volatile` is also a loud refusal:
CBMC reports `invalid integer address` three ways. The silent one is the architecture `cfg`: a false
harness under `arch/riscv64/` was never compiled on an aarch64 host. By file, every line in an
`asm!`-bearing file is under `arch/`, 15,001 of `kernel/src`'s 81,413, so DECISIONS §4's rule 1 has
kept that residue to at most 18% of the kernel. The 64,818 in risk 2 is also stale as a size.
[`what-kani-stops-at.md`](verus/what-kani-stops-at.md) has the verbatim output and the measurement.

## What Verus is

A rustc driver against a forked compiler, pinned to one exact stable toolchain where this tree pins
a nightly, so the verified crate has to build under the verifier's compiler. Releases are dated
pre-releases with no 1.0, and the project says features may be broken or missing.

It is deductive, not bounded, and that is the whole difference from Kani. The human writes
`requires`, `ensures`, `invariant` and `decreases`, and Verus discharges the conditions in Z3.
There is no unwind bound, so every bounded-model-checking wall this tree has hit is not a Verus wall.
The trade is the specification: the tree's Kani harnesses are typically twenty lines against code
nobody annotated, and Atmosphere paid 20.1K lines of proof for 6K lines of kernel. Verified code is
a dialect inside `verus!{}`, with mathematical `int` and `nat`, permission tokens for raw pointers
and limited `&mut`.

Its concurrency model is sequentially consistent. `vstd::atomic` hard-codes `SeqCst`, and the OOPSLA
paper says Verus does not support relaxed atomics. DECISIONS §4 (kernel shape: monolithic, deferred,
with two cheap rules) rule 4 assumes weak memory ordering. So a green Verus proof over a `Relaxed`
atomic in this kernel would not mean what a reader would take it to, which is the sharpest
nife-specific caveat after the hardware boundary. It does not support hardware intrinsics, `Drop`
or function pointers, and only partly supports `as` casts and trait objects, all load-bearing here.
[`what-verus-is.md`](verus/what-verus-is.md) has each of these with its source.

### What a pass means, and what it does not

A worked example this lane ran, which is the single most important thing in this note:

```rust
#[verifier::external_body]
pub fn read_counter() -> (r: u64)
    ensures r > 0,
{
    let v: u64;
    unsafe { core::arch::asm!("mrs {}, cntvct_el0", out(reg) v) };
    v
}

pub fn nonzero_counter() -> (r: u64) ensures r >= 1 { read_counter() }
```

```
verification results:: 1 verified, 0 errors
```

`#[verifier::external_body]` makes a postcondition an axiom. Verus checked nothing inside that
function, took `r > 0` on trust, and used it to discharge the caller. The claim happens to be *false*
(`CNTVCT_EL0` reads zero at reset, which is how this project learned QEMU does not pass a device tree
pointer in `x0`), and the run is green and silent about it. There is no warning, no count of trusted
items, nothing in the exit status.

That is the mirror image of Kani's behavior. Kani refuses loudly and proves nothing; Verus proves
whatever you assert and says nothing. Neither reaches the hardware. One tells you so.

The project is honest about the mechanism. Its `tcb.md` enumerates how assumptions enter: an `assume`
statement, *"any proof function introduced with `#[verifier::external_body]`"*, *"any exec function
introduced with `#[verifier::external_body]` or `#[verifier::external_fn_specification]`"*, and
`#[verifier::external]`. `assume_specification`, which attaches a contract to an
already-compiled function, carries its own warning: *"assume_specification statement is unchecked, it
can easily be used to subvert Verus's guarantees."* And of `assume` itself: *"successful verification
using assume provides no guarantees about program correctness."* A `--no-cheating` flag forbids
`assume` outright.

But `external_body` is the mechanism a kernel needs, and nothing forbids or counts it, which is
why Atmosphere's TCB section had to be written by hand. `notes/kernel-proofs.md`'s enumerated stub
boundary is the same discipline, arrived at independently: *"a proof with an unexamined stub is worse
than no proof, because it reads as coverage."* If Verus were ever used here, that enumeration stops
being good practice and becomes the only thing standing between a green run and a fiction.

## Does Verus reach `asm!` and MMIO? No, and it says so

Both probes run by this lane, inside `verus!{}`:

```
error: The verifier does not yet support the following Rust feature: inline-asm expressions
 --> inside.rs:7:14
  |
7 |     unsafe { core::arch::asm!("mrs {}, cntvct_el0", out(reg) v) };
```

```
error: Verus does not support this cast: `usize` to `*const u32`
 --> inside2.rs:5:39
  |
5 |     unsafe { core::ptr::read_volatile(0x0800_0000usize as *const u32) }
```

`asm!` is unsupported and an integer-to-pointer cast is unsupported, and the second is the MMIO
idiom, not an incidental one: every fixed-address device register in this kernel is reached that way.
Outside the `verus!{}` macro, or under `external_body`, both compile and are simply not verified.

`no_std` works: the `run_end_va` probe in [`examples.md`](verus/examples.md) is `#![no_std]` and
verified in 0.83 seconds on this Mac.

### And the paper draws the boundary in the same place, by hand

This is the finding that answers the brief's question. SOSP '25's own trusted computing base lists
172 lines of assembly and about 3,000 lines of trusted Rust: the IOMMU, interrupt setup, the APIC,
the IDT, the GDT and TSS, and the syscall entry. Read against this tree's `arch/x86_64/`, it is the
same file list, and in Atmosphere it is trusted, not verified. The rest of its trusted base is the
Verus frontend, Z3, the Rust toolchain and `core`, and about 700 lines of unverified axioms.
[`atmospheres-trusted-base.md`](verus/atmospheres-trusted-base.md) quotes the list.

**So Verus does not reach the code Kani cannot reach.** It reaches *around* it, by letting a human
write down what the hardware seam promises and then proving the rest against that promise. Kani
cannot do that, and that difference is real and is the honest case for Verus. But it is a different
claim from "the prover reaches the driver", and the timer re-arm drift, which lives in that seam,
would have been on the trusted side of an Atmosphere-shaped boundary too.

## The number that decides it: proof effort

Atmosphere's two papers give its proof-to-code ratio as 7.5:1 (KISV '23) and then 3.32:1 (SOSP
'25): 20.1K lines of proof for 6K lines of executable code, in about one and a half person-years of
verification work. Their own comparison table puts seL4 at 20:1 and CertiKOS at 14.9:1, though its
prose disagrees with its table. The most encouraging number is the swing in verification time, from
roughly 30 minutes to under 20 seconds, which is less time than compiling the kernel. seL4's first
proof took 11 person-years, by SOSP '25's account. Atmosphere's call/reply is within 3% of seL4's,
second-hand from [`notes/redleaf/crossing-costs.md`](redleaf/crossing-costs.md).
[`proof-effort.md`](verus/proof-effort.md) has the tables, the OOPSLA examples, and every figure
quoted with its source.

## So: would Verus reach the 64,818 lines?

The number is 81,413 today and the honest answer is in four parts.

1. Most of it is already reachable by Kani, so the question has changed. Since milestone 193 the
prover compiles `kernel/src`. What stops a harness is a call graph reaching `asm!` (loud), an
integer-to-pointer MMIO read (loud), or an architecture the host does not compile (silent). Verus
refuses two of those three by construction and has the third problem in a different form. On
reachability of the hardware seam, Verus and Kani are the same tool. The six kernel harnesses in
this tree are six because nobody has written more, not because Kani cannot see the code.

2. Where Verus is genuinely stronger is unboundedness, and this tree has the receipts for why that
matters. Every wall `notes/verification.md` and `notes/user-proofs.md` record is a bounded-model-
checking wall: unwind bounds, a symbolic index into a 3.5 KB struct, counting, division chains. None
of those is a Verus wall. A loop invariant costs a human an afternoon and costs the solver nothing.
If proofs about `kernel/src` ever stop because the solver ran out of memory rather than because the
code touched hardware, that is the moment this question becomes urgent. It has not happened yet:
the four kernel harnesses cost five seconds, almost all of it compiling.

3. The cost, in this project's units. Atmosphere verified 6K executable lines at 3.32 lines of proof
per line of code. `kernel/src` is 40,953 code lines, 6.8x that, so a linear extrapolation gives
roughly 136,000 lines of proof and ten person-years. Linear is the optimistic reading. Atmosphere
was designed for its proof, with flat permission storage and one big lock, where this tree runs SMP
with fine-grained locking. Atmosphere targets one architecture where nife has three. And Atmosphere
leaves concurrency as future work, the defect class `notes/proof-retrospective.md` finds most often
missed. [`proof-effort.md`](verus/proof-effort.md) has the extrapolation and each reason quoted.

Ten-plus person-years against one architect and a lane budget is not a schedule. Whole-kernel
functional correctness in Verus is not available to this project at this size.

4. A subset is a different question and is not obviously a no. The papers' unit of work is a
subsystem with a written-down abstract state. The ratio that matters for a first attempt is not
3.32:1 across a kernel but whatever one property costs. This lane wrote one, from a cold start,
knowing no Verus: the `run_end_va` probe above, verified in 0.83 seconds against a 42-minute
`script/verify`. It took one failed compile (`nat` versus `int`). That is not
evidence that a subsystem is cheap; it is evidence that the first hour is cheap, which is the only
thing a bounded experiment can establish and is exactly what one is for.

The strongest single argument for looking harder is not a proof at all, it is the 20 seconds. A
verifier that runs faster than the compiler changes what verification *is* in a workflow.
`script/verify` runs on one heavy job at a time because a harness reached
3.5 GB. It is the merge queue's long pole, and needed an `--affected-since` mode to avoid paying for itself on
changes it could not possibly affect. None of that is a property of proving things; it is a property
of bounded model checking.

## What this does not settle

- Nothing here compares soundness. Both tools trust their solver; both have a TCB; this lane did
  not evaluate either one's.
- Nothing here says the two are exclusive. `script/verify`'s name was chosen for a swap, but the
  papers suggest a split (Verus where a specification is worth writing, Kani where panic-freedom over
  hostile input is the property) at least as strongly as a replacement. That is a design question and
  it is not this note's.
- **The tree's own residue is not priced.** What it would cost to write an abstract state for
  `kernel/src/sched.rs` is unknown and is the thing a bounded experiment would learn.

## EXAMPLES

[`examples.md`](verus/examples.md) has each one whole: the `asm!` boundary reproduced in Kani, a
from-nothing Verus install and run on this Mac, the `no_std` `run_end_va` probe, and the false
axiom (`seam.rs`, the worked example above) passing green.

## BUGS

- The SOSP '25 copy this lane read is 10 pages of a 16-page paper. It ends at Section 5 with no
  evaluation, no related work and no bibliography. `dl.acm.org` returns 403 to an automated fetch of
  both the PDF and the full-text HTML, and the paper is CC-BY, so a human with a browser can get it.
  Everything in Section 6 onward is unread here: the microbenchmarks, the comparison against seL4 and
  Linux, any scaling data for verification time against kernel size, and any statement of what the
  proofs failed to catch. The cycle counts above are second-hand from `notes/redleaf.md`.
- No Verus *systems* paper other than Atmosphere was read. The OOPSLA paper was, and its Table 1
  is library-scale. The KISV '23 comparison table gives NrOS as 10:1, but that figure is quoted from
  Atmosphere's table rather than from the NrOS paper, and this lane did not read Anvil, IronSync, the
  verified page-table work, or Microsoft's verified-storage. Those exist and would materially change
  the extrapolation in part 3. A reported 3.9:1 for Microsoft's verified log and roughly 4.5-7.4 for
  Anvil's controllers reached this lane only as search-result summaries, were not read in the
  primary source, and are therefore not quoted above.
- The ten-person-year extrapolation is a ratio scaled linearly and nothing more. It is not a
  measurement, it is not an estimate anyone with Verus experience produced, and the three reasons
  above say it is more likely to be low than high. Treat it as an order of magnitude that says "not
  this project at this size", which is all it can support.
- **Verus was run on scratch files, never on a nife crate.** Not one line of this tree was put
  through it. Whether `crates/capability` or `kernel/src/syscall.rs` would even parse under Verus's
  Rust subset is unknown: the subset's boundaries (traits, generics, closures, `dyn`, lifetime
  shapes) were not tested here at all, and this tree leans hard on all of them.
- **The build story after verification is quoted, not observed.** KISV '23 describes two passes (*"The
  build system first invokes the Verus toolchain on the verified components. Then a regular Rust
  toolchain is used to compile the entire kernel with ghost code erased"*), using *"the same Rust
  version that the Verus toolchain is based on"*. This lane never compiled a verified crate to a
  binary, never tried a custom target JSON, and never tried `aarch64-unknown-none-softfloat`. That is
  the first thing that would break and it is untested.
- **Verus's own concurrency story was not exercised.** `tracked`, `AtomicInvariant` and VerusSync are
  quoted from the papers and the `vstd` sources, not run. Given that five of
  `notes/proof-retrospective.md`'s eighteen defects are concurrency, this is the largest untested gap
  in the note, and SOSP '25 explicitly leaves concurrent drivers as future work. The
  sequential-consistency finding above is quoted from the OOPSLA paper and from a reading of
  `vstd::atomic`. Nobody here ran a concurrent Verus proof to see what it does with a `Relaxed`
  atomic, and "not supported" could mean refused or could mean silently modeled as `SeqCst`. Those
  are very different, and this note does not know which.
- Three things the Verus documentation does not state, found only in its source. That `asm!` is
  rejected (`rust_to_vir_expr.rs`, `ExprKind::InlineAsm => unsupported_err!`); that `vstd` is
  `no_std`-capable (`#![cfg_attr(not(feature = "std"), no_std)]` and a default `std` feature); and
  anything at all about MMIO or a pointer at a fixed physical address. The first two this lane
  confirmed by running the tool. The third is unresolved in both directions: the docs neither permit
  nor forbid it, and the integer-to-pointer cast error above is this lane's only evidence.
- Whether Verus builds for a bare-metal target was never tested and is not documented. KISV '23
  asserts *"due to the lack of managed runtime, verified Rust code can be compiled and executed on
  bare metal"*, which is the authors' claim about their own x86_64 kernel, not a Verus project
  statement, and says nothing about a custom target JSON or `aarch64-unknown-none-softfloat`.
- This note names no winner and is not evidence for a decision either way. It was written to
  price a question, and the pricing has a factor-of-several uncertainty that only running Verus on
  real code would remove.
