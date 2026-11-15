---
status: BUILT
raised: 2026-07-23
built: 2026-07-23
---
# 18. Verify the capability core, then spread inward

In brief. Machine-checked proofs of `capability`, then IPC, then MMU isolation

Why it matters. the verification itself. Built: `capability`, IPC (rendezvous + one-shot Reply), and the MMU isolation invariants are all proved

Green-lit and started; see DECISIONS §14 and notes/verification.md. This is the verification
thesis as an actual work item rather than an aspiration.

Deliverable. Machine-checked proofs (Kani) of the security-critical logic, spreading inward from
the capability core. `crates/capability` is proved already: five harnesses covering "`derive` never widens
rights," "userspace cannot forge a right," and the subset order's reflexivity and transitivity, each
for *every* input rather than sampled cases (`script/verify`). Next, in order, IPC (the rendezvous
and the one-shot reply) and the MMU isolation invariants.

Why here. It is the differentiator (§14), and it is cheap to start: the §7 pure-logic crates
already compile for the host, and proofs live behind `#[cfg(kani)]` so they never touch an ordinary
build. It also interlocks with 14: proving properties *of the kernel* (not just its logic crates) at
scale wants a kernel that does not allocate.

Prior art. seL4 (Isabelle/HOL refinement, verified C) is the mountain; we took the tractable path
(bounded model checking, Rust). Verus is the deeper Rust option to revisit if a property needs
unbounded proof.

Status (2026-07-29), with milestone 35 (prove the DMA-confinement boundary) done. The proved set is
now broad: 13 crates, ~60 harnesses.
They cover `capability`, `ipc` (rendezvous, one-shot reply, the collected-sender path), and the MMU
codec on *both* formats (`paging`: VMSAv8-64 and Sv39, level-walk and leaf permission separation).
Then generational names (`slots`: a removed name never resolves again), frame allocation, region
split/destroy arithmetic, ELF parsing, the device-tree reader, ASID allocation, PCI decode. Then the
DMA-confinement validator (`dma_validator`) and the IOMMU domain's page set (`page_tables::domain`), both
milestone 35. An audit against the TCB (prompted by asking "what should we prove that we haven't")
found the boundaries proved with **one glaring exception: the DMA-confinement validator was
attacker-tested, never proved.** Milestone 35 closed it. The validator is extracted and proved for
every input, and the `Untyped::SPLIT` mint site is proved to hand a child *exactly* its parent's
rights. The IOMMU domain's *maps-exactly* property is proved too: its page set, in both directions,
format-independently, so one proof covers both IOMMUs. (The build-and-translate round trip stays on
the declined BMC wall and on tests.) What milestone 35 explicitly does **not** prove, and says so in
three places rather than leaving it to be inferred. It is addresses that reach a device inside a
command payload instead of a descriptor. The validator structurally cannot see them and only an
IOMMU stops them, so on a board without one they are unconfined. See DECISIONS §30 (the DMA boundary
is proved for descriptors) and notes/verification.md.

## Follow-on

- **Milestone 35.** The TCB audit's one glaring exception, the DMA-confinement validator that was
  attacker-tested and never proved. 35 extracted it, proved it for every input, proved the
  `MemoryRegion::SPLIT` mint site hands a child exactly its parent's rights, and proved the IOMMU
  domain's maps-exactly property in both directions.
- **Milestone 193.** The frontier this block names past the logic crates: proving properties *of the
  kernel* rather than of the pure-logic crates it depends on. 193 is the work that puts `kernel/src`
  within the prover's reach at all.
- **Recorded.** Addresses that reach a device inside a command payload rather than a descriptor
  are structurally invisible to the validator, so on a board with no IOMMU they are unconfined. The
  confinement that does exist there is tested rather than proved. `notes/verification.md` carries the
  measurement and the table of what each boundary rests on.
- **Recorded.** The IOMMU's build-and-translate round trip (a symbolic IOVA walking a built
  four-level table) stays on the declined bounded-model-checking wall and is covered by tests
  instead. `notes/verification.md` says why the smaller page-set property carries the weight.
- **Refused.** Verus, and unbounded proof generally. Bounded model checking on Rust was taken
  deliberately as the tractable path against seL4's Isabelle/HOL refinement, and the block keeps
  Verus as something to revisit only if a specific property needs a loop invariant rather than as
  work anybody owes.
  This refusal is milestone 466 (design/roadmap/0466-verus-and-unbounded-proof.md), which carries it
  with the condition that would change it.

## Index row

the verification itself
