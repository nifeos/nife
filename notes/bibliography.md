# The works this project is arguing with

**Provisional name** (`notes/bibliography.md`): the page's name has not been through
`script/names`.

**One rule, and it is what stops this page becoming a reading list nobody read.** An entry belongs
here only when some page in this tree *reads* it: quotes it, audits the tree against it, or takes a
number from it. A work somebody means to read is not an entry. That rule is the reason the list is
short, and the reason each row can say what was taken rather than what it is about.

The in-tree page is authoritative for every claim; this page is an index to where the reading
happened, not a summary of it. Where a note records that the copy it read was partial, or that an
automated fetch was refused, the caveat lives in that note and is not repeated here.

## Systems this project is measured against

| work | read in | what was taken |
|---|---|---|
| Elphinstone and Heiser, *From L3 to seL4: What Have We Learnt in 20 Years of L4 Microkernels?*, SOSP 2013 | [l4-lessons.md](l4-lessons.md) | a verdict on each original L4 decision, audited row by row against this kernel: 15 of 17 applied, one partial, two not, and the two misses are the process-kernel and direct-switch choices this tree never actually decided |
| Heiser, *The seL4 Microkernel: An Introduction*, seL4 Foundation whitepaper rev 1.4.1 | [trusted-base.md](trusted-base.md) | the TCB definition and size that nife's own 694 `unsafe` blocks are stated beside |
| Elphinstone, Zarrabi, Danis, Shen and Heiser, *An Evaluation of Coarse-Grained Locking for Multicore Microkernels*, arXiv 1609.08372 | [§138 (how a saturated workload is made to hand threads across cores)](../design/decisions/0138-cross-core-handoff-under-load.md) | that seL4 does not migrate threads between cores at all, which reframed a measured nife behavior from a defect into somebody else's deliberate design |
| Watson, Anderson, Laurie and Kennaway, *Capsicum: practical capabilities for UNIX*, USENIX Security 2010 | [capsicum-and-the-retrofit-question.md](../design/capsicum-and-the-retrofit-question.md) | what a capability system retrofitted onto Unix can and cannot reach, as the control for what building from the first instruction buys |
| Baumann, Appavoo, Krieger and Roscoe, *A fork() in the road*, HotOS 2019 | [milestone 52 (subshells without `fork`)](../design/roadmap/0052-subshells.md) | the case that `fork` is a poor abstraction rather than an expensive one, which is why subshells are not getting one |
| Li, Miller, Zhuo, Chen, Howell, Anderson, *An Incremental Path Towards a Safer OS Kernel*, HotOS '21 | [incremental-path.md](incremental-path.md) | the case that memory safety should reach the kernel people already run rather than a new one, which is risks 1 and 8 argued by strangers, plus the check of whether Linux is actually walking it |

## The opposite bet: isolation from the language, not the hardware

This is the counter-thesis, and `design/fatal-risks/README.md` risk 4 treats it as one rather than as
background reading: if a language-isolated crossing is as safe and cheaper, a capability crossing is
a cost this project chose rather than inherited.

| work | read in | what was taken |
|---|---|---|
| Levy, Campbell, Ghena, Pannuto, Dutta, Levis, *The Case for Writing a Kernel in Rust*, APSys '17 | [redleaf.md](redleaf.md), [trusted-base.md](trusted-base.md) | the founding argument, and its six categories of necessarily-`unsafe` kernel code, run against this tree, where the list came out **shorter** rather than longer |
| Narayanan, Huang, Detweiler, Appel, Li, Zellweger, Burtsev, *RedLeaf: Isolation and Communication in a Safe Operating System*, OSDI '20 | [redleaf.md](redleaf.md) | the whole system built on that bet, and its 124-cycle crossing against seL4's 834 |
| Burtsev et al., *Isolation in Rust: What is Missing?*, PLOS '21 | [redleaf.md](redleaf.md) | the authors' own retrospective on what the bet cost them |
| Chen, Li, Mesicek, Narayanan, Burtsev, *Atmosphere: Towards Practical Verified Kernels in Rust*, KISV '23, and Chen, Li, Zhang, Narayanan, Burtsev, *Atmosphere: Practical Verified Kernels with Rust and Verus*, SOSP '25 | [verus.md](verus.md), [redleaf.md](redleaf.md) | what verification actually costs when it is reported honestly: 3.32:1 proof to code, 1.5 person-years, against seL4's 20:1 and 22 |
| Lattuada et al., *Verus: Verifying Rust Programs using Linear Ghost Types*, OOPSLA 2023 | [verus.md](verus.md) | the tool's own account, read beside Verus and Kani both run on this tree rather than compared from memory |

## Method, measurement and what fools a benchmark

| work | read in | what was taken |
|---|---|---|
| Mytkowicz, Diwan, Hauswirth and Sweeney, *Producing Wrong Data Without Doing Anything Obviously Wrong*, ASPLOS 2009 | [milestone 188 (the IPC fastpath)](../design/roadmap/0188-ipc-fastpath.md) | that a Cargo feature inserting one symbol moves everything after it, which is what a +1.49% reading on this tree's own bench turned out to be |
| Curtsinger and Berger, *Stabilizer*, ASPLOS 2013 | [footprint-perturbation.md](footprint-perturbation.md), [milestone 370 (a layout control for the perturbation experiments)](../design/roadmap/0370-a-layout-control-for-the-perturbation-experiments.md) | that layout must be randomized repeatedly, which is why four draws are recorded in a `BUGS` section as bounding an effect loosely rather than proving one absent |
| Burckhardt, Kothari, Musuvathi and Nagarakatte, *A Randomized Scheduler with Probabilistic Guarantees of Finding Bugs*, ASPLOS 2010 | [milestone 245 (a soak cannot tell a flat run from a productive one)](../design/roadmap/0245-a-soak-cannot-tell-a-flat-run-from-a-productive-one.md) | that stress saturates: somebody else measured the coverage curve this project was about to buy hours of soak time to rediscover |

## Learning material, which is a different thing

These are not arguments this project is having. They are what someone starting on the same ground
would want.

- The **xv6 book** (MIT, ~100pp), for how a real Unix-shaped kernel is put together.
- [`rust-raspberrypi-OS-tutorials`](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials),
  for aarch64 mechanics.
- The [OSDev wiki](https://wiki.osdev.org), as a reference rather than a tutorial.
- [Compiler Explorer](https://godbolt.org), set to Rust + aarch64. The fastest way to build assembly
  intuition that exists. [reading-assembly.md](reading-assembly.md) is this tree's own companion to it.

## BUGS

- **This page can go stale silently and nothing gates it.** `script/citations` validates in-tree
  numbered records and cannot check a bibliography; a note that stops citing a work leaves a row here
  pointing at nothing. The same caveat `notes/redleaf/sources.md` states about the RedLeaf source
  table applies to this one: the discipline is the author's alone.
- **Second-hand readings are marked in the notes, not here.** `notes/verus.md` takes one SOSP '25
  table from `notes/redleaf.md` rather than from the paper, and says so; a reader who needs to know
  which claims are first-hand has to open the note.
