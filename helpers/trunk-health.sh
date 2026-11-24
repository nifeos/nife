#!/bin/sh
#
# Say when `main` is red, and say when it recovers.
#
#     helpers/trunk-health.sh             # watch until stopped
#     helpers/trunk-health.sh --once      # print the current state and exit
#     helpers/trunk-health.sh --repeat [--repo <owner>/<name>]
#                                         # print the repeat rows alone, for partsfoundry/nife or <repo>
#
# PROVISIONAL NAME. Minted 2026-08-04; not put to calef. See the `Name:` block below.
#
# # Why this exists
#
# Nothing owned trunk health, and the gap was in the role definitions rather than in anyone's
# execution. A developer works one milestone in one lane and cannot see `main` by design. The
# steward watches pull request checks, conflicts, at-risk work and idle lanes, and its charter never
# mentioned the trunk. The maintainer is told to keep hygiene, and that list is prune the worktree,
# delete the branch, relink `nife-dev`, leave no QEMU. So the role that merges is the role that
# breaks `main`, and the role that exists to compensate for the merger being busy was not pointed at
# the thing merging breaks.
#
# **The signal was never missing** was this file's claim from 2026-08-04, and on 2026-09-23 it was
# **falsified and is corrected here rather than deleted**, because the correction is the lesson. CI
# does run on every push to `main` and has all along; what was wrong is the step after it, which is
# this script's own: that a green CI conclusion means a healthy tree. It does not. `ci.yml` skips
# the steps of the build jobs (`test`, `swish-check`, `boot-check`, and `swish-check-graphical` until its 2026-10-03 fold into `swish-check`)
# when every changed path matches `notes/`, `design/` or a root `*.md`, so a
# documentation-only commit posts a green required check having built and run nothing. That is a
# sound rule for a kernel and a false one for `crates/documentation`, whose tests RENDER those very
# files: `every_character_survives` never ran, `main` was broken for hours, and every signal a reader
# or this watcher could reach said green. Pull request #1168 fixes the skip. The durable lesson is
# about this file: **a conclusion is a claim about what ran, not about the tree.**
#
# The original point stands underneath it. On 2026-08-04 `main` went red and was found by someone
# running `script/lint` by hand after a merge, which is luck rather than process. This reads the
# signal that already exists, and now says what that signal does not cover.
#
# # What it says, and what it deliberately does not
#
# It reports the transition to red, naming the failing workflows, and the transition back to green.
# A green reached by citing a merge-group run rather than repeating it says so, with the run IDs.
# It does **not** report every red poll, because a trunk that stays broken for an hour is one fact
# and not twenty-four.
#
# It says "nobody is assigned to this" on purpose. A red trunk with an owner is a task; a red trunk
# without one is the failure this script exists to surface, and the wording is the difference.
#
# **Recovery is reported too, deliberately.** A watcher that only speaks on failure trains its reader
# to treat silence as health, and silence is also what a dead watcher produces.
#
# # Where this runs, and the watch that was folded in here and then unfolded (2026-09-24)
#
# **The trunk half runs in GitHub Actions as `nife-smelter[bot]`**, on a five-minute schedule:
# `.github/workflows/trunk-health.yml`, which carries the reasoning, the tested premise, and the
# cadence BUGS. It used to run under `launchd` on patagonia as calef's own token.
#
# **`helpers/at-risk-check.sh` did not come along, and could not have.** It was folded into this
# script's loop on 2026-09-23 on the reasoning that a third watcher is a third thing to start and a
# third thing that can die silently, and that `com.nife.trunk-health` was already firing on the
# right interval. That reasoning was correct for as long as both halves ran on the same machine.
# They no longer do: everything else here reads GitHub, while the at-risk check reads **this
# machine's** worktrees and their `git status`. A runner has no lane worktrees, so carrying the fold
# into Actions would have produced a check that reports nothing forever while the hazard it exists
# for sat on somebody's laptop unwatched, which is worse than not having it.
#
# So the at-risk watch is per developer, one per machine, under its own `launchd` job, and it needs
# no GitHub credential at all. partsfoundry/record:notes/merge-queue.md has the plist and the retirement commands for
# the two jobs this replaces. `--once` here still reports it, because a person running this by hand
# on their own machine is exactly the case where both halves are true at once.
#
# # The thing that would prevent this rather than detect it
#
# GitHub's require-branches-to-be-up-to-date rule, applied 2026-08-04 (§73). Two pull requests, each
# green against the base it was cut from, merged in an order neither had ever been tested in and put
# `main` red. That rule forces a re-run against the new `main`, turning that failure into one re-run
# instead of a broken trunk. This script is the detection half; the rule is the prevention half.
#
# # BUGS
#
#   - **A green from a push that skipped its suites is sound, and says so** (A′, 2026-09-24). CI and
#     verify skip on a push to `main` when a `merge_group` run of the same workflow concluded
#     `success` at that exact commit, because the queue builds the very commit that lands. Those
#     runs conclude `success` with every gated job skipped, so this script reads them green, and
#     prints the merge-group runs it is relying on so a reader can tell that green from the one
#     below. It trusts the gate's lookup rather than re-deriving it: it does not check that the run
#     it names belongs to the same workflow as each skipped push run, only that some merge-group run
#     at this commit succeeded (CI or verify, whichever it found).
#   - **A green conclusion is not a healthy tree, and this script cannot tell the difference.** See
#     the correction above: a check whose steps were skipped reports `success`, so this watcher says
#     GREEN while the test that would have caught the breakage never ran. Nothing here reads whether
#     a job did any work, and a fix would mean reading each run's steps rather than its conclusion,
#     which is a different and much chattier API. Until then, a docs-only merge is a moment to
#     distrust this watcher rather than to be reassured by it.
#   - **An API failure is UNKNOWN, never green** (calef, 2026-10-16 UTC). `state()` prints
#     `UNKNOWN <sha>` when `gh run list` fails or returns something that is not a list; the repeat
#     half prints a `cannot ask` row naming the workflows whose run list failed; the merge-watcher
#     half prints one when `helpers/watcher_watch.py check` exits 2. `script/cadence-check` already
#     exits 2 for the same cause and has its row. Before this, a rate limit or an expired token
#     printed `main is green` and no DEAD or REPEAT rows, measured with a stub `gh` that exits 1.
#     Rate limiting is therefore loud now rather than silent: the repeat half's log reads cost two
#     API calls per failing run on every five-minute pass (about 30 per pass when this was
#     written), unchanged, and a pass that runs out says so. The watching loop announces UNKNOWN
#     once and does not let it change the RED/GREEN memory. A `cannot ask` row is a gap in the
#     stream, so it also means a REPEAT that stands is not being re-announced on that pass.
#   - **It reports and never resolves**, deliberately, which was the whole gap calef named on
#     2026-09-23. The response now exists as `helpers/queue-hold.sh` and briefs/main-is-red.md, and
#     a person still has to run it: this script does not, because holding the queue needs the
#     judgement partsfoundry/record:notes/merge-queue.md argues a watcher must not exercise.
#   - **It does not report its own death.** Shared with `helpers/merge-drain.sh`, accepted rather
#     than solved; partsfoundry/record:notes/merge-queue.md has the reasoning and the `launchd` plists.
#   - **"Transition" means newly observed, not newly due.** The red/green, cadence and DUE memories
#     (`prev`, `prev_cadence`, `prev_due`) live in the watching shell and die with it, so a restart
#     announces the current state on its first pass as if it were new: a cadence that has stood due
#     for a month is re-reported once by every restart, and a due that clears while nothing is
#     watching is never reported at all. No persistent state sits behind any transition in this
#     script; that has been true of the RED/GREEN and cadence halves all along and is stated once,
#     here.
#   - **The repeat half reads logs through `gh run view --log-failed`, which is a screen, not an
#     API.** Its two leading columns, its timestamp, its ANSI-marked script echoes and its runner
#     teardown lines are stripped by pattern, and the "final line" is the last content line before
#     GitHub's own `##[error]Process completed with exit code` annotation. A log that will not
#     parse lands in an unreadable-line group rather than a wrong one; the pattern strip is where
#     this breaks first if `gh` changes its log shape. Run 34109632590 (2026-09-07 UTC) is the
#     known case of teardown lines arriving ahead of the annotation, and the strip handles it.
#   - **The repeat half spends real API quota, on purpose and bounded.** One listing call per
#     scheduled workflow plus two calls per failing run examined, on every pass that reads it.
#     That cost is why the watching loop below does not call it; the Actions relay is the reader.
#   - **A repeat row reports, and the issue the relay opens is its home and its close condition.**
#     What the row asks for ("run the audit") is a lane and a day. `helpers/repeat_issue.py` opens
#     one issue per workflow on the `trunk-health.yml` pass, and the issue body names the close:
#     the audit lane's report pull request says `Closes #N`. This script's own output still closes
#     nothing, and the watching loop below opens nothing either.
#
# Name: unrecorded. Provisional, minted 2026-08-04 and not yet put to calef. `trunk` rather than
# `main` because the branch could be renamed and the concept could not, and because "trunk health"
# is the term the field already uses. See partsfoundry/record:notes/merge-queue.md.

