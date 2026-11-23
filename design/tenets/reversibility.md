# Move fast on what can be undone; be methodical on what cannot

*Appendix to [`AGENTS.md`](../../AGENTS.md), which carries the rule, the irreversible list and the
test. This file carries why each category is on that list, the two failures of record, and what
agents changed about the calculus. Moved here 2026-09-23 (UTC) on calef's authorization.*

calef, 2026-08-05. The ladder in `AGENTS.md` says how hard to make a thing hold. This says how much
care to spend deciding it, and the two are not the same question: a cheap decision still deserves a
mechanism, and an expensive one is not made safe by adding a gate afterwards.

Most decisions here are reversible and should be made quickly, by whoever is holding the
problem. Code, notes, roadmap wording, which milestone a lane takes, how a script is structured.
Getting these wrong costs an hour. Deliberating them costs more than that, and deliberating them
*with an architect* costs their attention, which is the scarcest thing in this project.
`helpers/merge-drain.sh` was rewritten three times in one evening, each version wrong in a way the
next one fixed, and that was cheaper than designing it correctly up front would have been.

Speed is worth something in itself, and not only because a reversible decision can be undone
later. While a decision waits, the work behind it waits too: a lane idles, a fix sits unmerged, and
the next question cannot be asked until this one is answered. So waiting is never free, and it is
not the safe default; holding a reversible decision needs a reason as much as making it does. The
same holds for uncertainty. Acting before every question is settled is expected here, once the risk
has been weighed: say what could go wrong, how likely it is and what it would cost, and check that
it can be undone. On a reversible decision a reasonable bet made now beats extensive study, because
the bet teaches what the study would only have guessed at, and a wrong bet costs one more revision.
The weighing is what separates a calculated risk from a careless one, and it is also what sends a
decision on the list below to the methodical side.

A few decisions are expensive, and the expense is almost never the code. It is the consequences
that cannot be recalled:

- Anything two programs agree on. A wire format, an opcode number, a packed word. The code is a
  morning's work; the un-shipping is not.
- Names. Trivial to change mechanically and expensive in every other way, because a name lands
  in 61 call sites, in a reader's head, and in the vocabulary people use to disagree. This is why
  names are an architect's, and why a lane ships a provisional one instead of waiting.
- Dependencies, §46 (thin primitives or whole subsystems), especially in the shipping graph.
  Adding one is a morning; removing one after a subsystem is built on it is a project.
- The system call surface: §10 (the capability-based microkernel process model) and §16 (object
  revocation), which is a boundary rather than a habit, and which every future program is written
  against.
- Facts that leave the machine, and this is the truly irreversible category. A published claim,
  a benchmark number a stranger quotes, a secret material once stored. §79 (holding
  password-equivalent material) is the case: approving an `NTOWFv2` beside an Argon2id tag was worth
  an hour of argument, because the decision cannot be unmade by deleting the code.

The test is not "can I revert the commit". It is "who else has already acted on this".

Two mechanisms here exist to widen a door that looks narrow, and both should be used rather than
deliberated around. A provisional name converts a naming decision from expensive to cheap by
saying out loud that it is not settled. A recorded limitation in a `BUGS` section does the same
for a design compromise, by making it a known cost rather than an implied promise. Reach for these
instead of stalling.

And one thing changed the calculus, which this project is unusual in having to notice. Agents
made *code* dramatically more reversible: a subsystem can be rewritten in an hour, so the old
instinct to design carefully before typing is now often the expensive choice. They made **records no
more reversible at all.** Nobody can un-publish a decision, un-teach a reader a name, or un-store a
secret. So the gap between the two categories is wider here than it is in ordinary projects, and the
mistake to guard against is spending on the wrong side of it: **deliberating over code while
committing quickly to a name.**

The failures on record are both of that shape. A blind `sed` swept a rename across the tree and
rewrote the very row recording that the name had been *refused*, which is a cheap edit destroying an
expensive record. And a maintainer endorsed a lane's provisional name without question, against a
refusal that already existed and that nobody could find. That is the whole reason for being of
milestone 115 (the names that were ratified, and the ones that were refused).
