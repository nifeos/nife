---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---

# 213. Writing standards: three countable rules, one review rule, and a ratchet

The rules, ratified by calef on 2026-09-23 (UTC) unless marked: a median sentence of 20 words or
fewer, no sentence over 40 words, and 4 or fewer bold spans per 1,000 words. Rule 4 is a review
question, and rule 5 is §298's, carried here. Rule 6, ratified 2026-10-25, holds new prose to 2
passive constructions per 1,000 words. Rule 7 spells out contractions. The
ratchet is the enforcement, and two proposed rules stay dropped.

His words, which are the whole brief:

> "We should also be clear, there is just a lot of long writing. We could be more dense with our
> writing. Amazon also had pretty strict writing standards to help."

Amended 2026-10-06 (UTC): calef ruled that prose and names use American spelling and that
measurements are metric. A product designation such as a 19-inch rack stays as named. The gate is
`helpers/house_style.py` in `script/lint`, a ratchet on both counts.

## What is being decided

How densely this tree's prose must be written, in numbers a script can count.

The sibling decision, [§212 (a prose budget)](0212-a-prose-budget-for-every-document.md), bounds how much a
document may be: 3,000 words of main body, with depth in capped appendices. This one bounds how
densely those words must be written. The two are separate on purpose, and this section links rather
than absorbs, which is 212's convention demonstrating itself.

## The evidence

Measured 2026-09-23 over every `.md` under `design/`, `design/decisions/`, `design/roadmap/`,
`notes/` and `briefs/`. Corpus: 1,004 documents, 2,057,764 words. Per-document statistics cover the
994 documents of at least 200 words. Fenced code blocks and table rows were stripped first. Then the
text was split into blocks on blank lines, headings and list-item starts, and sentences were split
within each block on `.!?` followed by whitespace and a capital, backtick, quote or asterisk. That
block-aware step is load-bearing, and the next section says why.

| measure | corpus | documents failing the ratified number |
|---|---|---|
| sentence length, median per document | median document is exactly 20 words | over 25: 11%. the share over 20 was not measured |
| sentence length, p90 per document | median document is 42 words | over 40: 61% |
| bold `**…**` per 1,000 words | median 16.1 | over 4: 100%. over 6: 98%. over 8: 95%. over 12: 77% |
| softeners per 1,000 words, narrow set | median 0.00 | over 0.5: 11%. over 1: 5%. over 2: 1% |

`rather than` appears 8,596 times, 4.2 per 1,000 words.

### The first measurement was wrong, and the error is instructive

In [`first-measurement.md`](0213-writing-standards/first-measurement.md): a splitter that ignored block boundaries inflated four figures, and the table above replaces them.

### Most bold in this tree is a heading that lost its syntax

calef's observation, and it is the more useful half of rule 3:

> "I think we bold entirely too much. We use bold for headings at times, which seems like we should
> just use headings. I think we just may be abusing bold."

Measured over the same corpus with code fences stripped, list markers ignored so that a bolded lead-in
inside a bullet still counts:

| pattern | count |
|---|---|
| all bold spans | 29,804 |
| bold that opens a line, the pseudo-heading | 18,712 (63%) |
| of those, ending in `.` `:` `?` `!`, a full claim as a lead-in | 12,474 |
| of those, the entire line is bold, a heading in all but syntax | 816 |
| genuinely inline bold | 10,510 |
| real markdown headings in the same corpus | 9,024 |

The tree has twice as many bold lead-ins as actual headings. The 29,804 figure is lower than an
earlier count of 33,528 because this pass strips code fences.

### What the corrected numbers say the defect is

Sentence length is close to the standard already. The measurable density defect is bold, at 16.1 per
1,000 words against a target of 4, with every one of the 994 documents failing. Two thirds of it is
the pseudo-heading above. The rest is the structural repetition rule 4 names, and no counter can see
that. So the diagnosis is bold and repetition, not long sentences.

