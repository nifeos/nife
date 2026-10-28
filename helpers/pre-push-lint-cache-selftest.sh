#!/bin/sh
#
# Selftest: .githooks/pre-push skips `script/lint --no-cargo` only when helpers/pre-push-lint-cache.sh
# holds this machine's marker that the same checks passed on the same tree, and degrades to a run on
# every failure. Run by script/lint.
#
# The hook runs in a throwaway repository with the real helper, a stub script/fmt, and a stub
# script/lint that records each call and passes or fails on demand. The marker directory is the
# selftest's own, via NIFE_PRE_PUSH_LINT_CACHE. What is proved:
#
#   1. No marker: the checks run, the push passes, and a marker is written.
#   2. The same tree again: the checks do not run, and the hook says it skipped.
#   3. Any changed input runs the checks again: the working tree (an untracked file counts), the
#      committed tip, the merge base, the staged index state, and the lint tools on PATH.
#   4. A failing lint refuses the push and writes no marker, so the next push runs again.
#   5. A cache that cannot be written never refuses a push and never skips one.
#   6. The marker never lands in the worktree.
#
# helpers/pre-push-selftest.sh holds the stranger's side (no `gh`, no fleet half); the key's
# input list is argued in helpers/pre-push-lint-cache.sh's header.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
hook="$here/../.githooks/pre-push"
helper="$here/pre-push-lint-cache.sh"
me="pre-push-lint-cache-selftest"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-pre-push-lint-cache-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
# Canonical from the start: git resolves the worktree through /var/folders' symlink to
# /private/var/folders, and the inside-tree guard below compares paths as strings.
tmp="$(cd "$tmp" && pwd -P)"
cache="$tmp/cache"
repo="$tmp/repo"
remote="$tmp/remote"
mkdir -p "$repo/script" "$repo/.githooks" "$repo/helpers" "$tmp/bin"
g() { git -C "$repo" -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }
export NIFE_PRE_PUSH_LINT_CACHE="$cache"

g init -q
g config user.name selftest
g config user.email selftest@invalid
cp "$hook" "$repo/.githooks/pre-push"
cp "$helper" "$repo/helpers/pre-push-lint-cache.sh"
printf '#!/bin/sh\nexit 0\n' >"$repo/script/fmt"
chmod +x "$repo/script/fmt"
# The lint stub records every call, then does what $NIFE_LINT_STUB_EXIT says (default pass).
printf '#!/bin/sh\necho ran >>"%s"\nexit ${NIFE_LINT_STUB_EXIT:-0}\n' "$tmp/lint-calls" >"$repo/script/lint"
chmod +x "$repo/script/lint"
echo one >"$repo/file"
g add -A
g commit -q -m one
before="$(g rev-parse HEAD)"
echo two >"$repo/file"
g commit -q -am two
after="$(g rev-parse HEAD)"
g update-ref refs/remotes/origin/main "$before"

# The hook's changed-files check measures against the ref line's remote sha, so no real remote is
# needed; the helper's merge base reads the local refs/remotes/origin/main above.
push() {
	(cd "$repo" && printf 'refs/heads/main %s refs/heads/main %s\n' "$after" "$before" |
		sh .githooks/pre-push origin "file://$remote") >"$tmp/out" 2>&1
}
ran() {
	[ -s "$tmp/lint-calls" ] || {
		echo "$me: $1, but script/lint did not run" >&2
		sed 's/^/  | /' "$tmp/out" >&2
		exit 1
	}
}
did_not_run() {
	[ ! -s "$tmp/lint-calls" ] || {
		echo "$me: $1, but script/lint ran" >&2
		sed 's/^/  | /' "$tmp/out" >&2
		exit 1
	}
}
key_in_repo() {
	(cd "$repo" && "$repo/helpers/pre-push-lint-cache.sh" key)
}

: >"$tmp/lint-calls"

