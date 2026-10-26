---
status: BUILT
raised: 2026-08-04
built: 2026-08-14
---
# 116. The fences with no partner

Built 2026-08-14 (PR #131). Minted 2026-08-04 by the integrator, from milestone 43's audit proposal C,
after the same mistake was found twice the same day by two methods that share nothing.

A release fence orders what came before it against a matching acquire on the reader. With no
acquire on the other side, it orders nothing that matters and the code reads as though it does. That
is worse than an absent fence, because the fence is the comment: a reader who sees
`fence(Ordering::Release)` stops asking the question.

Found twice on 2026-08-04, independently:

- Milestone 80's loom harness found the clock page's seqlock writer claiming the sequence with
  nothing ordering the claim ahead of the data stores. A reader could revalidate successfully and
  return a state from one publish beside an offset from another: a silently wrong wall clock. The
  instructive part is that `AcqRel` on the claim does not fix it and neither does `SeqCst`; it needs
  a `fence(Release)` between the claim and the stores, which is Linux's `smp_wmb()`.
- The audit of milestone 43 (a second security audit) found three release-side fences in the
  compositor's subsystem **whose acquire
  side did not exist**. It separately noted that the kernel's own stand-in for the same input ring
  does fence. That is what shows the gap is an oversight rather than a design.

Neither method could have found the other's, and **no gate in the tree can see either**. Clippy has
no lint for it, Kani does not model concurrency, Miri's stacked borrows are about aliasing, and the
suite passes because both ISAs' hardware happens to be forgiving at the sizes involved.

The work. Inventory every `fence`, every `Ordering::Release` store and every `Ordering::Acquire`
load outside test code, and pair them. For each unpaired site decide, on the record, whether it is a
bug (fix it, with the harness that proves it). Or it is sound for a stated reason (a single writer
with interrupts masked is a real reason; say so where the fence is), or unreachable. Then decide whether
the pairing can be checked mechanically at all.

Be honest about that last part, because it is the interesting question and the answer may be no.
A lint that pairs fences across functions is a dataflow problem, not a pattern match. Milestone 112
(the SAFETY comments that bind nobody)
already established this tree's posture: a narrow check that is true beats a broad one that is
aspirational. If the answer is "this is a review discipline plus a loom harness per protocol", say
that, record it as a limitation, and make the inventory itself the deliverable. Milestone 80's
`interleaving-check` is the tool that *can* decide a specific protocol, so the useful output may be
a list of protocols that want a harness rather than a gate.

**Scope note.** Not a rewrite. Nothing changes its ordering because of this milestone except where a
site is shown to be wrong, and each such change carries the argument for why the new ordering is
right.

## Follow-on

- **Milestone 43.** The three unpaired release fences in the compositor that half raised this
  milestone. The inventory adjudicated them; the ordering changes themselves are 43's audit's.
- **Milestone 80.** The output this block guessed at and got right: the useful answer is a list of
  protocols that want a loom harness rather than a lint, and `script/interleaving-check` is the tool
  that can decide one.
- **Recorded.** `notes/memory-ordering.md`: the `PAIR:` gate checks that a comment exists, not that
  it is true. A marker naming a function that does not exist, or the wrong one, passes. It is
  bookkeeping with a forcing function attached.
- **Recorded.** `notes/memory-ordering.md`: the 243 relaxed sites are not adjudicated. Only the
  compare-exchanges were checked, so a relaxed store used as a publication flag with data behind it
  would be this milestone's own bug and is outside the inventory.
- **Milestone 953.** Roughly half the soundness arguments rest on the kernel lock's orderings.
  The lock is the in-tree `crates/spin_lock`, whose loom models fail when its release store or its
  acquire weakens.
- **Recorded.** `notes/memory-ordering.md`: nothing in the inventory is a proof about aarch64 or
  riscv64. Every soundness argument in it is a C11-model argument about happens-before.

## Index row

A release fence with no matching acquire orders nothing and reads as though it does, which is
worse than no fence because the fence is the comment. Found twice on one day by two methods
sharing nothing (loom on the clock seqlock, the audit on the compositor), and no gate in the tree
can see either