set -e
cd "$(dirname "$0")/.."

# `--repeat` is the repeat half alone, and `--repo` names the repository it reads and is allowed
# only with it. The two exist for a repository that borrows this script rather than copying it:
# partsfoundry/record links nife's tree into its own checkout (its helpers/nife_tree.py) and runs
# `--repeat --repo partsfoundry/record` from a weekly workflow of its own. The repeat half needs only `gh`
# and the workflow files of the tree it runs in, so it is the one half that means the same thing
# in another repository; the rest of `--once` reads nife's own scripts and nife's trunk.
REPO="partsfoundry/nife"
once=""
repeat_only=""
repo_given=""
usage() {
	echo "usage: $(basename "$0") [--once | --repeat [--repo <owner>/<name>]]" >&2
	exit 2
}
while [ $# -gt 0 ]; do
	case "$1" in
	--once) once=1 ;;
	--repeat) once=1; repeat_only=1 ;;
	--repo)
		[ $# -ge 2 ] || usage
		case "$2" in
		*/*) REPO="$2"; repo_given=1 ;;
		*) usage ;;
		esac
		shift
		;;
	*) usage ;;
	esac
	shift
done
[ -n "$repo_given" ] && [ -z "$repeat_only" ] && usage

# A watcher must run from the MAIN checkout, never from a lane worktree, and this refuses rather
# than trusting anyone to remember. Measured cause, twice: `/bin/sh` reads a script LAZILY, so
# deleting the file under a running shell can kill it mid-loop. The merge drain died that way on
# 2026-08-18 when the worktree it was launched from was pruned, and `trunk-health.sh` died the same
# way later the same day during a 24-worktree cleanup, silently, while `main` was red on the
# fastpath gate for hours. The drain survived that second sweep only because it happened to have
# been relaunched with an absolute path into the main checkout.
#
# Only the watching form is refused. `--once` is a check anybody may run anywhere, including a lane
# gating its own work, and it exits long before a prune could reach it.
#
# `--git-dir` resolves to `.git/worktrees/<name>` in a linked worktree and to `.git` in the main
# checkout, which is the cheapest true test available; `--git-common-dir` points at the shared
# `.git` from both and cannot tell them apart.
if [ -z "$once" ] && [ "$(git rev-parse --git-dir 2>/dev/null)" != ".git" ]; then
	echo "$(basename "$0"): refusing to watch from a lane worktree." >&2
	echo "  A watcher outlives the lane that started it, and pruning that lane's worktree kills" >&2
	echo "  it silently, because /bin/sh reads a script lazily. Run it from the main checkout:" >&2
	echo "    cd <main checkout> && helpers/$(basename "$0") &" >&2
	echo "  ('--once' is fine from anywhere; only the watching form is refused.)" >&2
	exit 2
fi


state() {
	full=$(git ls-remote "$(git remote get-url origin 2>/dev/null || echo origin)" refs/heads/main 2>/dev/null | cut -f1)
	sha=$(printf '%.8s' "$full")
	[ -z "$sha" ] && { echo "unknown"; return; }
	# An empty list and a failed call are different answers: the first is a quiet branch, the
	# second is "nobody asked", and only the exit code tells them apart. A rate limit or an
	# expired token used to read as `[]` and print GREEN.
	if ! runs=$(gh run list --repo "$REPO" --branch main --limit 12 \
		--json workflowName,status,conclusion,headSha 2>/dev/null) ||
		! printf '%s' "$runs" | jq -e 'type == "array"' >/dev/null 2>&1; then
		echo "UNKNOWN $sha"
		return
	fi
	failed=$(printf '%s' "$runs" | jq -r --arg s "$sha" \
		'[.[] | select(.headSha[0:8] == $s) | select(.conclusion == "failure") | .workflowName] | unique | join(", ")' 2>/dev/null)
	running=$(printf '%s' "$runs" | jq -r --arg s "$sha" \
		'[.[] | select(.headSha[0:8] == $s) | select(.status != "completed")] | length' 2>/dev/null)
	if [ -n "$failed" ]; then
		echo "RED $sha $failed"
	elif [ "$running" = "0" ]; then
		# Say which kind of green (A′, 2026-09-24): a push the merge group already tested skips its
		# suites and concludes `success` having run nothing here, which is sound only because the
		# merge-group run at this same commit did run them. Naming that run is what separates it
		# from the skipped-docs green in BUGS below. An API failure prints the plain form.
		tested=$(gh api "repos/$REPO/actions/runs?event=merge_group&head_sha=$full&status=success" \
			--jq '[.workflow_runs[] | select(.name == "CI" or .name == "verify") | "\(.name) \(.id)"] | join(", ")' 2>/dev/null || true)
		if [ -n "$tested" ]; then
			echo "GREEN $sha (merge group tested this commit: $tested)"
		else
			echo "GREEN $sha"
		fi
	else
		echo "PENDING $sha"
	fi
}

# **A scheduled workflow's red is invisible to `state()` above, structurally rather than by
# oversight**, and that is why this second question is asked here (milestone 238). `state()` filters
# runs to `headSha == main`'s current tip, which is the right filter for "is the trunk broken" and
# the wrong one for a cadence: a weekly job's run matches the tip only until the next merge, and at
# this tree's merge rate that window is minutes. So `mutation testing` failed four Mondays running
# and this watcher, pointed straight at the same API, could not have seen any of them.
#
# `script/cadence-check` is where the judgment lives; this only decides when to speak. Same
# transition discipline as RED/GREEN, for the same reason: a cadence that has been dead for a month
# is one fact, not four hundred polls of it. The call is captured rather than passed through,
# because a dead cadence is its exit 1 and this script runs under `set -e`.
#
# **Two of the cadences are asked here rather than by `script/cadence-check`** (milestone 889 (the
# tree's gates stop depending on the fleet tooling), provisional, 2026-10-12 UTC). The weekly capture
# of machine effort and the two merge watchers measure the fleet that runs agent lanes, and
# cadence-check is a tree tool that a stranger's clone keeps, so it no longer calls either. Both
# are asked on the same five-minute pass as before, in the same row format, so the transitions and
# trunk-health.yml's DEAD warning read them exactly as they did.
#
# **A pass that could not ask gets a row, so the stream never empties on one.** `script/cadence-check`
# exits 2 with nothing on stdout when it cannot ask (no `gh`, no network, no credentials), and a
# nonzero exit with no rows prints a `cannot ask` row here, in the `due()` section's shape. That
# makes cannot-ask a state of the stream the `prev_cadence` compare reads, not a hole in it: the
# compare speaks on the transition into it, stays silent while it stands, and its recovery line
# fires only when a real answer empties the stream. The row is deliberately not DEAD-prefixed: the
# workflow that relays this script warns on a DEAD grep, and one blind pass is weather there, not a
# dead workflow. helpers/trunk-health-cadence-selftest.sh holds the failure cases (name
# provisional).
#
# The effort capture (milestone 519 (what this project costs, tracked where it cannot rot)) is a
# person running `script/effort --update` on the machine the lanes ran on, because only that
# machine has the session records; `--check` asks whether the committed `effort.csv` holds the
# newest closed week, read after it closed, which needs no network and no records. It prints its row only when dead, the
# `--quiet` rule. The merge watchers (milestone 723 (a stopped merge watcher is reported within
# three of its own intervals)) have their own rule, from their own cron, in helpers/watcher_watch.py.
cadence() {
	cadence_say=""
	if ! cadence_say="$(script/cadence-check --quiet 2>/dev/null)"; then
		if [ -z "$cadence_say" ]; then
			cadence_say="$(printf '%-32s %-24s %s\n' "cadence-check" "cannot ask" \
				"CANNOT-ASK: unknown; script/cadence-check without --quiet says what it could not read")"
		fi
	fi
	if [ -n "$cadence_say" ]; then
		printf '%s\n' "$cadence_say"
	fi
	if [ -x script/effort ]; then
		effort_say="$(script/effort --check 2>&1)" ||
			printf '%-32s %-24s %s\n' "effort capture" "not this week" "DEAD: ${effort_say#effort: }"
	fi
	# Exit 1 is a stopped watcher, with its rows on stdout. Exit 2 is "could not ask" with nothing
	# printed, and `|| true` used to read that as every watcher healthy.
	# Absent, as in helpers/trunk-health-cadence-selftest.sh's copy of the tree, it is not asked.
	watchers_rc=0
	if [ -f helpers/watcher_watch.py ]; then
		watchers_say="$(python3 helpers/watcher_watch.py check --quiet 2>/dev/null)" || watchers_rc=$?
		[ -n "$watchers_say" ] && printf '%s\n' "$watchers_say"
	fi
	if [ "$watchers_rc" -ge 2 ]; then
		printf '%-32s %-24s %s\n' "merge watchers" "cannot ask" \
			"CANNOT-ASK: unknown; helpers/watcher_watch.py check without --quiet says what it could not read"
	fi
	return 0
}

# --- DUE: a cadence that says "a person must now do something" ----------------------------------
#
# **A due cadence is not a dead one, and the two cadences that mean "due" reached this watcher only
# as the second thing** (milestone 495 (a cadence that says "due" reaches nobody, and the watcher
# cannot tell due from dead)). The stranger cadence (`script/stranger-test --due`) exists to say "a
# person must now do something", and so did the audit cadence until it left with the design record
# for partsfoundry/record, whose own workflow runs it. Red in the Actions
# tab is their correct output, not their failure. `script/cadence-check` answers only whether a
# scheduled workflow has stopped *succeeding*, so a job correctly reporting due and a job that is
# broken arrive as the same verdict, DEAD, a fortnight after the fact. The audits sat that way for
# five Mondays before anyone acted; that sentence with dates is milestone 495's evidence section.
#
# So this section asks the scripts themselves, not the workflows that relay them. Both `--due` modes
# read records already on disk (the stranger note's run headings, the audit index), need no network,
# no toolchain and no credential, and answer in seconds on the checkout this watcher already holds.
# That is milestone 495's option 1, taken for the reason it gives: it reuses the watcher a
# maintainer session is already told to confirm is alive, adds no scheduled workflow to the ones
# cadence-check polices, takes no `issues: write`, and opens no second channel. It is deliberately
# not a gate and not in `script/lint`, for the reason `script/cadence-check`'s header already wrote
# down: a due cadence is not a defect in the commit that happens to trip it.
#
# **The verdict speaks on transition, like everything else in this loop.** A cadence that stands due
# for a month is one fact, and the transition discipline is also what keeps DUE from becoming the
# standing alarm the block warns a due-echo would be: speak when something becomes due, speak when
# it clears, and not on the polls between. Clearing means the work was done (a run written up, or
# audits run and their rows added), or that the record moved underneath; both scripts' own BUGS
# sections name the edit-to-silence hole, and this inherits it rather than policing it. The recovery
# line, like RED/GREEN's, exists because a watcher that only speaks on failure trains its reader to
# treat silence as health.
#
# **A record that would not read gets a row rather than silence.** Exit 1 from a `--due` mode means
# "due" and also "the record could not be read" (a malformed or missing one), and the second case
# prints nothing on stdout, so the DUE sentence itself is what tells the two apart. Saying nothing
# about the second would recreate, in miniature, the silence this section exists to close.
due() {
	rows=""
	if st="$(script/stranger-test --due 2>/dev/null)"; then
		: # exit 0: a run is recorded as not due, and there is nothing to say
	else
		why="$(printf '%s\n' "$st" | sed -n 's/^stranger-test: DUE\. //p')"
		if [ -n "$why" ]; then
			last="$(printf '%s\n' "$st" | sed -n 's/^Last run: [0-9]*, \([0-9-]*\),.*/\1/p')"
			rows="$(printf '%-32s %-24s %s\n' "stranger-test" "last run ${last:-?}" "DUE: ${why}")"
		else
			rows="$(printf '%-32s %-24s %s\n' "stranger-test" "cannot ask" \
				"DUE: unknown; script/stranger-test --check says what is wrong with the run record")"
		fi
	fi
	if [ -n "$rows" ]; then
		printf '%s\n' "$rows"
	fi
}

