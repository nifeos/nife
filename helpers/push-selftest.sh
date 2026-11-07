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
# Then three pushes with a pre-push hook installed: a passing hook runs before git connects and
# again inside the push, a refusing hook stops the push before git connects, and `--no-verify`
# runs it neither time.
# No network: the origin is a local path. It runs in `script/lint --no-cargo`.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/push-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "push-selftest: $1" >&2; exit 1; }

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM
# The advisory queue check reads GitHub; this test has no network.
NIFE_SKIP_QUEUE_CHECK=1
export NIFE_SKIP_QUEUE_CHECK

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

# 4. The pre-push hook runs before git connects, then again inside the push. A hook that logs
# `hook` and a git that logs `push` before running the real one: a passing hook leaves the log
# `hook push hook`, so the checks ran before the connection and `git push` still ran them all.
# The hook also logs the `git` it finds and its directory, and the two runs must agree: the lint
# marker keys on the tools' paths, so a first run under another PATH would never be matched. It
# runs from a subdirectory, because git runs a hook from the top of the worktree wherever the
# push started.
rm "$work/bin/git"
printf '#!/bin/sh\n[ "$1" = push ] && echo push >>"%s/log"\nexec "%s" "$@"\n' "$work" "$real_git" >"$work/bin/git"
chmod +x "$work/bin/git"
mkdir "$work/hooks"
printf '#!/bin/sh\ncat >/dev/null\necho hook >>"%s/log"\necho "$(command -v git) $(pwd)" >>"%s/seen"\n[ ! -f "%s/refuse" ]\n' "$work" "$work" "$work" >"$work/hooks/pre-push"
chmod +x "$work/hooks/pre-push"
git config core.hooksPath "$work/hooks"
log() { tr '\n' ' ' <"$work/log" | sed 's/ $//'; }

: >"$work/log"
mkdir sub
(cd sub && PATH="$work/bin:$PATH" "$root/script/push") >"$work/out" 2>&1 || fail "a push with a passing hook exited non-zero: $(cat "$work/out")"
[ "$(log)" = "hook push hook" ] || fail "a passing hook did not run before the push and again inside it: $(log)"
[ "$(sort -u "$work/seen" | wc -l)" -eq 1 ] ||
    fail "the hook saw a different git or directory before the push than inside it: $(cat "$work/seen")"

# 5. A refusing hook stops the push before git connects: the log holds the hook and no push.
git commit -q --allow-empty -m three
touch "$work/refuse"
: >"$work/log"
if PATH="$work/bin:$PATH" "$root/script/push" >"$work/out" 2>&1; then fail "a push the hook refused exited 0"; fi
[ "$(log)" = "hook" ] || fail "a refusing hook did not stop the push before git connected: $(log)"
grep -q "NOT PUSHED" "$work/out" || fail "a push the hook refused did not say NOT PUSHED: $(cat "$work/out")"

# 6. --no-verify skips the hook here as git does inside the push.
: >"$work/log"
PATH="$work/bin:$PATH" "$root/script/push" --no-verify >"$work/out" 2>&1 || fail "a --no-verify push exited non-zero: $(cat "$work/out")"
[ "$(log)" = "push" ] || fail "--no-verify still ran the hook: $(log)"

echo "push-selftest: ok"
