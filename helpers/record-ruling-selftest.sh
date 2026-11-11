#!/bin/sh
# helpers/record-ruling-selftest.sh: prove script/record-ruling posts the ruling with the marker
# helpers/open-question.jq reads, then swaps needs-architect for architect-ruled through the GraphQL
# addLabelsToLabelable and removeLabelsFromLabelable mutations (no REST call is allowed), with a stub `gh` and no network. script/lint-fleet runs it. Cases: a plain
# ruling; a send-back adds held-by-lane; --partial posts a marker that closes nothing and edits no label; an item without needs-architect
# is not asked to remove it; a swap that does not take exits 1; an issue is read through the same query.
set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
tool="$here/../script/record-ruling"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
# The stub logs every call and speaks GraphQL only: any `gh api repos/...` (REST) call fails the
# run, as does `gh pr`/`gh issue` (comments and labels go through mutations). A query answers from
# $STUB_KIND and from a label file that addLabelsToLabelable and removeLabelsFromLabelable rewrite
# (label ids are the names prefixed `L_`), unless $STUB_EDIT_FAILS is set.
cat >"$tmp/gh" <<'STUB'
#!/bin/sh
echo "$*" >>"$STUB_LOG"
[ "$1" = api ] && [ "$2" = graphql ] || { echo "REST call: $*" >>"$STUB_LOG"; exit 1; }
shift 2
q=""
while [ $# -gt 0 ]; do
	case "$1" in
	-f) case "$2" in query=*) q="${2#query=}" ;; b=*) echo "BODY: ${2#b=}" >>"$STUB_LOG" ;; esac; shift 2 ;;
	*) shift ;;
	esac
done
case "$q" in
*'issueOrPullRequest'*)
	typename=Issue; [ "$STUB_KIND" = pr ] && typename=PullRequest
	nodes="$(while read -r l; do [ -n "$l" ] && printf '{"name":"%s"},' "$l"; done <"$STUB_LABELS")"
	printf '{"data":{"repository":{"issueOrPullRequest":{"__typename":"%s","id":"ITEM","labels":{"nodes":[%s{"name":"zz"}]}},"ruled":{"id":"L_architect-ruled"},"held":{"id":"L_held-by-lane"},"asking":{"id":"L_needs-architect"}}}}\n' "$typename" "$nodes" ;;
*addComment*) ;;
*addLabelsToLabelable*)
	[ -n "${STUB_EDIT_FAILS:-}" ] && exit 1
	echo "$q" | grep -o 'L_[a-z-]*' | while read -r id; do echo "${id#L_}" >>"$STUB_LABELS"; done ;;
*removeLabelsFromLabelable*)
	[ -n "${STUB_EDIT_FAILS:-}" ] && exit 1
	id="$(echo "$q" | grep -o 'L_[a-z-]*' | head -1)"
	grep -vx "${id#L_}" "$STUB_LABELS" >"$STUB_LABELS.new"; mv "$STUB_LABELS.new" "$STUB_LABELS" ;;
esac
STUB
chmod +x "$tmp/gh"
fail=0
# run <name> <want-exit> <kind> <labels> [args...]
run() {
	name="$1" want="$2" kind="$3" labels="$4"
	shift 4
	: >"$tmp/log"
	printf '%s\n' $labels >"$tmp/labels"
	STUB_LOG="$tmp/log" STUB_LABELS="$tmp/labels" STUB_KIND="$kind" PATH="$tmp:$PATH" \
		sh "$tool" 7 --repo o/r "$@" >/dev/null 2>&1
	rc=$?
	[ "$rc" = "$want" ] || { echo "FAIL $name: exit $rc, want $want" >&2; fail=1; }
}
has() { grep -qF -- "$2" "$tmp/$1" || { echo "FAIL $name: $1 lacks: $2" >&2; fail=1; }; }
lacks() { ! grep -qF -- "$2" "$tmp/$1" || { echo "FAIL $name: $1 has: $2" >&2; fail=1; }; }

run plain 0 pr "needs-architect ci-failing" --text '"Yes."'
has log '<!-- architect-ruling -->'
has log 'addComment'
lacks log 'REST call'
has log 'addLabelsToLabelable'
has log 'removeLabelsFromLabelable'
has labels architect-ruled
lacks labels needs-architect
[ "$(grep -n 'addComment' "$tmp/log" | cut -d: -f1)" -lt "$(grep -n 'addLabelsToLabelable' "$tmp/log" | head -1 | cut -d: -f1)" ] &&
	[ "$(grep -n 'addLabelsToLabelable' "$tmp/log" | head -1 | cut -d: -f1)" -lt "$(grep -n 'removeLabelsFromLabelable' "$tmp/log" | head -1 | cut -d: -f1)" ] ||
	{ echo "FAIL plain: the labels moved before the ruling was posted" >&2; fail=1; }

run sent-back 0 pr "needs-architect" --text 'Build it.' --held-by-lane
has log 'L_architect-ruled","L_held-by-lane'
has labels held-by-lane

run partial 0 pr "needs-architect" --partial --text 'Fork 1: yes.'
has log '<!-- architect-ruling partial -->'
lacks log 'addLabelsToLabelable'
has labels needs-architect

run no-hold 0 pr "" --link https://example.invalid/r
has log 'Recorded at https://example.invalid/r.'
lacks log 'removeLabelsFromLabelable'

export STUB_EDIT_FAILS=1
run swap-fails 1 pr "needs-architect" --text 'Yes.'
unset STUB_EDIT_FAILS

run issue 0 issue "needs-architect" --text 'Yes.'
has log 'addComment'
has labels architect-ruled
lacks labels needs-architect

run no-ruling 2 pr "needs-architect"

[ "$fail" = 0 ] && echo "record-ruling-selftest: ok"
exit "$fail"
