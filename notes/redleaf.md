# RedLeaf, and the opposite bet about where isolation comes from

*Provisional name (`notes/redleaf.md`), written 2026-09-20 by the `maintainer/redleaf-comparison`
lane. RedLeaf is the closest academic relative this project has and it appeared nowhere in this
tree: not in `notes/prior-art.md`, not in a roadmap block, not in `design/decisions/`. Anyone who
knows the literature will make this comparison whether or not we have.*

*It is one note covering two papers, because they are one argument. Levy et al. (APSys '17) make
the case that a memory-safe language can replace hardware protection **inside** a kernel; RedLeaf
(OSDI '20) builds a whole system on that case. Reading either without the other gets the bet wrong.*

**Read this note for the mechanism, not for a verdict.** Where nife stands relative to RedLeaf, in
public, is a claim that leaves the machine, and AGENTS.md puts those in the irreversible category.
The evidence is laid out here and the sentence is deliberately not written.

## What was read, and when

Seven documents and one clone, all read on 2026-09-20, are listed in
[`sources.md`](redleaf/sources.md) with where each was fetched. Everything quoted below is quoted
from one of them, and nothing is recalled.

## The ancestor: the 2017 case for writing a kernel in Rust

This is the argument RedLeaf inherits, and it is narrower and therefore clearer. Levy et al. (APSys
'17) state it in one sentence: *"This paper takes a more extreme approach: entirely throw away
hardware protection within the kernel and, instead, write the kernel in a memory-safe programming
language."* They reject Spin and Singularity on their garbage-collected runtimes and large unsafe
code bases. Their contribution is an enumeration: four library abstractions and six pieces of
kernel code that need `unsafe`, plus `TakeCell`, which they had to invent. The paper's whole
quantitative claim is *"The kernel's trusted computing base includes the Rust core library as well
as under 1000 lines out of over 6000 lines of kernel code."* So the famous ratio is the
kernel-authored part of the trusted base, and the larger half is `libcore`. The paper never says
"Tock."

It left three things open, and RedLeaf closed none of them. Multiprocessor: RedLeaf implements it,
but every published number is single-core and nothing says what concurrency did to its trusted base.
Page tables: RedLeaf dissolves the question by not using address spaces, at the price of `fork()`;
this tree answers it head-on in `crates/paging`. Size of the trusted base: RedLeaf regressed from a
number to a list, one that adds an IDL compiler and a trusted build environment.

This tree cannot say it in their terms, for an architectural reason. In Tock and RedLeaf the safe
part of the kernel is not trusted for isolation, so counting unsafe lines measures the trusted base.
In nife the whole kernel is trusted, because it programs the MMU, so the unsafe census measures proof
burden. The comparison that matches is with seL4, which draws the boundary in the same place. No
metric tracks the trusted core's size, which is proposed as
`design/roadmap/0535-the-trusted-core-has-no-size.md`.
[`the-2017-ancestor.md`](redleaf/the-2017-ancestor.md) has the reading in full, the three open
questions answered from the text, and this tree's numbers; `notes/trusted-base.md` has the units.

## Corrections to the sketch this lane was briefed with

The maintainer's from-memory sketch got three of five claims about RedLeaf right. It placed RedLeaf
at Utah, where Atmosphere is; RedLeaf is UC Irvine. It named ownership tracking as what makes
termination safe, when that is the private-heap invariant.
[`corrections-to-the-brief.md`](redleaf/corrections-to-the-brief.md) has each claim against the
text.

## What RedLeaf is

A microkernel where the boundary between components is enforced by the Rust type system instead of
by the MMU. *"RedLeaf is a new operating system developed from scratch in Rust to explore the impact
of language safety on operating system organization. In contrast to commodity systems, RedLeaf does
not rely on hardware address spaces for isolation and instead uses only type and memory safety of
the Rust language."*

Its unit is a *domain*: *"units of information hiding, fault isolation, and composition. Device
drivers, kernel subsystems, e.g., file system, network stack, etc., and user programs are loaded as
domains."* A cross-domain call is *"normal, typed Rust function invocations. Upon cross-domain
invocation, the thread moves between domains but continues execution on the same stack."* That is
the migrating-threads model, chosen deliberately over messages *"to avoid a thread context switch on
the critical cross-domain call path."*

Two things in it will read as familiar here. It is capability-shaped: *"In RedLeaf references to
objects and traits are capabilities"*, and a domain's default authority is exactly one thing,
*"the microkernel system call interface ... the only interface through which the domain can affect
the rest of the system."* And it has a supervision story: shadow drivers, *"lightweight shadow
domains that mediate access to the device driver and restart it replaying its initialization
protocol after the crash"*, which is DECISIONS §26 (the fault endpoint: thread death becomes a message) wearing
different clothes.

On top of it they built Rv6, a POSIX-subset personality following xv6, as a collection of
domains. Its one loud gap is the one a single address space forces: *"We do not support the full
semantics of the fork() system call as we do not rely on address spaces and hence cannot virtualize
and clone the address space of the domain."*

## Bet one: what each system has to trust

*`notes/trusted-base.md` is the measured companion to this section: the three incompatible
definitions of "trusted", the re-derived numbers for this tree, and their six unsafe categories run
against it. This section is the argument; that note is the units.*

This is the interesting axis, and both systems name their own trusted base honestly, which makes the
comparison possible at all.

RedLeaf trusts the Rust compiler, and says so first. *"The core assumptions behind RedLeaf are
that we trust (1) the Rust compiler to implement language safety correctly, and (2) Rust core
libraries that use unsafe code."* Its trusted base is then a list rather than a size (above). It
leaves unsafe Rust extensions and malicious devices out of scope, and does not protect against side
channels.

What happens when the trust is misplaced is where the two systems diverge, and it is structural
rather than a matter of degree. Suppose `rustc` miscompiles a bounds check in a RedLeaf domain, or
an `unsafe` block inside a whitelisted core library has a soundness hole. Then the isolation is gone
and nothing underneath notices, because there is nothing underneath: one ring, one address space,
one page table. If `rustc` miscompiles a nife program, the program is wrong and the kernel's page tables
are unmoved; the confinement claim is made by hardware the compiler did not emit. `notes/c-seam.md`
puts the arithmetic plainly for the worst case it could construct, which is C: *"what a bad index
reaches: any physical memory, any device register [in a monolith] | one page it was granted [confined
here]."*

Neither trust is free and this tree's version can come back red, which is the asymmetry worth
recording. nife's claim is DECISIONS §14 (a verified-Rust capability microkernel that runs real
workloads), and `design/fatal-risks/README.md`'s risk 2 exists precisely to ask whether the proofs prove
anything: its answer is AMBER, and milestone 191 (did the proofs catch the bugs?) found that *"no
Kani harness in this tree has ever caught a defect after the day it was written"* because
`cargo kani -p <crate>` never compiled the kernel. That is a worse-sounding sentence than anything in
the RedLeaf paper, and it is a *better* epistemic position, because it is a falsifiable claim that
was falsified and then acted on (milestone 193 (put `kernel/src` within reach of the prover)). RedLeaf's
trust in `rustc` is not the kind of claim an experiment in the RedLeaf tree can return red on.

The trusted compilation environment is the part easiest to underrate. RedLeaf's safety across
separately compiled domains depends on it checking that every domain was built with the same IDL,
compiler and flags, in safe Rust only. The authors later called it *"incomplete and error prone"*
(PLOS '21). The public tree does not enforce the safe-Rust rule: on the 2026-09-20 clone, 7 of 23
domain crate roots forbid `unsafe_code`, both flagship drivers among the 16 that do not, and the
signer appends a signature without checking safety. The paper's central invariant and the
artifact's enforcement of it are not the same thing.
[`the-trusted-build.md`](redleaf/the-trusted-build.md) has the quotations and the audit.

## Bet two: the cost per crossing

This is where RedLeaf's numbers will be quoted against `design/fatal-risks/README.md`'s risk 4 (*"the
architecture imposes a per-crossing cost that cannot be engineered away"*), and against milestone 168
(a multi-tasking workload benchmark) when it produces one. So the conditions matter more than the
figures.

RedLeaf's Table 1, on a CloudLab c220g2 with frequency scaling and seL4's meltdown mitigations off,
gives 124 cycles for a cross-domain invocation against seL4's 834. The quotable number is the ratio
on one machine, roughly 6.7x, not the absolute: the absolute should never be set beside a nife
figure. A RedLeaf crossing does not change address space or privilege level and does not schedule,
so seL4's row is a different guarantee. On one core under HVF, nife's `ipc_rtt_el0` costs about
4.6 times seL4's `Call` plus `ReplyRecv` ([arm-plan-under-hvf.md](arm-plan-under-hvf.md)), with
four syscalls to seL4's two. No EL0 bench times nife's `CALL`/`REPLY` path, so the matched
comparison is still unmeasured. Atmosphere's
Table 3, from RedLeaf's own authors, puts a hardware-isolated Rust kernel's call/reply at 1,058
cycles against seL4's 1,026. [`crossing-costs.md`](redleaf/crossing-costs.md) has both tables,
their conditions and the caveats.

A verified Rust microkernel with hardware isolation, built by the RedLeaf team five years later,
pays about a thousand cycles for call/reply and lands within 3% of seL4, on x86_64, on silicon.
nife's general path does not land there yet: on one core it is about 4.6 times seL4's fastpath. The
~6-8x that language isolation buys is therefore not a RedLeaf-specific artifact and not a tuning failure on
anyone's part. It is what a privilege and address-space crossing costs, and both halves of that
sentence have now been measured twice by the same group. Risk 4 says the per-crossing cost *"cannot
be engineered away."* On this evidence, that is not a fear. It is measured, and the question a
customer-facing workload actually asks is how many crossings the workload makes, which is exactly
what milestone 168 was minted to answer.

## Bet three: what each system can isolate

RedLeaf can isolate only code it compiled, from source, in its own build environment, in a language
it chose. That is not an oversight; it is the direct consequence of the mechanism. Isolation is the
type system, so anything outside the type system is outside the isolation. The paper's own dynamic
loading section states the invariant this rests on: *"types of all data structures that cross a domain
boundary, including the type of the entry point function, and all types passed through any interfaces
reachable through the entry function are the same, i.e., have identical meaning and implementation,
across the entire system."* A C binary cannot satisfy that. Neither can a Rust binary built somewhere
else.

Their own evaluation contains the cost of this, stated plainly, and it is the most honest paragraph in
the paper. On the key-value store: *"Despite our optimizations, RedLeaf achieves only 61-86%
performance of the C DPDK version"*, because safe Rust forced `Vec<T>` and three
`extend_from_slice()` calls where C used `memcpy`. And then: *"As an exercise, we implemented the
packet serialization logic with unsafe Rust typecast that allowed us to achieve 85-94% of the C's
performance. However, we do not allow unsafe Rust inside RedLeaf domains."* They measured the
speedup, and refused it, because taking it would have cost them the isolation. That is the bet
being paid for, visible in a single sentence.

nife's version of this axis is `design/fatal-risks/README.md`'s risk 1, and it has been run. Unmodified
`ripgrep` 14.1.1 from crates.io, forty transitive crates, zero patches, builds and runs on all three
architectures (milestone 121 (`ripgrep` on nife: enumeration as a capability), and the three transcripts are byte
for byte identical). DECISIONS §31 (the foreign-language seam) confines C that holds no capabilities and
makes no syscalls, demonstrated by faulting it deliberately and restarting it. A language-isolated
system has no move available on either.

The symmetric honesty: that door swings both ways. nife's isolation does not care what language a
component is written in, and it also does not give a nife program the intra-component memory safety
that a RedLeaf domain gets for free. A bad index inside a nife component corrupts that component; a
bad index inside a RedLeaf domain does not compile. RedLeaf buys fine-grained safety *within* a
component and forfeits the ability to run anything it did not build; nife buys the ability to confine
anything at all and leaves within-component safety to whatever language the component chose. Neither
is a subset of the other.

## Where RedLeaf is ahead, without hedging

Four places, and the first two are not close. The per-crossing cost: 124 cycles against seL4's 834
on one machine, which no hardware-isolated design reaches. Drivers measured against the fastest
thing available on real hardware: its ixgbe driver within 7% of DPDK at a batch of one and at line
rate at 32, and its NVMe driver 1% faster than SPDK. A recovery number under a crash loop, a 5% read
and 16% write throughput drop with the block driver crashing every second; nife has the mechanism
and has published no such number. And application numbers against commodity baselines, while
milestone 168 (a multi-tasking workload benchmark) was then PARTIAL (it was built on 2026-10-04,
corrected 2026-10-11 UTC). A fifth is a design
observation: RedLeaf's IDL generates both sides of every crossing from one definition, which is
rung one of AGENTS.md's ladder applied to an IPC surface.
[`where-redleaf-is-ahead.md`](redleaf/where-redleaf-is-ahead.md) has the quotations and figures.

## What happened after 2020

The repository is public, unarchived and stopped: last push 2022-05-09, last substantive commit
2022-01-09, no license, pinned to `nightly-2021-12-15`. The authors' own critique a year later
(*Isolation in Rust: What is Missing?*, PLOS '21) says what the design cost. It names a separate IDL to
express invariants Rust could not, costly continuations to unwind across domains, and type
identifiers an attacker could collide. Their four asks of the language had not landed as of this
reading.

And the group's next kernel took the other bet. Atmosphere (KISV '23, SOSP '25) is an L4-shaped Rust
microkernel verified with Verus, using hardware isolation and a conventional syscall interface, at a
3.32:1 proof-to-code ratio. That is the single most consequential fact in this note. The team that
built the strongest language-isolated OS in the literature spent the following five years building a
hardware-isolated, machine-verified Rust microkernel instead. Nothing in either Atmosphere paper says
RedLeaf was abandoned or why, and this note does not claim it. [`after-2020.md`](redleaf/after-2020.md)
has the repository's measurements, the PLOS '21 quotations and Atmosphere's figures.

## What could not be established

Six things stay unknown rather than inferred, among them why RedLeaf stopped and whether anyone
reproduced the 124-cycle figure. [`sources.md`](redleaf/sources.md) lists all six.

## BUGS

- **This note was written from seven documents and one shallow clone. It is not a survey.** Citing
  papers is not the same as tracking a field, and the isolation-in-Rust literature since 2021
  (KSplit, Theseus, Netbricks, Splinter, VeriSMo, NrOS, Asterinas) was seen only through these
  authors' related-work sections, which are not a neutral source about their neighbors.
- Nothing was read about what Tock became. The 2017 paper is treated here as an argument, which
  is what it is, and the nine years of Tock since it are out of scope. A claim in this note about
  the 2017 position is not a claim about Tock today, and if anyone wants the second thing it is a
  separate reading.
- **The "RedLeaf did not answer the 2017 questions" finding is an argument from absence**, which is
  the weakest shape of finding there is. It rests on the RedLeaf paper containing no TCB size figure
  and no multi-core measurement, both of which this lane established by reading the full text and
  grepping it. An absence in a ten-page paper is not an absence in the work.
- No RedLeaf number in this note was reproduced. Every figure is quoted with its stated
  conditions, and every one comes from a paper's own evaluation of its own system. That is the weakest
  class of benchmark evidence there is, and it is the only class available without CloudLab access.
- The `#![forbid(unsafe_code)]` audit is a grep over crate roots, so it undercounts (a crate could
  forbid unsafe in a submodule) and overcounts (a crate with no `unsafe` needs no attribute). It
  establishes that the invariant is not *mechanically* enforced. It does not establish that any
  shipped domain violates it in a way that matters.
- The line counts are raw lines including comments and blanks, on both sides. `notes/counted-claims.md`
  and `design/fatal-risks/README.md` both record that `kernel/src` is 40% comment by measurement, so any
  size comparison drawn between this tree and RedLeaf's 48,749 lines is invalid as written. It is
  included for order of magnitude only.
- **The PLOS '21 reading of what Rust still lacks was not re-checked against current Rust.** "As of
  this reading none of the four has landed" is this lane's belief from the language's own tracking
  history and was not verified against a 2026 nightly. Treat it as unconfirmed.
- Nothing here is a positioning statement, on purpose. The comparison is laid out and the
  concluding sentence is not written, the same way
  `design/roadmap/0528-cheri-capabilities-are-not-these-capabilities.md` stops short of one.
  Where nife stands relative to RedLeaf is an architect's to say.
