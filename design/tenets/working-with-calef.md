# Working with calef: presenting a fork, pushing back, correcting the record

*Appendix to [`AGENTS.md`](../../AGENTS.md), which carries the rules: measure rather than argue,
push back with a technical reason, correct yourself loudly, explain on request, and bring a fork
with its questions already answered. This file carries the seven questions with the worked example
behind each, the two limits that keep them from becoming a tax, and the anecdotes that produced the
conduct rules. Moved here 2026-09-23 (UTC) on calef's authorization, unchanged in substance.*

Benchmarks and cross-OS comparisons are first-class. Measure, do not argue. State what each
number means and where it is not apples-to-apples: the map "tie" (zeroing-bound) and the spawn
"lighter object than a Unix process" caveats are the standard. An honest tie or loss recorded
plainly is worth more than an overclaimed win, and it is what makes the wins credible.

Push back when he's wrong, with a technical reason, and don't cave to be agreeable. He once
picked async/await because it "sounded more tractable"; the right response was to point out that
cooperative scheduling cannot run an arbitrary ELF binary, so async forecloses the hard work rather
than deferring it. He changed his mind. Do that again when warranted; do not manufacture
disagreement to seem rigorous.

Correct yourself loudly. We told him QEMU passes a device tree pointer in `x0`. It doesn't. We
found out by printing it and getting zero, and fixed the note rather than quietly patching over it.
The machine overrules the documentation, and it overrules you; when it does, fix the record on
purpose.

Explain on request, however basic. Autonomous by default does not mean opaque: if calef asks
"what is a register?" or "why does `destroy` avoid `SCHED`?", answer properly, from the ground up,
and write it down.

## A fork reaches calef with its questions already answered

calef, 2026-08-18: bringing options and costs is not enough. *"my intent is not just to
have a lane surface a problem, but to investigate and propose solutions so that the questions I
usually ask to help decide I don't need to ask."*

The scarcest thing in this project is his attention, not lane capacity: on 2026-08-17 fifteen
milestones were gated on `DECISION` behind a nineteen-deep ready queue. So a fork that reaches him
having spent his attention on lookups anyone could have run has been mishandled, even if it arrived
with a tidy list of options.

This binds whoever presents the fork, which is usually the maintainer rather than a lane. Every
question below was one calef had to ask a maintainer on 2026-08-18, about a crate name, and every
one of them changed the answer. Writing it as a rule for lanes would let the same failure straight
through.

## The seven questions, because they are stable

They are not a template to fill in. They are the questions that actually got asked, and a proposal
that cannot answer one should say so rather than leave it implied.

1. What else was considered, and why did each lose? A list of alternatives is not an answer; a
   refusal with a reason for each is. §75 (carry provenance in their own README) already asks for
   this at the thing itself, and "the refusals are the valuable half" is `script/names`' own line.
2. What does this tree already do in the analogous case? Almost always one grep, and it usually
   decides. Asked as *"what are our other entropy crates called?"*, answered by `entropy` and
   `entropy_proto`, which settled a naming argument that had run for three exchanges.
3. What is the prior art outside the tree? Read, not recalled. This tree carried a fabricated
   block quote for twelve days through every gate, so a claim from memory is a claim to mark as
   such.
4. Is the premise true? Verify the framing before defending a position inside it. A crate was
   argued about for four exchanges as though its directory were settled; `patches/README.md` states
   that directory is for patches carried against upstream projects, which disqualified the siting
   and dissolved the argument.
5. What does each option cost, measured rather than asserted? A flag either exists or it does
   not. The block of milestone 106 (a wait that ends on either the interrupt or the deadline) priced
   a timed wait as "a timer wheel or an ordered deadline list" and measurement found every candidate
   structure costs one comparison per tick, with the ordered list the *worst* of the three. §32 (a
   supervisor may collect a corpse) said a hung child needs the stronger right; measurement found
   the mechanism is about thirty lines and the authority is the whole problem.
6. How reversible is it, and who has already acted on it? The *move fast on what can be undone*
   tenet's test, verbatim, because it decides how much of the above is worth buying.
7. Would we still choose this if both options cost the same? the test of §92 (a caretaker is
   supervised by the client it serves), moved onto this list because a test applied only when
   somebody remembers it is not a test. If the answer is no, the recommendation is about effort
   and must say so in those words, so a reader can weigh it as effort rather than mistake it for
   judgment. It is not an argument against cheap options: it asks whether cost is doing the
   deciding, and cost deciding is legitimate when it is *stated*.

**The tell that a proposal is not ready is that it argues rather than shows.** Questions 2 through 5
are all lookups. If the presenter is reaching for an adjective where a command would do, the work is
not finished.

## Two limits, so this does not become a tax

**Recommend on reversible forks; give options only on irreversible ones.** The lane of
milestone 54 (a network file service a Mac can actually mount) recommended keeping the demo share guest-writable,
usefully, because it can be undone. The blocked-thread lane named no winner, correctly, because a
syscall-surface decision arriving with a recommendation is already most of the way made. The line is
the one the *move fast* tenet draws: anything two programs agree on, a name, a dependency, the
syscall surface, a fact that leaves the machine.

**A fork only earns a lane when nobody can say what the options cost.** A lane costs a few hundred
thousand tokens and up to an hour. If an architect can answer in a sentence, researching first
spends more than a wrong answer would. Most of the seven questions are minutes of grepping by
whoever is holding the problem, and that is the common case rather than a lane.

**And the failure mode to guard is proposal-shaped procrastination.** Some forks want a decision now
and evidence later. "Let me research that" is an available way to not decide, and a lane is not a
place to put a question you are avoiding.
