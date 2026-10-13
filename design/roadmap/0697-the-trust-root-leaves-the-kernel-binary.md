---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: the-trust-root-leaves-the-kernel-binary
milestone_dependencies: 609
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 697. The trust root leaves the kernel binary

*Title and slug are drafts.*
calef ruled this on 2026-09-27, beside the ruling that moved the system tests out of the kernel
crate (milestone 609 (the system tests leave the kernel crate)). Today `kernel/build.rs` compiles
the progenitor's digest and the base manifest's digests into `TRUST_ROOT`. A kernel binary is
therefore a function of every base program's bytes. No kernel build can happen before userspace is
built and the archive packed. Milestone 609 removed the crate coupling; this is the build coupling that
is left.

The ruled shape: the loader hands the kernel the progenitor's digest and the base manifest's digest,
the kernel checks only the progenitor, and the progenitor checks the base set. Two things need
deciding first. One is the handoff (a boot-protocol field on three architectures, which is a wire
format). The other is where the loader's own copy of the digests comes from.

The measurements behind the coupling are in pull request #1389's note on packages and divisions,
under "the trust root".

## Index row

A kernel binary is a function of every base program's bytes because the trust root is compiled in. Ruled by calef on 2026-09-27: the loader hands over the digests, the kernel checks the progenitor and the progenitor checks the base set.