### Why a countable rule is worth writing at all

Em-dashes appear zero times in two million words. The `AGENTS.md` banned-word list has two violations
across the corpus. Both are conventions in `AGENTS.md` and nothing gates either one. A style rule
that can be counted has held this tree essentially perfectly, even uncounted. So the density rules
that can be counted should be counted, rather than written as advice and left to hold on their own.

## The standard

Rules 1 to 3 and rule 6 carry numbers. The fourth is the most valuable and no machine can check
it. The fifth has a phrase gate under it, and review does the rest.

1. A document's median sentence is 20 words or fewer. The corrected corpus median is exactly 20, so
   the tree already sits on Amazon's 15-to-20 standard, and the proposal's 25 was a floor set above
   where the tree is. A floor above the tree stops nothing. Rule 1's job is regression, not reform.
2. No sentence over 40 words. This one bites: 61% of documents have a p90 over 40, and the median
   document's p90 is 42. A 40-word sentence has to be re-read. Re-reading is the cost this pair of
   decisions exists to cut.
3. Bold marks a claim, not a clause, and it is two rules rather than one. First, the budget: 4 or
   fewer `**…**` per 1,000 words, against today's median of 16.1 with every document over it. Second,
   and this is where the volume is: a bold span that opens a line is a heading that lost its syntax.
   Promote it to a real heading, or drop the bold and let the sentence carry itself. Which fix
   applies to which bold is ruled below, under *A label that opens a line becomes a heading*. The 816
   whole-line bolds are unambiguous and mechanically convertible, so they are the first cut.
4. State a finding once, and never comment on your own finding. No number, and this is the rule that
   would cut the most. `design/fatal-risks.md` is the worked example a reader can go check. A finding
   there appears in a status line, again in its own subsection, again in the running-order table.
   Each appearance carries a sentence telling the reader how to feel about it: "which is the point",
   "and that is what makes it credible", "stated once so it is not re-litigated". The finding with
   its caveat is the content. The commentary on the finding is not, and it is often the longest part.
5. A document states what is in force now, and its history is in git (calef, 2026-10-13 UTC).
   [§298 (a document states what is in force now)](0298-a-document-states-what-is-in-force-now.md)
   is the rule, what stays, and its gate.
6. Write in the active voice: new prose carries at most 2 passive constructions per 1,000 words,
   and an old document's passive count only falls (calef, 2026-10-25 UTC).
   [`passive-voice.md`](0213-writing-standards/passive-voice.md) has the ruling, the measure and
   the two options refused.
7. Spell out contractions ("do not", "it is", "cannot") in prose, commit messages and pull request
   bodies. A quotation keeps its source's words. Ratified 2026-10-25 (UTC).

### A label that opens a line becomes a heading

Ruled by calef on 2026-09-26 (UTC). He took it to be the rule already, and it was not: rule 3 named
two fixes and never said which bold gets which. The volume is in the table above, where 18,712 of
29,804 bold spans open a line.

A label names the part of the document that follows: *Operations.*, *Spawn side.*, *Running the
gate.* A bold label that opens a paragraph becomes a real heading, one level below the heading it
sits under, without its trailing period or colon. The paragraph keeps its words.

A claim is out of scope. A bold sentence that opens a bullet and asserts a finding (*`std::net::TcpListener`
is bound*, *The multicast half was retired on 2026-09-15*) is emphasis, not a heading. Rule 3's
budget governs it, and dropping the bold is the usual fix. The test is what the bold does, not its
grammar. A label is something a reader might want to jump to, and a claim is something a reader
should not miss. So a sentence standing where its siblings are noun phrases is a label too.

Fields a script parses, such as `**Status:**`, `**Gate:**` and `**Built:**`, are neither. They wait
on the frontmatter ruling below.

A list of parallel items is decided by the length of its items:

