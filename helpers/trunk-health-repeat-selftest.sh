#!/bin/sh
#
# Selftest: the repeat half of helpers/trunk-health.sh reports a workflow whose scheduled failures
# say the same thing again, and reports churn without folding it in. Run by script/lint.
#
# Milestone 422 (a cadence job whose healthy state is red, and nothing that notices it repeating),
# provisional. The grouping thresholds and the log-frame strip decide whether a standing failure is
# seen at all, so this proves them offline rather than trusting the next week of real failures to
# do it: a stub `gh` serving synthetic run lists and synthetic `--log-failed` logs, a stub `git`
# answering the two calls `state()` makes, and one real `--once` pass whose repeat rows are then
# read back. No network, and the real repository's run history is never touched; the workflow names
# are this tree's so the scheduled-file grep finds them, and every date in the fixtures is a
# synthetic UTC stand-in for the real 2026-09..10 audit-cadence specimen.
#
# What each fixture pins, and what breaks if the code regresses:
#
#   - audit-cadence, four of six failing runs ending in one line: the REPEAT row, with the line
#     and the span of dates. Inside it, one run's log carries teardown noise ahead of the error
#     annotation (dropped by name, or the group reads 3), one carries only the echoed script
#     (dropped as a frame line, or the group reads CHURN), and one run failed on a different line
#     altogether and must stay out of the group. The newest log carries text after the annotation,
#     which the marker cutoff must exclude.
#   - falsifications, two failing runs on different lines: the CHURN row as its own finding.
#   - mutation, four failing runs where two share a line: CHURN, "no line three times", so a pair
#     never reads as a repeat.
#   - undefined-behavior-check, two cancelled and four failed runs: cancelled is not a failure, so
#     the row must say four, not six.
#   - vendor-watch, two failing runs on the same line: silence, because an unconfirmed repeat
#     waits for its third occurrence.
#   - toolchain-bump, all green: silence.
#   - trunk-health and merge-drain, fixture data present: silence, because the watcher files are
#     excluded the same way script/cadence-check excludes them.

set -eu
me="trunk-health-repeat-selftest"

# A watcher script may run under an exported GIT_DIR (helpers/scope-merge-base-selftest.sh has the
# 2026-10-03 incident); clear it so every git call reaches the stubs below.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-trunk-health-repeat-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
mkdir "$tmp/bin" "$tmp/fix"
fix="$tmp/fix"
export REPEAT_SELFTEST_FIXTURES="$fix"

esc="$(printf '\033')"

# `git`: only the two calls state() makes; everything else falls through to the real git.
cat >"$tmp/bin/git" <<EOF
#!/bin/sh
case "\$1" in
remote) [ "\$2" = "get-url" ] && { echo stub-origin; exit 0; } ;;
ls-remote)
	printf '0000000000000000000000000000000000000000\trefs/heads/main\n'
	exit 0 ;;
esac
exec /usr/bin/git "\$@"
EOF

# `gh`: canned answers per subcommand. `gh api --jq` filtering happens inside the real gh, so the
# fixtures are the post-jq shape the script reads. A non-workflow api call fails the way cadence-check
# expects a dead network to, which keeps its own rows out of the output.
cat >"$tmp/bin/gh" <<EOF
#!/bin/sh
case "\$1 \$2" in
"run list") cat "$fix/state-runs.json"; exit 0 ;;
"run view")
	[ -f "$fix/log-\$3.txt" ] && { cat "$fix/log-\$3.txt"; exit 0; }
	exit 0 ;;
esac
if [ "\$1" = "api" ]; then
	case "\$2" in
	*/workflows/*)
		wf=\$(printf '%s' "\$2" | sed -E 's|.*/workflows/([^/]+)/runs.*|\1|')
		[ -f "$fix/runs-\$wf.tsv" ] && cat "$fix/runs-\$wf.tsv"
		exit 0 ;;
	esac
	exit 2
