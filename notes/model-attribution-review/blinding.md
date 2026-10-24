# Blinding the review, and the three leaks it cannot close

*Name: provisional. After the parent's "Blinding, and the honest statement that it leaks". calef
names files.*

An appendix of [reviewing the work of one model](../model-attribution-review.md). The parent keeps
the conclusion: blinding is partial, and only the same-window control makes it hold. This file keeps
the mechanism, the five-step scheme and the leaks.

Blinding is implementable and is the difference between a study and a confirmation exercise. The
mechanism already exists in this tree. `script/stranger-test` runs a separate `claude` process
rather than a subagent, with its working directory set to the *parent* of the tree. Project
instructions load from ancestors and never from descendants. And it probes the isolation before the
run rather than assuming it. Milestone 117 (the stranger test) also withholds its own answer key by
amending the note out of the tree for the run.

Applied here, the scheme is:

1. A harness extracts each sampled commit as `cases/<random-id>.patch`: the diff from `git show
   --format=`, which emits no message and therefore no trailer, plus the commit subject and body
   with every `Co-Authored-By` and `Claude-Session` line stripped.
2. Cases are written in randomized order under ids that encode nothing. The case directory holds the
   patches and the rubric and no git repository, so `git log` is not available to the reviewer.
3. The reviewer is a separate `claude` process started in the case directory's parent,
   `--safe-mode`, no network.
4. Isolation is probed before the run: the reviewer is handed one case and asked which model wrote
   it. The run proceeds only if the answer is that it cannot be determined.
5. Attribution is joined to scores only after every score is written and committed.

Three leaks that this does not close are large enough that the blinding must be called partial
in the report rather than claimed:

- **Milestone numbers.** Sonnet's commits cite milestones 158 to 184; the earlier models' cite lower
  numbers. Stripping them mutilates the message that the rubric is partly scoring.
- Content that only exists after a date. An x86_64 file, `CapabilityTable`, `login_contract`. The
  post-rename vocabulary alone identifies the window.
- The window is the model. Because the two are perfectly confounded, any leak of the date is a
  leak of the attribution. Blinding cannot do better than the confound allows.

Option 1's same-window control is the only configuration where blinding actually holds, because
there the cases are contemporaneous and none of the three leaks discriminate. That is a second,
independent reason to prefer it.
