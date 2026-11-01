---
status: BUILT
raised: 2026-09-25
built: 2026-10-07
promoted_from: the-x86-64-progenitor-serves-entropy-from-rdseed
milestone_dependencies: 205, 206
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 595. The shell runs a `std` program, and `rg pattern` works at the prompt

Minted 2026-09-25 by the maintainer's lane `maintainer/shell-runs-std`, from
the gap #1314 recorded in `notes/foreign-program-arguments.md`'s `BUGS` section. *(Number and title
provisional: the integrator mints the number at merge, and the title is a draft until an architect
names it.)* The progenitor's `std` layout was built on 2026-09-26 by lane
`milestone/595-std-layout`. Its x86_64 half was promoted from the proposal
`the-x86-64-progenitor-serves-entropy-from-rdseed` and built the same day by lane
`milestone/595-x86-std`. Lane `milestone/595-rg-at-prompt` finished it on 2026-10-07. See "What
is built" below.

## The gap, checked 2026-09-25

The shell could not launch a `std` program at all: every program `swish` could name was native.
`ripgrep` ran only from the kernel test harness, which builds the address space `std` expects
(the eight slots `patches/std-nife/overlay/std/src/sys/pal/nife/rt.rs` fixes, the file-service page,
32 stack pages). The progenitor's loader had never built a child in that layout.

## The goal

From the prompt, `rg needle docs` runs confined to the directories the line granted and prints its
matches. A path outside the grant is not found, because nothing names it. This holds on aarch64,
riscv64 and x86_64 per DECISIONS §19 (architectural parity is a tenet), or a scope note says which
architecture is missing and why.

## What is built (2026-09-26, lane `milestone/595-std-layout`)

The progenitor builds a `std` child, and `std_exerciser` runs from the prompt. Proven by two
`script/swish-check` lines on aarch64 and riscv64, through the real `crates/system_initializer`
rather than the kernel's harness:

- `caps std_exerciser` prints the slots a `std` child gets, which are not a native child's. The
  heap's budget is at 0, stdout at 1, the clock at 5, entropy at 6, the configuration page at 7.
- `std_exerciser` prints its offline transcript. Each asserted phrase is a slot or a page landing
  where `std` reads it: the heap at 0 is `vec sum`, the empty slots 4 and 2 are the two `honestly
  unsupported` lines, and so on down to `config seeded`. When one is missing the program panics,
  and the gate fails on the fault. The line's comment in `xtask/src/swish_check.rs` maps each
  phrase to its slot.

What it took:

- `crates/std_runtime_contract` (provisional name): the eight fixed slots, the three shared
  pages' addresses and the 32 stack pages, in one crate. It is generated verbatim into the PAL as
  `runtimeproto`, which `rt.rs` now re-exports, and the kernel harness reads it too. Before this
  the numbers were written three times; the progenitor would have been a fourth.
- `grant_plan::Manifest::runtime` (provisional), `Runtime::Native` or `Runtime::Std`, and
  `Prog::StdExerciser` at wire id 16. The field is where a package's manifest will say the same
  thing under §219's option D. The enum variant is an archive program named the old way, and it
  decides nothing about D.
- `system_initializer::StdLayout` (provisional): the same decisions `spawn_service` already
  makes (which output, which directory, which of the manifest's pages), placed at `std`'s slots.
  A directory grant goes to slot 4 with the file page at `0x1100_0000`, though no manifest asks
  for one yet (see BUGS).
- One region per `std` job, which is also its heap: `grant_plan::STD_REGION_PAGES`, 384. A heap
  split off the job's region would stop `job_undertaker` reclaiming it (`SPLIT` pins a region
  until its children are destroyed), and a heap beside it would be a region nothing reclaims. The
  job pool grew by one such region.
- `caps` prints `std`'s slots for a `std` program. A `grant_plan` test,
  `a_std_program_declares_only_what_the_std_layout_can_hold`, refuses a `std` manifest that asks
  for anything the progenitor would plan and not deliver: a file, an input, `--mem`, a domain, a
  second stream or the network.

## What is built on x86_64 (2026-09-26, lane `milestone/595-x86-std`)

