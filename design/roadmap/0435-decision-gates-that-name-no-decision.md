---
status: BUILT
raised: 2026-09-19
built: 2026-09-19
---
# 435. Forty-five milestones are gated on a decision nobody wrote down

Built 2026-09-19, in one evening by three lanes. All forty-five blocks were read, and
every one of them resolved to one of the three outcomes. 33 are decisions written up as
[§160 (what)](../decisions/0160-what-a-subshell-copies.md) to
[§192 (bound)](../decisions/0192-a-checked-direct-map-reader-for-the-acpi-walk.md), 6 had a
decision and
gained a citation, and 6 gate tokens were simply wrong. Minted the same day by calef, from a
question he asked about one block that turned out to be true of forty-five.
*(Number provisional until the merge queue lands it.)*

It carried `Gate: NONE` while it was open. The argument: reading a block and deciding
whether its token is wrong or its decision is owed needs nobody's permission, and writing a
fork up is not the same as answering it. The line is gone because a finished block's gate can only
be stale. What the work produced is a queue for calef, not an answer: all 33 sections are
`PROPOSED`.

## Where it came from, which is one question about one block

calef ruled milestone 421's gate from `NONE` to `DECISION` on 2026-09-19, then asked: *"Does the
prior one have a numbered decision?"* It did not. Its ask lived in the block's own *What is needed
from calef* section, which is one rung above a chat message and below where AGENTS.md puts an open
decision:

> Open decisions live in a file, not in a conversation.
>
> -- notes/skills/decisions/SKILL.md

A roadmap block is not chat scrollback, so this is rung three rather than rung four. It is still not
the file AGENTS.md names, and the measurement says the exception is the rule.

## The measurement

58 blocks carry a `DECISION` token in their gate. In 45 of them the gate paragraph names no
`design/decisions/` section and no `§`. The other 13 do, so the convention exists and is followed
about a quarter of the time.

```
52 66 95 102 105 131 142 147 178 180 188 198 205 206 207 224 241 260
271 327 328 330 335 340 341 342 347 350 360 388 391 394 395 397 398
400 403 404 406 407 408 413 415 419 423
```

Twenty-four of the forty-five are numbered 327 or above, so they arrived hours earlier from
milestone 433 (drain)'s drain of the proposal pile. Promotion carried each block's gate token across
unchanged, which was right for that pass and means a quarter of this backlog is fresh rather than
aged. It is also the second thing 433 surfaced that it did not cause.

## What this is, and what it is not

It is not forty-five missing decision files. A block whose gate says `DECISION` may be wrong
about itself, which is what 421's turned out to be in reverse: it said `NONE` while owing a decision.
Some of the 45 will be tokens to correct rather than decisions to write, and a lane that assumes
otherwise will mint decisions nobody needs.

It is not an attempt to answer any of them. Writing up a fork is not deciding it. Every decision
this milestone creates lands `**Status: PROPOSED.**` and waits, which is the state that makes the
architects' queue readable instead of scattered.

And it is not a claim that the blocks are wrong to be gated. `script/roadmap --ready` correctly
excludes all 58. What is wrong is that for 45 of them a reader cannot find out *why* without reading
the whole block, and then finds only a paragraph addressed to one person.

## What a lane does, per block

1. Read the block and decide which of three it is. The token is wrong and should be `NONE`,
   `HARDWARE` or `MILESTONE n`; or a decision is genuinely owed; or the decision exists and
   the gate paragraph simply does not cite it.
2. Correct the token with the reason written where the gate is, the way 421's correction records
   that it carried `NONE` for two days and why that was the failure the vocabulary exists to prevent.
3. Or write the decision up, `**Status: PROPOSED.**`, with what is being decided, the options,
   a recommendation and its reason, and what is blocked until it is answered. The seven questions in
   the decisions skill (`nifeos/nife:notes/skills/decisions/SKILL.md`) are the standard.
