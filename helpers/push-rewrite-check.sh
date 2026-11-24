#!/bin/sh
#
# helpers/push-rewrite-check.sh: refuse a push that would rewrite a commit already on the remote.
#
#     helpers/pre-push-fleet.sh queued   # runs this first, with git's ref lines on stdin
#     helpers/push-rewrite-check.sh --selftest
#
# Why. A pushed commit is never rewritten (calef, 2026-10-18 UTC; notes/skills/worktree-commits/SKILL.md):
# no reset, rebase, amend or force-push on a commit the remote has. The reasons are in
# partsfoundry/record:design/tenets/git-in-a-worktree.md. Locally a rewrite is invisible until it is pushed, so the push
# is the one place it can be refused, and only a forced push can carry one.
#
# The rule. For each ref, the remote's sha must be an ancestor of the sha being pushed. A new ref
# (remote sha all zeros) and a deletion (local sha all zeros) pass. A remote sha this clone has never
# fetched fails too, because a fast-forward cannot be proven and a forced push over it would discard
# commits nobody here has seen: fetch, merge, and push again.
#
# Exit 0 lets the push go, exit 1 refuses it. No override: `git push --no-verify` is the only way
# past, and the lane rules forbid it. It needs no network.
#
# BUGS
#
# - It sees only pushes from a clone with `nife.fleet` set. The bot workflows force-push their own
#   branches from CI, which is why nothing server-side refuses a force-push on every branch.
#
# Name: provisional, minted 2026-10-19 (UTC) by lane/118-budget-rules-2-3 (milestone 118 (CLAUDE.md has a budget)'s budget
# rule 3), after helpers/push-queued-check.sh. calef has not ruled on it.

zero=0000000000000000000000000000000000000000

check() {
    status=0
    while read -r local_ref local_sha remote_ref remote_sha; do
        [ -n "$local_sha" ] || continue
        [ "$local_sha" != "$zero" ] || continue
        [ "$remote_sha" != "$zero" ] || continue
        if ! git cat-file -e "$remote_sha^{commit}" 2>/dev/null; then
            echo "push-rewrite-check: $remote_ref on the remote is at $(printf %.12s "$remote_sha"), which this clone has not fetched." >&2
            echo "  Fix: git fetch origin, then git merge the remote branch, then push. Never force." >&2
            status=1
        elif ! git merge-base --is-ancestor "$remote_sha" "$local_sha"; then
            echo "push-rewrite-check: pushing $local_ref to $remote_ref would rewrite commits already on the remote." >&2
            echo "  A pushed commit is never rewritten (notes/skills/worktree-commits/SKILL.md). Merge the" >&2
            echo "  remote back in (git fetch origin, then git merge $(printf %.12s "$remote_sha")), keep your" >&2
            echo "  version where they conflict, and push the merge." >&2
            status=1
        fi
    done
    return $status
}

selftest() {
    unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    fail() { echo "push-rewrite-check: selftest FAILED: $*" >&2; exit 1; }
    g() { git -c user.name=t -c user.email=t@t -c init.defaultBranch=main "$@"; }
    cd "$work" && g init -q r && cd r || fail "no repository"
    g commit -q --allow-empty -m a && a=$(git rev-parse HEAD)
    g commit -q --allow-empty -m b && b=$(git rev-parse HEAD)
    g commit -q --allow-empty --amend -m b2 && b2=$(git rev-parse HEAD)
    unseen=1234567890123456789012345678901234567890
    expect() { # NAME WANT LOCAL REMOTE
        printf 'refs/heads/x %s refs/heads/x %s\n' "$3" "$4" | check 2>/dev/null
        [ $? -eq "$2" ] || fail "$1: wanted exit $2"
    }
    expect "fast-forward" 0 "$b" "$a"
    expect "nothing new" 0 "$b" "$b"
    expect "new branch" 0 "$b" "$zero"
    expect "deletion" 0 "$zero" "$b"
    expect "amended pushed commit" 1 "$b2" "$b"
    expect "reset behind the remote" 1 "$a" "$b"
    expect "remote sha never fetched" 1 "$b" "$unseen"
    printf 'refs/heads/x %s refs/heads/x %s\nrefs/heads/y %s refs/heads/y %s\n' "$b" "$a" "$b2" "$b" |
        check 2>/dev/null && fail "a rewrite after a good ref in the same push passed"
    echo "push-rewrite-check: selftest passed"
}

case "${1:-}" in
--selftest) selftest ;;
"") check ;;
*)
    echo "usage: helpers/push-rewrite-check.sh [--selftest] (git's pre-push ref lines on stdin)" >&2
    exit 2
    ;;
esac
