---
status: BUILT
raised: 2026-09-03
built: 2026-10-15
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 352. DECISIONS 113's rename reaches the prose

Built 2026-10-15. The prose a reader meets now carries the vocabulary §113 (the plain object names)
decided, and every replacement was checked against the code it names, so a grep from the page lands
on the thing. The renamed surface, by kind: methods (`ThreadControlBlock::CONFIGURE`,
`Rendezvous::REAP`, `PageFrame::REVOKE`, `MemoryRegion::DESTROY` and `::SPLIT`); module paths
(`abi::rendezvous`, `abi::thread_control_block`, `abi::address_space`, `memory_region::destroy`,
`rendezvous_of`); objtype constants (`RENDEZVOUS`, `ADDRESS_SPACE`, `THREAD_CONTROL_BLOCK`); types
(`Table<ThreadControlBlockPointer, 256>`, `page_frames::PageFrame`, `Reply(ThreadId)`). The note
named for a retired word is `notes/thread-control-block.md`, indexed as "The thread control block",
and every pointer to the old path follows it, including three kernel and crate doc comments.

What moves and what stays follows design/naming.md's rename rule: status decides, not directory.
Live notes and the live roadmap blocks (0048, 0103, 0190) take the new names. Closed records keep
the ones they were written with: §113, the account directory of milestone 158 (the eleven kernel
object and identifier renames), and this block quote the rename. BUILT and REFUSED blocks are
accounts. Dated snapshots, incident records, transcripts and
machine-dump comparisons keep their own output. A citation gloss keeps the decision's own title
(§102 (a Frame names a run of pages)). A refused option keeps the spelling the deciding text
records (`Untyped::REAP`, `Tcb::NAME`, `Frame::UNMAP`), so the string still finds its refusal.
Generic English keeps its word: frame pointers and frame budget, the fault endpoint and the
interrupt endpoint, lowercase untyped as the material the ABI's own doc comments still name, and
informal TCB, which notes/thread-control-block.md keeps on purpose.

Census at the build: the seven spellings that cannot collide with ordinary prose now appear under
`notes/` and `design/roadmap/` only in §113, milestone 158's accounts, this block, BUILT or REFUSED
blocks, and notes that are themselves accounts. Those are the 2026-08-17 kernel snapshot, the
session handoff kept as a record, and the model-attribution table. The four ordinary words
(`Untyped`, `Endpoint`,
`Frame`, `Tid`) took the reader this block asked for. Their identifier uses moved, their English
uses stayed. 84 files changed across the lane.

Found while reading, fixed where a reader meets it: `notes/teardown.md`'s AddressSpace description
predates region-owned teardown and now says so in a BUGS entry. 0254's scope bullet claimed the
block and note of milestone 133 (ending a permanently blocked thread) still said `Tcb`, which had
stopped being true, and the bullet is gone (this lane swept them). `script/lint` refuses a
of dying several checks in. One retired name sits outside a lane's reach and is reported, not
edited: AGENTS.md's system-call-surface paragraph still says `Untyped::SPLIT`.

## Index row

§113 decided eleven plain names on 2026-08-23 and milestone 158 carried them through the code. The
prose kept speaking the retired vocabulary, so a reader met `ThreadControlBlock` in the identifiers
and `Tcb` in the explanations with nothing saying these were the same thing. This lane brought the
live prose to the decided vocabulary, checked each name against the code it names, and left closed
records, quotations, refusals and ordinary English as they were, per the rename rule in
design/naming.md. Nothing yet gates prose for retired identifiers; the residue that remains is
kept by the status rule, not by a mechanism.

## Follow-on

- **Done.** The `tomllib` guard in `script/lint`, carried by this lane.
- **Recorded.** `notes/teardown.md`: its BUGS entry says the AddressSpace description there
  predates region-owned teardown.
- **Recorded.** `AGENTS.md`'s system-call-surface paragraph still says `Untyped::SPLIT` and `DESTROY`;
  a developer lane does not edit that file, so it goes to a maintainer follow-up, listed on pull
  request #2213.