# --- REPEAT: a scheduled workflow that keeps failing with the same final line --------------------
#
# **Red is ambiguous for a job whose healthy state is red on purpose, and a repeat is the part a
# conclusion alone cannot say** (milestone 422 (a cadence job whose healthy state is red)). `audit-cadence` has failed every scheduled Monday
# since it was created, and almost all of those runs are the mechanism working: an audit was due,
# the workflow said so, and red is its correct output. Buried among them, on 2026-09-14, sat one
# run that failed on `FileNotFoundError` instead, broken for real and indistinguishable in the
# Actions tab from the correct reds on both sides of it. The `due()` section above cannot see that
# difference, because it asks the tree's scripts what is due now, and on 2026-09-14 the tree was
# fine: the workflow was broken. So this section asks the other witness, GitHub's own run history,
# the read-only source `script/cadence-check` already reads: what did each failing run's step
# actually print last.
#
# The sketch is the block's, unchanged: take the last six scheduled runs of each scheduled
# workflow, group the failed ones by the final line of their failing step, and report a workflow
# whose largest group holds three or more, naming the line and the span of dates (UTC). Three is
# the block's threshold, not this script's invention: one failure is weather, two same-line
# failures on a weekly cadence are a coincidence and a confirmation, and the row waits for the
# third, accepting the lag. A workflow whose failures disagree with each other is CHURN, a
# separate and less urgent finding on its own row, never folded into the repeat one; the block's
# word for it. A single line failing exactly twice is an unconfirmed repeat and stays silent until
# the third occurrence settles which it is.
#
# **The line is compared whole, digits included.** A mutation failure's final line names the mutant
# that killed it, and those lines differ for good reason; a key that normalized numbers away would
# merge genuinely different failures into one false repeat. The specimen needs no normalization:
# the audit-cadence runs end in an epilogue printed by `script/audits` itself ("design/
# audit-reports/README.md says how, ..."), identical across every due-red, which is why the
# FileNotFoundError run falls out of its group on its own.
#
# **The per-workflow runs endpoint is the point, not a style choice.** The all-workflows listing
# `script/cadence-check` uses caps at 100 runs, and at this tree's daily cadence that window no
# longer reaches back to a weekly's own last six: on 2026-10-16 it held a single audit-cadence
# run. cadence-check's BUGS records the same cap biting its own 15-day threshold.
#
# **Reading the final line is best effort, because `gh run view --log-failed` is a screen rather
# than an API** (the header's BUGS carries the caveat). The frame is stripped, then the content:
# `repeat_line` below says what each pattern is for. A log that yields nothing readable groups as
# "(final line could not be read)" instead of pretending to a line it never saw.
#
# **Cost, and why the loop below does not call this.** One listing call per scheduled workflow,
# plus two per failing run of any workflow with two or more failures in its last six: at this
# tree's failures that is a few dozen calls per pass. The watching loop polls every 90 seconds,
# and on that cadence the spend runs past what the credential the merge drain shares should pay
# for a report. `trunk-health.yml` runs `--once` every five minutes and is the reader that exists;
# a person running `--once` by hand gets the same rows. Like DEAD and DUE in that relay, a standing
# repeat re-announces on every pass, which that workflow's own BUGS owns.
#
# **A repeat row reports, and its home is the issue the relay opens**, not this script. What a
# repeat row asks for ("run the audit") is a lane and a day, and the home ruled for it is one open
# issue per workflow: `trunk-health.yml` hands this section's REPEAT rows to
# `helpers/repeat_issue.py`, which dedupes by workflow name, updates the body when the line drifts,
# and leaves the close to the audit lane's report pull request (`Closes #N`, routed by the
# audit-cadence step summary). This script opens nothing itself, and the watching loop below opens
# nothing either.
#
# Names here are provisional (milestone 422): `repeat`, `repeat_line`, and the REPEAT and CHURN
# row labels. So are the `--repeat` and `--repo` flags, minted 2026-11-24 (UTC) for
# partsfoundry/record#6, which runs this half for the record's own scheduled workflows.
repeat_line() {
	# Capture first, then parse: `gh` announces its own failures (a truncated download prints
	# "Error: ...") on stdout, and a pipeline would feed that announcement to the awk below as if
	# the step had printed it. A `gh` that exits nonzero yields no line at all.
	log=$(gh run view "$1" --repo "$REPO" --log-failed 2>/dev/null) || return 0
	printf '%s\n' "$log" |
	awk -v esc="$(printf '\033')" '
		# The log frame: two tab-separated columns, then a timestamp and one space. A line with
		# no frame is kept whole rather than stripped blind.
		{ if (sub(/^[^\t]*\t[^\t]*\t/, "")) sub(/^[^ ]* /, "") }
		# The annotation is where the failing step stopped saying anything of its own; everything
		# after it is later steps and runner teardown, none of it this failure.
		/^##\[error\]Process completed with exit code/ { done = 1 }
		done { next }
		# The resolved script echoed back, identical on every run, worthless as a key.
		index($0, esc "[36;1m") == 1 { next }
		/^##\[/ { next }
		/^\[command\]\// { next }
		# Runner teardown, dropped by name: run 34109632590 (2026-09-07 UTC) puts some of it
		# ahead of the annotation, where the marker alone would not screen it out.
		/^(Cleaning up orphan processes|Terminate orphan process:|Removing credentials config |Evaluate and set job outputs|Set output )/ { next }
		NF { last = $0 }
		END { print last }
	'
}

