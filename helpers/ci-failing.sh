#!/bin/sh
#
# helpers/ci-failing.sh: label and comment on a ready pull request whose head is failing a required
# check, so that neither calef nor a maintainer session has to notice.
#
#     helpers/ci-failing.sh              # one pass
#     helpers/ci-failing.sh --dry-run    # one pass, printing every write instead of making it
#
# PROVISIONAL NAMES: this script, `.github/workflows/ci-failing.yml`, helpers/ci-failing.jq and the
# label `ci-failing`. A rename is calef's call. See notes/merge-queue.md.
#
# # Why
#
# calef spotted failing pull request CI twice (#1611, #1597) before any session did, and a session
# may not run a watch loop (the refuse-ci-polling hook), so the watching has to be a workflow.
#
# # What it does
#
# For each open, non-draft pull request, read the check runs on its head commit and keep the ones
# named by the REQUIRED checks of the ruleset on `main`, looked up by API at the start of the pass
# and never hardcoded. The decision, including why a superseded failure does not count, is
# helpers/ci-failing.jq.
#
#   - Failing: add the label, and post one comment per head SHA naming each failing check with its
#     job URL. Deduplicated by a marker holding the SHA, so a head that flaps red, green, red is
#     still announced once, and a new push is a new SHA and a new comment.
#   - Green on every required check it has run: take the label off.
#
# The owner of a red pull request is a maintainer session, and this label alone never reached one:
# the merge drain turns 30 minutes of it into `needs-maintainer` (cause `red`, helpers/needs-maintainer.jq),
# which is the queue a session reads. Rename this label and that file must follow.
#
# It never dequeues, disables auto-merge, reruns or changes anything else. A queue reports, it does
# not resolve. Drafts are skipped because a draft is its lane's.
#
# # Where it runs
#
# `.github/workflows/ci-failing.yml`, as `nife-smelter[bot]`. From a checkout it is the same pass as
# whatever `gh` is logged in as; use --dry-run first.

set -e
cd "$(dirname "$0")/.."

REPO="nifeos/nife"
LABEL="ci-failing"
INSTANCE="${CI_FAILING_INSTANCE:-$(hostname -s 2>/dev/null || echo unknown)}"
ME="ci-failing[$INSTANCE]"
JQ="$(dirname "$0")/ci-failing.jq"

dry=""
case "$1" in
--dry-run) dry=1 ;;
"") ;;
*)
	echo "usage: $(basename "$0") [--dry-run]" >&2
	exit 2
	;;
esac

# Every write goes through here, so --dry-run changes nothing.
w() {
	if [ -n "$dry" ]; then
		echo "$ME: (dry run) would run: $(printf '%s ' "$@" | tr '\n' ' ' | cut -c1-200)"
	else
		"$@" >/dev/null
	fi
}

# The required checks of the ruleset that targets `main`, by name, so a change to the ruleset is
# followed without an edit here. Union over every active branch ruleset naming `main` or the
# default branch; the list call omits conditions, so each is read in full.
required=$(
	gh api "repos/$REPO/rulesets" --jq '.[] | select(.target=="branch" and .enforcement=="active") | .id' |
		while read -r id; do
			gh api "repos/$REPO/rulesets/$id" --jq '
				select(.conditions.ref_name.include | any(. == "~DEFAULT_BRANCH" or . == "refs/heads/main"))
				| [.rules[] | select(.type=="required_status_checks") | .parameters.required_status_checks[].context]'
		done | jq -sc 'add // [] | unique'
)
if [ "$required" = "[]" ]; then
	# Silence here would read as "nothing is failing". Fail the run instead; the run list shows it.
	echo "$ME: no required checks found on main; refusing to report anything" >&2
	exit 1
fi

if [ -n "$dry" ]; then
	echo "$ME: $(printf '%s\n' "$required" | jq length) required checks"
fi

gh pr list --repo "$REPO" --state open --limit 200 --json number,isDraft,headRefOid,labels,title |
	jq -r '.[] | select(.isDraft | not)
		| [.number, .headRefOid, (.labels | map(.name) | index("ci-failing") != null), .title] | @tsv' |
	while IFS="$(printf '\t')" read -r num sha labelled title; do
		runs=$(gh api "repos/$REPO/commits/$sha/check-runs?per_page=100" --paginate --jq '.check_runs[]' | jq -sc .)
		# The runs go in on stdin. `--argjson runs` put the whole check-run list in one argv entry,
		# which Linux caps at 128 KiB, and run 37227933655 died there ("Argument list too long")
		# before judging any pull request. `required` stays an argument: it is a few hundred bytes.
		bad=$(printf '%s\n' "$runs" | jq -c --argjson required "$required" "$(cat "$JQ") failing(\$required)")
		n=$(printf '%s\n' "$bad" | jq length)
		if [ "$n" -gt 0 ]; then
			echo "$ME: FAILING #$num at $(printf '%s\n' "$sha" | cut -c1-9): $(printf '%s\n' "$bad" | jq -r 'map(.name) | join("; ")') ($title)"
			if [ "$labelled" != "true" ]; then
				w gh label create "$LABEL" --repo "$REPO" --color B60205 \
					--description "a required check is failing on the head commit" --force
				w gh pr edit "$num" --repo "$REPO" --add-label "$LABEL"
			fi
			marker="<!-- ci-failing:$sha -->"
			seen=$(gh api "repos/$REPO/issues/$num/comments" --paginate --jq '.[].body' | grep -cF "$marker" || true)
			if [ "${seen:-0}" = "0" ]; then
				body="**Lane:** nife-smelter, posted by a scheduled workflow; nife-smelter[bot] is the author GitHub shows.

\`$ME\`: required checks failing on \`$(printf '%s\n' "$sha" | cut -c1-9)\`, most recent run of each:

$(printf '%s\n' "$bad" | jq -r '.[] | "- \(.name): \(.url)"')

This comment is posted once per commit. The \`$LABEL\` label comes off when every required check that has run is green on the head. Owner: a maintainer session. If it is still red in 30 minutes the merge drain adds \`needs-maintainer\`, which is where a session looks.

$marker"
				w gh pr comment "$num" --repo "$REPO" --body "$body"
			fi
		elif [ "$labelled" = "true" ]; then
			echo "$ME: CLEARED #$num at $(printf '%s\n' "$sha" | cut -c1-9) ($title)"
			w gh pr edit "$num" --repo "$REPO" --remove-label "$LABEL"
		fi
	done
