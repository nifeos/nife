# Is anyone walking the path: Rust in Linux, 2022 to 2026

*Name: provisional. For the evidence under the parent's "The claim to check: is anyone actually
walking the path". calef names files.*

An appendix of [the incremental path](../incremental-path.md). The parent keeps the question, the scorecard and what it does and does not prove. This file keeps
the evidence: the timeline, the line counts, the maintainers on the record, the policy, and the two
gaps at the top of the ladder. Every source is listed in [sources.md](sources.md).

## Releases and line counts

Rust is permanent and it is real. Support merged for Linux 6.1 in October 2022, and the first Rust
drivers landed in 6.8 in March 2024. The hedging ended at the Kernel Maintainers Summit in December
2025, with Linux 7.0 (April 2026) the first release not to mark Rust experimental. That is
not a project in trouble, and any reading that says otherwise is wrong.

But look at where the code is. At Linux 7.2.4 (2026-09-07), `rusted-kernel.com` counts 114,520
SLOC of Rust in a 30,520,324-line tree, 0.38%, of which 63,817 lines are vendored crates
(`syn`, `quote`, `proc-macro2`, `zerocopy`) and 50,703 lines, 0.166%, are first-party kernel
Rust. Against 19,625,567 lines of C. The per-subsystem split of the production drivers is the
telling part: GPU/DRM 11,825 SLOC, Android binder 7,018, block 290, PWM 274, networking 163, cpufreq
156. That growth is real and fast, 23.5x the SLOC of 6.1. It is also, without exception, new leaf
code.

## Not step 1

That is not step 1 of the paper's roadmap. It is the thing the paper distinguished itself from.
The roadmap is *gradual replacement module by module*: retrofit a modular interface onto an existing
Linux subsystem, then rewrite that subsystem behind it. Nothing in the 50,703 lines is a replacement
of an existing C subsystem. The kernel's stated position is that new code may be Rust and existing C
is not rewritten wholesale, and five years of commits agree with it. ext4 is the paper's own poster
child and is still C. So are overlayfs and btrfs. The 0.5%-of-lines-per-year bug rate the paper
measured in those three file systems is being paid today by exactly the code that was paying it in
2021.

## The friction, on the record

The friction is structural rather than incidental, and it is on the record from both sides. At
LSFMM+BPF in June 2024, Ted Ts'o stated the position that decides how far the path can go through a
mature subsystem. He will fix all affected C code when he makes a change, but *"because I don't know
Rust, I am not going to fix the Rust bindings, sorry"*. He called Rust bindings second-class
citizens for the foreseeable future, with the real question being *"where does the pain get
allocated"* (LWN, 2024-06-21). By March 2025 LWN was reporting a spectrum of subsystem postures,
with *"some subsystems are not working with the Rust developers in any way; they simply don't want
to know about the Rust code. That is the case for the DMA-mapping and XArray subsystems"* (LWN,
2025-03-27). The DMA-mapping maintainer stepped down over it.

And the project's own policy encodes the asymmetry. `rust-for-linux.com/rust-kernel-policy`
states that maintainers are not required to learn Rust, and that *"Exceptionally, for Rust, a
subsystem may allow to temporarily break Rust code"*, a carve-out from the kernel's own
no-known-breakage rule that exists for no other language in the tree. That is a sensible, generous
compromise, adopted precisely so the path is not blocked. It is also a written statement that Rust
in Linux is the side that yields.

## Two gaps at the top of the ladder

Two further gaps, both worth naming because they are about the top of the ladder. The kernel still
cannot be built with stable Rust: the Rust project's own 2026 roadmap item exists to *"Build Linux
kernel releases using the stable Rust language (no feature gates), support all the targets that
Linux supports"*, and lists arbitrary self types, a const-traits MVP and ADT const params among the
features targeted for 2026-2027 stabilization. Nine years into Rust's stable life and four years
into the kernel effort, step 2 of the roadmap still runs on `#![feature(...)]`. And step 4,
functional correctness, has not started in mainline at all. There is no verified module in Linux.
The paper's own exemplar for step 3 is Bento, whose FAST '21 paper is reference [43]. The paper
holds it up as the existing demonstration of limited ownership sharing at a Linux interface. Its
public repository was last pushed 2023-12-15. It did not go upstream.