repeat() {
	# The workflows that claim a schedule, leaving out the same two watcher files
	# `script/cadence-check` leaves out: `helpers/watcher_watch.py` watches those two, and a watcher
	# restarted five minutes ago must not become its own finding. Only files git tracks count,
	# because only a committed workflow has runs: in partsfoundry/record's checkout, nife's workflows sit
	# beside the record's as untracked links (helpers/nife_tree.py there), and asking the record's
	# runs for one of them would be a `cannot ask` row on every pass.
	scheduled=$(git ls-files -- .github/workflows 2>/dev/null |
		grep -E '^\.github/workflows/[^/]+\.yml$' |
		grep -v -e '/merge-drain\.yml$' -e '/trunk-health\.yml$' |
		while read -r file; do
			grep -q '^  schedule:$' "$file" 2>/dev/null && printf '%s\n' "$file"
		done | sort) || true
	[ -n "$scheduled" ] || return 0
	rows=""
	unasked=""
	for file in $scheduled; do
		name=$(basename "$file" .yml)
		if ! runs=$(gh api "repos/$REPO/actions/workflows/$name.yml/runs?event=schedule&per_page=6" \
			--jq '.workflow_runs[] | [.conclusion, .created_at, (.id|tostring)] | @tsv' 2>/dev/null); then
			unasked="${unasked:+$unasked, }$name"
			continue
		fi
		failed=$(printf '%s\n' "$runs" | awk -F'\t' '$1 == "failure" { n++ } END { print n + 0 }')
		# Below two failed runs no row is possible, and the log reads behind this line cost two
		# API calls each, so two is also where the spending starts.
		[ "$failed" -ge 2 ] || continue
		failed_runs=$(printf '%s\n' "$runs" | awk -F'\t' '$1 == "failure" { print $3 "\t" $2 }')
		lines=""
		while IFS="$(printf '\t')" read -r id when; do
			[ -n "$id" ] || continue
			# The line travels tab-free so it can be one field for the grouping that follows.
			line=$(repeat_line "$id" | tr '\t' ' ')
			[ -n "$line" ] || line="(final line could not be read)"
			lines="${lines}${when}	${line}
"
		done <<EOF
$failed_runs
EOF
		verdict=$(printf '%s\n' "$lines" | awk -F'\t' '
			# The NF guard is load-bearing: the here-doc hands this one row per failure, and the
			# trailing newline that carries the last one would otherwise arrive as an extra empty
			# record and be counted as a failure with an empty final line.
			NF {
				count[$2]++
				failed++
				if (!($2 in oldest) || $1 < oldest[$2]) oldest[$2] = $1
				if (!($2 in freshest) || $1 > freshest[$2]) freshest[$2] = $1
			}
			END {
				best = ""; bestn = 0; bestfresh = ""
				for (k in count) {
					if (count[k] > bestn || (count[k] == bestn && freshest[k] > bestfresh)) {
						best = k; bestn = count[k]; bestfresh = freshest[k]
					}
				}
				if (bestn >= 3) {
					printf "REPEAT\t%d\t%s\t%s\t%s\n", bestn, best, oldest[best], freshest[best]
				} else {
					distinct = 0
					for (k in count) distinct++
					if (distinct >= 2) {
						detail = "no line three times"
						if (distinct == failed) detail = "all " failed " different"
						printf "CHURN\t%d\t%s\n", failed, detail
					}
				}
			}')
		[ -n "$verdict" ] || continue
		case "$verdict" in
		REPEAT*)
			n=$(printf '%s' "$verdict" | cut -f2)
			text=$(printf '%s' "$verdict" | cut -f3)
			first=$(printf '%s' "$verdict" | cut -f4)
			last=$(printf '%s' "$verdict" | cut -f5)
			row="$(printf '%-32s %-24s %s\n' "$name" "$n of last 6 failed" \
				"REPEAT: ${text} (${first%T*}..${last%T*} UTC)")"
			;;
		CHURN*)
			n=$(printf '%s' "$verdict" | cut -f2)
			detail=$(printf '%s' "$verdict" | cut -f3)
			row="$(printf '%-32s %-24s %s\n' "$name" "$n of last 6 failed" \
				"CHURN: ${n} of the last 6 failed, ${detail}")"
			;;
		esac
		if [ -n "$rows" ]; then
			rows="$rows
