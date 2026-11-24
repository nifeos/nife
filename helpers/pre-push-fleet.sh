#!/bin/sh
#
# helpers/pre-push-fleet.sh: the half of .githooks/pre-push that asks GitHub, run only on a checkout
# that opted in to the fleet tooling.
#
# Milestone 889 (the tree's gates stop depending on the fleet tooling), provisional. Before it, every
# push from every clone asked GitHub twice: once whether the branch's pull request was in the merge
# queue (helpers/push-queued-check.sh), and once whether that pull request was ready while its block
# still said IN-PROGRESS. Both questions are about a lane in calef's queue, and a stranger's push
# should not depend on `gh`, a token or the network. The hook now calls this file only when
# `git config --bool nife.fleet` is true, which script/claim sets on the machine that cuts lanes.
# `git config nife.fleet true` turns it on by hand; `git config --unset nife.fleet` turns it off.
#
# Usage (from the hook, git's ref lines on stdin: `<local ref> <local sha> <remote ref> <remote sha>`):
#
#     helpers/pre-push-fleet.sh queued   # before the empty-push skip: is the branch in the queue?
#     helpers/pre-push-fleet.sh after    # after script/lint --no-cargo: lint-fleet, then ready status
#
# Exit 0 lets the push go, exit 1 refuses it. The reasoning for each check is where it was before
# this split: push-queued-check.sh's header, and the paragraphs below, moved whole from the hook.
#
# BUGS
#
# - The opt-in is per clone, not per lane: a fleet machine that never ran script/claim, or a clone
#   made by hand, pushes without these checks until `git config nife.fleet true` is set.
#
# Name: provisional, minted 2026-10-12 (UTC) by milestone 889: the hook's name with `-fleet`,
# the suffix script/lint-fleet takes for the same split. calef has not ratified it.

root="$(git rev-parse --show-toplevel)"
refs="$(cat)"

case "${1:-}" in
queued)
    # **A push that rewrites a pushed commit is refused**, and the stash block is put back if it
    # is missing (milestone 118 (CLAUDE.md has a budget)'s budget rule 3). Both run here, ahead of the empty-push skip,
    # because a forced empty commit rewrites history as well as any other. The headers of
    # helpers/push-rewrite-check.sh and helpers/stash-block.sh have the reasoning.
    printf '%s\n' "$refs" | "$root/helpers/push-rewrite-check.sh" || exit 1
    "$root/helpers/stash-block.sh" || true
    # **A push to a branch whose pull request is in the merge queue is refused** (provisional
    # milestone name: refuse a push to a queued branch, 2026-10-05 UTC). It runs before the hook's
    # empty-push skip because an empty push to a queued branch removes it from the queue just as
    # well. One `gh api graphql` call, fail-open with a one-line warning when `gh` cannot answer.
    # Override with `NIFE_PUSH_QUEUED=1` (provisional name). helpers/push-queued-check.sh has the
    # reasoning and partsfoundry/record:notes/merge-queue.md has the measurement.
    printf '%s\n' "$refs" | "$root/helpers/push-queued-check.sh"
    exit $?
    ;;
after) ;;
*)
    echo "pre-push-fleet: usage: helpers/pre-push-fleet.sh queued|after (git's ref lines on stdin)" >&2
    exit 2
    ;;
esac

# The fleet tooling's checks, which script/lint ran until milestone 889 moved them to
# script/lint-fleet. A fleet lane's push ran them before that, inside `script/lint --no-cargo`, and
# still does from here. No `[ -x ]` tolerance: on a checkout that opted in, a missing script is a
# broken gate and fails loudly.
out="$(mktemp "${TMPDIR:-/tmp}/nife-pre-push-fleet.XXXXXX")"
if ! "$root/script/lint-fleet" >"$out" 2>&1; then
    echo "pre-push: script/lint-fleet failed; CI's lint job runs it too and will fail the same way." >&2
    tail -n 15 "$out" | sed 's/^/  | /' >&2
    echo "  Fix:    script/lint-fleet   (read its output, fix, commit, push again)" >&2
    echo "  Bypass: git push --no-verify" >&2
    rm -f "$out"
    exit 1
fi
rm -f "$out"

# **A ready pull request may not carry an IN-PROGRESS block on its own branch** (milestone 630).
# The same question `.github/workflows/ready-status.yml` asks on `pull_request`, asked here for a
# push to a branch whose pull request is already ready, so the answer comes before the push rather
# than a runner later. The merge group's roadmap check fails on exactly this 20 to 38 minutes into
# a queue attempt, and the ejection cancels auto-merge (#1473, 2026-10-03).
#
# The branch is the one being pushed, from stdin (`<local ref> <sha> <remote ref> <sha>`), not
# HEAD's. It needs `gh` and the network; with neither, or with no pull request, or a draft, it
# skips silently, because a draft's block is honest to say IN-PROGRESS and CI asks again anyway.
# The block is read from the working tree, as lint's is.
if [ -x "$root/script/roadmap" ] && grep -q -- '"--ready-branch"' "$root/script/roadmap" 2>/dev/null &&
    command -v gh >/dev/null 2>&1; then
    while read -r _ _ remote_ref _; do
        case "$remote_ref" in refs/heads/*) ;; *) continue ;; esac
        branch="${remote_ref#refs/heads/}"
        draft="$(gh pr view "$branch" --json isDraft -q .isDraft 2>/dev/null || true)"
        [ "$draft" = "false" ] || continue
        if ! "$root/script/roadmap" --ready-branch "$branch"; then
            echo "pre-push: the pull request for $branch is ready, and its block above still reads IN-PROGRESS." >&2
            echo "  Bypass: git push --no-verify" >&2
            exit 1
        fi
    done <<ENDREFS
$refs
ENDREFS
fi
exit 0
