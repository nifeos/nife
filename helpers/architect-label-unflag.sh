#!/bin/sh
# shellcheck shell=sh
#
# helpers/architect-label-unflag.sh: take `needs-architect` off a pull request when the architect
# labeler put it on and nothing it reads still fires. Provisional name.
#
#     helpers/architect-label-unflag.sh <pr-number> <owner/repo>
#
# .github/workflows/architect-label.yml runs this only when helpers/architect-label-rules.py exited
# 1 against every merge base (no rule fired) and named no finding another pull request's ruling
# covered. This reads the pull request and lets helpers/architect-label-unflag.jq decide; its header
# lists the conditions, each of which keeps the label. When all hold it removes the label and posts
# one comment naming the rules that no longer match, marker-guarded per head. It never touches
# `architect-ruled` or a label a person added.
#
# Why: on #3450 a merge of `main` emptied the decision diff the labeler had matched, and the label
# stayed, so calef cleared a hold that held nothing to rule. The labeler had no way to take back its
# own flag. calef ruled on #1792 (2026-10-06 UTC) that the merge drain never removes the label, and
# it still does not; this is the labeler undoing only its own false flag.
#
# Exit 0 when it removed the label, 1 when it kept it (the reason on stderr), 2 when it could not
# read the pull request or remove the label. The workflow never fails on any of them.
# Selftest: helpers/architect-label-unflag-selftest.sh.

set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="architect-label-unflag"
pr="$1"
repo="$2"
owner="${repo%%/*}"
name="${repo#*/}"

query='query($owner:String!,$name:String!,$n:Int!){repository(owner:$owner,name:$name){pullRequest(number:$n){
  headRefOid body createdAt url
  labels(first:30){nodes{name}}
  comments(last:100){nodes{createdAt url body author{__typename login}}}
  labelEvents:timelineItems(last:100,itemTypes:[LABELED_EVENT]){nodes{... on LabeledEvent{createdAt actor{__typename login} label{name}}}}
  files(first:100){pageInfo{hasNextPage} nodes{path changeType}}}}}'

if ! node="$(gh api graphql -F owner="$owner" -F name="$name" -F n="$pr" -f query="$query" \
	--jq '.data.repository.pullRequest' 2>/dev/null)" || [ -z "$node" ] || [ "$node" = null ]; then
	echo "$me: could not read #$pr; needs-architect stays." >&2
	exit 2
fi
if ! verdict="$(printf '%s' "$node" | jq -c "$(cat "$here/open-question.jq" "$here/architect-label-unflag.jq")" 2>/dev/null)"; then
	echo "$me: could not decide on #$pr; needs-architect stays." >&2
	exit 2
fi
if [ "$(printf '%s' "$verdict" | jq -r '.remove')" != true ]; then
	echo "$me: needs-architect stays on #$pr: $(printf '%s' "$verdict" | jq -r '.why')." >&2
	exit 1
fi

head="$(printf '%s' "$verdict" | jq -r '.head')"
if ! gh pr edit "$pr" --repo "$repo" --remove-label needs-architect >/dev/null 2>&1; then
	echo "$me: could not remove needs-architect from #$pr." >&2
	exit 2
fi
echo "$me: removed needs-architect from #$pr at head $head"

marker="architect-label-unflagged:$head"
if gh pr view "$pr" --repo "$repo" --json comments --jq '.comments[].body' 2>/dev/null | grep -qF -- "$marker"; then
	exit 0
fi
rules="$(printf '%s' "$verdict" | jq -r 'if (.rules | length) > 0 then .rules[] | "- `\(.)`" else "- the rule named in the labeler'"'"'s finding comment above" end')"
at="$(printf '%s' "$verdict" | jq -r '.at')"
by="$(printf '%s' "$verdict" | jq -r '.by')"
body="$(cat <<BODY
\`needs-architect\` came off. The labeler ($by) put it on at $at for:

$rules

At head \`$head\` no rule in \`helpers/architect-label-rules.py\` matches the diff from any merge base with the base branch, no ruling is recorded here and no question for calef is open, so the hold had nothing left to rule. Only a label a bot added comes off this way; one a person adds stays until a ruling is recorded.

<!-- $marker -->
BODY
)"
if ! gh pr comment "$pr" --repo "$repo" --body "$body" >/dev/null 2>&1; then
	echo "$me: removed the label but could not post the comment on #$pr." >&2
fi
exit 0