$row"
		else
			rows="$row"
		fi
	done
	if [ -n "$unasked" ]; then
		# One row for the pass, in the `due()` shape: a skipped workflow is a REPEAT nobody looked for.
		row="$(printf '%-32s %-24s %s\n' "repeat-check" "cannot ask" \
			"CANNOT-ASK: unknown; gh api workflows/<name>.yml/runs failed for: ${unasked}")"
		rows="${rows:+$rows
}$row"
	fi
	if [ -n "$rows" ]; then
		printf '%s\n' "$rows"
	fi
}

# Relayed by `--once` only; the watching form no longer calls it. See the header section above for
# why the fold was undone. `helpers/at-risk-check.sh` reports and never acts; this only relays it,
# and on a machine with no lane worktrees it prints nothing rather than being wrong.
at_risk() {
	helpers/at-risk-check.sh 2>/dev/null || true
}

if [ -n "$repeat_only" ]; then
	r=$(repeat)
	[ -n "$r" ] && printf '%s\n' "$r"
	exit 0
fi

if [ -n "$once" ]; then
	s=$(state)
	case "$s" in
	RED*) echo "main is RED: ${s#RED }" ;;
	GREEN*) echo "main is green at ${s#GREEN }" ;;
	UNKNOWN*) echo "main: UNKNOWN at ${s#UNKNOWN }; gh run list failed, so this pass has no answer for main" ;;
	*) echo "main: ${s}" ;;
	esac
	c=$(cadence)
	[ -n "$c" ] && printf '%s\n' "$c"
	d=$(due)
	[ -n "$d" ] && printf '%s\n' "$d"
	r=$(repeat)
	[ -n "$r" ] && printf '%s\n' "$r"
	a=$(at_risk)
	[ -n "$a" ] && printf '%s\n' "$a"
	exit 0
