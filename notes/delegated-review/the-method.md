# The method behind the delegated-review study

*Name: provisional. calef names files.*

An appendix to [Does a delegated AI review catch what the gates and the maintainer
miss](../delegated-review.md), the run of milestone 521 (does an AI review of a pull request catch
anything the gates and the maintainer do not). It holds how blindness was enforced, what the corpus
leaves out, and how the runs were budgeted. The page carries the result and the verdict.

## How blindness was enforced, since nothing else here matters if it was not

The reviewer was never told what it was looking for, was never told which arm it was in, and was
never given the study, the ledger of verdicts, or the brief that named the defects.

- The prompt came out of two scripts and nothing else. `helpers/review-bundle.sh` takes a
  worktree and a commit and emits `git show` plus each changed file's pre-image. It holds no table of
  defects and takes no argument that could tell it which commit is which, so it cannot leak an answer
  it does not have. `helpers/review-trial.sh` wraps that in one of two fixed paragraphs and posts it.
- Every prompt is reconstructible and every answer is committed verbatim, in
  [`transcripts.tar.gz`](transcripts.tar.gz), one file per model, diff, posture and replicate. The
  page says how to unpack it and why it is an archive. Re-run `helpers/review-bundle.sh` on the same
  commit and diff it against what the transcripts were answering.
- The same prompt went to defective and clean diffs alike, so nothing in the wording separates
  the arms. The two postures differ by one sentence, quoted in `helpers/review-trial.sh`.
- The adversarial posture does not assert that a defect exists. It says one *may* be hiding and
  that there may be none. Asserting one would have guaranteed false positives on the clean diffs and
  made the arm that decides this milestone meaningless.
- What the reviewer did not get is recorded because it bounds the result: the rest of the tree, the
  branch's sibling commits, and `AGENTS.md`. Two findings are marked unfalsifiable precisely
  because a single-commit bundle cannot show a reader that the code landed in an earlier commit on
  the same branch. A reviewer with the whole worktree would score differently, in an unknown
  direction.

Quotations in the study and its appendices are verbatim, on their own `>` lines, which
`script/lint`'s em-dash rule permits since 2026-09-22: a quotation is not the quoter's prose, so a
source's punctuation survives being quoted here. The reasoning is at the check. The archive still
holds the originals, because bulk evidence is not quotation and does not belong inline.

## What the corpus leaves out

The third defect row is not a separate diff. A second model made D1's replacement independently,
and the ledger records it as the same replacement. Its branch was reset and the commit does not
survive, so it is reported as a repeat of D1 rather than scored twice against an artifact that no
longer exists.

What D2 is missing bounds the arm. The ledger's verdict on D2 also records that the lane
skipped the tests its brief required. Nothing in a diff can show that, so only half of D2's recorded
failure was reviewable at all.

## How the runs were budgeted

Two models, four diffs, two postures, three replicates: 48 reviews, plus seven supplementary runs.
Replicates are not decoration. A pilot at a lower token cap got a substantive review out of the same
model and bundle that later answered `NO CONCERNS` three times in a row, and a single call would have
reported whichever one it drew.

One condition differs and it is recorded rather than hidden. At the uniform 16000-token budget,
`kimi-k3` spent the entire budget thinking on the largest diff and returned no answer at all in
all six cells. Those transcripts are kept with a header saying so, because scoring a model's silence
as "no concerns" is the easiest available way to make this experiment lie. They were re-run at 48000
tokens in `transcripts-48k/` and that is the set scored. `qwen3-coder` never exceeded 693 output
tokens on any cell, so the budget was never binding for it and its runs are unchanged.
