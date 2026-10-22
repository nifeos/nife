---
status: NOT-STARTED
raised: 2026-09-17
promoted_from: what-the-weekly-miri-run-should-cost
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 428. What the weekly Miri run should cost

Promoted from the proposal `what-the-weekly-miri-run-should-cost`, filed
2026-09-17 by milestone 310's lane, on its first finished run's measurement. The cadence itself is
an architect's call; the measurement and the cut are not. *(Number provisional until the merge
queue lands it.)*

A lane can start today and get most of the value. Collecting per-crate wall clocks
and sampling the crates that dominate are `cfg(miri)` gates at the test site, the convention five
crates in this tree already follow, and neither needs a ruling. Only the last step is an
architect's, changing the weekly cadence or tightening `timeout-minutes`, and it is the cheapest
step of the three. Nothing is blocked meanwhile: the job is green as of milestone 310 (the Miri leak
check stays on) and finishes inside its current budget.

Premise re-checked 2026-09-19 and still true.
`.github/workflows/undefined-behavior-check.yml` still runs on `cron: "0 6 * * 1"` with
`timeout-minutes: 240`, and nothing in the tree collects a per-crate wall clock: the number this
block says is missing is still missing, which is what makes the argument below an argument rather
than a decision.

## The question the workflow asks and nobody has answered

`.github/workflows/undefined-behavior-check.yml`'s header says its 240-minute budget is "deliberately
generous rather than tuned", that "the honest current cost is not yet known", and asks for it to be
tightened "once `compositor` is sampled and a full run has been timed end to end". Neither had
happened, for the reason milestone 310 fixed: the job had never once succeeded, so nothing had ever
reached the end to be timed. A check that has been failing is also not measuring, and the header says
that too.

## The measurement

`script/undefined-behavior-check` on patagonia (Apple Silicon), full sampled workspace, 2026-09-17:

```
5429.38s user  18.55s system  47% cpu  3:09:41.64 total
[exited with code 0]
```

And in CI, `workflow_dispatch` on `milestone/310-miri-leak-free`, run 35256118545 on
`ubuntu-24.04-arm`: success in 2:58:13.

About three hours, on both machines. The 47% CPU is the load-bearing half of the local line:
most of this is one interpreter thread, so more cores do not buy the wall clock back. That the two
very different machines agree within eleven minutes is what makes three hours the job's cost rather
than one laptop's, and it means the 240-minute budget has about twenty percent of headroom.

## What the answer probably is, and what has to be measured before it is one

The cost is concentrated, not spread, which is why "run it less often" is the wrong lever and
"run less of it" is the right one. The evidence already in the tree:

- `serial_console` alone was **55 minutes** against roughly four for the whole rest of the workspace,
  and is excluded for exactly this reason. It contains no `unsafe` and no dependencies.
- `compositor`'s six full-screen sweeps are 317,856 pixels each; one was measured at over 44 minutes
  without finishing and is now strided, and the header calls the other five a milestone of their own.
- `globally_unique_identifier_partition_table`, `glob`, `calendar`, `credentialer` and `network_time_protocol` already gate their exhaustive
  sweeps down under `cfg(miri)`, and "Miri-clean means the sampled paths" already covers the posture.

So the pattern is that the wall clock is dominated by crates whose expensive tests are *breadth over
in-memory input*, which is precisely the thing Miri cannot judge. Sampling those the way five crates
already do costs nothing Miri could have told us, and the class of bug it does catch moves at the
pace of `unsafe` blocks rather than of test-vector counts.

**What is missing is the per-crate number.** Nothing here measured which crates dominate *today*;
the three facts above are from 2026-08 and one of them (`compositor`) has been partly addressed
since. `cargo miri test` prints per-target timings already and nobody has collected them. That is one
instrumented run and a table, and it turns the paragraph above from an argument into a decision.

## The recommendation, and it is a reversible one

1. Collect per-target wall clocks from one instrumented run. A lane, half a day, no coordination.
2. Sample the crates that dominate under `cfg(miri)`, at the site, with the reason beside the test,
   the way `globally_unique_identifier_partition_table` and `calendar` already do.
3. **Keep the weekly cadence** and tighten `timeout-minutes` to the measured figure plus headroom.

Against the alternative of dropping to monthly or on-demand: this job went five weeks red without
anyone noticing, and a longer interval makes that detection problem strictly worse. `script/cadence-check`
watches for the job going *quiet*, not for it going *red*. A cheap weekly check is worth more than an
honest monthly one.

## What is blocked until it is answered

Nothing. The job is green as of milestone 310 and the 240-minute budget accommodates the measured
run, so this is optimization rather than repair. It is written down because it is a question the
workflow's own header raises, and a question that lives only in a lane's report is in the medium
AGENTS.md abolished.

## Index row

`.github/workflows/undefined-behavior-check.yml` says in its own header that its 240-minute budget
is deliberately generous rather than tuned and asks to be tightened once a full run has been timed,
which nothing had ever done, because the job had never once succeeded. Milestone 310 fixed that and
measured it: about three hours, 3:09:41 on patagonia and 2:58:13 on `ubuntu-24.04-arm`, at 47% CPU
because most of it is one interpreter thread. So more cores buy no wall clock back and two very
different machines agreeing within eleven minutes makes three hours the job's cost rather than one
laptop's. The cost is concentrated rather than spread, which is why running it less often is the
wrong lever and running less of it is the right one. `serial_console` alone was 55 minutes against
four for the rest of the workspace, and five crates already gate their exhaustive sweeps down under
`cfg(miri)`. What is missing is the per-crate number, which `cargo miri test` prints already and
nobody has collected, and which turns the argument into a decision.
