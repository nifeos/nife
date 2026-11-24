# helpers/needs-maintainer.jq: which pull requests a maintainer session must pick up, and why.
#
# Milestone 727 (a queue eviction goes to a maintainer session), number provisional. calef's ruling
# on #1564, 2026-10-03 UTC: "I don't want the job of watching the queue." That day he was the
# queue's only working detector, while the drain re-armed and re-queued on his behalf and commented
# where nobody looked. So the drain stopped acting on the queue, and this decides the one thing it
# still owes: a label, `needs-maintainer` (name provisional), on a pull request a maintainer
# session must pick up, so the session finds it in the `needs-maintainer` listing
# (briefs/session-start.md) and not by watching. Nothing here arms, enqueues or re-queues; it only names.
#
# Fifteen causes, each a fact nothing else reports to anyone:
#
#   ejected   the last removal from the queue was neither `merged` nor `manual`, nothing put it
#             back since, and the head is still the one that was ejected. A removal whose event
#             names no group commit (a `merge_conflict`) cannot say which head it was, so it holds
#             until the conflict is gone instead. `manual` is excluded because a person, or the
#             drain's own `dequeue_held`, took it out on purpose; the `unarmed` cause catches it 30
#             minutes later if nobody follows up.
#   conflict  ready, and `mergeable` is CONFLICTING against `main`.
#   stale     a merge-queue entry whose pull request is no longer open. #1555 had one on
#             2026-10-03: enqueued in the same second it merged, and removed by hand 15 minutes
#             later. A closed pull request is never in `gh pr list`'s default listing, which is why
#             the session's command says `--state all`.
#   unarmed   ready, not armed, not in the queue, with no `CI` or `verify` run queued or in progress
#             on its head, for `$minutes` (30, calef's figure) since the
#             latest of: it was opened, it was marked ready, auto-merge was disabled, it left the
#             queue, its last `CI` or `verify` run finished. A lane arms in the same command as `gh pr ready`, so this is the one that
#             forgot. Not raised beside `ejected` or `conflict`, which already say what is wrong,
#             nor while a `Blocked-by:` pull request is still open, which is an unarmed pull
#             request waiting on purpose.
#   off-main  ready, not armed, and based on a branch other than `main`, for `$minutes` since it
#             was opened or marked ready. The merge queue drains `main` only, so nothing will ever
#             merge this, and `eligible` (below) never shows it to the other causes. #1640 on
#             2026-10-04: stacked on #1630's branch, green and mergeable, and labeled by nothing
#             for about three hours until calef merged it into its base by hand. A `Blocked-by:`
#             pull request still open holds it, as it holds `unarmed`.
#   red       wearing `ci-failing` (helpers/ci-failing.sh decides that, from the required checks
#             on the head) for `$minutes` since the label last went on. That label is a report with
#             no reader: a lane that ended `WAITING` is not running, and a session's queue is this
#             label, so #1617 and #1653 sat armed and red on 2026-10-04 until calef pointed at them.
#             Armed or not, any base: an armed pull request whose checks fail never enters the
#             queue, so nothing else here would see it. A push clears `ci-failing` until its checks
#             fail, and a fresh label starts a fresh episode.
#   stale-draft  a draft from this repository whose head commit is `nm_stale_draft_hours` (6,
#             calef's figure, 2026-10-05) old by its committer date. Not `updatedAt`, which every
#             bot comment and every retarget bumps. #1644 had no commit after 2026-10-04 22:11 UTC
#             once its lane's session ended, and every other cause skips a draft. A draft holding
#             only its claim commit is not exempt: that is the clearest case of a lane that died.
#             An open `Blocked-by:` holds it, as it holds `unarmed`, and so does `nm_parked_label`.
#   unmergeable  an open pull request whose merge-queue entry reads `UNMERGEABLE`: GitHub cannot
#             build it on top of the entries ahead of it, usually because it conflicts with one of
#             them rather than with `main`, so `conflict` never fires. The queue does not eject such
#             an entry; it leaves it waiting. #1795 sat like that behind #1745 on 2026-10-07 (UTC),
#             conflicting in partsfoundry/record:notes/package-boundaries.md, until calef found it on the queue page.
#             Keyed on the pull request's head, so one comment per head and a new one after a push
#             that does not fix it.
#   orphan   a branch holding work that no open pull request carries, for `nm_orphan_hours` (2,
#             the 2026-10-06 brief's figure) by its tip's committer date. calef, 2026-10-06 UTC: "It
#             seems like we have some work trees that don't have PRs. That seems like a problem that
#             leads to lack of visibility and progress on that work." Keyed on remote branches,
#             because they are the ledger every session can see. A branch is in scope unless it is
#             `main` or a `gh-readonly-queue/*` candidate, and it holds work when its tip is not an
#             ancestor of `main` (`compare.behindBy`, the commits it has that `main` lacks). Three
#             shapes, each labeled where a session already looks:
#               - its last pull request merged, and commits were pushed past the merged head:
#                 the merged pull request, closed, wears the label (`--state all` finds it);
#               - its last pull request closed without merging: the closed one wears it;
#               - it never had a pull request: there is nothing to label, so the decision says
#                 `adopt` and the shell opens a draft for it, marked with `nm_orphan_marker` and the
#                 head it adopted. That draft carries the cause until its head moves, it is made
#                 ready, or it closes (after which the closed shape applies to the branch).
#             `parked` (`nm_parked_label`) on the last pull request exempts all three, as it
#             exempts `stale-draft`: a branch kept on purpose, such as argon's waiting for a board
#             (#1738, #1732). Deleting the branch, landing it, or opening a pull request for it
#             takes the label off. The 2026-10-06 survey that prompted this is in #1787.
#   missing-check  armed, ready, and a required status check of the `main` ruleset has no check run
#             at all at the head, although every pull-request workflow run there has finished and the
#             drain has already rerun the owning workflow once (helpers/missing-check.jq decides that
#             half; the shell splices its verdict into the node as `missingChecks`). #1814 on
#             2026-10-07 (UTC): CI finished `failure` with every job green, and the required cpu
#             matrix job was never created. Keyed on the head, so one comment per head.
#   stuck-head  a merge-queue entry that reads `AWAITING_CHECKS` although every workflow run of its
#             merge group has completed, and the last of them did so `nm_stuck_head_minutes` (10)
#             ago. #1824 sat at the head of the queue from 16:38 to 19:42 UTC on 2026-10-07: its
#             runs were done by 17:01, CI and verify `failure` with every listed job green, so a
#             required check never posted and GitHub neither merged nor ejected it, and everything
#             behind it waited until the maintainer dequeued it by hand. The shell splices each
#             `AWAITING_CHECKS` entry's verdict (helpers/stuck-head.jq) into the entry as
#             `stuckHead`. **The one cause the drain acts on.** calef ruled on #1833 (2026-10-07 UTC)
#             that the drain recovers a stuck head itself, a narrow exception to #1564: the FIRST
#             stall of a pull request is `stuck-head-requeue`, which carries no label and makes the
#             shell dequeue the entry and enqueue it again at the back. A SECOND stall (the drain's
#             own earlier `needs-maintainer:stuck-head` comment is on the pull request) is plain
#             `stuck-head`: label and comment, no re-queue. Keyed on the group's head commit, so
#             every stall gets its own comment. Do not extend this to another cause.
#   hold-no-ask, ask-no-hold, surface-no-hold   the architect queue's invariant broken
#             (lane/architect-queue, 2026-10-06; definitions above `nm_decide`): `needs-architect`
#             with no open question, an open question without it, and a diff the architect-label
#             rules fire on with no hold or ruling label. They apply to drafts and held items too,
#             and the first two to open issues as well as pull requests.
#   budget   an open issue wearing `nm_budget_label`: a CI job's merge-group runs have passed about
#             85% of its milestone 721 (each merge-group CI job has a 20-minute budget) budget, and
#             `helpers/ci_job_times.py sync` opened the issue (milestone 808 (every gate accounts for
#             its time), §254 (a gate prints what each item cost)). calef ruled on 2026-10-06 (UTC)
#             that the issue must reach a session through this listing, because a label nothing
#             queries is rung zero. Keyed on the issue's number and the time it was opened, so one
#             comment per episode; the issue closing (the job back under the line) clears the label.
#
# `ejected`, `conflict` and `unarmed` apply only to what `eligible` admits (helpers/queue-eligible.jq,
# spliced in front of this file); `off-main` and `red` to a ready pull request from this repository
# on any base; `stale-draft` to a draft from this repository on any base. None of those applies to
# one held with `needs-architect` or `held-for-red-trunk`, and only `stale` and `stale-draft` apply
# to a draft: a draft is its lane's while the lane is alive (converting an ejected pull request to a
# draft is how a lane says it has it), a fork's pull request is a person's decision, and a held one
# is waiting on purpose. `stale` and `unmergeable` read the queue, not the pull request, and apply to
# whatever entry is there.
#
# A cause carries `key`, which names the episode: the ejection's time, the conflicting head, the
# entry's enqueue time, the unmergeable head, the moment it was last unarmed or made ready, the
# moment `ci-failing` went on, the draft's last commit. The shell posts one comment per cause per
# key, deduplicated by a marker in the comment, so a persisting cause never comments twice and a
# new episode does.
#
# Input: the GraphQL response the drain asks for (helpers/merge-drain.sh, `nm_query`), and:
#   $label     the label's name
#   $now       epoch seconds
#   $minutes   the grace before `unarmed`, `off-main` or `red`
#   $blockers  { "<number>": ["OPEN", "MERGED", ...] }, the states of the pull requests a ready
#              pull request's `Blocked-by:` line names (the shell resolves them; this file must
#              not call `gh`)
#
# Output, one object per pull request that has a cause or wears the label:
#   { number, kind: "pr" | "issue", action: "label" | "keep" | "clear" | "hold", hold: bool,
#     causes: [ { cause, key, ... } ] }
# `hold` true means the shell adds `needs-architect` (the architect causes, below).
# and one per orphan branch that never had a pull request, for the shell to open a draft for:
#   { number: null, branch, action: "adopt", causes: [ { cause: "orphan", ... } ] }
#
# helpers/needs-maintainer-selftest.sh feeds recorded responses through this; script/lint-fleet runs it.

