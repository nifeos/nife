---
status: SUPERSEDED
raised: 2026-09-03
superseded_by: 139
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 358. `revoke.rs`'s log-page walk restates one safety argument in six places

Overtaken by commit `f549607d1` (2026-10-07, milestone 139's round 8): `revoke::chain` now holds the
walk, `kernel/src/revoke.rs` calls `log_page` inside one `unsafe` block, and the lock and membership
are held by types. The text below is the block as filed.

Filed as a proposal on 2026-09-03 by the milestone 252 sweep, from
milestone 139's block; promoted by milestone 433 on 2026-09-19. Counted against the tree that day
and the number is still six: `kernel/src/revoke.rs` calls `log_page` inside `unsafe` blocks at lines
312, 333, 360, 418, 493 and 670, each with its own `SAFETY:` comment. Four of the six say the
same two things in four spellings ("pages in the chain are the log's own; SPACES is held", "chain
pages under the held SPACES lock"). The helper itself is at line 258. Nothing has collapsed them
since the file was written.

It is one file, the helper already exists, and the ratchet in `script/lint` is the
measurement that says whether the change worked.

**In brief.** `kernel/src/revoke.rs` walks its per-space log-page chain inside six separate
`unsafe` blocks, and each one restates in a comment what the helper's own safety section already
says. Milestone 139 (drive the unsafe count down, and cinch the ratchet behind it)'s round 8 named
this as its next target and asked for a milestone. Nobody minted
one, and the round-8 report was the only place the request lived. Collapse the six into whatever
shape carries the obligation once, the way rounds 3 through 7 did for the framebuffer, the register
maps and the mapped windows.

## Why this matters

Milestone 139's whole argument is that an `unsafe` block is a proof obligation, and that six copies
of one obligation is six chances to get it wrong while the ratchet counts it as six facts. The six
sites here are the largest remaining cluster in the kernel outside `arch/` and `sched.rs`, and
`sched.rs` is blocked on a typestate decision that is an architect's. This one is not blocked on
anything.

## What it needs

- Read the six sites and the helper's `# Safety` section, and decide whether the obligation is one
  argument or genuinely six.
- If one: a wrapper or an iterator that discharges it once, with the comment moved to the wrapper.
- If six: say so in `notes/unsafe-obligations.md` and record it as irreducible, which is a result
  and closes the item honestly.
- Re-measure the ceiling from the merged tree, per that note's own rule, rather than from this
  branch.

## BUGS

- **A wrapper can hide an obligation rather than discharge it.** That is the failure mode round 6
  flagged for the mapped windows: if the wrapper's own safety argument is weaker than the six it
  replaced, the count improves and the kernel does not. The measure is the argument, not the number.
- Nothing here has a customer. It is verification hygiene, so under the ranking function it
  loses to anything on a customer path the day one exists.

## Index row

`kernel/src/revoke.rs` walks its per-space log-page chain inside six separate `unsafe` blocks. Each
restates in a comment what `log_page`'s own safety section already says. Milestone 139's whole
argument is that an `unsafe` block is a proof obligation and that six copies of one obligation is
six chances to get it wrong while the ratchet counts it as six facts. This is the largest remaining
cluster in the kernel outside `arch/` and `sched.rs`, and `sched.rs` is blocked on a typestate
decision that is an architect's while this is blocked on nothing. The honest outcome may be that the
obligation is genuinely six arguments rather than one, in which case saying so in
notes/unsafe-obligations.md closes the item. A wrapper whose own safety argument is weaker than the
six it replaced improves the count and not the kernel.
