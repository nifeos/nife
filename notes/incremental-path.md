# The incremental path to a safer kernel, and why nife is not on it

*Provisional name (`notes/incremental-path.md`), written 2026-09-21 by the `notes/hotos-incremental-path`
lane. The HotOS '21 paper is the best-stated argument against doing what this project is doing, and
it appeared nowhere in this tree. Anyone who knows the literature will raise it, and a project whose
falsification list has nine entries should not be surprised by the tenth-looking one.*

This note argues with a paper and concedes its premise. The conclusion is not that Li et al. are
wrong. It is that their path requires standing somewhere this project does not stand, and that the
costs they name are already written down here, in `design/fatal-risks/README.md`, by people who had not
read them.

## What was read, and when

The paper is Li, Miller, Zhuo, Chen, Howell, Anderson, *An Incremental Path Towards a Safer OS
Kernel*, HotOS '21, DOI 10.1145/3458336.3465277, read in full on 2026-09-21. Everything quoted from
it below is quoted from its PDF. The other sources (Rust-for-Linux's policy, the Rust project's 2026
roadmap, a per-release Rust line count, two LWN articles, the Bento repository and
`design/fatal-risks/README.md` at `b6d8fa2d`) were read the same day. Each, with its URL, is in
[sources.md](incremental-path/sources.md). `script/citations` can check none of the external
material, so that table is the only mechanism here and the discipline is the author's.

## The argument, at full strength

Li et al. open by refusing the move this project made: *"This is a call to arms to evolve a widely
used operating system into one that is also safer and functionally correct."* The target is Linux,
and the first premise is scale rather than quality. Linux developers *"have been adding over 1.5M
lines of new code per year, to a codebase that already stretches to several tens of millions of
lines"*, and *"hundreds of new security vulnerabilities are reported each year, and lifetime analysis
suggests that the new code added this year has introduced tens of thousands of more bugs."*

The clean-slate answer is named and priced in one sentence, and it is this project's sentence:
*"One attractive option is to scrap Linux and start over."* They cite the whole family, strong
typing, linear types, and full verification, and dismiss it on adoption rather than on technique:
*"These OS kernels have significantly fewer features than Linux (Figure 1), impeding adoption."* Two
paragraphs later the same point lands harder: *"the cost of switching from Linux to these
clean-slate designs is prohibitive due to the established Linux and Android ecosystems."*

The CVE analysis is the empirical core, and its numbers are specific. They analyzed every Linux
CVE from 2010 up to Linux 5.10 and bucketed each by Common Weakness Enumeration ID: *"Among the 1475
total CVEs we examined, roughly 42% CVEs could be prevented with compile-time type and ownership
safety, and an additional 35% with functional correctness verification. The remaining 23% have a
variety of causes"*, which they enumerate as improper security design, numeric errors like integer
overflow, and other causes. So 77% of a decade of Linux CVEs is claimed to be in reach of the
techniques this project also uses, and the claim is an argument for applying them to Linux rather
than for leaving.

The ext4 finding is the part that stings, because it refutes the comfortable reading of the CVE
curve. One might hope vulnerabilities cluster in young code that matures out of them. They checked:
*"when we look at ext4, a Linux file system in wide use for 12 years, 50% of CVEs in ext4 were found
after 7 years or more of use."* And on the wider file system sample, *"Even after 10 years, there are
still new bugs (0.5% bugs per line of code each year) in all three file systems"*, the three being
overlayfs, ext4 and btrfs. Maturity is not convergence. Testing has been applied to these subsystems
for a decade, and the paper lists the tools by name, and the rate does not go to zero.

The roadmap is four steps, each a strictly stronger contract at a module interface. Step 1 is
modularity: *"callers of any module must only reference the modular interface and cannot directly
depend on any specific implementation."* Step 2 is type safety, a module rewritten *"without the
use of void pointers or casting values to incompatible types"*. Step 3 is ownership safety, which
they define as *"a multi-threaded version of memory safety"*, and they are explicit about what it
buys: modules in a language like Safe Rust *"are immune to entire classes of bugs, from NULL pointer
dereferences to buffer overruns to memory leaks to data races"*. Step 4 is functional correctness,
a module proved against a specification. The steps compose rather than merely sequence: *"Each
requirement strengthens the previous, so the interface for one step informs the interface for the
next."*

The research challenges are honest about what Linux's own design costs them, and a hostile reader
should not skip them. Each step meets a concrete obstacle in Linux as it is: TCP state smeared
through generic socket code, VFS interfaces passing `void` pointers, an `inode` field that is "only
maybe protected", and `buffer_head`'s sixteen state flags. The paper admits it is almost impossible
to know the right interfaces for composed verification before anything is built. The cases and
quotations are in [linux-research-challenges.md](incremental-path/linux-research-challenges.md).