fi
exit 0
EOF
chmod +x "$tmp/bin/git" "$tmp/bin/gh"

frame() {
	# One log line as `gh run view --log-failed` frames it: job, step, timestamp, content.
	printf 'job name\tUNKNOWN STEP\t2026-10-16T00:00:00.0000000Z %s\n' "$1"
}

# --- state(): a red tip, so the pass never reaches the green-only merge-group api call ----------
printf '%s\n' '[{"workflowName":"ci","status":"completed","conclusion":"failure","headSha":"0000000000000000"}]' >"$fix/state-runs.json"

# --- runs lists, newest first, the shape `.workflow_runs[] | [conclusion, created_at, id] | @tsv` ---
failure_runs() {
	printf 'failure\t2026-10-12T17:14:12Z\t%s\nfailure\t2026-10-05T16:41:50Z\t%s\nfailure\t2026-09-28T16:21:04Z\t%s\nfailure\t2026-09-21T14:45:40Z\t%s\nfailure\t2026-09-14T14:38:35Z\t%s\nfailure\t2026-09-07T13:52:17Z\t%s\n' \
		900000000001 900000000002 900000000003 900000000004 900000000005 900000000006
}
failure_runs >"$fix/runs-audit-cadence.yml.tsv"

printf 'failure\t2026-10-12T13:36:42Z\t900000000011\nfailure\t2026-10-05T12:45:23Z\t900000000012\nsuccess\t2026-09-28T12:04:31Z\ts\nsuccess\t2026-09-21T11:10:46Z\ts\nsuccess\t2026-09-14T11:01:12Z\ts\nsuccess\t2026-09-07T10:47:57Z\ts\n' >"$fix/runs-falsifications.yml.tsv"

printf 'failure\t2026-10-12T12:57:57Z\t900000000021\nfailure\t2026-10-05T12:10:59Z\t900000000022\nfailure\t2026-09-28T11:36:33Z\t900000000023\nfailure\t2026-09-07T10:06:05Z\t900000000024\nsuccess\t2026-09-21T10:35:26Z\ts\nsuccess\t2026-09-14T10:30:17Z\ts\n' >"$fix/runs-mutation.yml.tsv"

printf 'cancelled\t2026-10-12T13:40:40Z\tc\ncancelled\t2026-10-05T12:49:46Z\tc\nfailure\t2026-09-28T12:09:42Z\t900000000031\nfailure\t2026-09-21T11:14:35Z\t900000000032\nfailure\t2026-09-14T11:04:37Z\t900000000033\nfailure\t2026-09-07T10:52:06Z\t900000000034\n' >"$fix/runs-undefined-behavior-check.yml.tsv"

printf 'failure\t2026-10-12T18:40:01Z\t900000000041\nfailure\t2026-10-05T18:40:01Z\t900000000042\nsuccess\t2026-09-28T18:40:01Z\ts\nsuccess\t2026-09-21T18:40:01Z\ts\nsuccess\t2026-09-14T18:40:01Z\ts\nsuccess\t2026-09-07T18:40:01Z\ts\n' >"$fix/runs-vendor-watch.yml.tsv"

printf 'success\t2026-10-12T17:13:36Z\ts\nsuccess\t2026-10-05T17:13:36Z\ts\nsuccess\t2026-09-28T17:13:36Z\ts\nsuccess\t2026-09-21T17:13:36Z\ts\nsuccess\t2026-09-14T17:13:36Z\ts\nsuccess\t2026-09-07T17:13:36Z\ts\n' >"$fix/runs-toolchain-bump.yml.tsv"

# Fixture data under an excluded watcher name: the row must never exist, data or no data.
failure_runs >"$fix/runs-trunk-health.yml.tsv"
failure_runs >"$fix/runs-merge-drain.yml.tsv"

# --- failing-step logs -------------------------------------------------------------------------
epilogue="design/audit-reports/README.md says how, and which lens the last one lacked."