fi

prev=""
prev_unknown=""
prev_cadence="unknown"
prev_due="unknown"
while true; do
	s=$(state)
	case "$s" in
	UNKNOWN*)
		# Not a transition of RED/GREEN: `prev` keeps the last real answer, so RED then UNKNOWN then
		# GREEN still announces the recovery, and UNKNOWN then GREEN announces nothing.
		[ -z "$prev_unknown" ] && echo "main: UNKNOWN at ${s#UNKNOWN } -- gh run list failed; this watcher has no answer for main"
		prev_unknown=1
		;;
	RED*)
		prev_unknown=""
		[ "$s" != "$prev" ] && echo "MAIN IS RED at $(echo "$s" | cut -d' ' -f2) -- failing: $(echo "$s" | cut -d' ' -f3-) -- nobody is assigned to this"
		;;
	GREEN*)
		prev_unknown=""
		case "$prev" in
		RED*) echo "main recovered at $(echo "$s" | cut -d' ' -f2)" ;;
		esac
		;;
	esac
	case "$s" in
	UNKNOWN*) ;;
	*) prev="$s" ;;
	esac

	# Recovery is announced here too, and for the reason the header already gives about RED/GREEN: a
	# watcher that only ever speaks on failure teaches its reader that silence means health, and
	# silence is also what a dead watcher sounds like. The stream cannot empty on a pass that could
	# not ask (see cadence above), so this line only ever follows a real answer.
	c=$(cadence)
	if [ "$c" != "$prev_cadence" ]; then
		if [ -n "$c" ]; then
			printf '%s\n' "$c"
		elif [ "$prev_cadence" != "unknown" ]; then
			echo "every scheduled workflow is producing results again"
		fi
		prev_cadence="$c"
	fi

	# The DUE half keeps its own transition memory instead of folding its rows into the cadence
	# stream above, because what clears a due list is that a person did the work, and "every
	# scheduled workflow is producing results again" would say the wrong thing about that. Same
	# shape and same first-pass rule as prev_cadence: a fresh watcher announces what stands, which
	# is the "newly observed, not newly due" caveat in the BUGS list.
	d=$(due)
	if [ "$d" != "$prev_due" ]; then
		if [ -n "$d" ]; then
			printf '%s\n' "$d"
		elif [ "$prev_due" != "unknown" ]; then
			echo "nothing is due any more"
		fi
		prev_due="$d"
	fi

	sleep 90
done