And the closing argument is economic rather than technical, which is why it is the strong one.
*"Our guiding principle is incremental benefit for incremental work. Society should not have to wait
for Linux to be completely verified end-to-end to begin to see benefits in stability and security."*
Every step pays on the day it lands; a clean-slate kernel pays on the day someone switches, which
may be never. Measured on expected safety delivered per unit of effort, that is a better bet than
this project's, and it is not close.

They were also watching the right thing at the right moment. Written in May 2021, the paper cites
the Rust-for-Linux RFC as reference [8] and calls it out: *"In fact, the Linux community has just
taken a major step at introducing Rust into the kernel leaf modules."*

## The claim to check: is anyone actually walking the path

calef's answer below turns on an empirical assertion, *"they are not doing so consistently"*, and an
assertion in a note is worth nothing. Five years and change have passed since the paper. Rust in
Linux is the one place the incremental path was most available, most resourced and most publicly
committed to. Here is what it did.

Rust is permanent and it is real. Support merged for Linux 6.1 in October 2022, and Linux 7.0
(April 2026) is the first release not to mark Rust experimental. That is not a project in trouble.

But look at where the code is. At Linux 7.2.4 (2026-09-07), `rusted-kernel.com` counts 50,703
lines of first-party kernel Rust, 0.166% of a 30,520,324-line tree. That growth is real and fast,
23.5x the SLOC of 6.1. It is also, without exception, new leaf code.

That is not step 1 of the paper's roadmap. It is the thing the paper distinguished itself from.
The roadmap is gradual replacement module by module, and nothing in those lines replaces an
existing C subsystem. ext4, overlayfs and btrfs are still C, so the bug rate the paper measured in
them is being paid today by exactly the code that was paying it in 2021.

The friction is structural, and it is on the record from both sides. Maintainers will not fix
Rust bindings, and some subsystems decline to work with the Rust developers. A written policy lets
a subsystem break Rust code, which makes Rust the side that yields. The kernel still cannot be
built with stable Rust, and step 4 has not started in mainline at all. The timeline, the
per-subsystem counts, the quotations and their sources are in
[rust-in-linux.md](incremental-path/rust-in-linux.md).

The honest scorecard, then. Step 1, modularity retrofitted into existing subsystems: not
attempted at scale; the VFS and DRM interfaces Rust drivers bind to were already the modular ones.
Step 2, type safety: running, on new code, on unstable Rust. Step 3, ownership safety: running, on
new code, 0.166% of the tree. Step 4, functional correctness: not started. Five years, enormous
talent, the full backing of the largest software project in the world, and the incremental path has
reached a sixth of a percent of the code and none of the subsystems the paper measured.

**What that does and does not prove.** It does not prove the path is wrong, and the trajectory
argument is genuinely available. 23.5x in eleven releases compounds, and a strategy that only adds
safe code still bends the curve for new code, which is where the paper says the bugs are being
minted. A fair reading is that the incremental path is working on new code and has not begun on
old code, and that the old code is where ext4's 0.5% per year lives. What it does prove is that
*"the cost of switching from Linux"* is not the only prohibitive cost in the comparison. Retrofitting
has a cost too, it is being paid in maintainer relationships rather than in lines, and five years is
enough to say that it is not small.

## calef's answer, which concedes rather than disputes

2026-09-21, on being shown the paper:

> "If I was Linux, I would take the approach in that paper. They are not doing so consistently. I'm
> not part of Linux and I can't move Linux, so nife is a counterapproach with the deficiencies
> identified in that paper."

Three things are being said, and the first is the one the paper does not price.

1. The incremental path is only available to someone who can move an existing system. Every step
in the roadmap is an edit to Linux. Step 1 is a refactor of Linux's interfaces, and the paper's own
challenge section shows how deep that refactor goes: the TCP references smeared through socket code,
the `inode` field-by-field ownership, `buffer_head`'s sixteen flags. Doing any of it requires the
consent of the people who maintain that code, on their schedule, against their priorities. The paper
calls this a research challenge. It is more than that. It is a precondition, and it is unevenly
distributed: the number of actors on earth who can land a step-1 refactor in ext4 is small, and does
not include this project. The five years above are what that precondition costs the people who *do*
have it.

So the two positions are not in conflict, and it is worth being precise about why. **The paper
answers "what should Linux do", and this project answers "what should someone who is not Linux
do".** Those are different questions with different feasible sets, and both answers can be right at
once. A reader who takes the paper as a refutation of clean-slate work has read a scoping argument as
a universal one.

2. The deficiencies the paper identifies are already on this project's own risk list, and they got
there first. This is the strongest thing this note can show, and it is checkable. `design/fatal-risks/README.md`
was written on 2026-08-30 by people who had not read this paper; it is nine claims that, if false,
mean the project should stop. Two of them are the paper's critique of the clean-slate path, in this
project's own words.