# The newest audit-cadence run, with teardown and a later step's output after the annotation: both
# must stay out of the final line.
{
	frame '##[group]Run set -o pipefail'
	frame "${esc}[36;1mscript/audits --due | tee -a \"\$GITHUB_STEP_SUMMARY\"${esc}[0m"
	frame 'audits: 13 on record, DUE: documentation, security'
	frame 'Red means run the audit. Nothing here ran one, and nothing here can.'
	frame "$epilogue"
	frame '##[error]Process completed with exit code 1.'
	frame 'Cleaning up orphan processes'
	frame 'An audit is due. This job failing is information about the tree, not a defect'
} >"$fix/log-900000000001.txt"

{
	frame 'audits: 13 on record, DUE: documentation, security'
	frame "$epilogue"
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000002.txt"

# Only the echoed script and the annotation: nothing readable, so this run must land in the
# unreadable-line group rather than on a frame line.
{
	frame "${esc}[36;1mscript/audits --due | tee -a \"\$GITHUB_STEP_SUMMARY\"${esc}[0m"
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000003.txt"

# The real 2026-09-07 shape: teardown noise ahead of the annotation, which the named drops exist for.
{
	frame 'audits: 8 on record, DUE: documentation, security'
	frame "$epilogue"
	frame 'Cleaning up orphan processes'
	frame "Removing credentials config '/home/runner/work/_temp/git-credentials-fixture.config'"
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000004.txt"

# The run that broke for a different reason and must stay out of the repeat group.
{
	frame 'Traceback (most recent call last):'
	frame "FileNotFoundError: [Errno 2] No such file or directory: '/home/runner/work/nife/nife/user/Cargo.toml'"
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000005.txt"

{
	frame 'audits: 7 on record, DUE: documentation, security'
	frame "$epilogue"
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000006.txt"

{
	frame 'note: test exited abnormally; to see the full output pass --no-capture to the harness.'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000011.txt"

{
	frame '  survivor  system_tests::seeded_system_calls_agree_with_the_shadow_model'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000012.txt"

{
	frame '| universally_unique_identifier | `Uuid::v4_from_random` | `replace | with ^` | #1795 |'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000021.txt"

{
	frame '| video_terminal | `Vt::scrollback_depth` | `replace < with <=` | #1648 |'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000022.txt"

{
	frame '    or set it to 0 to run unbounded (helpers/memory-bounded-runner.sh).'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000023.txt"

{
	frame '| video_terminal | `Vt::scrollback_depth` | `replace < with <=` | #1648 |'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000024.txt"

{
	frame 'test tests::a_migration_overwrites_rather_than_accumulates ... '
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000031.txt"

{
	frame 'note: test exited abnormally; to see the full output pass --no-capture to the harness.'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000032.txt"

{
	frame 'note: test exited abnormally; to see the full output pass --no-capture to the harness.'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000033.txt"

{
	frame 'test cli::tests::a_build_without_boot_files_refuses_before_looking_at_disks ... '
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000034.txt"

{
	frame 'vendor: upstream moved past the pinned revision'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000041.txt"

{
	frame 'vendor: upstream moved past the pinned revision'
	frame '##[error]Process completed with exit code 1.'
} >"$fix/log-900000000042.txt"

# --- the pass, and what it must say ------------------------------------------------------------
PATH="$tmp/bin:$PATH" helpers/trunk-health.sh --once >"$tmp/out" 2>/dev/null

expect_row() {
	printf '%-32s %-24s %s\n' "$1" "$2" "$3" >"$tmp/expected"
	if ! grep -F -x -q -f "$tmp/expected" "$tmp/out"; then
		echo "$me: expected row absent:" >&2
		cat "$tmp/expected" >&2
		echo "$me: output was:" >&2
		cat "$tmp/out" >&2
		exit 1
	fi
}

expect_row audit-cadence "4 of last 6 failed" \
	"REPEAT: ${epilogue} (2026-09-07..2026-10-12 UTC)"
expect_row falsifications "2 of last 6 failed" \
	"CHURN: 2 of the last 6 failed, all 2 different"
expect_row mutation "4 of last 6 failed" \
	"CHURN: 4 of the last 6 failed, no line three times"
expect_row undefined-behavior-check "4 of last 6 failed" \
	"CHURN: 4 of the last 6 failed, no line three times"

n=$(grep -c 'REPEAT:' "$tmp/out" || true)
if [ "$n" -ne 1 ]; then
	echo "$me: expected exactly one REPEAT row, found ${n}" >&2
	exit 1
fi

for silent in vendor-watch toolchain-bump trunk-health merge-drain; do
	if grep -q "^${silent} " "$tmp/out"; then
		echo "$me: ${silent} should have produced no row:" >&2
		grep "^${silent} " "$tmp/out" >&2
		exit 1
	fi
done

# --- an API failure is UNKNOWN, never health ---------------------------------------------------
# A `gh` that fails the way a rate limit does (exit 1, message on stderr). The pass used to print
# `main is green` and no DEAD or REPEAT rows, because `state()` turned the failed call into an empty
# list, `repeat()` had `|| continue` on the runs fetch and the watcher check had `|| true`. Each
# guard has its own expectation below, so reverting any one of them turns this red.
mkdir "$tmp/failbin"
cat >"$tmp/failbin/gh" <<'EOF2'
#!/bin/sh
echo "gh: HTTP 403: API rate limit exceeded" >&2
exit 1
EOF2
cp "$tmp/bin/git" "$tmp/failbin/git"
chmod +x "$tmp/failbin/gh" "$tmp/failbin/git"
PATH="$tmp/failbin:$PATH" helpers/trunk-health.sh --once >"$tmp/out-fail" 2>/dev/null

expect_fail_out() {
	if ! grep -q -e "$1" "$tmp/out-fail"; then
		echo "$me: failing gh: expected <$1> in the output:" >&2
		cat "$tmp/out-fail" >&2
		exit 1
	fi
}
if grep -q 'main is green' "$tmp/out-fail"; then
	echo "$me: failing gh must never print 'main is green':" >&2
	cat "$tmp/out-fail" >&2
	exit 1
fi
expect_fail_out '^main: UNKNOWN at '
expect_fail_out '^repeat-check  *cannot ask  *CANNOT-ASK: unknown; .*audit-cadence'
expect_fail_out '^merge watchers  *cannot ask  *CANNOT-ASK'
expect_fail_out '^cadence-check  *cannot ask  *CANNOT-ASK'

# The other side of the exit-code test: a call that succeeds with an empty list is a quiet branch,
# and stays green. Without this, "UNKNOWN on anything empty" would pass the block above.
printf '[]' >"$fix/state-runs.json"
PATH="$tmp/bin:$PATH" helpers/trunk-health.sh --once >"$tmp/out-empty" 2>/dev/null
grep -q '^main is green at ' "$tmp/out-empty" ||
	{ echo "$me: an empty run list from a call that succeeded must read green" >&2; cat "$tmp/out-empty" >&2; exit 1; }

# --- the ruled home: one issue per distinct repeat, proved offline -------------------------------
# calef ruled the repeat report's home on 2026-10-16 (UTC): an issue per workflow on the
# helpers/watcher_watch.py pattern, deduped by workflow name, body updated when the failing line
# or span drifts, CHURN opening nothing, and the close being the audit lane's `Closes #N`. The
# stub gh here is a tiny GitHub: it records every call and keeps the issue store in a fixture, so
# the dedupe and the drift update are proved against state, not against mocks of one call.
iss="$tmp/issue"
mkdir -p "$iss/bin" "$iss/fix"
cat >"$iss/bin/gh" <<PYEOF
#!/usr/bin/env python3
import json, os, sys
fix = os.environ["REPEAT_ISSUE_FIXTURES"]
with open(os.path.join(fix, "calls.log"), "a") as log:
    # One JSON array per call: a multiline body stays on its own line, greppable as text.
    log.write(json.dumps(sys.argv[1:]) + "\n")
args = sys.argv[1:]
path = os.path.join(fix, "issues.json")
issues = json.load(open(path)) if os.path.exists(path) else []
if args[:2] == ["issue", "list"]:
    print(json.dumps(issues))
elif args[:2] == ["issue", "create"]:
    issue = {"number": 4200 + len(issues) + 1,
             "title": args[args.index("--title") + 1],
             "body": args[args.index("--body") + 1]}
    issues.append(issue)
    json.dump(issues, open(path, "w"))
    print("https://github.com/nifeos/nife/issues/%d" % issue["number"])
elif args[:2] == ["issue", "edit"]:
    number, body = int(args[2]), args[args.index("--body") + 1]
    for issue in issues:
        if issue["number"] == number:
            issue["body"] = body
    json.dump(issues, open(path, "w"))
PYEOF
chmod +x "$iss/bin/gh"

sync_rows() {
	REPEAT_ISSUE_FIXTURES="$iss/fix" PATH="$iss/bin:$PATH" python3 helpers/repeat_issue.py sync
}

audit_row="audit-cadence                    5 of last 6 failed       REPEAT: design/audit-reports/README.md says how, and which lens the last one lacked. (2026-09-07..2026-10-12 UTC)"
churn_row="falsifications                   2 of last 6 failed       CHURN: 2 of the last 6 failed, all 2 different"

expect_calls() {
	n=$(grep -F -c "$1" "$iss/fix/calls.log" 2>/dev/null || true)
	if [ "$n" -ne "$2" ]; then
		echo "$me: expected $2 of <$1>, saw $n" >&2
		cat "$iss/fix/calls.log" >&2
		exit 1
	fi
}

# A CHURN row opens nothing, updates nothing, asks nothing.
: >"$iss/fix/calls.log"
printf '%s\n' "$churn_row" | sync_rows >/dev/null
[ -s "$iss/fix/calls.log" ] && { echo "$me: a CHURN row must reach no gh call" >&2; exit 1; }

# The first pass opens exactly one issue, titled for the workflow, whose body states its close.
: >"$iss/fix/calls.log"
printf '%s\n' "$audit_row" | sync_rows >/dev/null
expect_calls '"issue", "create"' 1
grep -q 'scheduled failure repeats: audit-cadence' "$iss/fix/calls.log" ||
	{ echo "$me: the issue title must name the workflow and the repeat" >&2; exit 1; }
grep -q 'Closes #N' "$iss/fix/calls.log" ||
	{ echo "$me: the issue body must state its own close condition" >&2; exit 1; }

# The same row again: the open issue suppresses a second.
printf '%s\n' "$audit_row" | sync_rows >/dev/null
expect_calls '"issue", "create"' 1
expect_calls '"issue", "edit"' 0

# The failing line drifts: the body updates, still no second issue.
drift_row="audit-cadence                    5 of last 6 failed       REPEAT: a different line entirely (2026-09-14..2026-10-19 UTC)"
printf '%s\n' "$drift_row" | sync_rows >/dev/null
expect_calls '"issue", "create"' 1
expect_calls '"issue", "edit"' 1
grep -F -q '"issue", "edit", "4201"' "$iss/fix/calls.log" ||
	{ echo "$me: the edit must target the open issue" >&2; exit 1; }

# An issue that is closed no longer dedupes: the finding repeats, a new issue opens.
rm -f "$iss/fix/issues.json"
: >"$iss/fix/calls.log"
printf '%s\n' "$audit_row" | sync_rows >/dev/null
expect_calls '"issue", "create"' 1

echo "$me: repeat rows, thresholds, exclusions, silences, API failures and the ruled issue home all as expected"
