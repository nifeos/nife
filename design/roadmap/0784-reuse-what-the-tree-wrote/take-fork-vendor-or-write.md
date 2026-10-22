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

calef refined it the same day, for a patch that is our proofs and quality mechanisms. He asked
whether the test holds then, and said "yes" at 21:43 (UTC) to reading it this way:

- Condition 1 counts only changes to upstream's existing lines. Proofs, fuzz targets and tests we
  add are not divergence. Their cost is recorded on its own: harnesses an upstream release breaks,
  which each bump re-proves.
- Condition 3 reads: the crate isn't one we release, and proving it doesn't require restructuring
  upstream's code. Kani often needs bounded loops, no unbounded allocation and I/O it can stub.
  Restructuring code to get them is a real code change, which condition 1 then rightly counts.
  Proofs that sit beside unchanged code are not.
- A fork carrying our proofs is the fallback when upstream won't take them, because fork 8 of
  milestone 857 refuses the project or its maintainers decline. Contributing stays the first
  choice.

A fork is carried as `vendor/` carries `talc`: the published package, pinned by checksum, plus a
divergence patch that `script/vendor-verify` checks and `script/vendor-watch` compares with
upstream. It merges upstream by moving the pin to a new release and reapplying the patch, so a
dormant upstream leaves it nothing to merge. The cases read this way:

- `talc`, item (d), passes all three: 11 changed lines of 3,441, an active upstream, and a heap no
  proof covers. It is carried as a fork, with talc#57.
- `xhci`, item 10, fails the second: no commit since 2024-09. A fork has nothing to merge, so the
  definitions are vendored under rule 6 if they need a patch.
- `virtio-drivers`, item (c0), carries nothing of ours: the feature we need is upstream's #232. It
  passes all three, and is carried as a fork of 0.13.0 with #232 until a release includes it
  (calef, 2026-10-22 21:58 UTC).

## The exception a fork carries

This is a deliberate exception to the style gates, and a reader extending it should know that.
Upstream's lines in a fork are exempt from three style gates. They are function length under §283
(a function is held to 100 code lines by a lint ratchet), the comment style of §267 (a comment
states the constraint as it is now), and `missing_docs`. Today that holds by construction. The
function-length and comment-block ratchets and `helpers/house_style.py` skip `vendor/`, and a
vendored crate is outside the workspace, so the workspace's lints, clippy and `cargo fmt` never
reach it.

Proof, fuzz and mutation gates apply in full to what a fork carries of ours. Kani runs only the
harnesses `script/verify`'s table names, and fuzzing reaches vendored code only through our own
targets. `script/mutation` does not reach `vendor/` today, since `cargo mutants --workspace` sees
only workspace members. No fork carries our proofs yet. The first that does extends `script/verify`,
a fuzz target and `script/mutation` to its crate in the same change.

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
