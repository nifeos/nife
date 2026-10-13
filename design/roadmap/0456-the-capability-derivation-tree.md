---
status: REFUSED
raised: 2026-09-20
refused_by: 13, 14, 26, 448
---
# 456. The capability derivation tree, and subtree-granular revocation

Refused by milestone 13 (design/roadmap/0013-capability-revocation.md), milestone 14
(design/roadmap/0014-kernel-objects-from-untyped.md), and milestone 26
(design/roadmap/0026-object-revocation.md), each of which records it in its Follow-on. Milestone 448
(design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a number, a status
and a condition that would change it.

## The refusal, in its own words

From '13. Capability revocation + untyped reclamation', under `## Follow-on`:

> The full seL4-style capability-derivation tree, and with it subtree granularity (revoke Bob's
> copy while keeping Alice's). `design/decisions/0013-frame-revocation.md` argues it as a considered
> terminal design rather than a way-station: revoke-all-derivatives is the memory-safety-critical
> half and is exactly what reclamation wants, nothing on the roadmap needs subtree revoke, and if
> one ever does, the unmap side and the revoke-before-reclaim discipline are reused unchanged with
> only the holders index rebuilt as a tree.
>
> -- design/roadmap/0013-capability-revocation.md

From '14. Kernel objects from untyped: remove the kernel heap', under `## Follow-on`:

> The capability derivation tree. Revocation stays frame-scoped, which is the
> memory-safety-critical half and is what reclamation actually wants; subtree granularity has no
> driver on the roadmap, and the argument that this is a terminal design rather than a way-station
> is in design/decisions/0013-frame-revocation.md.
>
> -- design/roadmap/0014-kernel-objects-from-untyped.md

From '26. Object revocation: tear a process back down', under `## Follow-on`:

> No capability derivation tree. Region ownership plus generational staleness answers the same
> question for every case that exists, and what a CDT would additionally buy is the general
> non-LIFO return-of-pages-to-parent. `notes/object-revocation.md` records the refusal in the
> words "we still have no reason to build one", and the LIFO case is built.
>
> -- design/roadmap/0026-object-revocation.md

## Why it is here rather than only there

Three separate blocks refused the same thing in the same words, which is itself the argument for
giving it a number: a reader meeting any one of them cannot tell it is the tree's standing position
rather than one lane's scoping call. seL4 tracks every capability's derivation so that a revoke can
take Bob's copy while leaving Alice's. This tree revokes all derivatives of a frame instead, which
is the memory-safety-critical half and is exactly what reclamation wants.

## Revisit

- **Condition.** A caller that needs subtree granularity, which is the test all three refusals apply
  and none has met. The refusals are also explicit that the mechanism is not the obstacle: the unmap
  side and the revoke-before-reclaim discipline are reused unchanged, with only the holders index
  rebuilt as a tree.

## Index row

seL4's derivation tree is the best-known thing this kernel deliberately does not have, refused in
three separate blocks with one argument, and it had no single home a reader could find. The refusal
is conditional on a caller nobody has, and the blocks are explicit that the existing machinery is
reused rather than replaced if one appears.
