#!/bin/sh
#
# helpers/ci-failing-selftest.sh: the decision in helpers/ci-failing.jq against a recorded commit
# and against synthetic cases, and helpers/ci-failing.sh checked for staying a reporter. Provisional
# names, with the script. script/lint-fleet runs it.
#
# The recorded fixture is the check runs of one real pull request head, kept to five names: its
# `ready status` ran twice, failed first and passed on the later run, which is the stale failure
# #1592 showed and the reason the decision reads the newest run per name.

set -e
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="$(basename "$0")"

if ! command -v jq >/dev/null 2>&1; then
	echo "$me: jq is not installed, and the script cannot run without it either." >&2
	exit 1
fi

program="$(cat "$here/ci-failing.jq")"
# decide <runs JSON> <required JSON>: the failing names, joined.
decide() {
	printf '%s\n' "$1" | jq -r --argjson required "$2" "$program"' failing($required) | map(.name) | join(";")'
}
expect() {
	if [ "$2" != "$3" ]; then
		echo "$me: $1" >&2
		echo "  expected '$3'" >&2
		echo "  got      '$2'" >&2
		exit 1
	fi
}

req='["ready status (no IN-PROGRESS block on a ready branch)","clippy","rustfmt","architect hold (needs-architect label)"]'

# 1. Recorded: a failure superseded by a later success is not failing.
expect "a superseded failure was reported" \
	"$(decide "$(cat "$here/ci-failing-fixtures/recorded-2026-10-04-superseded-failure.json")" "$req")" ""

# 2. Synthetic.
runs='[
 {"id":1,"name":"clippy","conclusion":"failure","html_url":"u1"},
 {"id":2,"name":"rustfmt","conclusion":"success","html_url":"u2"},
 {"id":3,"name":"rustfmt","conclusion":"failure","html_url":"u3"},
 {"id":4,"name":"not required","conclusion":"failure","html_url":"u4"},
 {"id":5,"name":"architect hold (needs-architect label)","conclusion":"failure","html_url":"u5"}]'
expect "newest failure, an unrequired failure or the architect hold was mishandled" \
	"$(decide "$runs" "$req")" "clippy;rustfmt"
expect "a newer in-progress run must hide an older failure" \
	"$(decide '[{"id":1,"name":"clippy","conclusion":"failure"},{"id":2,"name":"clippy","conclusion":null,"status":"in_progress"}]' "$req")" ""
expect "a newer cancelled run must hide an older failure" \
	"$(decide '[{"id":1,"name":"clippy","conclusion":"failure"},{"id":2,"name":"clippy","conclusion":"cancelled"}]' "$req")" ""
expect "timed_out must count as failing" \
	"$(decide '[{"id":1,"name":"clippy","conclusion":"timed_out"}]' "$req")" "clippy"
expect "a required check with no run must not count" "$(decide '[]' "$req")" ""

# 3. The script reports and never resolves.
if grep -v '^[[:space:]]*#' "$here/ci-failing.sh" | grep -Eq 'dequeue|disable-auto|merge --auto|run rerun|enqueue'; then
	echo "$me: ci-failing.sh changes the queue or reruns CI; it must only label and comment." >&2
	exit 1
fi
# 4. A large head. A real head has well over a hundred check runs, each a few hundred bytes with
#    its URLs; 10,000 of them pass Linux's 128 KiB limit on one argument and macOS's 1 MiB total, which the script once
#    hit by passing the runs as `--argjson` ("Argument list too long", run 37227933655). The whole
#    pass runs against a stubbed `gh` so the real invocation is the one exercised.
stub="$(mktemp -d)"
trap 'rm -rf "$stub"' EXIT
jq -nc '[range(0; 10000) | {id: ., name: "check \(. % 50)", status: "completed", conclusion: "success",
	html_url: ("https://github.com/nifeos/nife/actions/runs/1/job/" + ("x" * 60))}]
	+ [{id: 9999, name: "clippy", status: "completed", conclusion: "failure", html_url: "https://x/9"}]' |
	jq -c '.[]' >"$stub/runs"
[ "$(wc -c <"$stub/runs")" -gt 1200000 ] || { echo "$me: the large fixture is not large" >&2; exit 1; }
cat >"$stub/gh" <<STUB
#!/bin/sh
case "\$*" in
"api repos/nifeos/nife/rulesets --jq"*) echo 1 ;;
"api repos/nifeos/nife/rulesets/1 --jq"*) echo '["clippy"]' ;;
"pr list"*) echo '[{"number":7,"isDraft":false,"headRefOid":"aaaaaaaaaaaa","labels":[],"title":"big"}]' ;;
*check-runs*) cat "$stub/runs" ;;
*comments*) : ;;
*) echo "unexpected gh \$*" >&2; exit 1 ;;
esac
STUB
chmod +x "$stub/gh"
out="$(PATH="$stub:$PATH" CI_FAILING_INSTANCE=selftest "$here/ci-failing.sh" --dry-run 2>&1)" || {
	echo "$me: ci-failing.sh failed on a large check-run list:" >&2
	echo "$out" >&2
	exit 1
}
case "$out" in
*"FAILING #7"*clippy*) ;;
*)
	echo "$me: the large head's failing clippy was not reported:" >&2
	echo "$out" >&2
	exit 1
	;;
esac
# 5. Escapes inside the JSON. A check run's `output.summary` holds "\n" as two characters, and
#    dash's `echo` turns them into a real newline, which is a control character inside a JSON
#    string: jq died with "Invalid string: control characters" and echo then reported
#    "I/O error" (scheduled runs of ci-failing from 2026-10-31, issue 2960). The script passes
#    JSON with `printf '%s\n'`, which never interprets backslashes. A red head must still be reported.
printf '%s\n' '{"id":1,"name":"clippy","status":"completed","conclusion":"failure","html_url":"https://x/1","output":{"summary":"line one\nline two\ttab \\ slash"}}' >"$stub/runs"
out="$(PATH="$stub:$PATH" CI_FAILING_INSTANCE=selftest "$here/ci-failing.sh" --dry-run 2>&1)" || {
	echo "$me: ci-failing.sh failed on check runs whose JSON holds backslash escapes:" >&2
	echo "$out" >&2
	exit 1
}
case "$out" in
*"FAILING #7"*clippy*) ;;
*)
	echo "$me: a failing head with backslash escapes in its check runs was not reported:" >&2
	echo "$out" >&2
	exit 1
	;;
esac
echo "$me: ok"
