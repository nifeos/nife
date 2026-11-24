#!/bin/sh
#
# Report pushed branches that have no pull request claiming them, and pushed
# branches whose pull request closed without a resolution anyone acted on.
#
#     helpers/lane-claim-check.sh          # one pass, then exit
#
# PROVISIONAL NAME. Minted 2026-08-31 by milestone 204's lane; not put to calef. See `Name:` below.
#
# **This reports; it never deletes anything.** Every line below is a person's judgment call, even
# the ones that say "safe to delete". Nothing here runs unattended and nothing here acts.
#
# # Why this exists
#
# AGENTS.md §90: a lane's first act is a draft pull request, and the reason is that **the draft is
# the claim.** It is how two lanes cannot silently take the same milestone, the board is
# `gh pr list --draft`, it costs one command, and a draft cannot be stuck in the merge queue because
# a draft cannot be merged.
#
# **Nothing checked it.** On 2026-08-31 the lanes for milestones 121 and 194 both pushed
# `milestone/*` branches and opened nothing; the board was empty while two milestones were being
# worked, and it was noticed only because calef asked. The instruction was in both briefs, in a
# section headed *First act*, with the exact command. That is rung four behaving exactly as
# AGENTS.md says rung four behaves, and it was the second instance of the shape in this project's
# history: the first was lanes ending their turn mid-gate.
#
# A brief is prose, and prose in a brief is not a weaker mechanism than prose anywhere else. It is
# the same rung.
#
# # What it deliberately is not
#
# **Not a gate.** Nothing should fail a build over this, because the lane that most needs telling is
# one that is mid-work and about to open its pull request anyway. `script/lint` was refused for
# exactly that reason. What was missing was never enforcement; it was anything that looks.
#
# **Not a nag.** Several false-positive shapes were designed out, because a report that cries wolf
# gets ignored and then the real case goes unread:
#
#   - **The legitimate window.** A lane pushes and then opens the pull request, and GitHub refuses a
#     pull request with no commits between the branch and `main`, so a lane must produce something
#     committable first. Measured on this script's own branch: 3 minutes from branch creation to
#     draft, and that included writing the file that made the branch non-empty. `GRACE_MINUTES` is
#     five times that. (`merge-drain.sh` had a 75-minute stale-draft report beside this until
#     2026-10-03, when calef's rulings on #1564 removed its stall comments.)
#   - **A merged lane's leftover branch.** A branch whose pull request merged and which nobody
#     deleted is hygiene, not a missing claim. It is reported on its own line, with the word
#     `LEFTOVER` and the pull request number, so nobody has to read it as an accusation. But
#     `LEFTOVER` means the branch tip itself is an ancestor of `main`, checked directly, not just
#     that some pull request from it merged: `maintainer/open-model-lanes` kept three commits after
#     its own #1089 merged, found while verifying this script against the live repository on
#     2026-09-23, and a check that trusted the pull request's state alone would have told a reader
#     to delete work the merge never included. That shape prints as `MERGED BUT DIVERGED` instead.
#   - **A branch someone opened a non-draft pull request for.** A ready pull request is a louder
#     claim than a draft, not a quieter one.
#
# # The clock, and why it is the branch's birth rather than its last commit
#
# The grace period runs from **branch creation**, taken from the repository activity feed, and a
# later push does not reset it. That is the whole point: a lane that keeps committing is precisely
# the lane whose missing claim matters, and a last-commit clock would go quiet for exactly the
# branches that are being worked hardest.
#
# There are two clocks, not one, because a `milestone/*` claim and a stray branch answer different
# questions. `GRACE_MINUTES` (default 15) protects the gap between a push and a draft pull request,
# and stays scoped to `milestone/*` because that is the only prefix §90 (the claim is a draft pull
# request; the status flip is a gate) governs.
# `GRACE_HOURS` (default 24) covers every other branch, where the question is not "has this lane
# opened its claim yet" but "has anyone come back to this at all"; see the survey below for the
# measurement that set it.
#
# Name: unrecorded. Provisional. `lane` and `claim` are both AGENTS.md's own words for these things
# (`§90`: "the draft is the claim"), and `-check` matches `script/qemu-check` and
# `script/stack-frame-check`. It lives in `helpers/` rather than `script/` for `merge-drain.sh`'s
# reason: it is a maintainer's tool, not a front door a contributor types.
#
# # BUGS
#
#   - **It detects the absence of a claim, not a collision.** Two lanes on the same milestone with
#     two drafts is the case §90 actually fears, and this calls that fine. Detecting the real
#     collision needs the milestone number parsed out of the branch name, which is a convention
#     nothing enforces.
#   - **It cannot see a lane that has not pushed at all**, which is the more dangerous state:
#     AGENTS.md says the pushed branch is the only ledger another session can read, and uncommitted
#     work in a worktree is the one thing no part of this system protects.
#   - **Every branch except `main` and `gh-readonly-queue/*`, not just `milestone/*`.** This was
#     narrower once: "widening the pattern would also sweep in short-lived maintainer branches that
#     are not claims and are not meant to be." That reason was a guess, and a 2026-09-23 survey of
#     25 branches carrying no open pull request falsified it: 18 were `maintainer/*` or `fix/*`, and
#     every one needed action (deleted as an empty claim, deleted as superseded, or turned into a
#     pull request). The predicted class of harmless short-lived branches did not appear; the two
#     closest cases were pushed by the maintainer mid-session and were real leaks caught by this
#     widening. What was true is that a stray `maintainer/*` branch answers a different question
#     than a `milestone/*` claim, which is why it gets its own, longer clock (`GRACE_HOURS`) instead
#     of sharing `GRACE_MINUTES`: a 24-hour window against that same survey flagged 23 of 24 and
#     deferred exactly the one branch still inside its legitimate window, with zero false positives.
#   - **The activity feed is read one page deep.** A branch created more than 100 repository events
#     ago has no visible birth, and is reported rather than skipped: an old branch with no claim is
#     the case worth seeing, so the fallback errs loud instead of silent. Widening past `milestone/*`
#     makes this more likely to bite, since 100 events cover less wall-clock time on a busier day.
#   - **It reports to stdout only.** A branch with no pull request has nowhere to be labelled or
#     told. Whoever reads the
#     drain's log reads this, and nothing reaches a lane that is not looking.
#   - **"Holds work" is sized by `git diff --shortstat`, not read.** It says how much changed, not
#     whether it matters. A person still has to look before deciding claim, land-and-delete, or
#     leave it.

