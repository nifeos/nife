#!/bin/sh
# shellcheck shell=sh
#
# helpers/architect-label-unflag-selftest.sh: helpers/architect-label-unflag.sh against a stub `gh`.
# No network. The stub prints the pull request node held in $tmp/node for any GraphQL call, logs
# every edit and comment to $tmp/log, and prints $tmp/comments for `gh pr view --json comments`.
#
# Each case starts from a pull request the labeler flagged (github-actions added `needs-architect`,
# its finding comment names `decisions` on a decision file) and changes one thing. Only the first
# case may remove the label; every other condition must keep it and never call `--remove-label`.

set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="architect-label-unflag-selftest"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/architect-label-unflag-selftest.XXXXXX")" || exit 1
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
cat >"$tmp/bin/gh" <<'STUB'
#!/bin/sh
d="$STUB_DIR"
case "$*" in
"api graphql"*) cat "$d/node" ;;
"pr edit"*) echo "gh $*" >>"$d/log"; [ -z "${STUB_EDIT_FAIL:-}" ] ;;
"pr view"*) cat "$d/comments" 2>/dev/null ;;
"pr comment"*) echo "gh $*" >>"$d/log" ;;
*) echo "stub gh: unexpected: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$tmp/bin/gh"

finding='`needs-architect` was added because this diff matched `helpers/architect-label-rules.py`.

- **Do you approve this edit to §130 (how the userspace clock service reads the x86 CMOS RTC)?**
  (decisions, `design/decisions/0130-cmos-rtc-delegation.md`, matched `# 130.`)

<!-- architect-label-rules-finding -->'
base="$(jq -n --arg f "$finding" '{
  headRefOid: "hhhh", body: "**Lane:** x", createdAt: "2026-11-11T00:00:00Z", url: "u",
  labels: {nodes: [{name: "needs-architect"}]},
  comments: {nodes: [{createdAt: "2026-11-11T00:39:30Z", url: "c1", body: $f,
                      author: {__typename: "Bot", login: "github-actions"}}]},
  labelEvents: {nodes: [{createdAt: "2026-11-11T00:39:25Z", label: {name: "needs-architect"},
                         actor: {__typename: "Bot", login: "github-actions"}}]},
  files: {pageInfo: {hasNextPage: false},
          nodes: [{path: "design/decisions/0130-cmos-rtc-delegation.md", changeType: "MODIFIED"}]}}')"

fail=0
# run <name> <want exit> <jq edit of the base node>
run() {
	rm -f "$tmp/log" "$tmp/comments"
	printf '%s' "$base" | jq "$3" >"$tmp/node"
	out="$(PATH="$tmp/bin:$PATH" STUB_DIR="$tmp" "$here/architect-label-unflag.sh" 42 o/r 2>&1)"
	got=$?
	if [ "$got" != "$2" ]; then
		echo "$me: $1: exit $got (want $2): $out" >&2; fail=1
	fi
	if [ "$2" != 0 ] && grep -q -- '--remove-label' "$tmp/log" 2>/dev/null; then
		echo "$me: $1: removed the label: $(cat "$tmp/log")" >&2; fail=1
	fi
}

# 1. Every condition holds: removed, and one comment names the rule and file that fired.
run "a stale bot flag comes off" 0 '.'
grep -qF -- 'pr edit 42 --repo o/r --remove-label needs-architect' "$tmp/log" || { echo "$me: case 1 did not remove the label" >&2; fail=1; }
grep -qF -- 'decisions: design/decisions/0130-cmos-rtc-delegation.md' "$tmp/log" || { echo "$me: case 1's comment does not name the rule: $(cat "$tmp/log")" >&2; fail=1; }
grep -qF -- 'architect-label-unflagged:hhhh' "$tmp/log" || { echo "$me: case 1's comment has no marker" >&2; fail=1; }
grep -q -- 'architect-ruled' "$tmp/log" && { echo "$me: case 1 touched architect-ruled: $(cat "$tmp/log")" >&2; fail=1; }