Risk 1 is titled "Only software written for nife runs on nife", and states the claim so it can
fail: *"the platform can run hand-written Rust and nothing else, so every piece of software anyone
wants has to be rewritten."* Its own assessment of its severity is unsparing: *"it is structural.
Optimization cannot fix 'nothing runs here', and no amount of kernel work changes it. A system in
this state is a research demonstrator forever."* That is Li et al.'s *"significantly fewer features
than Linux ... impeding adoption"*, written independently, and stated more harshly.

Risk 8 is titled "Nobody needs it": *"everything works and no one has a reason to run it."* And
it records that it has already fired once, when the project's first customer took their backups to
Linux in August 2026 because nife was not ready. That is Li et al.'s *"the cost of switching from
Linux to these clean-slate designs is prohibitive"*, observed in a sample of one, at home, with real
data.

**So the paper does not add a tenth risk. It sharpens two.** What it contributes that this tree did
not have is the *measurement* behind the objection: 1475 CVEs bucketed, ext4's 50%-after-seven-years,
and the incremental-benefit-for-incremental-work framing that makes the adoption cost an argument
rather than a worry. Risk 1's entry should be read knowing that a HotOS paper made the same point
about seL4, Hyperkernel, RedLeaf and Theseus in 2021 and that none of those four displaced anything.

3. And there is one thing the paper's own five years hand back, which is not a rebuttal and
should not be dressed as one. The paper's costs are real and this project pays them. Its *benefit*
argument assumes the incremental path converges, and the measurement above is that it has converged
on 0.166% of the tree and none of the subsystems it measured. Nife is a counterapproach with the
paper's deficiencies, in calef's words, and it is worth running partly because the path he would
take if he were Linux is going slowly in the one place it was most available.

## Why this is risk 8's counter-thesis and not risk 4's

`design/fatal-risks/README.md` risk 4 already carries a published counter-thesis, and this is a different
one. Conflating them would cost the file its precision, so the distinction is worth stating.

Risk 4 is "The architecture imposes a per-crossing cost that cannot be engineered away", and its
counter-thesis is about what a boundary costs. The 2017 Rust-kernel paper and RedLeaf argue that
language isolation removes the crossing, so a capability crossing is *"a cost this project chose
rather than inherited"*. That argument is settled by a number: milestone 168 (a multi-tasking
workload benchmark) produces it, and both sides agree in advance what the number would mean. It is an
argument inside the design space of new kernels, between two ways of getting isolation.

Risk 8 is "Nobody needs it", and this paper's counter-thesis is about which path gets memory safety
into production. Li et al. take no position on capabilities, on IPC cost, or on where isolation
comes from. They would be equally content with a verified module, a Rust module, or a
capability-confined one; their four steps are agnostic about mechanism and specific about *location*,
which is inside the kernel people already run. No benchmark decides it. The deciding evidence is
adoption over years, on both sides: whether Linux subsystems get replaced, and whether anything
clean-slate gets run.

Two consequences follow from keeping them apart. **A win on risk 4 does not touch this argument at
all**: a microkernel that beats Linux on every crossing benchmark and that nobody runs has lost the
argument Li et al. are making, and risk 8 says that plainly. And this argument cannot be answered by
building, which is its uncomfortable property and exactly why risk 8's entry says *"There is no
experiment here"* and puts itself last in the numbering. The only reply available is a customer.

## What this note does not claim

It does not recommend anything, and it is not a verdict on Rust-for-Linux, which is a large project
with good people whose difficulty here is with a thirty-million-line C codebase and not with Rust.
Nothing about nife's design changes because of this paper; DECISIONS §14 (a verified-Rust capability
microkernel that runs real workloads) is untouched, and so is every technique. What changes is the
framing a reader should bring to `design/fatal-risks/README.md` risks 1 and 8, which is that the objection
behind them has a citation, a measurement, and a name.

## BUGS

- The Rust-in-Linux line counts are from a third party, `rusted-kernel.com`, and were not
  reproduced by counting a kernel tree here. They are cited with the release (7.2.4, 2026-09-07) and
  the date read so a reader can check them, but this note did not check them.
- The paper's three figures were not read, only its prose. Figure 2a (CVEs per year), 2b (the
  ext4 CDF) and 2c (bugs per line per year) are images in the PDF. Every number attributed to them
  above is taken from the sentences in Sections 2 and 5 that describe them. The underlying CVE
  categorization was not re-derived, so the 42/35/23 split is quoted, not verified.
- The step-1 scorecard is an argument from absence. "No existing C subsystem has been replaced
  behind a retrofitted modular interface" rests on the published policy, the per-subsystem line
  counts, and the LWN coverage. A retrofit in progress somewhere that has not landed would not
  appear in any of those.