4. Or cite the decision that exists, in the gate paragraph, where a reader meets the gate.
5. Cite the new section from the block's gate paragraph either way, so the two records agree.

## What the first slice found, 2026-09-19

Slice A, fifteen blocks: 52 66 95 102 105 131 142 147 178 180 188 198 205 206 207. One lane,
one reading each, no code touched.

| outcome | count | blocks |
|---|---|---|
| a decision was genuinely owed and is now written up | 12 | 52, 66, 95, 102, 105, 131, 142, 147, 178, 180, 205, 206 |
| the decision existed and the gate did not cite it | 2 | 188 ([§95](../decisions/0095-a-proven-ipc-fastpath.md)), 198 ([§151](../decisions/0151-repository-goal-is-independent-release.md)) |
| the token was wrong | 1 | 207, corrected to `NONE` |

Sections minted: §160 to §171 (where), provisional until the merge queue lands them.

The denominator argues against this block's own framing. This
milestone was written as though the work were mostly writing forks up. In the first slice it mostly
was, but three of the fifteen were records that had fallen behind the tree, and a fourth and
fifth were blocks whose gate deferred to a decision that had since been taken:

- Milestone 105 (forks)'s first fork was decided six days before the block was minted.
  [§32](../decisions/0032-reap-without-build.md) (a supervisor may collect a corpse without being able
  to build one) ruled it on 2026-07-29 and `abi::rendezvous::REAP` ships it, authorized by the
  supervision relationship rather than the rights bit the block proposes. The block restated a
  settled question as open.
- Milestone 142 (text)'s gate names a decision half of which §104 (rich) took on 2026-08-20.
- Milestone 147 (profiler)'s gate defers to milestone 75 (read), whose question §139 (read) answered on 2026-09-02.
  Milestone 75's own block still says `NOT-STARTED` and does not mention §139, which is the
  same defect one file over. It is outside this milestone's list because 75's gate paragraph happens
  to contain a `§` (to §10) and so was counted as citing.

So the failure this milestone names has a second direction, and it is the worse one. A gate that
says `DECISION` for a reason that was answered months ago spends an architect's attention on a
decision made. There is nothing in the tree that would notice.

And the three-way judgment was not the hard part; the stale half was. Every block in this slice
was legible about what it wanted. What no reading of the block alone could tell was whether somebody
had since answered it, which took a grep of `design/decisions/` per block and found four.

## Slice B, fifteen blocks, resolved 2026-09-19

`224 241 260 271 327 328 330 335 340 341 342 347 350 360 388`, on `milestone/435-slice-b`, with
sections minted in a reserved range of twenty numbers beginning at §172, provisional until the
merge queue lands them.

Five tokens were wrong, which is a third of the slice and is the answer to the question this
milestone could not ask in advance:

- 224 and 260 wait on a person at a machine, which is `HARDWARE`'s second sense. 224's gate
  had been contradicting the status line directly above it since calef answered it on 2026-09-04.
- 271 confused its output with its gate: the `CR4.PCIDE` ruling is what the milestone delivers,
  and the measurement behind it needs xenon, because icount charges the added gate and credits
  nothing for the removed flush. Milestone 335 (silicon) measured exactly that on the riscv64 twin.
- 330 read a provisional name as a blocker, which is the opposite of what `design/naming.md`
  makes a provisional name for. It is now `NONE` and startable.
- 335 carried a second token for a question the tree had answered by practice: board numbers
  live in `bench/<board>-<date>/`.

Three had their decision and gained a citation rather than a file: 241 (calef's own
dated deferral, quoted in the gate). 340 gained [§97 (gates)](../decisions/0097-advisory-checks.md), whose
ruleset edit is `DECIDED` and unperformed and whose `BUGS` predicted this block. 388
gained [§154 (the acronym test is whether the phrase is
spoken)](../decisions/0154-the-acronym-test-is-whether-the-phrase-is-spoken.md), which rules four of
that block's five names by name.