set -e
cd "$(dirname "$0")/.."

REPO="partsfoundry/nife"
GRACE_MINUTES=${GRACE_MINUTES:-15}
GRACE_HOURS=${GRACE_HOURS:-24}

branches=$(git ls-remote --heads origin 2>/dev/null |
	sed 's|.*refs/heads/||' |
	grep -v '^main$' |
	grep -v '^gh-readonly-queue/' |
	sort)
[ -z "$branches" ] && exit 0

# Every pull request that has ever named one of these branches as its head, in any state. `--state
# all` is what separates a missing claim from a merged lane's leftover branch, and getting that
# wrong is the failure mode this whole report is designed around.
# A `gh` that is missing or signed out would make every branch read UNCLAIMED, which is a false
# report rather than an empty one, so refuse instead.
if ! gh auth status >/dev/null 2>&1; then
	echo "lane-claim-check: gh is not signed in; cannot tell a claimed branch from an unclaimed one" >&2
	exit 2
fi
prs=$(gh pr list --repo "$REPO" --state all --limit 200 \
	--json number,headRefName,state,isDraft 2>/dev/null || echo '[]')

# Branch birth times. `branch_creation` is the only event that answers "when did this claim become
# due"; commit dates cannot, because a branch pushed empty carries `main`'s commit date and would be
# reported the instant it existed.
activity=$(gh api "repos/$REPO/activity?per_page=100" 2>/dev/null || echo '[]')

