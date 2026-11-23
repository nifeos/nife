# Milestone 117 appendix: the two halves and the predicted worklist

*An appendix to [milestone 117 (the stranger test)](../0117-newcomer-onboarding.md), moved out of
that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This is the plan as minted on 2026-08-05, with the notes later lanes added to it.*

## The two halves, and only one is mechanical

The build. From a clean clone on a machine with nothing installed: does `script/setup` then
`script/test` reach green, following only what the repository says? This is checkable and probably
partly broken, because nobody has run it from cold in weeks and every contributor's machine is
already warm. Record what it actually took, including anything the reader had to know that no file
said.

The mental model. Harder, and it needs a rubric written before the run so the result cannot be
graded generously afterwards. A candidate set, each a question the tree *claims* to answer:

- What is a capability here, and what does it mean that designation is authorization?
- Why is there no ambient network, and what would a program have to hold to reach one?
- Where does architecture-specific code live, and what breaks if it lives elsewhere?
- What does `BUILT` mean on a roadmap row, and what does `RECORDED` mean?
- Why is there a `crates/` and a `user/src/`, and what decides which a thing goes in?
- What is the frame in a socket contract, and why does a listener not have one?
- How would you add a program, and what would you have to declare about it?

Grade against what the tree actually says, not against what a maintainer knows. A question the tree
answers only in a commit message is a question the tree does not answer. The rubric that was
actually written on 2026-08-14 is in [notes/stranger-test.md](../../../notes/stranger-test.md).

## What the work will be, and it is not writing prose

The output is a worklist of defects, and the shapes are predictable enough to name now:

- Entry point. `README.md` has eleven sections and no stated reading order. A stranger does not
  know whether to start at "Try it", "Quick start", or "The notes are the point".
  *(Landed 2026-08-18 as a `## Start here` section, provisional. Run 3 is the evidence it was
  needed and the evidence it is not enough. Its stranger built a good order by instinct, and reached
  `AGENTS.md` twelfth from a pointer at line 226. It reached `crates/moho_abi/src/lib.rs`, four system call
  numbers and the whole design on one screen, far too late to help it.)*
- No `CONTRIBUTING.md`. GitHub links it from the pull request UI and it does not exist, so the
  answer to "how do I propose a change here" is nowhere. `CLAUDE.md` is the closest thing and it is
  addressed to an agent, not to a person. *(Landed 2026-08-18. It links to `AGENTS.md` rather than
  restating it, because the two have different readers and milestone 118 (CLAUDE.md has
  a budget) is shrinking `AGENTS.md`; a second copy would be work in the other
  direction. Untested at landing: no stranger had seen it, since run 3's clone predates it.)*
- 119 notes with no path through them. `notes/README.md` is an index, which answers "what exists"
  and not "what do I read first".
- The conventions that are load-bearing and unstated for a human: that a lane is a worktree plus a
  branch, that `origin/*` moves under a worktree, that names are ratified, that a `BUGS` section is
  a promise rather than an apology.

Fix what the run finds, then run it again with a second stranger. One pass measures; two passes
show whether the fixes worked, which is the difference between an audit and a milestone.
