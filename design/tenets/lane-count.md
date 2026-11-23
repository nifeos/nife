# What bounds lane count: the collision surface, the memory, and the disk

*Appendix to `nifeos/nife:AGENTS.md`, which carries the rules for launching lanes and pruning
their worktrees. This file carries the measurement that overturned the old queue-depth rule, and the
three ceilings with the days each one was learned. The commands for a merge and its cleanup are in
`nifeos/nife:briefs/merge-and-cleanup.md`; the queue machinery is in
[`notes/merge-queue.md`](../../notes/merge-queue.md). Moved here 2026-09-23 (UTC) on calef's
authorization, unchanged in substance.*

**Lane count is set against the collision surface, not against queue depth** (calef, 2026-08-16,
overturning his own 2026-08-04 delegation on measured evidence). The reason survives its own rule:
throughput is measured in merged work. What changed is what bounds it.

The old rule counted open pull requests, and was right when it was written: under
require-branches-up-to-date a serial drain landed one thing at a time, every merge staled every
other branch, and lanes past that rate manufactured merge debt. GitHub's merge queue, which is
milestone 119 (the merge queue is the bottleneck), enabled 2026-08-15, retired both facts. It lands
groups of up to five in one CI run and rebases the group itself, so a deep queue is a batch
rather than a backlog, and depth stopped predicting anything.

The measurement that overturned it. In the twenty hours after the queue went live: 28 merges
through 8 group builds, with four to five lanes running against a queue that sat six to twelve deep
for most of it. The old table would have prescribed one lane for nearly all of that night. Of the
four things that actually stalled the queue, three do not scale with lane count at all. A lint
rejected GitHub's own synthetic branch names and so failed every group build (one bug, fixed).
GitHub does not auto-retry evictions (queue behavior, and the operator must re-enqueue). Seven pull
requests failed `rustfmt` (now caught by the pre-push hook). Only the fourth scales, and it scales
through files rather than numbers: four merge conflicts, every one of them in the same small hotspot
where every lane wires its test (`kernel/src/user/tests.rs`, the QEMU runners, `xtask/src/main.rs`).
Lanes in disjoint subsystems collided zero times.

So the question to ask before launching is not "how deep is the queue" but "what files will this
lane touch, and who else is in them". Concretely:

- Disjoint subsystems: launch freely. Four is a reasonable working number, not a ceiling.
- Two lanes in the test-wiring hotspot: expect to resolve a conflict by hand, and brief the
  second one to fold into the first's shape rather than inventing a third. It is often cheaper to
  sequence those two than to merge them.
- The real ceilings are elsewhere. Name them so they are decided rather than discovered: the
  attention to read reports and resolve conflicts, the token budget, and runner concurrency, since
  group builds and per-branch CI compete for the same machines.

And there is a second ceiling, which is the machine and not the collision surface (2026-08-31,
learned three times in one day). Lane count is bounded by files; concurrent heavy jobs are bounded
by memory, and the two are independent. `script/verify` defaults to four parallel solver jobs
because its own header records a harness reaching 3.5 GB and four fitting the dev Mac, so two
lanes gating at once is eight solvers against a budget tuned for four. That day cost an
out-of-memory kill that took the session with it, a `ci-build` whose timing assertions failed at
2.7x oversubscription and told nobody anything, and two `script/verify` runs killed by SIGTERM
mid-CBMC.

**So: at most one full `script/verify` at a time on this machine, and never a mutation sweep beside
lanes.** When two lanes must gate together, `VERIFY_JOBS=2` each shares the budget rather than
doubling it. The tell is the same every time and is easy to misread: a heavy job dying with no
failing assertion, reported as a cancellation or a timing failure rather than as memory.

And a third ceiling, which was disk and is no longer the binding one (2026-09-14, met four times
in one session; repriced 2026-09-22). Five lanes at roughly 3 GB of `target/` each, plus 7.2 GB in
the main checkout's own, took a 252 GB volume to 1.9 GB free and then to a command failing
mid-write with `No space left on device`. Pruning fixed it, and what decides lane count now is the
memory and cores above. So the lever moved with it. Lanes gate in CI rather than here
(`briefs/gate-in-ci.md`), which takes QEMU and `script/verify` off this machine and unbinds lane
count from 16 GB. The trade is wall-clock, 23 to 29 minutes a round. Lanes are asynchronous, so it
costs nothing a person waits for. Two habits survive the repricing. Run any local gate from a lane's
worktree rather than the main checkout, the build tree nobody watches because it never appears in
`git worktree list`. And prune promptly, because disk is still the only pressure here that destroys
work rather than delaying it, and deletes keep succeeding while writes fail.

The prover is the queue's long pole, not the queue itself: a group's CI goes green while
`verify` is still running, every time. Milestone 119 (the merge queue) measured that and is BUILT.

**Prune a lane's worktree the moment its pull request merges**, and never prune one with uncommitted
work in it. Those are the two clauses that have to be known before the cleanup starts.
`briefs/merge-and-cleanup.md` has the commands, the order, and both recorded failures (eight
finished worktrees, one at 3.3 GB; and 2026-07-31's zero bytes free with 42 worktrees holding 78 GB,
which killed two lanes mid-work).