# 1b. The marker for this head is already there: removed, no second comment.
rm -f "$tmp/log"
printf '%s' "$base" >"$tmp/node"
echo "earlier <!-- architect-label-unflagged:hhhh -->" >"$tmp/comments"
PATH="$tmp/bin:$PATH" STUB_DIR="$tmp" "$here/architect-label-unflag.sh" 42 o/r >/dev/null 2>&1
if grep -q 'pr comment' "$tmp/log"; then echo "$me: case 1b commented twice for one head" >&2; fail=1; fi

# 2. Each condition that keeps the label.
run "a person added it" 1 '.labelEvents.nodes[0].actor = {__typename: "User", login: "calef"}'
run "a person re-added it after the bot" 1 '.labelEvents.nodes += [{createdAt: "2026-11-11T01:00:00Z", label: {name: "needs-architect"}, actor: {__typename: "User", login: "calef"}}]'
run "no label event in the window" 1 '.labelEvents.nodes = []'
run "the label is not on" 1 '.labels.nodes = []'
run "architect-ruled is on" 1 '.labels.nodes += [{name: "architect-ruled"}]'
run "held-by-lane is on" 1 '.labels.nodes += [{name: "held-by-lane"}]'
run "a ruling was recorded" 1 '.comments.nodes += [{createdAt: "2026-11-11T01:00:00Z", url: "c2", body: "ruled\n<!-- architect-ruling -->", author: {__typename: "User", login: "calef"}}]'
run "a partial ruling was recorded" 1 '.comments.nodes += [{createdAt: "2026-11-11T01:00:00Z", url: "c2", body: "fork 1\n<!-- architect-ruling partial -->", author: {__typename: "User", login: "calef"}}]'
run "architect-ruled was applied once" 1 '.labelEvents.nodes = [{createdAt: "2026-11-10T00:00:00Z", label: {name: "architect-ruled"}, actor: {__typename: "User", login: "calef"}}] + .labelEvents.nodes'
run "a question is open" 1 '.comments.nodes += [{createdAt: "2026-11-11T01:00:00Z", url: "c2", body: "## What I need from you\nIs this a wire format?", author: {__typename: "User", login: "calef"}}]'
run "a question in the body" 1 '.body = "## What I need from you\nRule on the layout."'
run "it adds a correction of error" 1 '.files.nodes += [{path: "notes/coes/2026-11-11-a-thing.md", changeType: "ADDED"}]'
run "the file list is longer than a page" 1 '.files.pageInfo.hasNextPage = true'
run "the file list is missing" 1 'del(.files)'

# 3. GitHub cannot be read, or the removal fails: exit 2, and no comment.
run "unreadable" 2 'null'
rm -f "$tmp/log"
printf '%s' "$base" >"$tmp/node"
PATH="$tmp/bin:$PATH" STUB_DIR="$tmp" STUB_EDIT_FAIL=1 "$here/architect-label-unflag.sh" 42 o/r >/dev/null 2>&1
got=$?
[ "$got" = 2 ] || { echo "$me: a failed removal exited $got, want 2" >&2; fail=1; }
if grep -q 'pr comment' "$tmp/log"; then echo "$me: a failed removal still commented" >&2; fail=1; fi

# 4. The workflow runs this only when the rules found nothing, and never removes architect-ruled.
wf="$here/../.github/workflows/architect-label.yml"
grep -q 'helpers/architect-label-unflag.sh' "$wf" || { echo "$me: architect-label.yml does not run the unflag step" >&2; fail=1; }
grep -q "steps.rules.outputs.unflag == 'true'" "$wf" || { echo "$me: the unflag step is not gated on the rules finding nothing" >&2; fail=1; }
if grep -v '^[[:space:]]*#' "$wf" "$here/architect-label-unflag.sh" | grep -q -- '--remove-label architect-ruled'; then
	echo "$me: something removes architect-ruled" >&2; fail=1
fi

[ "$fail" = 0 ] && echo "$me: ok"
exit "$fail"
