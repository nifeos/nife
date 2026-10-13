---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: the-kernel-trusts-two-digests-from-the-loader
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 695. The kernel trusts two digests handed to it by the loader

*Title and slug are drafts.*
Filed by the maintainer, recording calef's ruling on pull request #1389, 2026-09-27 (UTC): "T4 with
T2". That pull request is design work for a still-open fork on how packages build the OS and the
tree divides by what releases together. The section it will land in is not yet merged, so this
proposal cites the pull request rather than a section number. Every name here is provisional.

## What changes

Today `kernel/build.rs` compiles `TRUST_ROOT` into the kernel: the progenitor's digest and the
table of base program digests, `PROGRAM_MEASUREMENTS`. Any base service change relinks the kernel.
That is why milestone 104 (the measurement continues past init) makes the kernel release as one
unit with the base image.

Under T4 with T2, the boot loader hands the kernel two digests at handoff, on all three
architectures. One is the progenitor's own digest. The other is a digest over the base manifest, the
package set an image is built from. The kernel checks only the progenitor's digest before running
it, and stops parsing a program table at all. The progenitor, once running, checks the base set
against the manifest digest the loader gave it. That is the same way it already refuses an unlisted
program (milestone 104).

## What this buys, and what it costs

The kernel binary stops depending on the base set. It still depends on the progenitor, so the two
release together, but a base service can change without relinking the kernel. §151 (the goal of the
repository split is independent release) is not fully reachable this way. T2's own costs stay.
Trust moves to the loader, which only milestone 500 (a stick that boots with Secure Boot on) or the
slot checksum protects. There are also three handoff paths to change (device tree `/chosen` on
aarch64 and riscv64, PVH on x86_64). Milestone 525 (a bad upgrade cannot brick the machine) records
that the device-tree path has one initrd slot.

Signatures stay exactly where §220 (signed builds) put them, at install and update time, out of the
boot path. This does not move a signature check to boot. It only changes what the kernel is handed
and what it checks before running the progenitor.

Comparable system: Fuchsia checks the `system_image` Merkle root, carried in signed boot arguments,
in userspace rather than in the kernel.

## Dependency and what is calef's

This depends on the base manifest: an image assembled from base packages by digest, which pull
request #1389 proposes and which is not yet built. The loader-to-kernel handoff is a layout two
programs agree on, so its exact bytes come to calef, as they always have.

One consequence needs a decision of its own, not folded into this one. Once the kernel can update
separately from userspace, the syscall surface becomes a versioned ABI that already-installed
programs rely on. Pull request #1389 does not settle that, and this proposal does not either.

## Index row

`kernel/build.rs` compiles the progenitor and base program digests into the kernel. Ruled by calef on pull request #1389 (T4 with T2): the loader hands the kernel two digests at handoff on all three architectures.
