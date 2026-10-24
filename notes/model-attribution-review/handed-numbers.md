# The numbers this lane was handed do not reproduce

*Name: provisional. For the parent's "First: the numbers this lane was handed do not reproduce".
calef names files.*

An appendix of [reviewing the work of one model](../model-attribution-review.md). The parent keeps
the measured counts and the conclusion; this file keeps both tables and both derivations, so the
counts can be re-run.

## First: the numbers this lane was handed do not reproduce

The brief gave a table of commits per model. Measured on `origin/main` at `0b993b27`, it does not
come out that way, and the discrepancy is not rounding.

| Model | Handed to the lane | Measured here |
|---|---|---|
| Claude Opus 5 (1M context) | 658 | 623 |
| Claude Fable 5 | 360 | 338 |
| Claude Opus 4.8 | 233 | 233 |
| Claude Sonnet 5 | 181 | 180 |
| Claude Opus 5 | 35 | 32 |
| (no trailer) | 648 | 648 |
| total non-merge | 2054 | 2054 |

The total and the untrailered count agree exactly; four of the five per-model rows do not. The
handed per-model rows sum to 1467 against 1406 commits that carry a trailer at all, so that table
double-counts 61 commits somewhere. Nothing here reproduces it, and no commit in this tree carries
more than one `Co-Authored-By: Claude` line, which is the obvious way it could have happened.

Two derivations, checked against each other, both giving the measured column:

```sh
# via git's trailer parser
git log origin/main --no-merges \
  --format='%(trailers:key=Co-Authored-By,valueonly)' \
  | sed 's/ *<.*//' | grep '^Claude' | sort | uniq -c | sort -rn

# via the raw message, in case the trailer block is malformed somewhere
git log origin/main --no-merges --format='%H%x00%B%x00%x00' | python3 -c '
import sys,re
for rec in sys.stdin.read().split("\x00\x00"):
    p=rec.split("\x00")
    if len(p)>1: print(*set(re.findall(r"(?im)^\s*Co-Authored-By:\s*(Claude[^<\n]*?)\s*<",p[1])) or ["(none)"])
' | sort | uniq -c | sort -rn
```

This is milestone 125 (a number in the prose is a claim) arriving uninvited. The point is not that
someone miscounted. It is that the first number in a study about trustworthiness was itself
unverified, which is exactly the failure the study would be looking for.
