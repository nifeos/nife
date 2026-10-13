---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: processes-share-a-verified-images-code-pages
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 692. Processes share a verified image's code pages

*Title and slug are drafts.*
Filed by the maintainer, from a finding raised by the lane building milestone 595 (the shell runs
a std program): while a child is staged, one program's image is held three times at once. The
copies are shell staging, the progenitor's own copy, and the child's mapped pages. Name
provisional.

## What is true today

The progenitor copies a program's image into each child it spawns. Two instances of the same
program running at once share no code pages; each holds its own copy of the same bytes. Linux, by
contrast, maps a binary's code once and shares it read-only across every process running it. That
sharing gap is what a memory comparison against Linux would show, more than static linking would.
Static linking is a separate, smaller cost: it duplicates `std` and runtime code into every
program, an estimated 100 to 150 KiB per `std` program.

## Proposal

Map a verified image's code pages read-only into every instance that runs it, instead of copying
them per child. The image is already verified once, by digest, before anything runs it. Milestone 104 (the
measurement continues past init) built that check, so nothing new needs proving to share the pages: the same bytes are already trusted, whichever process maps them.

## Not measured yet

This is not costed. The first step is measuring resident memory for N instances of the same
program under today's per-child copy, so the size of the gap is a number rather than an inference
from Linux's behavior. That measurement should come before any implementation work, per this
tree's practice of measuring before deciding a remediation.

## Index row

Two instances of one program share no code pages, and a staged child's image is held three times at once. Proposed: map a verified image's code once and share it read-only across processes, the sharing gap a memory comparison with Linux would show.