# The label `helpers/ci_job_times.py sync` puts on a job's tracking issue (milestone 808). The name
# is provisional; that script holds the same string and the selftest checks they agree.
def nm_budget_label: "near-budget";

# An open tracking issue for a job near its budget. Closed ones carry no cause, so a closed issue
# still wearing `needs-maintainer` (found by the search for labeled, closed items) is cleared.
def nm_budget:
  select(([.labels.nodes[]?.name] | index(nm_budget_label)) != null and (.state // "OPEN") == "OPEN")
  | { cause: "budget", key: "\(.number)@\(.createdAt)", opened: .createdAt, url: (.url // null),
      title: (.title // null) };

# `held-by-lane` is the hold a send-back places: calef has ruled, and the lane owes a change.
def nm_held: ["needs-architect", "held-by-lane", "held-for-red-trunk"];

def nm_ts: if . == null or . == "" then 0 else fromdateiso8601 end;

def nm_unheld:
  ([.labels.nodes[].name]) as $names
  | select(nm_held | map(. as $h | $names | index($h)) | all(. == null));

def nm_ready_unheld: eligible | nm_unheld;

# Ready, from this repository, unheld, on any base: `eligible` without its `main` clause, for the
# two causes that exist because something is not on its way to `main`'s queue.
def nm_ready_unheld_any_base:
  select(.isDraft == false and .isCrossRepository == false) | nm_unheld;

# The label helpers/ci-failing.sh puts on a red head. Its name is that script's, provisional there.
def nm_red_label: "ci-failing";

# How long a draft's head may sit without a commit before its lane is presumed gone.
def nm_stale_draft_hours: 6;

# A draft held on purpose for work outside the lane system, such as calef's GLM runs (#1745,
# 2026-10-06), or a closed pull request whose branch is kept on purpose (#1738). It exempts
# `stale-draft` and `orphan`, and the pull request must carry a comment giving the reason it is
# parked. The name is provisional.
def nm_parked_label: "parked";

# How long a branch may hold work no open pull request carries, by its tip's committer date. A tip
# clock, not a birth clock: a lane still pushing is never called an orphan, and one that stopped is
# called one two hours later. helpers/merge-drain.sh's `needs_maintainer` header has its BUGS.
def nm_orphan_hours: 2;

# The line the shell writes into a draft it opened for an orphan branch, followed by the head it
# adopted and ` -->`. The draft keeps the `orphan` cause only while its head is still that one.
def nm_orphan_marker: "<!-- needs-maintainer:orphan adopted ";

def nm_parked($labels): ($labels | index(nm_parked_label)) != null;

# Every in-scope branch with work past `main`, no open pull request and an old enough tip, with the
# last pull request it had (or null). A ref whose comparison GitHub could not make reads as no work.
def nm_orphan_branches($label; $now):
  .data.repository.refs.nodes[]?
  | select(.name != "main" and (.name | startswith("gh-readonly-queue/") | not))
  | ([.associatedPullRequests.nodes[]?] | sort_by(.number)) as $prs
  | select(($prs | map(select(.state == "OPEN")) | length) == 0)
  | select((.compare.behindBy // 0) > 0)
  | select((.target.committedDate | nm_ts) <= $now - nm_orphan_hours * 3600)
  | ($prs | last) as $last
  | select($last == null or (nm_parked([$last.labels.nodes[]?.name]) | not))
  | { number: ($last.number // null),
      labeled: ($last != null and ([$last.labels.nodes[]?.name] | index($label)) != null),
      causes: [ { cause: "orphan", key: .target.oid, branch: .name, head: .target.oid,
                  since: .target.committedDate, ahead: .compare.behindBy,
                  shape: (if $last == null then "never" else $last.state end) } ] };

# A draft the shell opened for an orphan branch, still at the head it adopted.
def nm_orphan_adopted:
  . as $pr
  | select(.isDraft == true and .isCrossRepository == false)
  | nm_unheld
  | select(nm_parked([.labels.nodes[].name]) | not)
  | ((.body // "") | capture("<!-- needs-maintainer:orphan adopted (?<oid>[0-9a-f]{40}) -->").oid) as $oid
  | select($oid == $pr.headRefOid)
  | { cause: "orphan", key: $oid, branch: .headRefName, head: $oid,
      since: (.commits.nodes[-1].commit.committedDate // null), shape: "adopted" };

# Armed, and the drain found a required check absent and has already rerun (or cannot rerun) the
# workflow that owns it. `missingChecks` is not a GraphQL field: helpers/merge-drain.sh splices it
# in from helpers/missing-check.jq. A rerun still in flight is `tried: false`, so no cause yet.
def nm_missing_check:
  select(.isDraft == false and .autoMergeRequest != null and .missingChecks != null
         and .missingChecks.tried == true and (.missingChecks.absent | length) > 0)
  | { cause: "missing-check", key: .headRefOid, head: .headRefOid, absent: .missingChecks.absent };

def nm_ejected($queued):
  . as $pr
  | nm_ready_unheld
  | (.removed.nodes[-1] // null) as $r
  | (.added.nodes[-1].createdAt // "") as $added
  | ($r.beforeCommit.parents.nodes[1].oid // null) as $was
  | select($r != null and $r.reason != "merged" and $r.reason != "manual"
           and $added < $r.createdAt and ($queued | index($pr.number) | not))
  | select(if $was == null then $pr.mergeable != "MERGEABLE" else $was == $pr.headRefOid end)
  | { cause: "ejected", key: $r.createdAt, reason: $r.reason, at: $r.createdAt,
      group: ($r.beforeCommit.oid // null), head: $pr.headRefOid };

def nm_conflict:
  nm_ready_unheld
  | select(.mergeable == "CONFLICTING")
  | { cause: "conflict", key: .headRefOid, head: .headRefOid };

# The workflows whose runs a ready pull request waits on before it can be armed (briefs/gate-in-ci.md:
# push, mark ready, CI runs, arm once green). That wait is 25 to 47 minutes, longer than the grace, so
# the grace counts only once none of them is queued or running on the head, and restarts when the
# last one finishes. The head's check runs
# come from the query's `commits(last: 1)`; a fixture or response without them reads as idle.
def nm_gating_workflows: ["CI", "verify"];
def nm_gating_runs:
  [ (.commits.nodes[-1].commit.statusCheckRollup.contexts.nodes // [])[]
    | select(.__typename == "CheckRun"
             and (.checkSuite.workflowRun.workflow.name as $w | nm_gating_workflows | index($w) != null)) ];
def nm_ci_in_flight: nm_gating_runs | any(.status != "COMPLETED");
# When the last gating run finished, or null. The grace restarts there, so a pull request whose CI
# ended at minute 40 gets its 30 minutes to be armed, not none.
def nm_ci_idle_since: [ nm_gating_runs[] | .completedAt | select(. != null) ] | max;

def nm_unarmed($queued; $now; $minutes; $blockers):
  . as $pr
  | ([.createdAt, (.unarmed.nodes[-1].createdAt // null), nm_ci_idle_since] | map(select(. != null)) | max) as $since
  | ($blockers[$pr.number | tostring] // []) as $bs
  | nm_ready_unheld
  | select(.autoMergeRequest == null and ($queued | index($pr.number) | not))
  | select(($bs | index("OPEN")) == null)
  | select(nm_ci_in_flight | not)
  | select(($since | nm_ts) <= $now - $minutes * 60)
  | { cause: "unarmed", key: $since, since: $since, blockers: $bs };

def nm_off_main($now; $minutes; $blockers):
  . as $pr
  | ([.createdAt, (.unarmed.nodes[-1].createdAt // null)] | map(select(. != null)) | max) as $since
  | ($blockers[$pr.number | tostring] // []) as $bs
  | nm_ready_unheld_any_base
  | select(.baseRefName != "main" and .autoMergeRequest == null)
  | select(($bs | index("OPEN")) == null)
  | select(($since | nm_ts) <= $now - $minutes * 60)
  | { cause: "off-main", key: $since, since: $since, base: $pr.baseRefName, branch: $pr.headRefName };

# When the label went on: the newest LabeledEvent for it among the ones the query fetched. A label
# older than that window reads as the pull request's creation, which is long enough ago to count.
def nm_red($now; $minutes):
  . as $pr
  | nm_ready_unheld_any_base
  | select([.labels.nodes[].name] | index(nm_red_label) != null)
  | ([.labelEvents.nodes[]? | select(.label.name == nm_red_label) | .createdAt] | max // $pr.createdAt) as $since
  | select(($since | nm_ts) <= $now - $minutes * 60)
  | { cause: "red", key: $since, since: $since, head: $pr.headRefOid, branch: $pr.headRefName,
      armed: ($pr.autoMergeRequest != null) };

def nm_stale_draft($now; $blockers):
  . as $pr
  | ($blockers[$pr.number | tostring] // []) as $bs
  | (.commits.nodes[-1].commit.committedDate // null) as $at
  | select(.isDraft == true and .isCrossRepository == false and $at != null)
  | nm_unheld
  | select([.labels.nodes[].name] | index(nm_parked_label) == null)
  | select(($bs | index("OPEN")) == null)
  | select(($at | nm_ts) <= $now - nm_stale_draft_hours * 3600)
  | { cause: "stale-draft", key: $at, since: $at, head: $pr.headRefOid, branch: $pr.headRefName, blockers: $bs };

# The three architect causes. calef's queue is `needs-architect`, and it is complete and honest only
# while that label sits on exactly the items with an open question (helpers/open-question.jq, spliced
# in front of this file, decides "open"). Each cause is one way the invariant breaks; none is held
# by another label, because a held pull request is exactly where a stale or missing hold hides.
# Each waits `$minutes` after the fact that started it, so a lane that labels and then asks, or
# asks and then labels, is not caught between the two.
#
# The repair is asymmetric, by calef's ruling on #1792, 2026-10-06 (UTC): "Approve: the bot adds
# labels itself and only flags removals." `ask-no-hold` and `surface-no-hold` set `hold`, and the
# shell ADDS `needs-architect` itself, without `needs-maintainer`: a wrong add costs calef a glance.
# `hold-no-ask` only flags: a wrong removal could let a pull request merge without his ruling, which
# cannot be undone, so a maintainer checks before the label comes off. Nothing here ever removes
# `needs-architect`; helpers/needs-maintainer-selftest.sh fails if the drain learns to.

# When `needs-architect` last went on: the newest LabeledEvent the query fetched, else creation.
def nm_architect_since:
  ([.labelEvents.nodes[]? | select(.label.name == "needs-architect") | .createdAt] | max) // .createdAt;

# `needs-architect`, and no open question: the item sits in calef's queue with nothing to answer,
# because a ruling was recorded without the label coming off (#1783) or nobody ever asked.
def nm_hold_no_ask($now; $minutes):
  select(oq_has_label("needs-architect") and oq_open == null)
  | nm_architect_since as $labeled_at
  | select(($labeled_at | nm_ts) <= $now - $minutes * 60)
  | { cause: "hold-no-ask", key: (oq_last_ruling // $labeled_at), since: $labeled_at, ruled: oq_last_ruling };

# An open question, and no `needs-architect`: calef's queue cannot show it.
def nm_ask_no_hold($now; $minutes):
  oq_open as $ask
  | select($ask != null and (oq_has_label("needs-architect") | not))
  | select(($ask.at | nm_ts) <= $now - $minutes * 60)
  | { cause: "ask-no-hold", key: $ask.at, at: $ask.at, url: $ask.url, line: $ask.line };

# A diff helpers/architect-label-rules.py fires on, and no hold or ruling label: the labeler did not
# run or did not finish. #1745 conflicted with `main`, and GitHub runs no `pull_request` workflow on
# a conflicting pull request, so its system call change was never read. `architectSurface` is not a
# GraphQL field: the shell runs the rules on each head and splices their lines into the node.
def nm_surface_no_hold:
  select(((.architectSurface // []) | length) > 0)
  | select((oq_has_label("needs-architect") or oq_has_label("architect-ruled") or oq_has_label("held-by-lane")) | not)
  | { cause: "surface-no-hold", key: .headRefOid, head: .headRefOid, rules: .architectSurface };

# The causes the drain repairs by adding `needs-architect` (calef, #1792; see above).
def nm_hold_causes: ["ask-no-hold", "surface-no-hold"];
# Causes the shell answers by acting on the queue, not by labeling (calef, #1833). One, by design.
def nm_requeue_causes: ["stuck-head-requeue"];

def nm_architect($now; $minutes): [nm_hold_no_ask($now; $minutes)] + [nm_ask_no_hold($now; $minutes)];

# An entry's pull requests ahead of it, by `position` (1 is the front), with their heads so the
# shell can say which one a head conflicts with.
def nm_unmergeable($entries):
  . as $e
  | select(.pullRequest.state == "OPEN" and .state == "UNMERGEABLE")
  | { number: .pullRequest.number, labeled: false,
      causes: [ { cause: "unmergeable", key: .pullRequest.headRefOid, head: .pullRequest.headRefOid,
                  entry: .state, position: .position, enqueued: .enqueuedAt, id: .pullRequest.id,
                  ahead: [ $entries[] | select(.position < $e.position)
                           | { number: .pullRequest.number, head: .pullRequest.headRefOid } ] } ] };

# GitHub merges or ejects a finished group within 0.2 to 0.6 minutes (four merges, 2026-10-07 UTC),
# so ten is some seventeen times the slowest measured latency and still about ten minutes against
# 3 hours lost on #1824. The clock starts at the last run's completion, never at the group's start.
def nm_stuck_head_minutes: 10;

# `$prs` is the open pull requests, for the drain's earlier comments: a marker is the only record
# that a re-queue was already tried, and it survives the entry, the group and the drain's own state.
def nm_stuck_head($now; $prs):
  . as $e
  | select(.pullRequest.state == "OPEN" and .state == "AWAITING_CHECKS" and .stuckHead != null
           and (.stuckHead.completedAt | nm_ts) <= $now - nm_stuck_head_minutes * 60)
  | ([ $prs[] | select(.number == $e.pullRequest.number) | .comments.nodes[]?
       | select(.body | contains("needs-maintainer:stuck-head")) ] | length) as $prior
  | { number: .pullRequest.number, labeled: false,
      causes: [ { cause: (if $prior == 0 then "stuck-head-requeue" else "stuck-head" end),
                  key: .headCommit.oid, head: .pullRequest.headRefOid,
                  group: .headCommit.oid, position: .position, enqueued: .enqueuedAt,
                  completed: .stuckHead.completedAt, runs: .stuckHead.runs, id: .pullRequest.id } ] };

def nm_decide($label; $now; $minutes; $blockers):
  .data as $d
  | [$d.repository.mergeQueue.entries.nodes[]? | .pullRequest.number] as $queued
  | [nm_orphan_branches($label; $now)] as $orphans
  | ( [ $d.repository.pullRequests.nodes[]
        | . as $pr
        | ([nm_ejected($queued)] + [nm_conflict] + [nm_red($now; $minutes)] + [nm_missing_check]) as $hard
        | { number, labeled: ([.labels.nodes[].name] | index($label) != null),
            causes: ($hard + (if $hard == [] then [nm_unarmed($queued; $now; $minutes; $blockers)]
                                                + [nm_off_main($now; $minutes; $blockers)] else [] end)
                     + [nm_stale_draft($now; $blockers)] + [nm_orphan_adopted]
                     + nm_architect($now; $minutes) + [nm_surface_no_hold]) } ]
    + [ $d.repository.issues.nodes[]?
        | { number, kind: "issue", labeled: ([.labels.nodes[].name] | index($label) != null),
            causes: (nm_architect($now; $minutes) + [nm_budget]) } ]
    # Open budget issues by their own search, so one older than the fifty newest issues above is
    # still seen; the merge by number below folds the two sightings into one.
    + [ $d.budget.nodes[]? | select(.number != null)
        | { number, kind: "issue", labeled: ([.labels.nodes[].name] | index($label) != null),
            causes: [nm_budget] } ]
    + [ $orphans[] | select(.number != null) ]
    + [ $d.repository.mergeQueue.entries.nodes[]? | nm_stuck_head($now; $d.repository.pullRequests.nodes) ]
    + [ $d.repository.mergeQueue.entries.nodes[]?
        | select(.pullRequest.state != "OPEN")
        | { number: .pullRequest.number, labeled: false,
            causes: [ { cause: "stale", key: .enqueuedAt, enqueued: .enqueuedAt, entry: .state,
                        state: .pullRequest.state, merged: .pullRequest.mergedAt, id: .pullRequest.id } ] } ]
    + [ ($d.repository.mergeQueue.entries.nodes // []) as $es | $es[] | nm_unmergeable($es) ]
    + [ $d.search.nodes[]? | select(.number != null)
        | { number, kind: (if .__typename == "Issue" then "issue" else "pr" end),
            labeled: ([.labels.nodes[].name] | index($label) != null), causes: [] } ] )
  # A closed pull request with a stale entry also appears in the search for labeled ones, and an
  # open one can only appear once; merge by number, keeping every cause and any labeled flag.
  | group_by(.number)
  | map({ number: .[0].number, kind: (map(.kind // "pr") | first), labeled: (map(.labeled) | any),
           causes: (map(.causes[]) | unique_by(.cause)) })
  | map(select(.labeled or (.causes | length) > 0))
  # `hold`: the drain adds `needs-architect` itself. Those causes need no maintainer, so they do not
  # count toward `needs-maintainer`; an item with only them and no label gets action "hold".
  | map(. + { hold: (.causes | any(.cause as $c | nm_hold_causes | index($c) != null)),
              requeue: (.causes | any(.cause as $c | nm_requeue_causes | index($c) != null)),
              flags: (.causes | map(select(.cause as $c | (nm_hold_causes + nm_requeue_causes) | index($c) | not))) })
  | map(. + { action: (if (.flags | length) > 0 then (if .labeled then "keep" else "label" end)
                       elif .requeue then "requeue"
                       elif .labeled then "clear" else "hold" end) })
  | map(del(.labeled, .flags))
  | . + [ $orphans[] | select(.number == null) | { number, kind: "pr", branch: .causes[0].branch, action: "adopt", causes } ]
  | .[];
