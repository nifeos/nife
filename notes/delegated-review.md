# Does a delegated AI review catch what the gates and the maintainer miss

*Name: provisional, shared with `notes/delegated-review/`. The page sits beside its appendices by
calef's ruling on #1921 (§212 (a prose budget: 3,000 words of main body)).*

This is the run of milestone 521 (does an AI review
of a pull request catch anything the gates and the maintainer do not), executed 2026-09-22 against
the defects three rented models had produced the same morning. The threshold was written into that
milestone's block and committed **before the first model call**, which `git log` on the block can be
checked against: the registering commit precedes every transcript commit on this branch.

The short version, and it is not the one the block's own prior predicted. The expensive
reasoning model, `moonshotai/kimi-k3`, found **ten true defects in a diff a human reviewer had
already judged clean**, and produced no false positive at all on that diff across six reviews.
It missed the defect on the four-line diff entirely, while reaching the right observation in its
own reasoning and talking itself out of it. The cheap coding model, `qwen/qwen3-coder`, said
`NO CONCERNS` to the same ten-defect diff on five of six reviews.

So the finding is not "AI review works" or "AI review is noise". It is that **findings scale with
reviewable surface, and both the true and the false ones do.** On 465 lines of prose the reasoning
model was better than the humans and the gates combined; on four lines it manufactured concerns on
the clean diff and passed the defective one.

## How the evidence was kept

The reviewer was blind: it was never told what it was looking for or which arm it was in, and never
saw the study, the ledger of verdicts, or the brief that named the defects. The prompt came out of
`helpers/review-bundle.sh` and `helpers/review-trial.sh` and nothing else, and the same prompt went to
defective and clean diffs alike. How each of those holds, and what the reviewer did not get, is in
[the method](delegated-review/the-method.md).

Every prompt is reconstructible and every answer is committed verbatim, in
`delegated-review/transcripts.tar.gz`, one file per model, diff, posture and replicate:

    tar xzf notes/delegated-review/transcripts.tar.gz -C /tmp

It unpacks to `transcripts/` (the 48 uniform-budget runs) and `transcripts-48k/` (the seven
supplementary ones).

Why it is an archive and not fifty-five files, which a reader meeting a `.tar.gz` in a notes
directory is owed. Loose, the transcripts failed four gates:

- the citation scan wanted the milestone numbers a model cited on an unmerged branch to resolve on
  `main`;
- `script/citations` wanted glosses on them;
- the counted-claims check measured a reasoning dump's 4735-byte line against the corpus
  `documentation`'s renderer is sized for;
- and the em-dash rule wanted a rented model's punctuation rewritten.

The first attempt at this note added an exclusion to each. calef refused that on 2026-09-22:
*"I don't want to change the style rules."* A style gate that grows an exception per corpus stops
being a rule and becomes a list. The em-dash rule in particular exists so that prose here reads as a
person wrote it, which an exemption for prose a model wrote would empty out. A gzip is not prose, so
no prose gate has an opinion about it and nothing in the evidence had to be edited to pass a check.
That is `script/citations`' own argument for the captured `*.log` fixtures, that nobody wrote those
lines as prose, carried one step further: unlike a machine log, model output contains the very
characters the style rule forbids.

## The corpus, and why the gates are not its control group

Three defect rows and two clean rows from the delegation ledger of 2026-09-22, which a sibling lane
is landing as `notes/delegation/ledger.tsv`. Every row in it passed `script/lint`,
`script/citations --ratchet` and `script/roadmap --check`. That is what makes it the right corpus:
the gates are not the thing being compared against, they are the thing that already failed.

| id | what it is | verdict when it was written |
|---|---|---|
| **D1** | a two-file, four-line renumber that **replaced** the sentence *(Number provisional until the merge queue lands it.)* with its own renumbering note | defect |
| **D2** | an install-offer implementation that emitted `" (unpartitioned or unreadable)"`, folding "safe to wipe" and "we could not read it" into one string on the prompt that asks consent to destroy a disk | defect |
| **C1** | the same renumber done by a different model, which kept the provisional sentence and appended instead | clean |
| **C2** | a 465-line documentation commit recording a calibration fix, carried through a rebase | clean |

The third defect row is a repeat of D1 by a second model whose commit does not survive, so it is not
scored separately. Only half of D2's recorded failure was reviewable, since the ledger also records
skipped tests. Both are explained in [the method](delegated-review/the-method.md).

## The runs

Two models, four diffs, two postures, three replicates: 48 reviews, plus seven supplementary runs.
At the uniform 16000-token budget `kimi-k3` returned no answer at all on the largest diff, so those
six cells were re-run at 48000 tokens and that is the set scored. Why replicates matter and how the
budget was handled are in [the method](delegated-review/the-method.md).

## Arm 1: did it name the defect?

Six reviews per model per defect diff. The question is not whether it said something sensible.

| model | D1 (the removed sentence) | D2 (the consent string) | defects named |
|---|---|---|---|
| `qwen3-coder` | **0 / 6** | **0 / 6** strict, 1 / 6 partial | **0 of 2** |
| `kimi-k3` | **0 / 6** | **5 / 6** | **1 of 2** |

