#!/bin/sh
#
# helpers/trunk-health-cadence-selftest.sh: prove the watcher's cadence stream treats a pass that
# could not ask as its own state, and never as an empty stream.
#
#     helpers/trunk-health-cadence-selftest.sh
#
# What it proves, against a scratch tree that copies the real helpers/trunk-health.sh and stands in
# fakes for everything that call reaches:
#
#   1. API down once, after rows: the stream carries an explicit `cannot ask` row and no recovery
#      line. This is the defect that opened the lane: the empty stream used to read as recovery.
#   2. API down across passes: the row prints once and the standing cannot-ask stays silent.
#   3. Recovery: when a real answer empties the stream, exactly one recovery line prints.
#   4. Rows after cannot-ask: real dead rows return with no recovery line beside them.
#   5. --once on an API-down pass prints the row too, being a snapshot rather than a watcher.
#
# The five passes come from a fake `sleep` that counts loop iterations and ends the watch when the
# scenario table is exhausted. A fake `git` answers as the main checkout to the watcher's worktree
# refusal and fails everywhere else, so `state()` reads an unknown tip and the test stays on the
# cadence half. `script/stranger-test` is stubbed to a quiet exit 0, and
# `script/effort` and `helpers/watcher_watch.py` are absent, so every other contribution to the
# cadence and DUE streams is empty by construction. Nothing here reaches the network or `gh`.
#
# Name: provisional. Minted 2026-10-16 (UTC) by the cadence-cannot-ask lane. `trunk-health` names
# the script under test, `cadence` the half of it, and `-selftest` is the suffix this directory
# already uses. calef has not ruled on it.
#
# BUGS: the fake sleep ends the watch by signaling the watched shell, so a change to how the loop
# sleeps or stops can break the harness before it breaks an assertion.

set -e
cd "$(dirname "$0")/.."

script="helpers/trunk-health.sh"
[ -f "$script" ] || { echo "trunk-health-cadence-selftest: $script is missing" >&2; exit 1; }

dir="$(mktemp -d "${TMPDIR:-/tmp}/trunk-health-cadence-selftest.XXXXXX")"
trap 'rm -rf "$dir"' EXIT
fail() { echo "trunk-health-cadence-selftest: $1" >&2; exit 1; }

mkdir -p "$dir/script" "$dir/helpers" "$dir/bin" "$dir/state"
cp "$script" "$dir/helpers/trunk-health.sh"

# The scenario table for the fake script/cadence-check: line 1 is its exit code, the rest is its
# stdout. A pass number is a loop iteration, read before that pass's cadence() call.
row_a="$(printf '%-32s %-24s %s\n' "mock-dead-cadence" never "DEAD: never succeeded")"
row_b="$(printf '%-32s %-24s %s\n' "mock-live-again" "2026-10-01T00:00:00Z" "DEAD: last success 2026-10-01")"
{ echo 1; printf '%s\n' "$row_a"; } >"$dir/state/scenario.0"   # the stream starts loud
{ echo 2; } >"$dir/state/scenario.1"                           # the API is down once
{ echo 2; } >"$dir/state/scenario.2"                           # and down again
{ echo 0; } >"$dir/state/scenario.3"                           # a real answer: all live
{ echo 1; printf '%s\n' "$row_b"; } >"$dir/state/scenario.4"   # rows return

cat >"$dir/bin/git" <<'EOF'
#!/bin/sh
# fake git: the main checkout's answer to the watcher's worktree refusal, failure everywhere else.
if [ "$1 $2" = "rev-parse --git-dir" ]; then
	echo .git
	exit 0
fi
exit 1
EOF

cat >"$dir/bin/sleep" <<'EOF'
#!/bin/sh
# fake sleep: counts passes and ends the watch when the scenario table is exhausted.
state="${CADENCE_SELFTEST_STATE:?}"
pass="$(cat "$state/pass" 2>/dev/null || echo 0)"
pass=$((pass + 1))
echo "$pass" >"$state/pass"
if [ "$pass" -ge 5 ]; then
	kill -TERM "$PPID" 2>/dev/null
fi
exit 0
EOF

cat >"$dir/script/cadence-check" <<'EOF'
#!/bin/sh
# fake script/cadence-check: answers from the scenario table for this pass.
state="${CADENCE_SELFTEST_STATE:?}"
pass="$(cat "$state/pass" 2>/dev/null || echo 0)"
scenario="$state/scenario.$pass"
[ -f "$scenario" ] || exit 0
code="$(head -n 1 "$scenario")"
tail -n +2 "$scenario"
exit "$code"
EOF

printf '%s\n' '#!/bin/sh' 'exit 0' >"$dir/script/stranger-test"
chmod +x "$dir/bin/git" "$dir/bin/sleep" "$dir/script/cadence-check" \
	"$dir/script/stranger-test"

# The watch form, five passes, output to a file. The harness's own sleep stays real; only the
# watched shell sees the fakes on PATH. A watchdog bounds the run, so a loop that will not end
# fails the test instead of hanging it. The signaling is deliberate, so the whole arrangement sits
# in a subshell whose stderr is dropped: the shell's own job-termination notices are expected here
# and are not part of what the test checks.
(
	CADENCE_SELFTEST_STATE="$dir/state" PATH="$dir/bin:$PATH" \
		sh "$dir/helpers/trunk-health.sh" >"$dir/watch.out" 2>"$dir/watch.err" &
	watch_pid=$!
	( sleep 60; kill -9 "$watch_pid" 2>/dev/null ) &
	watchdog_pid=$!
	wait "$watch_pid" 2>/dev/null || :
	kill "$watchdog_pid" 2>/dev/null || :
	wait "$watchdog_pid" 2>/dev/null || :
) 2>/dev/null || :

passes="$(cat "$dir/state/pass" 2>/dev/null || echo 0)"
[ "$passes" = 5 ] || fail "the watch ran $passes passes, wanted 5; it ended early or the watchdog fired"

# What the five passes must have said, in order: the first-pass announce of the standing rows, the
# cannot-ask row on the transition into a blind pass, silence while it stands, exactly one recovery
# line on the first real answer, and the returning rows with no recovery beside them. The row text
# is the contract; it must match the row helpers/trunk-health.sh prints.
cannot_row="$(printf '%-32s %-24s %s\n' "cadence-check" "cannot ask" \
	"CANNOT-ASK: unknown; script/cadence-check without --quiet says what it could not read")"
cat >"$dir/expected" <<EOF
$row_a
$cannot_row
every scheduled workflow is producing results again
$row_b
EOF

if ! diff -u "$dir/expected" "$dir/watch.out"; then
	sed 's/^/  watcher stderr: /' "$dir/watch.err" >&2
	fail "the cadence stream said the wrong thing across the five passes (diff above)"
fi

says="$(grep -c 'CANNOT-ASK:' "$dir/watch.out" || :)"
[ "$says" = 1 ] || fail "wanted exactly one CANNOT-ASK row, got $says"
clears="$(grep -c 'producing results again' "$dir/watch.out" || :)"
[ "$clears" = 1 ] || fail "wanted exactly one recovery line, got $clears"

# --once is a snapshot anybody may run anywhere; a blind pass reaches it too.
echo 1 >"$dir/state/pass"
once="$(CADENCE_SELFTEST_STATE="$dir/state" PATH="$dir/bin:$PATH" \
	sh "$dir/helpers/trunk-health.sh" --once)" || fail "--once exited nonzero on an API-down pass"
printf '%s\n' "$once" | grep -q 'CANNOT-ASK:' \
	|| fail "--once printed no cannot-ask row on an API-down pass"

echo "trunk-health-cadence-selftest: ok"
