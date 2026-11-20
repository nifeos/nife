# RISC-V Summit Europe 2026, read for what it changes here

*Name provisional (`riscv-summit-2026`), like every name a lane mints. Written 2026-09-20 by
`maintainer/riscv-summit-research`, on calef's ask for the most recent RISC-V Summit talks relevant
to nife.*

Everything below carries a URL. Where this note characterizes what someone said, it quotes them.
No gate in this repository can check an external citation, so the only protection is that the
pages were read rather than recalled, and that the three categories below are kept apart.

The three categories, used on every entry, because conference talks blur them and the difference
is the whole value of the note:

- **CLAIM**: a speaker said it. It may be true and it is not a standard.
- **RATIFIED**: it is in a ratified RISC-V specification, verified against a second source that is
  not the talk.
- **SHIPPING**: somebody can buy or rent the silicon today.

## Which summit this is, and why

Three editions are live on `riscv.org`'s summits page
(https://riscv.org/community/risc-v-summits/, read 2026-09-20):

| Edition | Dates | State on 2026-09-20 |
|---|---|---|
| North America | *"February 16 to 18"*, San Francisco, **no year given**, future tense, *"More details to come"* | **not yet held.** The prior North America summit was 2025-10-22/23 in Santa Clara: its Linux Foundation page says *"This event has passed"* (https://events.linuxfoundation.org/riscv-summit/) |
| **Europe 2026** | *"Bologna from Monday 8th to Thursday 11th June, 2026"* (https://riscv-europe.org/summit/2026/) | **held, and the materials are public** |
| China | *"October 18 to 20"*, Shenzhen | not yet held |

So the most recent summit that has happened and published anything is RISC-V Summit Europe 2026,
Bologna, 2026-06-08 to 2026-06-11, and that is what this note reads. The core conference ran
Tuesday 9 to Thursday 11; Monday 8 was tutorials and Friday 12 side events.

### What could be read, and what could not

Read in full: the program with every abstract and speaker bio, at
https://riscv-europe.org/summit/2026/presentations. Each talk has a stable anchor (`#P-XXXXXX`)
used below. Read as slides: three PDFs pulled from
`https://riscv-europe.org/summit/2026/media/proceedings/` and extracted with `pdftotext` (Asanović,
Dellow, Krčmář). **Not read**: every video. They are public on YouTube and unwatched here, so no
claim below rests on one, and where the abstract is all there is, the entry says *abstract only*.
Nothing was behind a login or a paywall.

**One warning about this material, because it caught this lane.** An automated summarizer asked for
the Thursday half of the program invented slide and video URLs of the shape
`...-12h30-SLIDES.pdf` and `youtu.be/VIDEO_ID`, with `Speaker: TBD`. They do not exist. Every URL in
this note was taken from the page's own HTML, not from a summary of it.

## The talks that change something here

### 1. RISC-V Server Platform 1.0 (Radim Krčmář, Qualcomm): **RATIFIED**

https://riscv-europe.org/summit/2026/presentations#P-SPMRUA

The slide deck's own task-group page says "Ratified May 2026", and that is confirmed
independently: `riscv-non-isa/riscv-server-platform` tag `v1.0`, published `2026-05-06T20:48:48Z`,
release note *"First ratified release."*
(https://github.com/riscv-non-isa/riscv-server-platform/releases/tag/v1.0).

The deck's content slide lists what a conforming server is: RVA23S64, Server SoC 1.0, IOMMU,
PCIe 6.0, IMSIC, BRS-I 1.0 meaning UEFI and ACPI, SBI HSM, `Sstc`, `Sv48`, and a security
model of root of trust, secure boot and attestation. The abstract's framing: *"enables OS and
hypervisor developers to target a single portable binary."*

What it means here. This is the first time riscv64 has a boot-and-discovery contract of the kind
aarch64 has had, and it is **ACPI plus UEFI, not device tree**. This tree's riscv64 boot path is
device-tree throughout (`notes/visionfive2.md`: *"entered in S-mode with OpenSBI behind the SBI
calls, `a0` = boot hart id, `a1` = device-tree pointer"*), while
milestone 322 (one machine matrix for three architectures) already carries an ACPI path for x86_64. A server-class riscv64 host,
including any rented one under milestone 88 (nife on rented silicon), will hand this kernel ACPI and
UEFI rather than a DTB. That is a concrete port item, and it is the subject of the proposal filed
beside this note.

### 2. Beyond Privilege: the RISC-V isolation toolbox (Andy Dellow, Qualcomm): **CLAIM**

https://riscv-europe.org/summit/2026/presentations#P-USADYC

The single most on-thesis talk at the summit, given by the chair of RISC-V International's Security
Horizontal Committee. Read from the slides, not the abstract. It lays the isolation mechanisms out
as layers: privilege levels, ePMP, virtual memory, and the hypervisor extension. Then **RISC-V Worlds**
(a World ID tag *"Carried with transactions"* and checked at the device side or in the
interconnect), Supervisor Domains (SDIDs, a *"Root Domain Security Manager"*, per-domain
`SDSM`s), and Memory Protection Tables (*"Additional page table walk"*, *"One MPT per SDID"*,
*"RWX per leaf node granule"*).

The abstract is explicit that this is direction and not standard: Supervisor Domains are
*"introduced as an emerging architectural direction within this toolbox"* and *"still evolving"*.
Worlds+ gets a slide reading *"Watch this space…."*.

What it means here. Nothing to build, and a framing worth having. This project's isolation is
capabilities in software over an MMU; the hardware roadmap is heading for *more* coarse hardware
domains beneath the supervisor, not fewer. The place it touches the tree is
DECISIONS §20 (IOMMU-backed DMA isolation): a World ID or an SDID is a second, lower authority over
the same device traffic an IOMMU mediates. If silicon ever arrives that has both, the kernel's assumption
that it is the only thing between a device and memory needs re-reading.

### 3. CHERI as a new base ISA: RV32Y / RV64Y: **CLAIM**, and the one calef should see

Two independent places said this at Bologna.

Krste Asanović, RISC-V State of the Union
(https://riscv-europe.org/summit/2026/presentations#P-N9KRDZ), slide 12, *"RISC-V New Security
Extensions in Progress"*, lists among seven items:

> CHERI
> - New base ISAs (RV32Y/RV64Y) bringing capabilities to RISC-V

Tariq Kurd, Why the industry needs CHERI to be able to meet the EU Cyber Resilience Act
(https://riscv-europe.org/summit/2026/presentations#P-PC8KYU), argues the regulatory case, abstract
only: the CRA *"is fully enforced in for all products 'with a digital element' sold in the EU from
December 2027"*, requires products to be *"secure by design and by default"* and to have *"no known
vulnerabilities"* at sale, and *"CHERI systems have memory safety bu[i]lt-in which resolves 70% of
vulnerabilities seen in weaker non-CHERI legacy systems."* The 70% figure is the speaker's and is
not sourced on the page.

Status, checked away from the talk. The RISC-V CHERI specification is
`v0.9.10-draft-a3b39b6, 20260918`, marked **"DRAFT---NOT AN OFFICIAL RELEASE"** and *"in the Stable
state"* with *"Assume anything could still change, but limited change should be expected"*
(https://riscv.github.io/riscv-cheri/). So: not ratified, not shipping, and not an extension.
It is a new base ISA family, `RV64LYA` being the 64-bit base plus capability encoding.

What it means here. This is the only item at the summit that speaks to the project's thesis
rather than its port list, and it cuts both ways, which is why it is an architect's to weigh rather
than a lane's. DECISIONS §14 (a verified-Rust capability microkernel that runs real workloads) bets
on capabilities enforced by a microkernel over an MMU. CHERI offers capabilities enforced by the
hardware on every pointer. They are not the same object (CHERI capabilities are memory-safety
capabilities over an address space; nife's name objects and rights), and an argument that they are
substitutes would be wrong. But a stranger who hears "capability OS" in 2028 may have RV64Y in mind,
and the tree currently says nothing about the difference. That is a documentation gap, not a code
one, and it is the second proposal filed beside this note.

### 4. RVA23 in the Linux kernel, against a shipping part (Guodong Xu, Charlie Jenkins): **RATIFIED** profile,
SHIPPING silicon

https://riscv-europe.org/summit/2026/presentations#P-B7EASJ (abstract only)

RVA23 is ratified: the abstract says *"ratified by RISC-V International in October 2024"*, and
RISC-V International's own announcement is dated 2024-10-22
(https://riscv.org/blog/risc-v-announces-ratification-of-the-rva23-profile-standard/), with the
specification at https://docs.riscv.org/reference/rva23/v1.0/index.html.

The talk's content is the software half. **SpacemiT's K3** is described as *"the first mass-produced
RVA23 SoC"*; kernel coverage of RVA23's mandatory extensions *"stuck near 68% for a year"* and
*"Linux v7.0 reached 100%"*. The classification claim is the useful one for a kernel writer:
extensions sorted *"by whether an extension adds architectural state the OS must save and restore"*,
which *"show[s] roughly two-thirds are stateless and only need to be discoverable, not
implemented."* (One sentence in the original, split here because the dash joining it is one this
tree's own lint forbids.)

What it means here. Two things. First, the two-thirds/one-third split is the right shape for
this kernel's own RVA23 question, and this tree has never asked it. The work is context-switch state
for the mandatory extensions, and everything else is `hwprobe`-style discovery a userspace program
asks for. Second, `Ovlt` and `Oilsm` from Asanović's slides 6 and 7 are a new kind of thing to
track. They are *"optimization guidance options"* that say how fast something is rather than whether
it exists. They are *"Intended to be mandatory for future RVA profiles"*, a CLAIM: no profile after RVA23 is
ratified.

### 5. State Sensitive Counter, `Sssscnt` (Fengxue Zhang, Bohua Kou, Alibaba DAMO): **CLAIM**

https://riscv-europe.org/summit/2026/presentations#P-WWT8EV (abstract only)

A proposal, not a specification. It fills the RISC-V gap against *"Intel APERF/MPERF, ARMv8.4-AMU"*
so that the Linux scheduler's PELT load tracking can be frequency-invariant: counters that *"enable
the derivation of real-time operating frequency and normalized utilization without costly
synchronous queries."*

What it means here. It is the same problem milestone 74 (cycle counters: SBI PMU on RISC-V,
`PMCCNTR_EL0` on aarch64) and DECISIONS §139 (who may read the cycle counter, and by what authority)
are about, arriving from the scheduler's side rather than the benchmark's.
`notes/riscv-cycle-counters.md` already records the exact failure this proposal exists to prevent:
the counters are M-mode state, the supervisor must go through SBI PMU, and *"an `ecall` inside a
cycle measurement measures the `ecall`."* `Sssscnt` is a CSR read where nife has a firmware call.
No action: it is a proposal with no ratified home, and the right response is to have read it, not
to build against it.

### 6. Building the software ecosystem for a RISC-V datacenter (Jon Taylor, Canonical): **CLAIM**

https://riscv-europe.org/summit/2026/presentations#P-BTUW3M (abstract only)

*"Canonical moved to requiring RVA23 with the release of Ubuntu 25.10"*, and the talk *"discusses
the data center Canonical will be building to include RISC-V RVA23 silicon supporting the
Launchpad.net community website"*. Future tense; no date, no capacity, no public access claimed.

Paired with Asanović's slide 5: *"First RVA23 server-class systems appearing in 2026"* and, on what
the ecosystem still lacks, *"Need large-scale deployments, cloud instances to support devs."*

What it means here. It is the honest answer to whether rented riscv64 capacity is about to get
easier. **It is not, yet.** Milestone 89 (Scaleway EM-RV1: a second RISC-V implementation, rented)
still names the only concrete rentable riscv64 box this tree has found, and the summit's own chief
architect lists cloud instances as a thing the community *needs* rather than has. That is a useful
negative: it means 89 should not be deferred in the hope that a better rental appears.

### 7. Heuristic-free system call interception on RISC-V, `vpoline` (Monticelli, Colonnelli, Santimaria): **CLAIM**

https://riscv-europe.org/summit/2026/presentations#P-3EZXZV (abstract only)

*"the first fully heuristics-free system call interception library for RISC-V"*, which *"leverag[es]
the RISC-V linker relaxation mechanism"* to do what `zpoline` does on x86-64 *"while overcoming the
intrinsic limitation of requiring privileged access."*

What it means here. Adjacent rather than actionable, and interesting for one reason: it is a
technique for putting a policy layer in front of a program's system calls without the kernel's help,
which is the thing a capability kernel claims you should not need. Worth a reader's attention next
to milestone 205 (how a foreign program is told what to do) if the argument-vector work ever grows a
shim.

### 8. AIA, IOMMU and sub-system verification (Adnan Hamid, Breker): **CLAIM**, and mostly an absence

https://riscv-europe.org/summit/2026/presentations#P-MEXBCF (abstract only, no slides published; a
ten-minute demo-theater slot)

The only talk at the summit with IOMMU in the title. It is about verifying an IOMMU sub-system,
*"leveraged to show examples of sub-system behavior verification"*, from a commercial test-generation
vendor.

What it means here, and the finding is the absence. Fatal risk 6 asks whether a capability-confined
userspace driver can drive real hardware at real speed, and the RISC-V half of that is waiting on
milestone 143 (silicon IOMMU: carrying 16b's driver to a board that ships the ratified spec).
Nothing at this summit announced a shipping RISC-V IOMMU part. The IOMMU appears once as a
mandatory line item on the Server Platform 1.0 content slide and once as a verification example. So
143's gate has not moved, and the honest reading is that the first IOMMU silicon a stranger can buy
arrives with the first Server Platform 1.0 server, not before.

### 9. Confidential computing via MPT (Haoyuan Liu, BOSC): **CLAIM**

https://riscv-europe.org/summit/2026/presentations#P-EXN7PD (abstract only)

*"the first open-source hardware implementation of the MPT draft specification (v0.4)"*, measured at
*"only 2.32% average SPEC06 performance overhead and a 0.244% core area overhead"*, offered as *"a
hardware reference for SMMPT standardization"*. Draft, by the speaker's own word.

What it means here. Nothing to do. It is listed because it is the concrete number behind
Dellow's MPT slide, and because a second physical-memory permission check underneath the supervisor
is the kind of thing that would eventually want a line in `notes/iommu.md` if it ratifies.

## What was at this summit and is not relevant, and why

Naming this is the point; a note listing only hits is a filter nobody can disagree with. By volume,
most of the program was AI silicon and vector/matrix extensions, and none of it touches a
microkernel:

- Matrix extensions got two invited talks and a chunk of the State of the Union (four competing
  approaches: dot-product, matrix-in-vector, vector-matrix, separate-matrix; Asanović slide 9:
  *"expect freeze/ratifications for most this year"*). This is compute ISA. nife does not vectorise
  anything, and a matrix extension is context-switch state only once a profile mandates it.
- Edge AI and smart eyewear (Fariselli, Magno, Lingardo), automotive ISO 26262 / ASIL-D
  (Nuclei, Mauderer), HPC application porting (OpenFOAM, `llama.cpp`, Lustre, Tenstorrent
  N-body). All applications on top of Linux.
- Hardware verification methodology (DiffTest-H, SVM, VASCO). This is RTL verification, a
  different discipline from the software proofs of fatal risk 2, and nothing in it transfers.
- The 128-bit proof of concept (Pétrot, https://riscv-europe.org/summit/2026/presentations#P-MUFY8Z)
  is a delight and is not work: an ELF128 format, a GNU toolchain, QEMU support and CVA6 changes for
  the draft RV128I. Noted so the next person who finds it does not have to decide again.

And one absence worth recording: there was no seL4, no microkernel and no formal-verification-of-a-kernel
talk in the program. Searching the full text of the presentations page for those terms returns
nothing. That is not evidence about seL4. It is evidence that this summit is a silicon and ecosystem
event, and that milestone 127 (the seL4 machine: a Jetson TX1, so identical silicon referees the
comparison) will find its material at a systems conference rather than here.

## BUGS

- No video was watched. Every entry is grounded in a published abstract, a slide PDF, or both,
  and says which. A talk can say something its abstract does not, and three of the entries above
  (5, 6, 7) rest on an abstract alone.
- Only three of roughly thirty published slide decks were read. They were chosen by this note's
  own relevance filter, which means a talk whose title looked like AI silicon was never opened. The
  filter is stated above so it can be disagreed with; it has not been tested.
- Per-extension ratification status could not be read from a table. RISC-V International's
  specification-status wiki does not render its entries to a fetcher, and `docs.riscv.org` redirects
  to a library page. So RVA23, the Server Platform and CHERI each got a second source found by hand.
  The other extensions named here (Worlds, Supervisor Domains, IOPMP, SPMP, lightweight memory
  tagging) are marked CLAIM on Asanović's slide alone, with no independent check of how far along
  they are.
- The China summit runs 2026-10-18 to 20 and will be more recent than this one. This note is
  correct as of 2026-09-20 and has a known expiry.
