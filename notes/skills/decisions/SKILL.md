---
name: decisions
description: >-
  How nife decides, recommends, and takes a question to an architect. Load before choosing between
  options, making a recommendation, or deciding how much care a choice deserves; and when something
  may be an architect's call (a design fork, a wire format, a name, the syscall surface, a new
  dependency, a design/decisions/ section, a published number), when holding work for an architect,
  adding the needs-architect label, writing a "## What I need from you" comment or a PROPOSED
  decision file. Holds: move fast on what can be undone and be methodical on what cannot (the test
  is who else has already acted on it); elegance and performance beat implementation convenience
  (would I still choose this if both options were the same amount of work? if not, say the
  recommendation is about effort); how a ruling is asked (plain background first, recommendation
  first, one decision at a time); the seven questions a fork must answer before it reaches an
  architect; and why requesting a review from calef silently does nothing.
---

# Deciding, and taking a decision to an architect

Where this says "this file", it means `AGENTS.md`. Its name was ratified by calef on 2026-10-06
(UTC); notes/skills/README.md records it.

## Move fast on what can be undone; be methodical on what cannot

calef, 2026-08-05. The ladder above says how hard to make a thing hold. This says how much care to
spend deciding it. The reasoning behind each category, and the two failures of record, are in
[design/tenets/reversibility.md](../../../design/tenets/reversibility.md).

Most decisions here are reversible and should be made quickly, by whoever is holding the problem.
Code, notes, roadmap wording, which milestone a lane takes, how a script is structured. Deliberating
them costs more than getting them wrong, and deliberating them *with an architect* costs an
architect's attention, which is the scarcest thing in this project.

A few decisions are expensive, and the expense is almost never the code. Be methodical on these:

- Anything two programs agree on. A wire format, an opcode number, a packed word.
- Names. A name lands in dozens of call sites, in a reader's head, and in the vocabulary people use
  to disagree. This is why names are an architect's call, and why a lane ships a provisional one
  instead of waiting.
- Dependencies, §46 (thin primitives or whole subsystems), especially in the shipping graph.
- The syscall surface: §10 (the capability-based microkernel process model) and §16 (object
  revocation), which every future program is written against.
- Facts that leave the machine: a published claim, a benchmark number a stranger quotes, a secret
  material once stored. This is the truly irreversible category.

The test is not "can I revert the commit". It is "who else has already acted on this".

Two mechanisms exist to widen a door that looks narrow, and both should be used rather than
deliberated around. A provisional name converts a naming decision from expensive to cheap by saying
out loud that it is not settled. A recorded limitation in a `BUGS` section does the same for a
design compromise. Reach for these instead of stalling.

Agents made code dramatically more reversible and records no more reversible at all. So the mistake
to guard against is spending on the wrong side of the line: deliberating over code while committing
quickly to a name.

## Elegance and performance beat implementation convenience

calef, 2026-08-16: "We wouldn't be building this project out of convenience. This whole enterprise
is inconvenient." An argument that reduces to "this option is less work to build" is arguing against
the project's reason for existing. The failure mode is not laziness, it is a recommendation that
sounds like design: a case made in the vocabulary of architecture whose actual load-bearing clause
is effort. The worked example, and why agents moved the balance further, are in
[design/tenets/elegance-over-convenience.md](../../../design/tenets/elegance-over-convenience.md).

The test, and it is one question. *Would I still choose this if both options were the same amount of
work?* If the answer is no, the recommendation is about effort and must say so out loud, in those
words, so the reader can weigh it as effort rather than mistake it for judgment.

It is not a license to gold-plate. Elegance here means the option with fewer moving parts, fewer
things to remember, and fewer places to be wrong. It does not mean more abstraction, more
generality, or more machinery: those are usually *less* elegant and always more to maintain.

Performance belongs in the sentence for the same reason. Measure rather than argue (`script/bench`,
the icount tripwire, the honest ties). A recommendation that trades measurable performance for a
prettier structure owes numbers, not adjectives.

## Open decisions, and work waiting on an architect

Open decisions live in a file, not in a conversation. One waiting on an architect goes in
`design/decisions/` marked [`status: PROPOSED`](../../../design/decisions/README.md), one file each: what is
being decided, the options, the recommendation with its reason, and what is blocked until it is
answered.

And work waiting on an architect carries its own label and its own ask (calef, 2026-08-04), both at
the moment the decision to hold is made and not later:

