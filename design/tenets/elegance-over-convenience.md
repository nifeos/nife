# Elegance and performance beat implementation convenience

*Appendix to `nifeos/nife:AGENTS.md`, which carries the rule and its one-question test. This
file carries calef's framing, the worked example of §92 (a caretaker is supervised by the client it
serves), and why agents moved the balance. Moved here 2026-09-23 (UTC) on calef's authorization,
unchanged in substance.*

calef, 2026-08-16: "We wouldn't be building this project out of convenience. This whole enterprise
is inconvenient." Nobody writes a capability microkernel from the first instruction because it was
the easy way to get a file server. The whole thing is a bet that doing the harder correct thing
compounds, so an argument that reduces to "this option is less work to build" is arguing against the
project's reason for existing.

**The failure mode is not laziness, it is a recommendation that sounds like design.** The tell is a
case made in the vocabulary of architecture whose actual load-bearing clause is effort. §92 is the
worked example, recorded there on purpose. The caretaker-lifetime question got a first
recommendation (job membership) over the better one (supervision: §40 (a supervisor's death is its
subtree's death)), for the honest reason that it was the smaller change to make. It took calef
asking "why not option 2?" to surface that, and the better answer was not merely better, it was
*simpler to live with*: the chained case fell out for free instead of needing bookkeeping.

The test, and it is one question. *Would I still choose this if both options were the same
amount of work?* If the answer is no, the recommendation is about effort and must say so out loud,
in those words, so the reader can weigh it as effort rather than mistake it for judgment.

Why the balance moved, and this is what makes the tenet current rather than pious. Agents made
*code* dramatically cheaper: a subsystem can be rewritten in an hour. They made everything code is
measured against exactly as expensive as before, because a reader's understanding, a constraint the
next person inherits, a wire format two programs agree on, and a name in somebody's head are all
unchanged. So the thing convenience buys shrank by an order of magnitude while the thing elegance
buys held still. An argument from implementation cost was always the weakest one available here;
it is now weaker than it has ever been.

## What this does not license

**It is not a license to gold-plate, and the difference is falsifiable.** Elegance here means the
option with fewer moving parts, fewer things to remember, and fewer places to be wrong. It does not
mean the option with more abstraction, more generality, or more machinery: those are usually *less*
elegant and always more to maintain. The tree already refuses speculative trait-ification for
exactly this reason, and §46 (thin primitives or whole subsystems) refuses a dependency taken for
tidiness.

Performance belongs in the sentence for the same reason. A design that is slow is not elegant,
whatever it looks like on the page, and this project measures rather than argues (`script/bench`,
the icount tripwire, the honest ties). A recommendation that trades measurable performance for a
prettier structure owes numbers, not adjectives.

And it does not overturn the tenet below. *Move fast on what can be undone* is about how much
deliberation a decision deserves; this is about what you decide once you are deliberating. Deciding
quickly and choosing the harder-to-build option are not in tension: most of tonight's decisions took
minutes and several went to the more expensive answer.
