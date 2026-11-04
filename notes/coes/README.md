# Corrections of error

*Name: provisional. `coes` and this directory were minted by the maintainer session on 2026-09-30
(UTC) for calef's commission; the abbreviation spells out above and is his to ratify or rename.*

When the machine or a sweep overrules how the tree was being run, the correction is recorded
here, one file per error, dated UTC. A correction follows §210 (a correction of error, and its
action items are decisions, proposals or milestones). It has what happened, one timeline with
times, the impact, and a five whys section rooted at the problem as the affected party saw it.
Where the first answer splits into independent causes, each branch is followed to its own fifth
why. Then the root cause, and the action items that replace it. Each why is answered from evidence.

The action items are one `## Action items` section, a bullet each, opening with the `## Follow-on`
vocabulary: `**Milestone N.**`, `**Proposed.**` naming its file, `**Decision.**` naming its file,
or `**Done.**`, `**Recorded.**` or `**Refused.**` with what carried it. `**None.**` is refused,
since an error with no mechanism under it is a prediction of its own recurrence.
`script/roadmap --check` fails an item that resolves to nothing, and `--coe-actions` lists the
open ones. `script/metrics` charts the open count each week so it can be driven down
(helpers/coe_actions.py, calef's review of #1513). An item is open while the milestone, proposal or
decision it names is not done.

An action item and its correction name each other (2026-11-04 UTC). A correction writes its action
items as one table, `| # | what | status | record |`, one row each. The status cell says
`delivered` or `proposed`, and the record cell opens with the vocabulary above and links the
proposal, milestone, pull request or note. Every proposal or milestone a row names carries, on a
line of its own under its title, `COE action item: notes/coes/<file>.md, action <N>.` A note or a
helper that a delivered item left behind carries the same line where it states the mechanism, and a
merged pull request carries it in a comment. `script/roadmap --check` holds the table to its
records in both directions for every file under `design/roadmap/`, and holds each status to what its
record resolves to. A correction still in bullets keeps only the first direction, because its
proposals predate the line. Notes, helpers and pull request comments have no check yet: a follow-up
could extend the same check to every tracked file that carries the line.

This index holds one line each; the detail lives in the files.

Part of the [notes index](../README.md), which says how to add a line.

- [2026-09-30: lane follow-through lived in memory](2026-09-30-lane-follow-through.md): five
  unexecuted maintainer endings in one session, and the PENDING-at-dispatch contract that replaced
  remembering them.
- [2026-10-03: the merge rate fell](2026-10-03-the-merge-rate.md): a CI job that doubled and a
  queue that ejected 43% of its entries slowed every merge for a week. Then a weekly usage limit
  stopped the work, about 190 merges were deferred, and nothing read either signal.
- [2026-10-03: the queue's automation fought itself, and calef watched the queue](2026-10-03-the-queue-judged-one-pull-request-at-a-time.md):
  the drain re-queued what the calef account dequeued, the pre-push hook outran the agents'
  timeouts, and 33 group builds failed on facts only the merged tree could make true.
- [2026-11-03: the merge queue drained slowly](2026-11-03-merge-queue-backlog.md): every push ran the
  heavy gates against a 60-job pool, and every naming pull request rewrote one note, which left 33
  ready pull requests waiting at the end of the Pacific day.
