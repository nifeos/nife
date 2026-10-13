---
status: BUILT
raised: 2026-07-23
built: 2026-07-23
---
# 14. Kernel objects from untyped: remove the kernel heap

In brief. Retype TCBs, endpoints, page tables; delete the kernel heap

Why it matters. critical path: a verifiable kernel cannot allocate. **Built:** the kernel has no
allocator; see design/kernel-objects-from-untyped.md

Deliverable. Retype TCBs, endpoints, and page tables out of untyped memory, the way milestone 11
already does for user pages, and delete the kernel heap and slab.

Why. This finishes §10's deferred axis. Milestone 11 stopped the kernel allocating for *user*
memory; the kernel's own objects still come from its heap. It is also the real prerequisite for the
"small enough to verify" endgame: seL4's proof leans on a kernel that never allocates. Biggest item
here, and the seL4 long tail by reputation.

On the critical path (§14 (the project's direction: a verified-Rust capability microkernel)).
Verifiability is the goal, so this is not an optional purity win. A verifiable kernel cannot
allocate dynamically, so removing the heap is a prerequisite for verifying the kernel at scale
rather than only its pure-logic crates. It still also buys the smaller payoff on its own terms: the
kernel-heap-exhaustion class disappears entirely.

## Follow-on

- **Refused.** Sub-page object packing, meaning a decision about whether several TCBs share a page.
  Object sizes were not measured when the milestone ran, and a packing commitment made ahead of the
  measurement is a guess that later code would be built on. This refusal is milestone 457
  (design/roadmap/0457-sub-page-object-packing.md), which carries it with the condition that would
  change it.
- **Refused.** The capability derivation tree. Revocation stays frame-scoped, which is the
  memory-safety-critical half and is what reclamation actually wants; subtree granularity has no
  driver on the roadmap, and the argument that this is a terminal design rather than a way-station
  is in design/decisions/0013-frame-revocation.md. This refusal is milestone 456
  (design/roadmap/0456-the-capability-derivation-tree.md), which carries it with the condition that
  would change it.
- **Refused.** Any change to the user-facing map and retype semantics for frames. Milestone 11's
  paths keep working, and reworking them would have cost every existing user program for nothing
  this milestone needed.
- **Recorded.** `design/kernel-objects-from-untyped.md` holds the four-step path to capability-only
  naming and why the capability payloads fall last. They still carry a generational name rather than
  a TCB pointer, which is safe because a dead name can never resolve again; the pointer form waits
  on the derivation tree refused above.

## Index row

removes the kernel heap: the prerequisite for "small enough to verify"