Seven were genuinely owed and are written up `PROPOSED`: §172 (327), §173 (328), §174 (341),
§175 (342), §176 (347), §177 (350), §178 (360).

The rate differs sharply between the aged blocks and the promoted ones, which is a measurement
about milestone 433's drain rather than about these blocks. Of the four numbered under 327, three
carried a wrong token and one cited its decision, and none owed a new file. Of the eleven
numbered 327 or above, **seven owed a decision**, two had one and two were wrong. The
promotion carried tokens across unchanged and that was right. The proposals it drained were mostly
written by lanes that had met a real fork, while the older blocks had drifted away from gates that
were true when they were set.

Two premise corrections were found on the way, both recorded in the blocks themselves. xenon has
booted since 2026-09-05, so half of milestone 241's trigger has fired, and milestone 330 cited a
constant, `KERNEL_WRITER_ANCHORS`, that exists nowhere in the tree.

**And one thing the reserved ranges cost, which the integrator has to resolve rather than a lane.**
`script/decisions --check` fails on a gap in the numbering, so a slice minting from §172 upward
while the twenty numbers below it are held by another lane is red on its own branch by
construction. The numbers are
provisional for exactly this reason; closing the gap is a renumber at merge, in the same breath as
every other global name.

## What the slices found

**Slice c: 391 394 395 397 398 400 403 404 406 407 408 413 415 419 423**, resolved 2026-09-19 on
`milestone/435-slice-c`. Fifteen blocks, all of them promoted hours earlier by milestone 433, which
makes this slice the cleanest available measurement of whether that drain carried bad tokens across.

