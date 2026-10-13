---
risk: 2
color: amber
updated: 2026-08-31
---
# Appendix to risk 2: The proofs prove trivia, and the real bugs live where Kani cannot reach

*An appendix to [`2-the-proofs-prove-trivia.md`](2-the-proofs-prove-trivia.md) (risk 2). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

The verification half of DECISIONS §14 (the project's direction: a verified-Rust capability
microkernel that runs real workloads) is real but narrow, and narrow in the direction that does not
matter.

### Evidence today

112+ Kani harnesses and `notes/verification.md`. Against: the VisionFive 2's undelivered-wake bug
was found by a bench on three harts, invisible in QEMU, and no proof was positioned to see it.
*(Corrected 2026-09-25 under §216 (fatal-risk facts are correctable, and verdicts are the architect's): that reading is retracted. `notes/visionfive2.md`'s fifth bench stop, 2026-08-15,
found a completed tour's terminal state rather than a stranded receiver, and milestone 201 (is
multicore reliability converging) keeps it as row D6, class `retracted`. The timer re-arm drift
below is the counterfactual that stands.)*

### The experiment

Milestone 191 (did the proofs catch the bugs?), against this project's own defect history, with a
second pass over the harnesses asking which prove a property that could plausibly have been false.

### The verdict of record

RUN, 2026-08-30. AMBER. The red half is that no standing proof has caught a regression: every
defect a proof caught was caught while its harness was being written. The second reason is the
roughly 18% of `kernel/src` in files calling `asm!`, which no harness passes. notes/proof-retrospective.md
has the study; PR #589. *(Reworded 2026-09-25 on the architect's ruling that day. It said "the red
half is structural", naming a crate boundary milestone 193 (put `kernel/src` within reach of the
prover) removed on 2026-08-30.)*

- No Kani harness in this tree has ever caught a defect after the day it was written. All eighteen
  defects in the corpus were found by something else: a flaky suite, a boot on real silicon, a
  fuzzer, the mutation sweep, loom, a code read, or a CI lint. No red `script/verify` run appears
  anywhere in the record.
- On 2026-08-30 the cause was one line of `script/verify`'s own header, verified rather than
  inferred: *"`cargo kani -p <crate>` never compiles the kernel, the user programs, or xtask."* So
  64,818 lines of `kernel/src` were out of reach by construction. And that is exactly where every
  concurrency, hardware-contract and resource-accounting defect lived. The proofs were not failing
  to catch bugs in the code they covered; they did not cover the code the bugs were in.
  *(Corrected 2026-09-25 by milestone 536 (two records still say the prover cannot see
  `kernel/src`), under §216: this bullet stayed in the present tense for three weeks. Milestone
  193 made the header line false the same day, and PR #1276 corrected the header itself.
  Measured from the merged tree on 2026-09-25: `kernel/src` is 86,528 lines, 42,953 of them
  non-blank non-comment, and eight harnesses prove code in it. What stops a harness now is `asm!`, a
  fixed-address MMIO read, or an `arch/` subtree the host does not compile. Files calling `asm!`
  hold 15,966 lines, every one under `arch/`; that overcounts, because Kani refuses on the call
  graph rather than the file. [`notes/kernel-proofs.md`](../../notes/kernel-proofs.md) has
  the per-host table.)*