`std_exerciser` runs from the x86_64 prompt, and so do the four `uuid` lines that leg had omitted
since milestone 182 (x86_64's own interactive-boot entry point). `script/swish-check --arch x86_64` now types 81 of 85 lines on its first boot; the four it
omits need a NIC (two network runs, two package fetches). Green on patagonia under OVMF three
times, the last after rebasing onto #1318's merge (442 s for the first boot).

The gap was entropy. The progenitor built its entropy service only from a virtio-rng on a
virtio-mmio slot, and `q35` has no mmio bus.

### Who confirms `RDSEED` exists: the kernel

The proposal's open question, decided as reversible, for three reasons not about effort:

- The tree already does it this way. `entropy_service::is_instruction_backend_available` reads the
  feature bit from `arch::isa`'s boot record, `components/src/entropy.rs` says it trusts its
  spawner's choice of mode, and the installer (`install_service`) already takes the instruction
  service on the booted x86_64 path.
- It is the only answer with an aarch64 twin. `ID_AA64ISAR0_EL1` is not readable at EL0, so a
  progenitor that ran `CPUID` itself would be an x86_64 special case (DECISIONS §19).
- Detection stays in `kernel/src/arch/`, and `crates/system_initializer` gains no
  `cfg(target_arch)`.

### How it reaches the progenitor: a built service, not a flag

`kernel::user::boot_instruction_entropy`
(provisional) calls `entropy_service::ensure(Bus::Instruction)` when no virtio-rng was granted,
reads the readiness report before the progenitor starts, and grants the request endpoint at slot 16,
`BootEndowment::entropy_ep` (provisional). This is the file service's shape (`fs_ep`, slot 5). The
three `START` words are spent, and a bit in a page would be a second format for one fact. No new
syscall, method or object type. The progenitor probes slot 16 before its first retype, because
after that a first-free object could sit there.

### A machine without the instruction refuses loudly

Falsified once with `NIFE_CPU='max,-rdseed'`.
The kernel printed `entropy : NONE. No virtio-rng device, and this CPU has no seed instruction
(RDSEED, RNDRRS), so nothing at the prompt can draw random bytes and there is no login.` Then the
gate's new entropy-source check failed on that sentence, and `std_exerciser` died with `std::random`'s
own refusal. A first draw of all zeros gets a `REFUSED` sentence and no grant. There is no fallback to
`RDRAND` or to software.

The gate now reads the source on every leg: swish-check asserts the progenitor's
`entropy service up` sentence, naming a virtio-rng on aarch64 and riscv64 and the seed instruction
on x86_64, with the kernel's refusal as the negative.

### The machines, and the login stack

QEMU's `-cpu max` implements `RDSEED`, so no `-cpu` change was needed. xenon's i5-7500T (Kaby Lake)
has it per Intel's documentation, not read on xenon. With entropy, x86_64 builds the login stack at
boot for the first time: one more line before the prompt, and a capability-slot peak at hand-over of
22 of 24 (17 before), read by a temporary instrument, matching aarch64 and riscv64.

## What it waited on, and what finished it

Every fork was ruled by 2026-09-27. §219 (how the shell names an installed program to the
spawner) option D lets an unvouched `rg` run by its path; installing it with `jig` is milestone 121
(`ripgrep` and enumeration)'s, on calef's 2026-10-07 ruling. Milestones 205 (how a foreign program
is told what to do), 665 (designating a foreign program's words) and 206 (a program image has under
896 KiB) gave it words, a grant from a word, and room.

## What is built: an image as large as `rg` (2026-09-27, lane `milestone/595-std-image`)

`spawnproto::IMAGE_MAX_PAGES` went from 64 pages to 1024 (4 MiB), and `rg`'s image is about 670.
Proven by `script/swish-check`: `installed/std-echo-large big`, `std_echo` padded with zeros to 768
pages, prints its words. The kernel harness already loads a segment that large; this proves the
path from the prompt.

- The progenitor stages and builds an image in a pool of its own, `IMAGE_POOL_PAGES` (2,432
  pages, provisional). The job pool stays small, so the gate still runs more jobs through it than
  it holds.
- An image's region is `grant_plan::image_region_pages`: a named program's region plus the
  image's pages, so a large `std` image keeps the same heap.
- The shell's budget is one constant both sides read, `spawnproto::SHELL_BUDGET_PAGES`
  (provisional), 128 plus one image's staging. It was two constants kept equal by hand.
- The shell's image window is now 4 MiB and crosses 2 MiB page-table spans. Its tables come from a
  small region split once (`IMAGE_TABLE_PAGES`) rather than one primer page, so no table is ever
  paid from the budget above a staging region.
- Milestone 604 (the builder's scratch cursor is bounded)'s compile-time check now counts the
  image pool too, since its pages are built through the same scratch window: 4,086 of the 8,192
  pages it allows.

## What is built: `rg` at the prompt (2026-10-07, lane `milestone/595-rg-at-prompt`)

Unmodified `ripgrep` 14.1.1 from crates.io, run by its path at the prompt, searches the directory a
word on the line granted and nothing else. Proven by `script/swish-check` on aarch64, riscv64 and
x86_64 under TCG on patagonia, 2026-10-07, after `helpers/build-ripgrep.sh`. The aarch64
transcript (riscv64's and x86_64's are the same, byte for byte; x86_64 typed 150 lines in 516.9 s):

```
$ /installed/rg needle docs ../hay/secret.txt
  rg: ../hay/secret.txt: IO error for operation on ../hay/secret.txt: `..` would leave the granted directory, which no capability designates
docs/n.txt:find the needle here
$ /installed/rg needle
  rg: failed to get current working directory: operation not supported on this platform
did your CWD get deleted?
```

`secret.txt` sits beside `docs` in `/hay` and holds the needle too. `caps /installed/rg needle docs`
previews `cap 4  endpoint  dir  /hay  (only docs in it, named on the line)`, read-only, and
`provenance: unvouched`.

What it took:

- **A note for a program whose source is somebody else's.** `cargo xtask foreign-note`
  (provisional) compiles `grant_plan::UNVOUCHED_STD_MANIFEST` into a relocatable object of its
  own, through `manifest_note::Note::of`, the function `carry!` expands to. It is compiled for
  the bare-metal target of the same ISA, so the compiler writes the header and riscv64's
  float-ABI flags. `helpers/build-ripgrep.sh` links it with `-Clink-arg`, and the shared link
  script keeps it in the `PT_NOTE`. Before this the shell ran `rg` by its path as a native
  program that hears no words. The source is untouched.
- **Its symbol table off the disk copy.** With only debug info stripped `rg` was 4.6 MiB on
  aarch64, and the shell refused it as "larger than an image may be (4 MiB)". `--strip-all`
  leaves 2.7 to 2.9 MiB on all three architectures (`disk::without_symbols`); nothing that runs
  a program by path reads its symbols. The archive's copy keeps them.
- `disk::INSTALLED_RIPGREP` (`installed/rg`, provisional) is written to the swish-check disk when
  `rg` was built, and swish-check types three lines to it in `/hay`, beside `std_grep`'s.

## What it unblocks

- Milestone 121's three outstanding items: the confined demonstration, the loud refusal of a
  directory lacking `ENUMERATE`, and the benchmark that prices the walk. Each becomes a typed line.
- Milestone 123 (the demonstration: somebody else's software, running narrow). Its first element is
  a ported tool run here and on Linux over one corpus. A tool a person cannot type is a harness
  result, not a demonstration.

## Which fatal risks it serves

Risk 1 (only software written for nife runs on nife): a stranger's program now does its job at the
prompt, where a stranger would type it. Risk 7 (the confinement claim is false) gets its most
legible test: a search that cannot see outside its grant.

## The gate that proves it

Two halves, because `rg` is absent from CI. Both are `script/swish-check` lines rather than the
kernel boot test (`shell_runs_std_tests.rs`) this block first proposed, because swish-check types
at the real prompt through the real `crates/system_initializer`, which a scripted shell in the
kernel harness would only imitate.

- In every build, on all three architectures, CI included: `/installed/std-grep needle docs`,
  an in-tree `std` program run by path with a string argument and a directory grant a word made,
  prints `docs/n.txt:find the needle here`. With no word naming anything it is granted nothing
  and says so. `installed/std-echo one 'two words'` prints its argv back.
- When `rg` was built: the three `/installed/rg` lines above. They skip, and the leg says so,
  wherever `target/ripgrep/<triple>/rg` is absent, CI included. Making CI build `rg` is the §46 (thin
  primitives or whole subsystems) decision nobody has made, and nothing in this milestone needed it.

The answers are substrings of the transcript, as for every swish-check line, not the
byte-for-byte comparison first proposed.

## BUGS

- The hardware bytes are served as the instruction returns them: unmixed, and not health-tested
  after the first draw. That is DECISIONS §137 (a hardware TRNG with no published health-test claim)'s option A, which every other backend here already
  does. §137 is `DECIDED` (2026-10-28 UTC): continuous SP 800-90B health tests in the driver, with a
  fall back to another source on failure if one is present and a loud refusal otherwise. Those tests
  are not built for this backend, so nothing may claim cryptographic-quality randomness on x86_64
  either.
- aarch64 takes the same path on a CPU with `FEAT_RNG` and no virtio-rng, since the function is
  arch-neutral. That arm has not run at the prompt. Every aarch64 swish-check boot has a
  virtio-rng, and the default TCG CPU (`cortex-a72`) has no `RNDRRS`.
- On a boot where the installer ran first, `entropy_service::ensure` has already handed it the
  readiness report and the kernel grants the service unread. The installer does not check the
  verdict either, so a condemned source would reach the progenitor; it answers `NO_ENTROPY` to
  every request, the password draw fails, and no login stack is built. Loud enough to notice, not
  a sentence.
- A `std` child holds `WRITE` on the region it is built in, because that region is its heap. A
  program that `SPLIT`s it pins it, and `job_undertaker` can then never reclaim it: one region
  lost from the pool until reboot. nife's `std` never splits (its allocator only `MAP`s), so this
  takes a program written to do it. Closing it needs a right that allows `MAP` and not `SPLIT`,
  which is a syscall-surface question.
- The directory half of the layout is built and unproven at the prompt. No `std` manifest
  declares a directory. §170 ruled the designation half on 2026-09-26, and the manifest that would
  declare it travels in an ELF note (§197 (a package is one archive file), M2), which is unbuilt.
- The network half is not wired. Slots 2 and 3 exist in the contract; the progenitor does not
  mint the socket frames' budget slot 3 needs, so a `std` manifest may not declare the network yet.
- `STD_REGION_PAGES` is the harness's number, not a measurement. 256 pages of heap is what
  every `std` program here has been proven under; `rg` over a real tree needs more, and a budget a
  person sizes at the prompt is an argument, which milestone 205 carries.

- An image is held three times while it is built: the shell's frames, the progenitor's copy
  and the child's pages. So an image of 4 MiB costs about 13.5 MiB of the progenitor's 48, set
  aside at boot whether or not one ever runs. Hashing as it copies into the child would drop the
  middle copy. Not done because it is more work in the tree's one loader, not because the copy is
  better; `IMAGE_POOL_PAGES`' BUGS in `crates/system_initializer` says the same.
- `caps` on a `std` image still names a 384-page region. The region an image is built in is that
  plus its own pages, and the preview has no length to add.
- `installed/std-echo-large` is padding past the last segment. It proves the transport, the
  pools and the page tables, not a 3 MiB segment.
- The CI half proves the mechanism with a program this project wrote. Only the second half answers
  risk 1, and it runs only on a machine that built `rg`.
- Designation is ruled, not built. Under §170, `docs` becomes a capability because the line
  grants it as a directory or because `rg`'s manifest says a resolved word may be one, never
  because the shell guessed. An unvouched `rg`'s own manifest grants nothing, so a named file
  reaches it read-only and a directory reaches it only as the line's grant.
- **`rg` with no word naming a directory prints the wrong guess.** It asks for its working
  directory before anything else. The PAL refuses that by design for a process that holds no
  directory (`patches/std-nife/overlay/std/src/sys/paths/nife.rs`), so nothing is searched, but
  the person reads `rg`'s own guess, "did your CWD get deleted?". `std_grep` says "no directory was
  granted to search". The refusal is right and the sentence is `rg`'s; the gate asserts it, so a
  change to either side shows.
- **The `rg` half of the gate never runs in CI**, so a regression on the path only `rg` takes (a
  linked foreign note, a stripped image near 3 MiB) is found by whoever next builds `rg`. The
  `std_grep` and `std-echo-large` lines cover most of that path in CI; the foreign note does not
  have a CI twin.

## Follow-on

- **Done.** x86_64's missing entropy service, promoted from
  `design/roadmap/proposals/the-x86-64-progenitor-serves-entropy-from-rdseed.md` into this
  milestone. See "What is built on x86_64".
- **Recorded.** A `std` child's `WRITE` on its own region, the unexercised directory half and the
  unwired network half are in `crates/system_initializer/src/lib.rs`, in `StdLayout`'s BUGS.
- **Done.** `rg` at the prompt, 2026-10-07; see "What is built: `rg` at the prompt".
- **Milestone 121.** Milestone 121 (`ripgrep` and enumeration) keeps three items. The loud refusal of a directory lacking `ENUMERATE` cannot be typed: a
  word's grant is `READ | ENUMERATE | DESCEND` and the line has no syntax that narrows it, so the
  refusal stays proven in the kernel harness (`ripgrep_tests.rs`), which is milestone 121's. So
  do installing `rg` with `jig` and pricing the walk.

## Index row

The shell could not launch a `std` program, and `ripgrep` ran only from the kernel test harness.
Now unmodified `rg` from crates.io runs by its path at the prompt and searches only the directories
its words granted, on all three architectures, proven by `script/swish-check` wherever `rg` was
built. It built on §219's option D, milestone 205 (how a foreign program is told what to do) for
arguments and milestone 206 (a program image has under 896 KiB) for image size. It unblocks milestone 121's remaining items and milestone 123's demonstration.
