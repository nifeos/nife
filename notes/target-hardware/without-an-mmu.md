# Isolation without an MMU, and what each version costs

*Name: provisional.*

An appendix to [`notes/target-hardware.md`](../target-hardware.md), whose requirement 0 is an MMU.
It records the alternatives to that requirement and why none is being taken.

Recorded 2026-09-04, because item 0 as written reads as though nobody had considered it, and a
reader who knows Tock or seL4's MPU variant will assume we had not. None of these is being taken,
and the reason in each case is a cost rather than an oversight.

An MPU instead of an MMU. Cortex-M and Cortex-R carry Memory Protection Units: region-based
permissions, no address translation, typically eight to sixteen regions. seL4 has an MPU variant
and Tock OS pairs an MPU with Rust's type system, so this is proven rather than theoretical. It is
also the only one of the three that preserves this project's thesis, because it isolates compiled
binaries rather than requiring the system to have compiled them.

What it costs comes in three parts. There are no virtual addresses, so every program is
position-independent or linked per deployment. There is a hard ceiling of eight-ish simultaneously
separated things, where the capability model assumes as many address spaces as there are processes.
And there is no demand paging, no copy-on-write, and no shared mappings. It is a port rather than a
configuration, and most of `kernel/src/arch/*/mmu.rs` and the `paging` crate would have no
counterpart.

Software fault isolation (Native Client, WebAssembly): bounds-check every memory access, enforced
by the compiler.

Language-based isolation (Microsoft Research's Singularity): software-isolated processes sharing
one address space, safe because every binary is verified type-safe before it runs.

Those last two are foreclosed by DECISIONS §14 (the project's direction) rather than by taste. Both
isolate only code the system itself compiled or verified. §14's demonstrator claim is a kernel that
runs real workloads which were not written for it, and design/fatal-risks/README.md's first risk is
exactly *"only software written for nife runs on nife"*, measured on 2026-08-31 with an unmodified
`ripgrep`. An SFI or language-based nife would make that risk permanently red by construction, which
is a stranger thing to ship than a kernel that needs an MMU.

Why this is a note and not a milestone: there is no machine. The form factors an MPU port would
open are the ones where no open MMU-class device exists today. A proposal nobody can act on is
the backlog graveyard milestone 247 exists to avoid. What would change that is a specific device,
and at that point this section is the starting analysis rather than a blank page.