- The `needs-architect` label, so the queue is `gh pr list --label needs-architect` rather than a
  paragraph somebody has to have read. It names the role, not the person. A thing lands there when
  it is outside standing merge authority: the syscall surface, a new dependency, or a
  `design/decisions/` section owed. It means waiting on calef and nothing else, so the list is his
  worklist. An edit to a decision is his to approve unless it is bookkeeping: a link retargeted, a
  backticked path moved, a paragraph rewrapped (calef, 2026-10-11 UTC; §88 (`needs-architect` as a
  required check)'s amendment). A changed word still asks, a renamed identifier included.
- A send-back swaps it for `held-by-lane` (2026-10-06). When a ruling asks the lane for a change,
  take `needs-architect` off, put `held-by-lane` on, and add `architect-ruled` if the ruling covers
  the diff. The `architect hold` check fails on `held-by-lane` even beside `architect-ruled`, the
  labelers do not re-add `needs-architect` while it is on, and the merge drain dequeues it. The lane
  removes it in the push that carries the change. It is a label rather than a return to draft
  because drafts skip CI, and the rework needs its gates.
- A `## What I need from you` comment naming the specific ask. It must be answerable without reading
  the diff, it must say what happens if the architect says no, and it must separate what is blocking
  from what is eventually the architect's.

Every question asked of calef is also posted on its pull request or issue as a `## What I need from
you` comment, with `needs-architect`, including one first raised in a chat (lane/architect-queue,
2026-10-06 UTC; calef: "I need a reliable work queue just like your agents"). A question that lives
only in a chat is missing from his queue, `script/architect-queue`, which lists an item when it
carries the label or has an unanswered ask. Record his answer with `script/record-ruling`, which
posts it and swaps `needs-architect` for `architect-ruled` (plus `held-by-lane` on a send-back) in
one step. In the tree, write the ruling as the rule in force, with its date, rewritten in place. The
ruling it replaces and the conversation that led to it stay on the pull request and in git, and a
replaced decision's frontmatter names its successor (§298 (a document states what is in force now)). Where the label and the open questions disagree, the drain repairs one direction and
flags the other (calef's ruling on #1792, 2026-10-06 UTC): an open question or a moved surface with
no label gets `needs-architect` added by the bot, since a wrong add costs calef a glance; a label
with no open question gets `needs-maintainer` and is never removed by the bot, since a wrong removal
could merge a pull request without his ruling (helpers/needs-maintainer.jq).

The watchers run unattended as `nife-smelter[bot]` in scheduled Actions workflows (calef,
2026-09-23; the watch that reads a machine's own lane worktrees stays per developer). A session
confirms they are alive *and reads what they already found*, because `merge-drain.sh` posts once per
stall and then goes quiet by design. [`briefs/session-start.md`](../../../briefs/session-start.md) is that
check, deferring the queue read to [`briefs/survey-the-queue.md`](../../../briefs/survey-the-queue.md). A
queue reports, it does not resolve. [`notes/merge-queue.md`](../../../notes/merge-queue.md) has the
workflows.

Do not try to route this by requesting a review. GitHub silently refuses a review request from the
pull request's own author: `gh pr edit N --add-reviewer calef` returns success and sets zero
reviewers, because every pull request here is authored under calef's account by the `gh` token.
Assignees and labels do work; reviewers do not.

Stop for an architect only when it is genuinely the architect's call: a design fork not
already decided, a test that will not pass after real effort, a hardware or external dependency, or
the machine contradicting the plan. Otherwise proceed and report what you did.

## A ruling is asked plainly, one at a time

Ruled 2026-10-24 (UTC), after three questions in a row came back unanswerable for want of context.
Each answer changed once the context came first: one moved off the recommended option, and one
gained two hosts nobody had listed.

So each question put to an architect, in a chat or a `## What I need from you` comment, follows one
shape:

- Two to four short paragraphs come first. They say what the thing is, why it matters now, what
  constrains it and what each option costs.
- Then the question, with the recommendation first and the options lettered after it.
- No lane jargon in the question itself: no queue layouts, crate names or milestone shorthand that
  the paragraphs above did not explain.
- Name the constraint and not only your shortlist. An architect often brings an option nobody
  listed.
- One decision at a time. Ask, wait for the answer, then ask the next.

## A fork reaches an architect with its questions already answered

calef, 2026-08-18: *"my intent is not just to have a lane surface a problem, but to investigate and
propose solutions so that the questions I usually ask to help decide I don't need to ask."* A fork
that reaches an architect having spent that attention on lookups anyone could have run has been
mishandled, even if it arrived with a tidy list of options. This binds whoever presents the fork,
which is usually the maintainer rather than a lane.

The seven questions. A proposal that cannot answer one should say so rather than leave it implied.

1. What else was considered, and why did each lose? A refusal with a reason for each, not a list.
2. What does this tree already do in the analogous case? Almost always one grep, and it usually
   decides.
3. What is the prior art outside the tree? Read, not recalled; a claim from memory is marked as
   such.
4. Is the premise true? Verify the framing before defending a position inside it.
5. What does each option cost, measured rather than asserted?
6. How reversible is it, and who has already acted on it?
7. Would we still choose this if both options cost the same? If the answer is no, the recommendation
   is about effort and must say so in those words.

The tell that a proposal is not ready is that it argues rather than shows. Questions 2 through 5 are
all lookups. If the presenter is reaching for an adjective where a command would do, the work is not
finished.

Two limits, so this does not become a tax. Recommend on reversible forks; give options only on
irreversible ones (the *move fast* tenet's list). And a fork only earns a lane when nobody can say
what the options cost: if an architect can answer in a sentence, researching first spends more than
a wrong answer would. Guard against proposal-shaped procrastination, because a lane is not a place
to put a question you are avoiding.
