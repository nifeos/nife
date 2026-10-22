# Take, fork, vendor or write

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md): the rules 784
applies to a crate we cannot take unmodified.

Codebase rule 6, from §46 (thin primitives or whole subsystems), sets the order: take or adapt
existing code first, write only with a recorded reason, and vendor only what needs a patch. Two
criteria weigh a crate we cannot take unmodified. Both are calef's, from 2026-10-22 (UTC), and the
live candidates are measured against them in [an appendix](upstreams-against-the-fork-test.md).

## The fork test

It sits beside rule 6 and does not replace it. calef, 2026-10-22 at about 21:06 (UTC): *"a fork wins
over our own crate when all three of these hold: 1. Our changes are small and likely to stay small,
measured as changed lines as a share of the crate. 2. Upstream is active (commits and releases in
the last year). 3. The crate isn't one we release or prove. Those must meet our standards, so the
fork would diverge."*

A fork is carried as `vendor/` carries `talc`: the published package, pinned by checksum, plus a
divergence patch that `script/vendor-verify` checks and `script/vendor-watch` compares with
upstream. It merges upstream by moving the pin to a new release and reapplying the patch, so a
dormant upstream leaves it nothing to merge. The cases read this way:

- `talc`, item (d), passes all three: 11 changed lines of 3,441, an active upstream, and a heap no
  proof covers. It is carried as a fork, with talc#57.
- `xhci`, item 10, fails the second: no commit since 2024-09. A fork has nothing to merge, so the
  definitions are vendored under rule 6 if they need a patch.
- `virtio-drivers`, item (c0), carries nothing of ours: the feature we need is upstream's #232. It
  passes all three, so the test favors a fork of 0.13.0 carrying #232 over keeping our drivers. The
  ruling stands as calef gave it: no fork, wait for a release.

## The AI-submissions criterion

Whether nife contributes to a project is ruled by fork 8 of milestone 857 (crates released on their
own). Offers go only to projects whose policy admits disclosed AI work. Where AI is banned, nife
sends no code or proofs. calef, 2026-10-22 (UTC), carries the same question into choosing a
dependency: *"I think AI submissions is likely to be a key criteria for evaluating our dependencies
unless we can take them unmodified."* This tree's patches are written by agents, so for a
dependency an upstream's written policy on agent-written contributions weighs this way:

- Take unmodified. It is a risk line, weighed by how likely we are to need a fix: heavy for a
  hostile-input parser with open defects, light for a stable leaf crate.
- Fork or write. An upstream that refuses our patches still lets us merge its changes, one way. A
  ban with small changes points to a fork. A ban with large changes points to building our own,
  since our side of the fork would be the larger part and could never shrink by going upstream.
- The record. Only a written policy counts, with a link, a short quote and the UTC date it was
  read. A missing policy is recorded as missing, never as acceptance. Asking a maintainer is
  calef's act, never a lane's.