- Items that are sections, several sentences each, dissolve. Each label becomes a heading, and its
  item becomes a paragraph under it. `notes/net.md`'s socket contract is the worked example: five
  items of three to eight sentences, from *A socket is a socket id.* to *One binary, one archive
  entry.*, became five `###` headings on 2026-09-26.
- Items that are entries, a sentence or two each, stay a list and lose the bold. The list marker
  already shows where each entry starts. A heading per short entry would bury the outline under its
  own table of contents. The roadmap's repeated *Recorded.* and *Refused.* tags are this kind.

### Why the label rule is rung 3 for now

A gate was measured before it was refused, on 2026-09-26 over the ratchet's scope of 1,216
documents. The candidate: a line-opening bold span of five words or fewer that ends in a period or
colon, with more text after it in the same paragraph. It matches 5,720 spans in 967 documents.

Of 80 sampled matches, 24 were labels a heading fixes. 23 were repeated tags like *Recorded.* and
*Outstanding.*, which lose their bold instead. 12 were parsed fields, 15 were claims (*That is a
syscall.*, *The seam is the payoff.*) and 5 were definition terms. Under a third of the hits are
what this rule converts. It also misses long labels: 5 of 40 sampled non-matches were labels over
five words, such as *Why this is the selling point, and safe.* Telling a claim from a label is a
question about verbs, and a regular expression cannot ask it.

So the rule is rung 3, a record at the thing: this section, and the headings a converted document
now carries. The ratchet's line-opening bold count is the rung-2 backstop, and it already stops a
document over the budget from gaining a label.

### The budget has no irreducible floor, and an earlier claim that it did was wrong

A maintainer session claimed a document of nothing but claims cannot go below roughly 6.5 bold spans
per 1,000 words. That is false, and `AGENTS.md` is the counterexample. Its split form measures 6.3 per
1,000, and 34 of its 37 spans open a line. Converting or dropping those puts it at 0.5.

So the budget and the heading fix are the same work. Four per 1,000 is reachable because
pseudo-headings are where the volume is, and not because genuine emphasis has to be sacrificed.

## Two rules refused by measurement

The refusals are the valuable half, so both are recorded with their numbers.

A hedging limit was proposed and dropped. It looked justified at 4.8 softeners per 1,000 words. That
count included modal `may` and `might`, which do real work in specification prose (*a lane may
report in two shapes*). On the narrow softener set the corpus median is 0.00 per 1,000 words, and
only 5% of documents exceed 1. This tree does not have a hedging problem. The gate would never fire.

A limit on `rather than` was proposed and dropped. Its 8,596 uses looked like a tic. Read, they are
mostly load-bearing contrast, and they are this tree's actual idiom (*a fact rather than a plan*).

## Enforcement is a ratchet, not a cliff

Rule 3 fails every document in the tree, so a cliff would be 994 instant failures and a migration
project nobody wants. The gate sits at rung 2 of the `AGENTS.md` ladder, a check that fails loudly,
and it is [milestone 586 (a prose ratchet in lint)](../roadmap/0586-a-prose-ratchet-in-lint.md), shared with §212's.
#1230 built it on 2026-09-24 as `helpers/prose_ratchet.py`, run by `script/lint`. The milestone
is PARTIAL: one week of baseline-churn measurement remains.

- A document's median sentence length and longest sentence may not rise. That is the shape of the
  unsafe-count ratchet and the icount tripwire already in this tree. It turns a wall into a
  direction.
- A document a change touches meets the bold budget outright, and its baseline grants it no bold.
  calef, 2026-09-26 (UTC): *"4 bolds per 1000 is the right ratio for our written prose. That it
  was previously written without density is irrelevant. Bold should be rare."* The gate first held
  bold as two counts, because condensing a document raises its density. The answer now is that
  whoever condenses a document removes its bold too. Bold a script parses (`**Status:`, `**Built:**`,
  a Follow-on tag) is syntax, not emphasis, and is not counted; the ratchet reads that set from the
  parsers' own source (maintainer ruling on #1311, 2026-09-26). An untouched document keeps its baseline
  counts, so the tree did not go red that day. He chose not to sweep the backlog. The weekly bold
  chart in `notes/project-metrics.md` drives it down, and retires itself at zero.
