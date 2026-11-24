#!/bin/sh
#
# Fail when a merge changes no file: the tree at TIP equals the tree at BASE.
#
#     helpers/empty-diff-check.sh BASE TIP
#
# Milestone 627 (a pull request that changes nothing does not merge): a pull request that changes nothing does not merge. Run by
# `.github/workflows/empty-diff.yml`, which says what BASE and TIP are for each event, and by
# `helpers/empty-diff-selftest.sh`, which is wired into `script/lint`.
#
# Exit 0: the merge changes something. Exit 1: it changes nothing. Exit 2: BASE or TIP did not
# resolve, which fails the check too, because a required check that fails open is a check that
# examined nothing and reported clean.
#
# **Trees, not diffs.** Two commits have the same tree object exactly when `git diff` between them
# is empty, and comparing the ids needs no file contents, so this works in a `filter: blob:none`
# clone and cannot be fooled by rename detection or whitespace settings.
#
# **Claim commits shape the message and nothing else.** An empty diff is the defect. A pull request
# whose commits are all `claim:` subjects (the §90 (the claim is a draft pull request; the status flip is a gate) convention, an empty commit that opens a lane)
# is its common cause, so that case gets a pointed hint; a claim plus a commit and its revert fails
# the same way with the generic message. See partsfoundry/record:design/roadmap/0627-*.md.
#
# Name: provisional, minted 2026-10-02 for milestone 627 (a pull request that changes nothing does not merge); calef has not ruled on it.

set -u

if [ "$#" -ne 2 ]; then
    echo "usage: empty-diff-check.sh BASE TIP" >&2
    exit 2
fi
base=$1
tip=$2

base_tree=$(git rev-parse --verify --quiet "$base^{tree}") || {
    echo "empty-diff: cannot resolve BASE '$base'" >&2
    exit 2
}
tip_tree=$(git rev-parse --verify --quiet "$tip^{tree}") || {
    echo "empty-diff: cannot resolve TIP '$tip'" >&2
    exit 2
}

if [ "$base_tree" != "$tip_tree" ]; then
    echo "empty-diff: ok, this merge changes files"
    exit 0
fi

subjects=$(git log --no-merges --format=%s "$base..$tip" 2>/dev/null)
total=$(printf '%s\n' "$subjects" | grep -c .)
claims=$(printf '%s\n' "$subjects" | grep -c '^claim:')

echo "empty-diff: this pull request changes no file against its base." >&2
if [ "$total" -gt 0 ] && [ "$total" -eq "$claims" ]; then
    echo "Its only commits are claim commits:" >&2
    printf '%s\n' "$subjects" | sed 's/^/    /' >&2
    echo "If the work exists, it was probably pushed to another branch. Check:" >&2
    echo "    git ls-remote --heads origin '*<milestone-slug>*'" >&2
    echo "and push that work to this branch, or open a pull request from it." >&2
else
    echo "Its commits net out to nothing (a change and its revert, or work already on the base)." >&2
fi
echo "If nothing was meant to land, close this pull request." >&2
exit 1