`kimi-k3` named D2 in the ledger's own terms once, and four other reviews reached the same string by
noticing that `format_disk_description` never consults `F_MBR`. `qwen3-coder`'s one partial was the
right line for a wrong reason, and the registered threshold counts a partial as a miss.

Nothing named D1. But `kimi-k3`'s adversarial reviews found the observation and argued themselves out
of it, and the same model recommended D1's defect as a fix on the clean renumber, C1. Perception was
not the failure; judgment was. The transcripts behind each of these are quoted in
[the findings](delegated-review/the-findings.md).

## Arm 2: what does it cost in noise, and the arm that decided this

Every finding on a clean diff, adjudicated true, false, or unfalsifiable.

| model | diff | reviews | findings | true | false | unfalsifiable | **false per review** |
|---|---|---|---|---|---|---|---|
| `qwen3-coder` | C1 | 6 | 4 | 0 | 4 | 0 | **0.67** |
| `qwen3-coder` | C2 | 6 | 3 | 0 | 3 | 0 | **0.50** |
| `kimi-k3` | C1 | 6 | 12 | 0 | 8 | 4 | **1.33** |
| `kimi-k3` | C2 | 6 | 50 | 38 | **0** | 12 | **0.00** |

C2 was not clean, and that outranks the score. `kimi-k3` found a justifying paragraph that names the
wrong statistic, and an estimator called unbiased where the property is consistency. It found a
correction notice that over-certifies a cross-boot ratio and a cross-reference to the wrong section.
It found two deliverables that contradict each other, a dangling paragraph and an incomplete
inventory. None of that is
arguable and none of it was caught. So arm 2's denominator is wrong: C2's false-positive count is
zero partly because the diff was not clean.

The noise lives on the small diff. On C1 `kimi-k3` asserted things that are not true of this tree,
and `qwen3-coder`'s false findings asked the commit to prove what it had already stated. One finding
on C2 is recorded as disputed rather than resolved. Each finding, with the reviewer's own words where
they matter, is in [the findings](delegated-review/the-findings.md).

## The threshold, and what it says

Registered before the run: adopt at a third or more of arm 1's defects named with under one false
positive per clean diff; refuse under a fifth, or over two false positives per clean diff;
anything else null.

| model | arm 1 | arm 2, mean per clean diff | verdict against the registered threshold |
|---|---|---|---|
| `qwen3-coder` | 0 of 2, which is 0% | 0.58 | **REFUSE**, on arm 1 |
| `kimi-k3` | 1 of 2, which is 50% | 0.67 | **ADOPT**, on both arms |

And the threshold should be distrusted on this corpus, in that order. `kimi-k3` clears both
bars, and the number that clears the second one is an average of 1.33 and 0.00 over exactly two
clean diffs, one of which turned out not to be clean. Two defect diffs is a denominator where one
scoring call moves the verdict from adopt to null. **Read the D1 transcripts and the C2 findings;
do not read the table.**

## What this says about routing delegated work

`notes/open-model-lanes.md` states the rule that makes a cheaper model safe: the gates are the
oracle, so work with a crisp gate goes to the open model and work whose output is a judgment stays
on Claude. This run does not overturn that and it does add a third case. The gates could not see
any of the five defects here, the human reviewer could not see the ten in C2, and a rented reasoning
model could see those ten and not the four-line one.

So the shape to take from it, stated as a hypothesis this corpus is too small to have proved:

- **A large prose diff is where delegated review earns its cost.** Fifty findings, thirty-eight true,
  zero false, on a document nobody was going to reread.
- **A small diff is where it is worst**, in both directions at once. It passed the defective
  four-line diff and it invented concerns on the clean one, and the model's own reasoning shows why:
  with little to read it looks for something to say.
- **It cannot be trusted to weigh a finding it has already made.** The strongest evidence in this run
  is a reviewer holding the right answer and rejecting it as pedantic. Review that surfaces a
  candidate for a human to weigh is a different and more defensible product than review that decides.

## BUGS

- The adjudication was done by an agent, which is the conflict milestone 521's block named. The
  block asked for the adjudicator to be stated and the disputed cases recorded rather than only the
  totals. Both are done, the disputed case in [the findings](delegated-review/the-findings.md). The
  recurring findings were each checked against the file rather than accepted from the prose. It is still one party grading a model of its own kind, and nothing
  here fixes that. The transcripts are committed so the grading can be disagreed with.
- The corpus is five diffs, of which one turned out to be mislabeled. No rate computed here
  should be quoted as a rate. Two defect diffs means a single scoring call moves a verdict.
- The defects are all of one kind: a true sentence removed, or two meanings folded into one.
  Nothing here says anything about review catching a race, a lifetime bug or an off-by-one.
- The bundle is one commit and its pre-images, not the worktree. Several findings ask for
  evidence from files the reviewer was never shown, and they are counted unfalsifiable rather than
  false. A reviewer with the repository would score differently in a direction this run cannot
  predict.
- Temperature is the gateway's default, so the transcripts are a record and not a reproducible
  build. The replicate spread is large enough that a single re-run will not match them.
- Cost was not measured. `helpers/open-lane.sh` reads OpenRouter's credit balance around a lane
  and this harness does not, so what these 55 reviews cost is not recorded and the adopt case is
  missing its price.
