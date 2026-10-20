#!/bin/sh
#
# helpers/push-selftest.sh: prove script/push's verdict comes from origin's ref, not git's exit status.
#
#     helpers/push-selftest.sh
#
# A scratch clone of a scratch bare origin, three pushes:
#   1. a push that lands: exit 0, and the last line names HEAD's commit;
#   2. a push origin refuses (its pre-receive hook exits 1): exit 1 and NOT PUSHED, which is the
#      shape of the dropped connection in #2478, git non-zero and the branch not moved;
#   3. a `git push` that exits 0 having sent nothing (a stub on PATH): still exit 1 and NOT PUSHED.
# The third is the one a check of git's status alone would pass, so it is the defect this holds.
# No network: the origin is a local path. It runs in `script/lint --no-cargo`.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/push-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "push-selftest: $1" >&2; exit 1; }

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM

git init -q --bare "$work/origin.git"
git init -q -b lane "$work/clone"
cd "$work/clone"
git config user.name selftest
git config user.email selftest@example.invalid
git remote add origin "$work/origin.git"
git commit -q --allow-empty -m one

# 1. Lands.
"$root/script/push" >"$work/out" 2>&1 || fail "a push that lands exited non-zero: $(cat "$work/out")"
tail -n 1 "$work/out" | grep -q "is at $(git rev-parse --short=12 HEAD)" ||
    fail "a push that lands did not end by naming HEAD's commit: $(tail -n 1 "$work/out")"

# 2. Origin refuses.
git commit -q --allow-empty -m two
printf '#!/bin/sh\nexit 1\n' >"$work/origin.git/hooks/pre-receive"
chmod +x "$work/origin.git/hooks/pre-receive"
if "$root/script/push" >"$work/out" 2>&1; then fail "a refused push exited 0"; fi
grep -q "NOT PUSHED" "$work/out" || fail "a refused push did not say NOT PUSHED: $(cat "$work/out")"
rm "$work/origin.git/hooks/pre-receive"

# 3. git push exits 0 and sends nothing.
real_git="$(command -v git)"
mkdir "$work/bin"
printf '#!/bin/sh\n[ "$1" = push ] && exit 0\nexec "%s" "$@"\n' "$real_git" >"$work/bin/git"
chmod +x "$work/bin/git"
if PATH="$work/bin:$PATH" "$root/script/push" >"$work/out" 2>&1; then
    fail "a git push that exited 0 and sent nothing was reported as pushed"
fi
grep -q "NOT PUSHED" "$work/out" || fail "a silent non-push did not say NOT PUSHED: $(cat "$work/out")"

echo "push-selftest: ok"
