---
status: AMENDED
raised: 2026-08-16
decided: 2026-08-25
ratified_by: calef
---

# 88. `needs-architect` as a required check, rather than as a script's restraint

calef, 2026-08-25, in conversation, ratifying the recommendation below as
written: *"Ratify it."* (raised 2026-08-16, when the merge queue reduced `helpers/merge-drain.sh` to
an admission policy and made the weakness of that policy the only thing left in it.)

**Number is provisional**: §87 was in flight in an unmerged pull request when this was written, so
the integrator may renumber at merge.

What. `needs-architect` means a pull request is outside standing merge authority: it touches the
system call surface, adds a dependency, or owes a `DECISIONS` section. Today the label is enforced by
*one script choosing not to arm it*. Nothing stops a merge by any other route, and nothing stops the
work merging if the label was never applied.

Why it comes up now. GitHub's merge queue took over the ordering half of `merge-drain.sh` on
2026-08-16, and what remains is the admission policy: which pull requests get enqueued at all. That
policy is now the script's entire reason to exist, which makes its enforcement worth looking at
rather than inheriting.

The weakness, in the tree's own words. notes/merge-queue.md's `BUGS` has said since the script
was written: *"A pull request that should be held but was never labelled will be merged by it. The
label is applied by hand at the moment the decision to hold is made, so a maintainer that forgets the
label has bypassed the gate rather than tripped it."* That is rung four of CLAUDE.md's ladder, which
is the rung the ladder exists to talk people out of. It is also the exact failure §69 and milestone
115 record from the naming side: a decision that lived in one person's attention.

The proposal. A required check that fails while the label is present and passes when it is
absent. Then:

- Holding is enforced by branch protection rather than by a loop declining to act, so no other merge
  route bypasses it. Rung two, and it fires without being remembered.
- Arming everything becomes unconditionally safe, and `merge-drain.sh` reduces to arming plus stall
  reporting, small enough to be a scheduled workflow rather than a session-lifetime loop. That kills
  notes/merge-queue.md's first `BUGS` entry, which is that neither watcher survives the session that
  starts it.
- Removing the label re-runs the check, so the release is a deliberate act with a timestamp on it,
  which the current arrangement does not produce.

**What it does not fix, stated plainly.** It enforces the label; it cannot apply the label. A pull
request that *should* be held and never gets labeled is exactly as unheld as it is today. The
mechanisms that could close that are separate and harder: a `CODEOWNERS` entry on
`kernel/src/system_call.rs` and on `design/decisions/`, or a check that reads the diff and demands the
label when the system call surface or the dependency graph moved. The second is the real answer and
it is a bigger piece of work; this proposal is the cheap half that makes the expensive half optional
rather than urgent.

The argument against. A required check that fails on purpose reads as a broken build to anyone
who has not met the convention, which is precisely the newcomer milestone 117 is written about. If
this is taken, the check's output must say what it is and how to clear it, in one sentence, rather
than reporting a bare failure. Two lines of text are what separate a mechanism from a trap.

A second cost worth naming. It adds a required check to every pull request in a repository whose
merge cycle milestone 119 already measures as the bottleneck. The check itself is a label read and
costs nothing, but it is one more thing that must report before anything can merge, and a check that
fails to report is indistinguishable from one that failed.

Blocked on nothing. `merge-drain.sh` works as reduced. This decides whether the holding rule
lives in a script's restraint or in the platform, which is a question about where authority is
recorded rather than about throughput.

**Not yet built.** Ratifying the decision does not itself add the required check or change this
repository's branch protection rules. That is real, live infrastructure change with blast radius on
every currently open pull request, several of which are mid-flight through the merge queue at the
moment this was ratified. So it needs its own careful, deliberate rollout rather than landing as a
side effect of a documentation change. Whoever builds it should write the check's own failure message
first, per "the argument against" above: one sentence saying what `needs-architect` means and how to
clear it, not a bare failure a newcomer has to go look up.

## Amendment, 2026-10-11: a bookkeeping edit to a decision does not need an architect

The label's reach was built after this was ratified. `.github/workflows/architect-label.yml` reads
each pull request's diff, and its rule 5 fired on any edit to a decision file's body. That included
#1953, which changed only two link targets in §147 (a timer a userspace service cannot hold) after
the note they pointed at was split. calef approved it by hand, then ruled on 2026-10-11 (UTC):
*"I need to review and approve decisions, but I don't think I need to approve minor bookkeeping
edits like this one. Can we update our standards accordingly?"*

So the standard is now this. An edit to a decision needs an architect unless it is bookkeeping: the
decision reads the same once link targets, repository paths in backticks, line wrapping and
whitespace are set aside. Retargeting a link, moving a cited path to where the file now lives, and
reflowing a paragraph are bookkeeping. Changing a word, a number, a quotation, a heading, a list
item or a table cell is not, and neither is adding or removing a link. A renamed identifier is a
word, so a rename sweep still asks. `CLAUDE.md` still asks on any edit.

`helpers/architect-label-rules.py` holds the exact terms in `rule_decisions`, with fixtures and a
measurement over 75 merges: 23 of 387 file firings go quiet, and none of them changed a word.