for branch in $branches; do
	pr=$(printf '%s' "$prs" | jq -r --arg b "$branch" \
		'[.[] | select(.headRefName == $b)] | sort_by(.number) | last // empty' 2>/dev/null)

	if [ -n "$pr" ]; then
		state=$(printf '%s' "$pr" | jq -r '.state')
		num=$(printf '%s' "$pr" | jq -r '.number')
		# `state` is GitHub's own PullRequestState enum (OPEN, CLOSED, MERGED), not a flag
		# this script infers, so it alone tells MERGED from CLOSED reliably; `mergedAt`
		# carries no information `state` does not already have.
		case "$state" in
		OPEN)
			continue # claimed, draft or ready; nothing to say
			;;
		MERGED)
			# "Merged" describes the pull request, not the branch: a lane can keep
			# pushing to the same branch after its own pull request lands, and a
			# leftover check that trusts the PR state alone would tell a reader to
			# delete work the merge never included. Ask the branch tip itself.
			if git merge-base --is-ancestor "origin/$branch" origin/main 2>/dev/null; then
				echo "lane-claim-check: LEFTOVER. $branch's #$num merged; delete the branch"
			else
				stat=$(git diff --shortstat "origin/main...origin/$branch" 2>/dev/null)
				n=$(git rev-list --count "origin/main..origin/$branch" 2>/dev/null || echo '?')
				echo "lane-claim-check: MERGED BUT DIVERGED. $branch's #$num merged, but" \
					"$n commit(s) since are not on main ($stat). Read what's new before" \
					"deleting; claim it with a fresh pull request or land it in notes/"
			fi
			continue
			;;
		*)
			# CLOSED without merging: abandoned, or rejected, or superseded. The branch
			# may be the only place that work still exists. Never fold this into
			# LEFTOVER; a reader who deletes on sight here can destroy work.
			echo "lane-claim-check: CLOSED, NOT MERGED. $branch's #$num closed without" \
				"merging; read it before deciding anything, do not delete on this line alone"
			continue
			;;
		esac
	fi

	# No pull request in any state. Split by branch shape so a milestone claim still gets
	# its short, tuned window, and everything else gets the wider one the survey measured.
	case "$branch" in
	milestone/*) grace_seconds=$((GRACE_MINUTES * 60)) ;;
	*) grace_seconds=$((GRACE_HOURS * 3600)) ;;
	esac

	born=$(printf '%s' "$activity" | jq -r --arg b "refs/heads/$branch" '
		[ .[] | select(.ref == $b) | select(.activity_type == "branch_creation")
		  | .timestamp | fromdateiso8601 ] | min // empty' 2>/dev/null)

	if [ -n "$born" ]; then
		age=$(($(date -u +%s) - born))
		if [ "$age" -lt "$grace_seconds" ]; then
			continue # inside the legitimate window for this branch's shape
		fi
		mins=$((age / 60))
	else
		mins="?" # older than the activity page; see BUGS
	fi

	# Empty claim versus real work is the difference between "delete it" and "somebody has
	# to look", and a branch name alone cannot say which. `--shortstat` against origin/main
	# answers it in one call without checking anything out.
	commits=$(git rev-list --count "origin/main..origin/$branch" 2>/dev/null || echo 0)

	if [ "$commits" -eq 0 ]; then
		echo "lane-claim-check: UNCLAIMED, EMPTY. $branch has no commits past main after" \
			"$mins minutes. Safe to delete; nothing is lost."
	else
		stat=$(git diff --shortstat "origin/main...origin/$branch" 2>/dev/null)
		echo "lane-claim-check: UNCLAIMED, HOLDS WORK. $branch has $commits commit(s)" \
			"after $mins minutes ($stat). AGENTS.md §90: gh pr create --draft to claim it," \
			"or land what it found in notes/ and delete."
	fi
done