# 1. No marker: the checks run, the push passes, and the marker is written.
push || {
	echo "$me: the hook refused a first push with every gate stubbed to pass:" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
}
ran "the first push had no marker"
key="$(key_in_repo)"
(cd "$repo" && "$repo/helpers/pre-push-lint-cache.sh" check "$key") ||
	{ echo "$me: a passing lint run wrote no marker" >&2; exit 1; }

# 2. The same tree again: the checks are skipped, and the hook says so.
: >"$tmp/lint-calls"
push || {
	echo "$me: the hook refused a push whose tree already has a pass marker:" >&2
	sed 's/^/  | /' "$tmp/out" >&2
	exit 1
}
did_not_run "the second push on an unchanged tree"
grep -q 'script/lint --no-cargo skipped' "$tmp/out" ||
	{ echo "$me: the skip left no line in the hook's output" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }

# 3. Any changed input runs the checks again. Each case: move one input, push, expect a run.
#    a. The working tree, by an untracked file (the lint-cost lane's lint failed on exactly this).
: >"$tmp/lint-calls"
echo untracked >"$repo/untracked.md"
push || { echo "$me: the hook refused a push after an untracked file appeared" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "an untracked file changed the tree"
rm "$repo/untracked.md"

#    b. The committed tip, by a commit of the tree (a lane's push always follows one).
: >"$tmp/lint-calls"
echo three >"$repo/file"
g commit -q -am three
after="$(g rev-parse HEAD)"
push || { echo "$me: the hook refused a push after a commit" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "the committed tip moved"

#    c. The merge base, by origin/main moving under the branch.
: >"$tmp/lint-calls"
g update-ref refs/remotes/origin/main "$after"
push || { echo "$me: the hook refused a push after origin/main moved" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "the merge base moved"

#    d. The index alone: staged content whose working-tree copy is back at HEAD, the state a
#       `git add` plus a revert leaves. Tree and tip are where they were; the checks read the
#       staged state through git grep and the commit-first refusal, so the key must move anyway.
: >"$tmp/lint-calls"
staged_key="$(key_in_repo)"
echo staged >"$repo/file"
g add file
g show HEAD:file >"$repo/file"
[ "$(key_in_repo)" != "$staged_key" ] ||
	{ echo "$me: a staged change with the worktree reverted left the key unchanged" >&2; exit 1; }
push || { echo "$me: the hook refused a push after a staged index change" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "the index moved"
# Undo the staged state whole: checkout -- restores the worktree from the index, which still holds
# the staged blob, and the rest of the selftest reads a clean status.
g reset -q --hard HEAD

#    e. The lint tools, by a different python3 earlier on PATH. PATH is exported and restored by
#       hand rather than prefixed on the function call, because whether a VAR=x prefix on a
#       function outlives the call is unspecified between shells.
: >"$tmp/lint-calls"
tools_key="$(key_in_repo)"
printf '#!/bin/sh\nexit 0\n' >"$tmp/bin/python3"
chmod +x "$tmp/bin/python3"
old_path="$PATH"
PATH="$tmp/bin:$PATH"
export PATH
(cd "$repo" && "$repo/helpers/pre-push-lint-cache.sh" key) >"$tmp/key2"
[ "$(cat "$tmp/key2")" != "$tools_key" ] ||
	{ echo "$me: a different python3 on PATH left the key unchanged" >&2; exit 1; }
push || { echo "$me: the hook refused a push with a different python3 on PATH" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "the lint tools moved"
rm "$tmp/bin/python3"
PATH="$old_path"

# 4. A failing lint refuses the push, writes no marker, and the next push runs the checks again.
#    The cache is emptied first, so the failing run is judged on a tree it cannot skip out of, and
#    "no marker after the failure" reads straight off the empty directory.
: >"$tmp/lint-calls"
rm -rf "$cache"
NIFE_LINT_STUB_EXIT=1
export NIFE_LINT_STUB_EXIT
if push; then
	echo "$me: the hook passed a push whose script/lint failed" >&2
	exit 1
fi
unset NIFE_LINT_STUB_EXIT
grep -q 'script/lint --no-cargo failed' "$tmp/out" ||
	{ echo "$me: the refusal did not name script/lint" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
[ -z "$(ls "$cache" 2>/dev/null)" ] ||
	{ echo "$me: a failed lint run left a pass marker" >&2; ls "$cache" >&2; exit 1; }
: >"$tmp/lint-calls"
push || { echo "$me: the hook refused a push after a failed one was fixed" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "the push after a failed lint"

# 5. A cache that cannot be written: the push passes through a real run, and no marker claims it.
#    NIFE_PRE_PUSH_LINT_CACHE is exported and restored by hand, for the same reason PATH above is.
: >"$tmp/lint-calls"
mkdir -p "$tmp/ro"
chmod 500 "$tmp/ro"
NIFE_PRE_PUSH_LINT_CACHE="$tmp/ro/cache"
export NIFE_PRE_PUSH_LINT_CACHE
push ||
	{ echo "$me: the hook refused a push whose cache is unwritable" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "an unwritable cache"
sh "$repo/helpers/pre-push-lint-cache.sh" write "$(key_in_repo)" >/dev/null 2>&1 &&
	{ echo "$me: a write into an unwritable cache claimed success" >&2; exit 1; }
sh "$repo/helpers/pre-push-lint-cache.sh" check "$(key_in_repo)" >/dev/null 2>&1 &&
	{ echo "$me: an unwritable cache matched a marker" >&2; exit 1; }
chmod 700 "$tmp/ro"
export NIFE_PRE_PUSH_LINT_CACHE="$cache"

# 5b. A cache inside the tree, or any relative path (it would resolve against the hook's working
#     directory, which is a worktree), never matches and never takes a write. The helper is called
#     from the repository, as the hook calls it: the guard reads the tree from the working
#     directory.
for bad in "relative/cache" "$repo/in-tree"; do
	(cd "$repo" && NIFE_PRE_PUSH_LINT_CACHE="$bad" sh "$repo/helpers/pre-push-lint-cache.sh" check "$(key_in_repo)") >/dev/null 2>&1 &&
		{ echo "$me: a cache at $bad matched a marker" >&2; exit 1; }
	(cd "$repo" && NIFE_PRE_PUSH_LINT_CACHE="$bad" sh "$repo/helpers/pre-push-lint-cache.sh" write "$(key_in_repo)") >/dev/null 2>&1 &&
		{ echo "$me: a marker was written at $bad" >&2; exit 1; }
done
[ ! -e "$repo/in-tree" ] ||
	{ echo "$me: a marker landed inside the worktree anyway" >&2; exit 1; }

# 6. The marker never lands in the worktree: a write leaves status and the untracked list alone,
#    and a hook run through a real key writes nothing under $repo.
: >"$tmp/lint-calls"
status_before="$(g status --porcelain --untracked-files=all)"
key_now="$(key_in_repo)"
(cd "$repo" && "$repo/helpers/pre-push-lint-cache.sh" write "$key_now") ||
	{ echo "$me: the marker write failed in a writable cache" >&2; exit 1; }
[ "$(g status --porcelain --untracked-files=all)" = "$status_before" ] ||
	{ echo "$me: the marker write left something in the worktree" >&2; exit 1; }
push || { echo "$me: the hook refused a push after a manual marker write" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
[ -z "$(g status --porcelain --untracked-files=all)" ] ||
	{ echo "$me: a hook run left something in the worktree: $(g status --porcelain --untracked-files=all)" >&2; exit 1; }
did_not_run "the push after the manual marker write"

# 7. A tree whose hook has no helper (an older tree beside a newer one, or a partial revert) runs
#    the checks rather than skipping and rather than failing.
: >"$tmp/lint-calls"
rm "$repo/helpers/pre-push-lint-cache.sh"
push || { echo "$me: the hook refused a push with no helper beside it" >&2; sed 's/^/  | /' "$tmp/out" >&2; exit 1; }
ran "a tree with no helper"

echo "pre-push-lint-cache selftest: the marker skips only what it saw pass, and every failure runs"
