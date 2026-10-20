#!/bin/sh
#
# helpers/pull-request-by-hand.sh <branch> <title> <body>
#
# A scheduled workflow's pull request, when the workflow cannot open it: the run summary says the
# branch is pushed, gives the compare link a person opens it from, and carries the title and body
# the workflow would have used. Exits 0, so the run ends green.
#
# # Why
#
# Without the automation App (a fork, or a repository it is not installed on) the workflows fall
# back to `github.token`, which pushes but may not open a pull request unless the repository allows
# "GitHub Actions to create and approve pull requests". calef ruled on #2442 (2026-10-20 UTC) that
# the fallback is push-only and that setting stays off: the run measures, writes, pushes and ends
# green, and a person opens the pull request. Milestone 889 (the tree's gates stop depending on the
# fleet tooling) measured the refusal on a fork before the ruling (calef/nife runs 39364815886 and
# 39364805264). The callers are `falsifications`, `mutation`, `vendor-watch` and
# `verify-shard-refresh`, each right after its `git push`, only when no App token was minted.
#
# Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/889-fork-dispatch`.

set -eu

if [ "$#" -ne 3 ]; then
    echo "usage: helpers/pull-request-by-hand.sh <branch> <title> <body>" >&2
    exit 2
fi
branch="$1"
title="$2"
body="$3"
server="${GITHUB_SERVER_URL:-https://github.com}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset; this runs inside a workflow}"
link="$server/$repo/compare/main...$branch?expand=1"

echo "Pushed $branch. No App token, so no pull request was opened; open it from $link"
{
    echo "## The pull request was not opened"
    echo
    echo "This run has no automation App token, and \`github.token\` may not open pull requests"
    echo "(calef's ruling on #2442). \`$branch\` is pushed; open the pull request from"
    echo "[$link]($link)."
    echo
    echo "**Title:** $title"
    echo
    echo "**Body:**"
    echo
    printf '%s\n' "$body"
} >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
