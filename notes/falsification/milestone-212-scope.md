# Milestone 212: the ratio was a fraction of one directory

*Name: provisional.*

An appendix to [`notes/falsification.md`](../falsification.md), which states the result of
milestone 212 (`script/falsifications` walks `crates/` only).

The walk was `os.walk("crates")`, so the number this script prints as a statement about the tree's
proofs described `crates/` and said nothing about saying so. That is not an undercount. It is a
mislabelled fraction, and §134's whole argument is that a claim about proofs must not rest on an
unstated scope.

Three things were invisible to it by the time it was noticed, all within two days of the mechanism
landing. Milestone 197 (`user/` and `xtask` are out of reach of the prover) put a replayable record
under `user/`. Milestone 193 (put `kernel/src` within reach of the prover) wrote the first two
proofs over the kernel's own source and neither carried a `Falsification:` block, which nothing
reported. Milestone 202 (every confinement test is a ritual until somebody breaks the confinement)
created `kernel/falsifications/` and nothing swept it.

The scope now comes from `cargo metadata`, which is where `script/lint`'s verify-table check already
gets the same fact. A hand-kept list is the same defect one iteration later. `script/verify` has
recorded that failure twice: `multicast_dns_protocol` and then `jh7110_entropy`, each carrying
harnesses nothing ran, each invisible because a suite whose scope is too small goes green *faster*.

Three things fell out of following packages rather than a directory.

The module path now comes from the Cargo target rather than from counting path components. The
old derivation read `crates/<crate>/src/<...>` and would have called `components/src/printenv.rs`'s
harness `printenv::proofs::push_never_writes_past_the_buffer_it_was_given`. It is a `[[bin]]` root,
so it contributes no module segment at all, and the qualified name Kani answers to is
`proofs::push_never_writes_past_the_buffer_it_was_given`. The path and the sweep's `--exact` filter
are one fact with the separators changed, so getting this wrong would have produced a patch path
that could not be swept.

The sweep learned two package-shaped facts, both derived rather than listed. A package with more
than one binary needs `--bin`, because `cargo kani -p user` compiles all 68 programs and fails on
the first `#![no_std]` root that does not mention Kani. `script/verify` solves the same problem by
deriving a `--bin` list from the tree, and a sweep wants exactly one of them. A package containing
`global_asm!` needs `-Z unstable-options --ignore-global-asm`, or Kani refuses to compile it; that
is the kernel, and it is found by grepping for the macro rather than by naming the package.

**A record can exist for something no sweep can run, and that is not rot.** Milestone 202's patch
falsifies a kernel `#[test_case]` that boots under QEMU. §134 checks both directions precisely so a
patch cannot rot into a file nobody runs, so a patch with no harness is normally a failure. This one
is neither, and hardcoding an exemption in the script whose milestone was removing a hardcoded scope
would have been the joke it sounds like. The distinction is drawn mechanically instead: a patch may
open with ``Falsifies `<fully::qualified::name>` ``, and it is a known gap when that name agrees
with the filename and resolves to a function that exists in the package and carries no
`#[kani::proof]`. Everything else is rot, and the rot check stays sharp in the direction that
matters, because a renamed or deleted harness no longer resolves to a function at all. Known gaps
are printed after the ratio and counted in neither half of it.

### What the correction cost, measured

| | before | after |
|---|---|---|
| harnesses seen | 141 | 145 |
| packages seen | 24 | 26 |
| replayable | 25 (18%) | 27 (19%) |
| records nothing can sweep | 0, and one existed | 1, printed |

The ratio was expected to get worse and barely moved, which is worth stating rather than
enjoying. The four harnesses the old walk could not see are two in `user` (one already replayable,
from milestone 197) and two in `kernel`. One kernel record was written here to measure what a kernel
falsification costs, which milestone 212's block asked for before promising one. It is an ordinary
sweep entry, 3.1 seconds, restoring milestone 142's MAJOR 4 itself (`run_end_va` checking only
the addition while the multiply wraps). The `user` sweep, which exercises the `--bin` path, is 4.4
seconds. So the honest reading is not that the denominator was fine. It is that `crates/` held 97%
of the harnesses, and the number was still a claim about a scope nobody had stated.

`script/lint`'s `kani-harnesses` and `harness-crates` counts had the same defect and are fixed in
the same breath. Their derivation walked `crates/` too, with a docstring giving the reason as "the
kernel, the user programs and xtask are not packages it compiles", which stopped being true one
milestone later. Left alone, the tree would have carried two derived numbers for one question, 141
and 145. That is exactly what `<!--count:-->` exists to prevent one level up. The marker name
`harness-crates` now counts packages; it is a name and therefore calef's, and renaming it is
tracked in milestone 212's report rather than done here.
