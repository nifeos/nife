# §213 rule 6: passive voice

*Name: provisional, minted 2026-10-25 (UTC) by milestone/952-a-passive-voice-ratchet. calef names
files.*

An appendix of [`0213-writing-standards.md`](../0213-writing-standards.md), which states the rule.

Ruled by calef on 2026-10-25 (UTC), choosing option A:

> "Lets implement A. I think we also want a metric graph of the passive voice in the tree so that
> we can drive it down over time."

The counter is the `write-good.Passive` rule, a form of "be" followed by a word ending in "ed" or
an irregular past participle. `helpers/passive_voice.py` adopts its word list under its MIT
license; Vale itself stays out of the tree. Over the tree's 2,775 Markdown files on 2026-10-25,
Vale flagged 32,897 passives in 3.5 million words, 9.4 per 1,000, and 2,615 files sat over 2 per
1,000. A 12-hit random sample held only true passives. The ladder's noun "rung" fooled it, so the
counter keeps a short list of words it never reads as participles, each with its reason.

- A document of at least 200 prose words meets 2 per 1,000, the floor the bold budget uses.
- `design/passive-baseline.tsv` holds every document over the limit on the day the gate landed.
  Its count may not rise above its row, nor above its count at the merge base.
- A `writing-standards` exception marker exempts a document, as it does rules 1 to 3.
- `notes/project-metrics.md` charts the tree's total and its rate each week, so the backlog has a
  direction without a sweep.

calef refused two options. B, zero passives in a new document: the regular expression cannot tell
a useful passive from a lazy one, and "the row is refused" is often the clearest sentence. C, no
limit for a new document and only no growth for an old one: it freezes today's rate of 9.4 per
1,000 and never pushes it down.

The counter misses a passive with a word inside it ("is not used"), and a stative adjective
("is unchanged") reads as one. [Milestone 952 (a passive-voice ratchet in lint)](../../roadmap/0952-a-passive-voice-ratchet.md)
records both.