- A mechanical rename is not a touch (calef, 2026-09-27T05:23Z; ruling in design/roadmap/586).
  `rename_masked` reads such a line as the form it replaced.
- The check reports line-opening and inline bold as two counts. They have different fixes, and a
  single density number hides which one a document has.
- A new document meets the standard outright. So does a document being rewritten wholesale, which is
  how the tree converges without a sweep.
- A marked exception in the document, carrying its reason. The ladder permits an exception and
  requires it to say so out loud, because an unmarked exception reads as a design and the next person
  extends it.
- Rule 4 is a review question, and no check can see it. `AGENTS.md` already refuses to gate what a
  lint cannot distinguish from an observation. This section does not pretend otherwise.
- The gate must split on block boundaries before it splits sentences. Otherwise it fails documents
  that pass, for the reason recorded above.

`AGENTS.md`'s `## Style` section links to this section as the record, rather than growing a list of
numbers. The maintainer added that link at merge.

## What it costs

Two risks, neither softened.

A sentence-length limit can be satisfied by chopping one clear 45-word sentence into two murky
22-word ones. That is worse prose passing a green check, and the check cannot tell the difference. A
bold limit can push emphasis into italics, capitals or a heading. Italics and capitals are the same
evasion wearing different clothes. A real heading is not, which is the point of rule 3's second half,
and it is the one case where the cheap way out is also the fix.

Both are why rule 4 and a human reviewer remain the real mechanism. The countable rules are a floor,
not the standard itself.

The third cost is the ratchet's own: it never finishes. A monotone check on 994 documents converges
only as documents are rewritten for other reasons, so most of the tree stays over the bold number for
a long time. That is the price of not running a migration.

## Markup a gate parses is counted, and frontmatter is the likely answer

**Ratified by calef on 2026-09-24 (UTC).** The budget counts every bolded span, including markup a
gate reads. No exclusion was carved, and the table-cell precedent was not extended to it.

The collision is real and appeared in three subsystems. `design/decisions/` wrote `**Status:` until
milestone 582 (a decision's status becomes a field) moved it into frontmatter,
`design/fatal-risks.md` writes `**The ... experiment ...:**`, and every citation in the tree writes
`**milestone 41 (dead code)**`. Each is a field a script parses, wearing emphasis. `design/fatal-risks.md`
measures exactly 4.0 per 1,000 words with all seventeen of its spans being machinery, so it meets
the budget and has nothing left for emphasis.

calef's reasoning, 2026-09-24: *"We likely want to use something different like frontmatter. Let's
see how our frontmatter experiment goes before deciding."* So this is deferred rather than settled.
Pull request #1195 (decision status becomes frontmatter) is the experiment, and if it holds, the fields
move out of bold entirely and the collision stops existing instead of being carved around. Until
then a document whose budget is spent on markup records that in its own `BUGS` section.

## BUGS

- The corpus numbers were measured on 2026-09-23 with a regular expression, and the first pass of
  that measurement was wrong in the way described above. The corrected figures are one agent's
  second attempt, not an independent check.
- Rule 3's budget is arithmetic from a target, not evidence about readers. Nothing here shows that 4
  bold spans per 1,000 words is better for a reader than 8. Its second half is better grounded: 816
  whole-line bolds are heading syntax written as emphasis, and that is a defect whatever the budget
  is.
- The label gate's 80-sample classification is one agent's hand judgment on 2026-09-26, and the
  line between a short claim and a label is exactly where two readers would disagree.
- This section meets its own three numbers, which tests that they are livable in a document carrying
  numbers and citations. It does not test them on a note explaining a mechanism, and that is the
  longer half of the tree.
- Rule 7 has no gate, and about 600 contractions stood in the tree's Markdown on 2026-10-25.