**It did not.** Fourteen of the fifteen owed a decision that nobody had written down, and became
§§179-192 (`PROPOSED`; the lane reserved a range of twenty and the integrator compacted it at
merge, which is the renumber slice B's last paragraph predicted). **Zero tokens were wrong.** One, milestone 391, was the
third outcome: a decision existed and the gate did not cite it. The decision is §149 (endpoint),
which refused this exact question on the ground that the general case must not be settled on the
narrow one's momentum. It said to answer it when milestone 269 (`machine` at the prompt) gives it a consumer. So 391 gets a
citation and deliberately gets no new section.

**Two blocks arrived with a problem recorded, and both readings held.** Milestone 406 (tour)'s
title is false as written and its narrower half is the work; that lane judged the gate and left the
retitling to calef. Milestone 415 was suspected of carrying a token that had gone too strong, and
the suspicion turns out to be backwards. The item that needed nobody's permission is the one that
landed. So what remains is more purely `DECISION` than when the gate was written. A **different**
argument that the token is too strong does exist, on the block's own reversibility paragraph, and
§190 puts it to calef as its first question rather than a lane deciding it.

**What that rate means for the other two slices, which hold aged blocks rather than fresh ones.**
Fourteen of fifteen is a high owed-decision rate, and the reason is visible in the blocks. A
promoted proposal was written by a lane that hit the fork. So its `DECISION` token was minted by
somebody who had just met the question. An aged block's token is more likely to have been inherited,
which is the condition under which it goes stale. So slice c's rate should be read as a floor for
the promoted quarter rather than as a prediction for the other 21.

## BUGS

- A written-up fork is still a fork, and this milestone makes the architects' queue longer to read
  before it makes it shorter. Forty-five paragraphs scattered across blocks become some number of
  files in one directory; that is an improvement in findability and not in volume. The honest claim
  is that a queue you can list is cheaper than one you cannot, not that this reduces what is owed.
- Section numbers are the expensive half. Each lane holds a reserved range so two cannot collide,
  and every number in them is provisional until the merge queue lands it, which is the standing rule
  for anything global to the tree.
- The reserved ranges make every slice branch fail `script/decisions --check` on its own, and
  this was measured rather than predicted. Slice c holds the range beginning at 197. With the two
  ranges below it living on branches it cannot see, its own gate reports a gap in the numbering
  across the whole of 157 to 196 and exits 1. (The numbers are written without their sigil here on
  purpose: a range written with it is read as a citation, and `script/decisions` then reports the
  range's endpoints as citations to sections that do not exist.)
  The check has no escape hatch and is right not to have one, since a hole is almost always a
  renumber on merge that nobody meant. **It resolves at merge and only at merge**, which puts two
  duties on the integrator. One: land the three slices in one merge-queue group so the group build sees a
  contiguous tree. Two: compact the numbering if a lane did not fill its range, because three ranges
  sized 20 against slices that minted 14, 20 and 20 would leave the same hole in the merged tree.
  A lane that renumbered into another lane's range to make its own branch green would be claiming a
  global name, which is the rule this scheme exists to keep.
- The three-way judgment is the part no gate can check. Whether a token is wrong or a decision
  is owed is a reading, and a lane that guesses wrong either mints a decision nobody needs or puts a
  block on the ready list that a lane will stall on. The second failure is worse and lanes are told
  so.
- **Nothing here stops a future block gating on an unwritten decision.** A gate that tried would have
  to read prose for intent, which AGENTS.md priced at `git grep -w TODO`'s 82% false-positive
  rate. What this milestone buys is one sweep and a convention stated where the gate vocabulary
  lives; keeping it true is a habit.

## Follow-on

- **Done.** *Milestone 75 carried the same defect and the filter could not see it.* Its gate defers
  to a question [§139](../decisions/0139-cycle-counter-authority.md) answered on 2026-09-02, under a
  title identical to the block's own, and the block still read `NOT-STARTED` with `Gate: DECISION`.
  It was outside the forty-five because its gate paragraph happens to cite §10 and so counted as
  citing a decision. Corrected in this milestone's own commit: the gate is `NONE` and the block
  cites §139.
- **Done.** *Thirty-three forks that lived as paragraphs addressed to one person now live as files.*
  That is the whole of this milestone's output and its home is `design/decisions/`, one `PROPOSED`
  section each, listed by `script/decisions --unanswered`. Answering them is an architect's and is
  not this block's to hold open; the `BUGS` section above is honest that writing them up makes his
  queue longer to read before it makes it shorter.
- **Recorded.** *Milestone 406's title is false by its own first sentence*, found by slice c's lane,
  which judged the gate and correctly left the retitling alone. A title is a name, so it is an
  architect's and it does not become a decision file. Recorded in 406's own status line, where a
  reader meets the claim before they meet anything else in the block.
- **Recorded.** *Nothing stops the next block gating on an unwritten decision.* Priced in this
  block's `BUGS` and in the roadmap README's gate vocabulary, where the convention now lives.
- **Recorded.** *A slice minting into a reserved range is red on its own branch by construction.*
  Recorded in slice B's paragraph and in `BUGS`; it resolved at merge, as they said it would.

## Index row

Fifty-eight milestones carry a `DECISION` gate and forty-five of them name no decision anywhere a
reader can open, so the ask exists only as a paragraph inside the block, addressed to one person.
AGENTS.md says open decisions live in a file rather than in a conversation, and a roadmap block is
rung three rather than the rung four of chat scrollback, which is why this went unnoticed. calef
found it by asking whether milestone 421's freshly corrected gate had a decision behind it; it did
not, and a decision was minted on the spot (now §193). **Two sessions collided on a section number
four times in two and a half hours**, which is why anything global stays provisional until the queue
lands it. This one was minted §156; the other session's §156 landed first, then its §157, then its
§158, then its §159, and each landing displaced this branch's whole range by one. calef ruled the interleaving
stays (2026-09-19): *"We are modeling multiple contributors in the code base and interleaving seems
like how we would handle that."* Twenty-four of the forty-five arrived the same
evening from milestone 433's drain, which carried each proposal's gate token across unchanged. The work is a
reading per block and one of three outcomes: correct a token that is wrong, write up a fork that is
genuinely owed, or cite a decision that exists.
