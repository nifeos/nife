# Mechanisms, not memory: the ladder and why each rung exists

*Appendix to `nifeos/nife:AGENTS.md`, which carries the ladder itself as a rule. This file
carries the evening that produced it, the worked examples on each rung, and the three failures that
showed what rung zero costs. Moved here 2026-09-23 (UTC) on calef's authorization, unchanged in
substance.*

calef, 2026-08-04, after an evening in which three separate duties turned out to belong to whoever
happened to notice, and none of them noticed. This is the tenet the roles in `AGENTS.md` exist to
serve, so read it first: it explains why there is a steward and a merge drain at all, rather than
a list of things a careful maintainer would simply do.

Design for coordinating many, not for one attentive person. A convention that works when one
person holds the whole system in their head fails the moment there are eleven lanes, a conversation
in progress, and a queue draining in the background. That is this project's normal condition, not
its worst case.

The ladder, strongest first. When something must not go wrong, reach for the highest rung that
fits:

1. Make the wrong state unrepresentable. A required struct field with no default is the
   strongest form there is, because the mechanism is the compiler and the exception surface is zero.
   Milestone 50 (pipes and redirection: one sink protocol) turned `InputSpec::Required` from a unit
   variant into one carrying `writes_while_reading`, and that single choice means a program which
   writes while it reads cannot be declared without saying so. A pull request comment had been
   written to remind the integrator of the same thing; the type made the reminder redundant.
2. A gate that fails loudly, in `script/lint` or CI. Weaker, because somebody has to write it
   and it can be wrong about the tree: the check rejected the repository's second-commonest prefix
   until §77 (the branch-prefix list now describes the tree). But it fires without being remembered.
3. A written record at the thing itself, which is the shape of milestone 115 (the names that
   were ratified, and the ones that were refused): provenance beside the name, not in a registry. It
   does not fire on its own, but the next person to touch that code is already reading it.
4. A note, a report, or a comment on a pull request. This is the floor, and it is what
   everything that failed on 2026-08-04 was relying on.

**"Somebody will notice" is not a mechanism.** It is rung zero and it belongs on no list.

**An exception is allowed and must say so.** Sometimes the higher rung costs more than the failure
does, and taking the lower one is the right call. When that happens, write down that it is an
exception and that it is a foot gun, in the place a reader meets it. An unmarked exception reads
as a design, and the next person extends it.

The tell that you are on too low a rung: a fact that exists only at a call site or in a report,
with no artifact anyone can read. That shape recurred three times in one day, each wearing different
clothes. Roadmap status was wrong in both records and invisible to the gate comparing them (§76
(what catches a milestone status wrong in both places)). Naming decisions lived in one table cell
nobody could find (milestone 115). And a merge-order coupling that only a lane's report mentioned.
When you notice it, move up a rung.

## We are all owners: see a problem, drive it to an owner, and if none, own it

calef, 2026-09-23: *"We are all owners here so let's make certain we don't let things fall through
the cracks if we see a problem drive it to the appropriate owner first and if there isn't one then
own it."*

The complement to "Nobody remembers, so build the mechanism that does not need them to": that
tenet says do not rely on anyone noticing; this one says that once you have, the problem is yours to
route, not to leave for whoever's pull request it lands on. A load-sensitive flake was written into
`notes/load-sensitive-assertions.md` twice and left both times, because "every individual encounter
looked like someone else's problem." Noticing is not owning until the problem has an owner.

**And owning is not recording.** calef, 2026-09-23, after a defect was reported, not fixed: *"Rather
than raise it as a concern, wouldn't it make sense to address it? That's Bias for Action."* The test
is *move fast on what can be undone*'s, applied to the fix: cheap and reversible, fix it now and the
record is a byproduct; on that tenet's irreversible list, write it up and stop. A `BUGS` entry is
the right answer to the second case and an evasion in the first. Acting on what you half understand
is worse than reporting it, so a refusal carrying its reason is an action: milestone 323 (the
falsification record is incomplete in five ways) refused two gaps for want of hardware.
