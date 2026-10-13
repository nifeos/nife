---
status: REFUSED
raised: 2026-09-20
refused_by: 164, 448
---
# 461. An SSE-enabled x86_64 userspace target

Refused by milestone 164 (design/roadmap/0164-x86-64-fs-server-aes.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From "164. x86_64 userspace can't build `aes` (and therefore `fs_server`): no SSE, no scalar
fallback", under `## Follow-on`:

> Route 2, an SSE-enabled x86 userspace target, is not owed for this blocker.
> `kernel/src/arch/x86_64/` saves and restores no FPU/SSE state anywhere, so it would mean an
> `FXSAVE` area per thread and save/restore in the context-switch path, and none of that is needed
> to compile `aes`.
>
> -- design/roadmap/0164-x86-64-fs-server-aes.md

## Why it is here rather than only there

This is the refusal that prompted milestone 448, and it is worth being exact about why it is a good
refusal that went stale rather than a bad one. In September the question was whether `aes` could be
compiled for x86_64 userspace, and the answer turned out to be a cfg nobody had set. Route 2 would
have meant an `FXSAVE` area per thread and save/restore in the context-switch path, which
`kernel/src/arch/x86_64/` does none of today, and none of it was needed to compile `aes`. The
refusal was correct on its own question and it said so precisely: "not owed for this blocker".

What has happened since is calef's, reported on 2026-09-20. Milestone 442 (a crypto provider `rustls` can use on all three
bare-metal targets) needs five force-soft build flags; a crate that detects AVX2 at runtime dies in ring 3 with `vector
6 (invalid opcode)`; and the bitsliced cost the tree declined to measure is starting to have a
workload. None of that was compared against this refusal, because a refusal had no home that
anything reads, and calef reopened it by hand.

## Revisit

- **Condition.** A second blocker, which the refusal's own wording invites: it is "not owed for this
  blocker" rather than not owed. As of 2026-09-20 calef reports that condition met in part, by an
  x86_64 userspace that needs five force-soft flags and by a runtime AVX2 probe faulting in ring 3, so
  this block opens already rung rather than waiting.

## Index row

The refusal that prompted the REFUSED status: an SSE-enabled x86_64 userspace target was declined in
September on the narrow and correct ground that compiling `aes` did not need FP state, and the
ground eroded over the following fortnight with nothing comparing the two. It is recorded here with
its original reasoning intact and its condition stated.
