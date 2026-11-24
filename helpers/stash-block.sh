#!/bin/sh
#
# helpers/stash-block.sh: make `git stash` fail in every worktree that shares this `.git`.
#
#     helpers/stash-block.sh              # install the block if it is missing; say why if it cannot
#     helpers/stash-block.sh --selftest   # prove it in a throwaway repository
#
# Why. The stash stack is one ref, `refs/stash`, shared by every worktree on the machine, so one
# lane's `git stash pop` can pop another lane's entry (notes/skills/worktree-commits/SKILL.md; the
# incident is in nifeos/record:design/tenets/git-in-a-worktree.md). Saying so did not stop it, so this takes the
# state away instead of warning about it.
#
# How. Git refuses a ref whose name is a directory of other refs. With `refs/stash/<BLOCK>` present,
# `refs/stash` cannot be created, and `git stash` (push, save, `-u`, and every `--autostash`) fails
# with "Cannot save the current status" before it touches the working tree. Loose and packed refs
# both enforce it. The block points at main's root commit, which is reachable anyway.
#
# Exit 0 when the block is in place (already, or installed now). Exit 1 when it cannot be installed
# because a stash already exists: those entries are some lane's work, so this lists them and never
# drops them. Whoever owns them turns each into a patch file and drops it, and the next run installs
# the block.
#
# Run by script/claim on every lane it cuts, and by helpers/pre-push-fleet.sh on every fleet push,
# so a clone that opted in to the fleet is blocked within one claim or push.
#
# BUGS
#
# - The error git prints names neither this file nor the rule. `git for-each-ref refs/stash` shows
#   the block, and its name says where to read.
# - `git update-ref -d` on the block removes it. Nothing reinstalls it until the next claim or push.
#
# Name: provisional, minted 2026-10-19 (UTC) by lane/118-budget-rules-2-3 (milestone 118 (CLAUDE.md has a budget)'s budget
# rule 3). calef has not ruled on it.

BLOCK=refs/stash/no-git-stash-see-worktree-commits-skill

install() {
    if [ -n "$(git for-each-ref --format=x "$BLOCK")" ]; then
        return 0
    fi
    if git rev-parse -q --verify refs/stash >/dev/null; then
        echo "stash-block: a stash exists in this .git, so git stash stays usable until it is gone:" >&2
        git stash list >&2
        echo "  Its owner turns each entry into a patch (git stash show -p <entry> > /tmp/<lane>-<what>.patch)" >&2
        echo "  and drops it; the next claim or push installs the block." >&2
        return 1
    fi
    root_commit="$(git rev-list --max-parents=0 HEAD | tail -n 1)"
    # The empty old value makes this create-only, so two lanes installing at once cannot clash.
    git update-ref "$BLOCK" "$root_commit" "" 2>/dev/null ||
        [ -n "$(git for-each-ref --format=x "$BLOCK")" ]
}

selftest() {
    # A hook exports GIT_DIR; without this the throwaway repository below would be the real one.
    unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    fail() { echo "stash-block: selftest FAILED: $*" >&2; exit 1; }
    g() { git -c user.name=t -c user.email=t@t -c init.defaultBranch=main "$@"; }
    cd "$work" && g init -q repo && cd repo || fail "no repository"
    g commit -q --allow-empty -m root && echo a > f && g add f && g commit -q -m a
    g worktree add -q ../lane 2>/dev/null || fail "no worktree"

    # A stash that exists is reported and kept, and the block is not installed over it.
    echo b > f && g stash -q || fail "could not make the stash this case needs"
    if install 2>/dev/null; then fail "installed over an existing stash"; fi
    g rev-parse -q --verify refs/stash >/dev/null || fail "the existing stash was touched"
    g stash pop -q || fail "pop"
    g checkout -q f

    install || fail "install failed on a repository with no stash"
    install || fail "a second install is not a no-op"
    for dir in "$work/repo" "$work/lane"; do
        echo "edit" >> "$dir/f"
        if (cd "$dir" && g stash -q) 2>/dev/null; then fail "git stash succeeded in $dir"; fi
        if (cd "$dir" && g stash push -u -q -m tagged) 2>/dev/null; then fail "git stash push -u succeeded in $dir"; fi
        grep -q edit "$dir/f" || fail "the refused stash still touched the working tree in $dir"
    done
    g pack-refs --all
    if (cd "$work/lane" && g stash -q) 2>/dev/null; then fail "git stash succeeded once refs were packed"; fi
    echo "stash-block: selftest passed"
}

case "${1:-}" in
--selftest) selftest ;;
"") install ;;
*)
    echo "usage: helpers/stash-block.sh [--selftest]" >&2
    exit 2
    ;;
esac
