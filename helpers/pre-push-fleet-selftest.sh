#!/bin/sh
#
# Selftest for helpers/pre-push-fleet.sh, the fleet half of .githooks/pre-push. Run by
# script/lint-fleet.
#
# Milestone 889 (the tree's gates stop depending on the fleet tooling), provisional. The hook calls
# the fleet half only when `git config nife.fleet` is true. This checks the half that runs then, in a
# throwaway repository holding the real hook, the real fleet helper and the real queued-push check,
# with stubs for script/fmt, script/lint and script/lint-fleet, and a `gh` on PATH that records each
# call and fails (as `gh` does offline, which the queued check must survive):
#
#   1. With the opt-in, a push reaches `gh` (the queued check and the ready-status read).
#   2. With the opt-in, a failing script/lint-fleet refuses the push.
#   3. Without it, neither happens: no `gh` call, and a failing script/lint-fleet is not run.
#   4. With the opt-in and NIFE_PUSH_QUEUED=1, the override `script/push --requeue` sets, the push
#      passes and the queued check does not ask GitHub.
#
# The override is unset first. `script/push --requeue` exports it to the hook, the hook runs
# script/lint-fleet, and script/lint-fleet runs this file, so an inherited value would skip the
# queued check in case 1 and fail the very push that set it.
#
# helpers/pre-push-selftest.sh, run by script/lint, holds the stranger's side on its own.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
me="pre-push-fleet-selftest"
unset NIFE_PUSH_QUEUED
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-pre-push-fleet-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
repo="$tmp/repo"
mkdir -p "$repo/script" "$repo/.githooks" "$repo/helpers" "$tmp/bin"
g() { git -C "$repo" -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }

g init -q
g config user.name selftest
g config user.email selftest@invalid
cp "$here/../.githooks/pre-push" "$repo/.githooks/pre-push"
cp "$here/pre-push-fleet.sh" "$here/push-queued-check.sh" "$here/push-rewrite-check.sh" "$here/stash-block.sh" "$repo/helpers/"
stub() { printf '#!/bin/sh\nexit %s\n' "$2" >"$repo/script/$1"; chmod +x "$repo/script/$1"; }
stub fmt 0
stub lint 0
stub lint-fleet 0
echo one >"$repo/file"
g add -A
g commit -q -m one
before="$(g rev-parse HEAD)"
echo two >"$repo/file"
g commit -q -am two
after="$(g rev-parse HEAD)"

calls="$tmp/gh-calls"
printf '#!/bin/sh\necho "gh $*" >>"%s"\nexit 1\n' "$calls" >"$tmp/bin/gh"
chmod +x "$tmp/bin/gh"

push() {
	: >"$calls"
	(cd "$repo" && printf 'refs/heads/main %s refs/heads/main %s\n' "$after" "$before" |
		PATH="$tmp/bin:$PATH" sh .githooks/pre-push origin "file://$tmp/remote") >"$tmp/out" 2>&1
}
fail() {
	echo "$me: $1" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
}

# 1. Opted in: the queued check asks GitHub, fails open on the stub's failure, and the push passes.
g config nife.fleet true
push || fail "with nife.fleet and every gate stubbed to pass, the hook refused the push"
grep -q '^gh api graphql' "$calls" || fail "with nife.fleet, the queued-branch check never called gh"

# 2. Opted in: a failing script/lint-fleet refuses the push and says which script.
stub lint-fleet 1
if push; then fail "with nife.fleet, a failing script/lint-fleet did not refuse the push"; fi
grep -q 'script/lint-fleet failed' "$tmp/out" || fail "the refusal did not name script/lint-fleet"

# 3. Not opted in: the same failing script/lint-fleet is not run, and gh is not called.
g config --unset nife.fleet
push || fail "without nife.fleet, the hook refused a push that only script/lint-fleet would refuse"
[ ! -s "$calls" ] || fail "without nife.fleet, the hook called gh: $(cat "$calls")"

# 4. Opted in, with the override script/push --requeue sets: the push passes without the queued
# check's GraphQL call.
g config nife.fleet true
stub lint-fleet 0
(NIFE_PUSH_QUEUED=1 && export NIFE_PUSH_QUEUED && push) ||
	fail "with nife.fleet and NIFE_PUSH_QUEUED=1, the hook refused the push"
if grep -q '^gh api graphql' "$calls"; then fail "with NIFE_PUSH_QUEUED=1, the queued-branch check still called gh"; fi

echo "pre-push-fleet selftest: the opt-in reaches gh and lint-fleet, the requeue override skips the queued check, and nothing else does"
