# Concept notes

*Name: ratified (§75 (directories carry provenance in their own README) covers this directory). `notes` predates every convention here and stays for the reason `elf` stays: it is the plain word for what the files are, and renaming it would spend a reader's recognition to buy nothing. `notes` is an ordinary English word and not an acronym, so DECISIONS §154 (the acronym test is whether the phrase is spoken) does not reach it.*

Running glossary for nife, written as concepts come up. If something in the code or the
conversation does not make sense, it belongs here.

This page is an index: one line per note, saying what the note is, and the detail lives in the note.
The lines are grouped by area, one page per area under [`notes/README/`](README/README.md), so this
page stays short while the list grows. Naming conventions are a rule rather than a note, and live in
[design/naming.md](../design/naming.md).

To add a note, add one line to the area page that fits, in the shape of the lines already there.
The area pages sit one directory down, so the link is `../your-note.md`. Keep the line short; commentary belongs in the note.
`script/lint` fails if a `notes/*.md` file has no line on this page or on an area page. A new area
gets its own page under `notes/README/` and a line in the list below.
A new *collection* of notes, several that belong together as in `notes/coes/`, gets a directory with its own
`README.md` instead. Add one line here pointing at that README (milestone 702 (the notes index pages become
area directories), ruled B). Appendices under
`notes/<stem>/` need no line, because the note they belong to links them.

## Start here

- [Acronyms](acronyms.md): every acronym expanded and linked; look here first.
- [How this is built](how-this-is-built.md): the agent-built method as a claim, with caveats.
- [The stranger test](stranger-test.md): the instrument for whether a newcomer succeeds unaided.
- [Adding a user program](adding-a-program.md): the steps to add a program, task-oriented.
- [Why this isn't a general-purpose OS](why-not-general-purpose.md).
- [Things this project has already gotten wrong](corrections.md).
- [The works this project is arguing with](bibliography.md): every outside work some page here actually reads.

## By area

- [The machine](README/machine.md): what the hardware is and how the kernel meets it; read these before any kernel code.
- [Rust without std](README/rust-without-std.md): `no_std`, the `alloc` collections and the heap.
- [Memory](README/memory.md): frames, page tables, address spaces and TLB shootdown.
- [The kernel](README/kernel.md): threads, capabilities, IPC, and how authority ends.
- [Drivers and devices](README/drivers-and-devices.md): virtio, PCIe, NVMe, DMA confinement and the display stack.
- [Programs, std and services](README/programs-std-and-services.md): what runs at EL0: the std port, the shell, components, and the services they call.
- [Storage](README/storage.md): the filesystem server, directory capabilities and on-disk formats.
- [Verification and security](README/verification-and-security.md): proofs, fuzzing, mutation testing, audits and what confinement claims.
- [Performance](README/performance.md): benchmarks, the instruction clock, bench runbooks and virtualization.
- [Ports and boards](README/ports-and-boards.md): the RISC-V and x86 ports, and the real boards.
- [Build, boot and install](README/build-boot-and-install.md): the toolchain, the boot formats, packages and installing.
- [How the tree is run](README/how-the-tree-is-run.md): gates, records and the merge queue that keep many lanes honest.
- [Delegation and the method](README/delegation-and-the-method.md): how agents do the work here, and what it costs.

`suspend-and-resume.md`: Name: provisional, minted 2026-10-18 (UTC) by lane/48-tcb-suspend-resume.

`xenon-bench-2026-10-18.md`: Name: provisional, minted 2026-10-18 (UTC) by lane/xenon-bench-2026-10-18.

`dependency-standards-census.md`: Name: provisional, minted 2026-10-19 (UTC) by maintainer/dependency-standards-census.

`ci-toolchain-download.md`: Name: provisional, minted 2026-10-19 (UTC) by maintainer/rustup-retry.

`turning-secure-boot-off.md`: Name: provisional, minted 2026-10-18 (UTC) by maintainer/500-secure-boot-ruled.

`lint-cost-2026-10-18.md`: Name: provisional, minted 2026-10-18 (UTC) by lane maintainer/lint-cost.

`aarch64-modern-board-survey.md`: Name: provisional, minted 2026-10-21 (UTC) by survey/aarch64-modern-board.

`arm-plan-under-hvf.md`: Name: provisional, minted 2026-10-21 (UTC) by lane/hvf-arm-reach.

`system-initializer.md`: Name: provisional, minted 2026-10-23 (UTC) by comments/929-crates-08.

`smp.md`: Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-smp-memory.
