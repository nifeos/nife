#!/bin/sh
#
# Selftest: .githooks/pre-push runs no `gh` command on a checkout that has not opted in to the
# fleet tooling. Run by script/lint.
#
# Milestone 889 (the tree's gates stop depending on the fleet tooling), provisional. The hook used
# to ask GitHub two questions on every push from every clone; a stranger's push should pass or fail
# on the tree alone. This builds a throwaway repository with the real hook, stubs for script/fmt and
# script/lint (so it costs nothing), and a `gh` on PATH that records any call and fails. A push that
# changes a file goes through the whole hook, and the record must stay empty. Then the opt-in is
# set with no fleet helper in the tree, as in a clone with the fleet tooling moved out, and the hook
# must say the fleet checks did not run. helpers/pre-push-fleet-selftest.sh, run by
# script/lint-fleet, holds the fleet half, and proves the same stub sees a call when one is made.
#
# No network, and the real repository is never touched: every git command names the throwaway one.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
hook="$here/../.githooks/pre-push"
me="pre-push-selftest"

# A hook exports GIT_DIR, and this may run under one (helpers/scope-merge-base-selftest.sh has the
# 2026-10-03 incident). Clear it so every command below acts on the throwaway repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-pre-push-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
repo="$tmp/repo"
mkdir -p "$repo/script" "$repo/.githooks" "$tmp/bin"
g() { git -C "$repo" -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }

g init -q
g config user.name selftest
g config user.email selftest@invalid
cp "$hook" "$repo/.githooks/pre-push"
for s in fmt lint; do
	printf '#!/bin/sh\nexit 0\n' >"$repo/script/$s"
	chmod +x "$repo/script/$s"
done
echo one >"$repo/file"
g add -A
g commit -q -m one
before="$(g rev-parse HEAD)"
echo two >"$repo/file"
g commit -q -am two
after="$(g rev-parse HEAD)"

# The stub records its arguments and fails, as `gh` does offline.
calls="$tmp/gh-calls"
printf '#!/bin/sh\necho "gh $*" >>"%s"\nexit 1\n' "$calls" >"$tmp/bin/gh"
chmod +x "$tmp/bin/gh"

push() {
	: >"$calls"
	(cd "$repo" && printf 'refs/heads/main %s refs/heads/main %s\n' "$after" "$before" |
		PATH="$tmp/bin:$PATH" sh .githooks/pre-push origin "file://$tmp/remote")
}

if ! push >"$tmp/out" 2>&1; then
	echo "$me: the hook refused a push with every gate stubbed to pass:" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
fi
if [ -s "$calls" ]; then
	echo "$me: without nife.fleet the hook still called gh:" >&2
	sed 's/^/  | /' "$calls" >&2
	exit 1
fi

# The opt-in reaches the fleet half. Without its helper here the hook must say so rather than pass
# in silence, which is what a stranger's tree with the fleet tooling moved out would see.
g config nife.fleet true
push >"$tmp/out" 2>&1 || true
if ! grep -q 'nife.fleet is set but helpers/pre-push-fleet.sh is missing' "$tmp/out"; then
	echo "$me: with nife.fleet set and no fleet helper, the hook did not say the fleet checks were skipped:" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
fi

echo "pre-push selftest: no gh call without nife.fleet; the opt-in without its helper says so"
# The fleet half runs beside the text checks and is joined after them (2026-10-31). A stub helper
# answers `queued` with success and `after` as the files in $tmp/fleet say: `started` is touched
# when it begins, `exit` holds its exit status, `hold` holds seconds to wait before it ends.
mkdir -p "$repo/helpers" "$tmp/fleet"
cat >"$repo/helpers/pre-push-fleet.sh" <<'STUB'
#!/bin/sh
cat >/dev/null
[ "$1" = after ] || exit 0
: >"$FLEET_DIR/started"
sleep "$(cat "$FLEET_DIR/hold" 2>/dev/null || echo 0)"
echo "fleet stub: ran"
if [ "$(cat "$FLEET_DIR/exit" 2>/dev/null || echo 0)" != 0 ]; then
	echo "fleet stub: failed on purpose" >&2
	exit 1
fi
STUB
chmod +x "$repo/helpers/pre-push-fleet.sh"
FLEET_DIR="$tmp/fleet"
export FLEET_DIR
# The lint stub proves overlap: it waits (at most 5 s) for the fleet stub to have started, which an
# in-order hook never lets happen, and then passes or fails as $FLEET_DIR/lint says.
cat >"$repo/script/lint" <<'STUB'
#!/bin/sh
n=0
while [ ! -f "$FLEET_DIR/started" ] && [ "$n" -lt 50 ]; do sleep 0.1; n=$((n + 1)); done
[ -f "$FLEET_DIR/started" ] || { echo "lint stub: the fleet half had not started" >&2; exit 2; }
if [ "$(cat "$FLEET_DIR/lint" 2>/dev/null || echo 0)" != 0 ]; then
	echo "lint stub: failed on purpose" >&2
	exit 1
fi
STUB
chmod +x "$repo/script/lint"
g add -A
g commit -q -m stubs
after="$(g rev-parse HEAD)"
reset_fleet() { rm -f "$FLEET_DIR"/*; }

reset_fleet
if ! push >"$tmp/out" 2>&1; then
	echo "$me: with both halves passing the hook refused the push, or the fleet half did not start before the text checks ended:" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
fi
grep -q 'fleet stub: ran' "$tmp/out" || { echo "$me: the passing fleet half's output was dropped" >&2; exit 1; }

reset_fleet
echo 1 >"$FLEET_DIR/exit"
if push >"$tmp/out" 2>&1; then
	echo "$me: a failing fleet half did not fail the push" >&2
	exit 1
fi
grep -q 'fleet stub: failed on purpose' "$tmp/out" || { echo "$me: the failing fleet half's message was not shown" >&2; exit 1; }

reset_fleet
echo 1 >"$FLEET_DIR/lint"
echo 1 >"$FLEET_DIR/exit"
if push >"$tmp/out" 2>&1; then
	echo "$me: a failing text check did not fail the push" >&2
	exit 1
fi
grep -q 'lint stub: failed on purpose' "$tmp/out" || { echo "$me: the failing text check was not reported" >&2; exit 1; }
if grep -q 'fleet stub: failed on purpose' "$tmp/out"; then
	echo "$me: with both failing, the fleet half was reported before the text checks" >&2
	exit 1
fi

# A text-check failure does not wait out a slow fleet half.
reset_fleet
echo 1 >"$FLEET_DIR/lint"
echo 12 >"$FLEET_DIR/hold"
t0=$(date +%s)
push >"$tmp/out" 2>&1 || true
if [ $(($(date +%s) - t0)) -ge 10 ]; then
	echo "$me: a failed text check waited for the fleet half" >&2
	exit 1
fi

echo "pre-push selftest: the fleet half runs beside the text checks, is joined after them, and never outlasts a text failure"