- Why it is amber and not red. Two real defects were caught *while harnesses were being written*
  (`dtb::be32`'s unchecked `at + 4`, reachable from a corrupt device tree on the boot path;
  `pci::intx_irq`'s pin-0 underflow). That is the survivorship asymmetry this file's rule 1 warned
  about, showing up as evidence rather than as an excuse.
- The strongest counterfactual is nearly a measurement. The timer re-arm drift of 
milestone 6 (threads, the context switch, and preemption), 100 Hz configured and ~70 Hz delivered, has its
  property already proved in this tree, over already-written code, in `crates/timetable`'s
  `next_after`. The timer does not call it.
- The numbers were wrong and are now counted: 145 harnesses, not the roadmap's "112+".
  `script/verify` runs 140. 31,725 of 206,728 source lines are reachable, though both sides of that
  ratio count comments. And `kernel/src` is 40% comment by measurement (25,762 of 64,818 lines). So
  any published figure should be in code lines rather than raw ones. 19 `kani::cover!` vacuity
  guards exist, in 4 of 24 harness crates. And a vacuous harness reports `SUCCESSFUL`.
- The reverse pass found real chaff, which is what makes the green half credible:
  `capability::subset_is_reflexive` proves `a & !a == 0`, a tautology no plausible implementation
  error breaks. And twelve of the 26 `paging` harnesses are per-ISA restatements of six properties.

### What it changes

The verification claim should be stated as what it is: proofs over the pure crates, with the kernel
itself largely unverified.

Milestone 197 (`user/` and `xtask` are out of reach of the prover) closed the second half on
2026-08-31, and half-refuted its own premise. `user/` was argued to deserve the prover more than the
kernel did, because it holds parsers over bytes this system did not produce. It mostly does not any
more: rule 7 and the host-testability discipline already lifted the initrd, ELF, GPT, mDNS,
directory-entry, terminal-escape, shell and glob parsers into crates, every one already in
`script/verify`'s table. The prize had been collected under other numbers, which is the tree working
as designed rather than a disappointment.

It still found a live defect, and found it before any harness ran. `rmle`'s save buffer was sized
for the document's text while `save` writes rows `\n`-joined, so a full document staged 3,231 bytes
into a 3,200-byte buffer and panicked the editor on `^S`. Eight months old and invisible to any test
that does not hit both limits at once. It came out of *writing the property down*, which was the
first time the two constants were compared. And the fix is a `const` assertion beside the buffer
rather than a harness: rung one, because the claim is a relationship between compile-time constants.

And it measured a trap worth more than the proof. Three properties were tried and abandoned with
numbers. A sum over 32 symbolic values: no answer in 20 minutes on two solvers. A symbolic index
into a 3.5 KB struct: CBMC out of memory in 3m23s. Any claim downstream of 20 divisions: no answer
in 10 minutes. The last is the lesson: the same harness asserting only a length bound returns in 0.3
seconds, because `--slice-formula` discards the divisions. A fast harness can be evidence that the
assertion asked nothing, and nothing in this tree currently distinguishes those two cases.

### And the amber moved the same day

Milestone 193 (put `kernel/src` within reach of the prover) was minted from this finding and built
hours later. Two properties proved over `kernel/src/syscall.rs`, with nothing moved into a crate
first. Both were falsified before being believed, by re-introducing the real wrapping-multiply
defect of milestone 142 (a text display good enough that people use it instead of a GUI) and
watching them turn red. That is the counterfactual this study said the tree did not have, and it now
exists. It cost about 10 seconds of `script/verify`. `kernel/src/arch/`, `user/` and `xtask` were
still out of reach that day, so the amber stood; what changed is that the reason became a worklist
rather than a wall. *(Corrected 2026-09-24: this read "are still out of reach" long after it
stopped being true. `user/` came within reach with milestone 197 above, `xtask` compiles and is
refused on value, and milestone 304 below put `arch/x86_64/` beside `arch/aarch64/`. Only
`arch/riscv64/` is still out of reach, and the amber now rests on that and on the survivorship
caveat, not on reach.)*

And on 2026-09-16 a proof caught a real kernel defect, on an architecture the prover had never
compiled. Milestone 304 (only ever compiled one architecture, and it was the runner's) found that
`cargo kani -p kernel` selects its `arch/` subtree by `#[cfg(target_arch)]`, which under Kani is the
host. So every CI job and the dev Mac had been proving `arch/aarch64/` and nothing of the other two.
The premise was measured rather than assumed: two `assert!(false)` probes placed in `arch/riscv64/`
and `arch/x86_64/` produced *"4 successfully verified harnesses, 0 failures"*, because neither
subtree was compiled.

The first proof ever pointed at `arch/x86_64/irq.rs` went red. `gsi_vector` is a flat
`GSI_VECTOR_BASE.wrapping_add(gsi)`, and `MAX_REDIRECTION_ENTRIES` is documented as the reason it
"cannot silently wrap onto an exception vector" while actually bounding the IO APIC's entry count,
not the GSI. A second IO APIC based at global interrupt 200 with the 24 entries every real part has
admits GSI 210, and `gsi_vector(210)` wraps onto vector 2, the NMI. Nothing has hit it because every
single-socket PC gives its one IO APIC `gsi_base` 0; a nonzero base is legal ACPI and exists on
multi-socket servers.

That is a latent defect on hardware this project does not own, found by a model checker, which no
test in this tree could have reached. It is the class this risk was written to ask about, and it is
the first time this tree has an instance of it. The survivorship caveat above still applies with
full force: the harness caught it *while being written*, like `dtb::be32` and `pci::intx_irq` before
it. So it is evidence that pointing the prover somewhere new pays, not yet evidence that a standing
proof catches regressions. riscv64 remains unreachable to the prover and nobody here can change
that: no GitHub image, no Kani cross-target flag, and CBMC needs a goto-binary for its own host.
*(Corrected 2026-09-25 under §216: "nobody here can change that" is too strong. PR #1276 measured
that `arch/riscv64/iommu.rs`, which has no `asm!`, compiles unchanged under Kani on an aarch64 host
as a proof-only module. It also read from Kani 0.67's source that Kani supports only the x86_64
and aarch64 machine models, so a riscv64 runner would not have helped either. The files that do call
`asm!` fail in rustc before Kani runs. Milestone 432 (the RISC-V IOMMU driver has no counterpart to
the SMMU's proofs) then proved that file from an aarch64 host on 2026-09-25 Corrected again 2026-09-25 under §216: CI run 36088566670 proved riscv64 with a
patched Kani.)* The
fix was deliberately not made in that lane, because it changes a public signature and a documented
policy. It was raised as a proposal with gate `DECISION`, calef chose to route by redirection index
on 2026-09-16. It was built the same day as milestone 308 (a GSI reaches its vector by redirection
index, not by its own number). That block is
[roadmap/0308-route-gsi-by-index.md](../roadmap/0308-route-gsi-by-index.md). The fix does not add to
this risk's evidence and slightly complicates it: the harness's `kani::assume(base == 0)` is gone.
So a standing proof now covers the case. But no machine here can execute the path, which
`kernel/src/arch/x86_64/irq.rs`'s module `BUGS` records where a reader meets the feature. A proof
that catches a regression on hardware nobody owns is still the honest shape of what this risk asks
about.

### Added 2026-10-03 (§216, from #1286): riscv64 proofs are checked against the host's model

The riscv64 `kernel` row that milestone 589 (Kani can prove riscv64 from the hosts we already
have) made provable is checked by CBMC against the host's machine model, not riscv64's. Our
patched Kani links `kani_lib.c` as C source after the Rust symbol table, and `goto-cc` compiles it
with the host's configuration, which replaces the `__CPROVER_architecture_*` symbols Kani wrote.
Measured on patagonia on 2026-10-03: the linked model reads `arm64` and `macos`, with signed `char`
and a 64-bit `long double`, where riscv64 has unsigned `char` and a 128-bit one. Pointer and
integer widths and endianness agree. In CI, on the `ubuntu-24.04-arm` runner, the same link order
should give `arm64` and `linux`; that is reasoned, not observed.

It changes no verdict today. All seven harnesses were rerun with `kani_lib.c` linked last as a
precompiled object, which gives the riscv64 model. For each one, the checks, their statuses and the
SAT variable and clause counts were identical under both models. The differing fields only matter
where CBMC interprets C, and none of these harnesses reaches a C model that reads them. So this
is a gap waiting to bite, not a hole: a later harness that reaches such a model could verify
differently, and nothing would say so. It moves neither half of the amber, survivorship or reach,
since no verdict changed; calef ruled on 2026-10-03 (UTC) that risk 2 stays AMBER. The gate and the
fix are milestone 635
(riscv64 proofs check against the riscv64 model); see [its block](../roadmap/0635-riscv64-proofs-check-against-the-riscv64-model.md).

*(Observed 2026-10-03 under §216 (fatal-risk facts are correctable, and verdicts are the
architect's), by milestone 635's gate: the CI sentence above was reasoned, and it held.)* In
`verify` run 37108539047, job `prove the kernel on riscv64` on `ubuntu-24.04-arm`, every one of the
seven linked binaries read `arch` `"arm64"` and `os` `"linux"`. aarch64 Linux shares riscv64's
unsigned `char` and 128-bit `long double`, so in CI `arch` was the only field Kani wrote that the
link changed; the two C-visible differences measured on patagonia do not occur there. The fix in
the same milestone links `kani_lib.c` precompiled and last, and `script/verify-riscv64` now fails
unless every field Kani wrote survives into every linked binary. `os` is not one of them, since
Kani's model does not carry it, so the linked binary still names the host's OS. The color is
untouched.

### Added 2026-10-06 (§216): the falsification backlog is done

§134 (a harness carries a machine-replayable falsification record, or it is not evidence) asks
every Kani harness to say what evidence it carries that it can fail. In 2026W40, 56 of 225 said
`unfalsified`. Three lanes worked the list down: #1697 (batch 1, merged 2026-10-05), #1701 (batch 2,
2026-10-06) and #1713 (batch 3, the cover-only and x86_64 harnesses, 2026-10-06). Measured from the
merged tree by `script/falsifications` on 2026-10-06: 229 harnesses in 33 packages, 220
`replayable`, 7 `attested`, 2 `unfalsifiable` (1 after #1747, below) and 0 `unfalsified`. So 227 of 229 (99%) carry a
falsification record, and 220 (96%) carry one a machine replays.

The two that are left are deliberate, and §134 gained a fourth state for them the same day,
`unfalsifiable`, which a lint holds to a written reason of at least 200 characters:

- `inter_process_communication::signal_preserves_the_invariant` proves "at most one queue is
  non-empty" over `signal`, which takes no node: its branches pop a receiver or increment a
  counter, so no mistake in it can enqueue anything. It guards against `signal` growing an enqueue
  path later. It is not evidence about the code today.
- `direct_memory_access_validator::a_descriptor_mutated_after_validation_cannot_reach_the_device`
  was listed here as holding by design. Corrected 2026-10-06 (UTC) by #1747: it was falsifiable
  all along. Its memory model was a fixed table, so a double fetch could not show. `ChainMem::racing`
  models a racing driver, and a double-fetch patch turns the harness red. `unfalsifiable` stands at 1.

What this does and does not move. It closes the question milestone 191 opened about chaff, whether
harnesses prove properties nothing could have made false: every harness but two now has a recorded
defect it catches, and the two say why they have none. It says nothing about either amber reason.
A falsification record is made on purpose, by the person writing it, which is the survivorship rule
1 names: it shows a harness *can* go red, never that one *has* caught a regression nobody planted.
The nearer evidence is milestone 741 (does a standing proof notice a regression?), a mutation
measurement on 2026-10-04, and its defects were planted too.
And the record covers only what is harnessed, so the roughly 18% of `kernel/src` in files calling
`asm!` is as far out of reach as it was. The color is calef's (§216) and is untouched.
