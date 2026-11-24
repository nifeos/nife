#!/bin/sh
#
# Selftest for helpers/empty-diff-check.sh, run by script/lint. Builds throwaway repositories, so it
# needs no tree and no network. Each case is a merge commit made with --no-ff, the shape the merge
# queue produces, checked as BASE=first parent, TIP=merge. The failing cases are the point: a green
# run of the check alone looks the same as a check that cannot fire (milestone 627 (a pull request that changes nothing does not merge); see
# partsfoundry/record:design/roadmap/0640-a-gate-is-not-evidence-until-it-has-failed.md).

set -eu
check="$(cd "$(dirname "$0")" && pwd)/empty-diff-check.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null

fail() { echo "empty-diff-selftest: $*" >&2; exit 1; }
g() { git -c commit.gpgsign=false "$@"; }

# expect NAME WANT_EXIT [WANT_TEXT]: run the check on the last merge of the current branch.
expect() {
    name=$1 want=$2 text=${3:-}
    set +e
    out=$(sh "$check" HEAD^1 HEAD 2>&1)
    got=$?
    set -e
    [ "$got" -eq "$want" ] || fail "$name: exit $got, wanted $want ($out)"
    if [ -n "$text" ]; then
        printf '%s' "$out" | grep -q "$text" || fail "$name: output lacks '$text' ($out)"
    fi
}

g init -q -b main .
echo one > a; g add a; g commit -q -m "base"

# 1. The #1460 shape: one empty claim commit.
g checkout -q -b c1 main
g commit -q --allow-empty -m "claim: a thing"
g checkout -q main; g merge -q --no-ff c1 -m "merge c1"
expect "claim only" 1 "only commits are claim commits"

# 2. A claim plus a merge from main: the branch carries main's commits, adds nothing of its own.
g checkout -q -b c2 main~1 2>/dev/null || g checkout -q -b c2 main
g commit -q --allow-empty -m "claim: another"
echo two > b; g checkout -q main; g add b; g commit -q -m "main moves"
g checkout -q c2; g merge -q --no-ff main -m "merge main into c2"
g checkout -q main; g merge -q --no-ff c2 -m "merge c2"
expect "claim plus a merge from main" 1

# 3. A change and its revert: not all claims, so the generic message.
g checkout -q -b c3 main
g commit -q --allow-empty -m "claim: third"
echo x > c; g add c; g commit -q -m "add c"
g rm -q c; g commit -q -m "revert add c"
g checkout -q main; g merge -q --no-ff c3 -m "merge c3"
expect "change and revert" 1 "net out to nothing"

# 4. A claim plus one real file: must pass.
g checkout -q -b c4 main
g commit -q --allow-empty -m "claim: fourth"
echo y > d; g add d; g commit -q -m "add d"
g checkout -q main; g merge -q --no-ff c4 -m "merge c4"
expect "claim plus a file" 0

# 5. An unresolvable revision fails closed (exit 2), not open.
set +e
sh "$check" nosuchrev HEAD >/dev/null 2>&1
rc=$?
set -e
[ "$rc" -eq 2 ] || fail "unresolvable BASE: exit $rc, wanted 2"

echo "empty-diff-selftest: 5 cases, each failed or passed as it should"
