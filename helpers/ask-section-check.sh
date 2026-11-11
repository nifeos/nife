#!/bin/sh
#
# helpers/ask-section-check.sh: refuse a pull request body whose `## What I need from you` section
# asks no question.
#
#     helpers/ask-section-check.sh < body
#     helpers/ask-section-check.sh --selftest
#
# Name: provisional, minted 2026-11-11 (UTC) by lane/after-this-merges-asks-nothing.
#
# The merge drain reads every `## What I need from you` section as a question for calef, unless its
# first line is "Nothing." or "None.", and adds `needs-architect` to a pull request that has one
# with no ruling since (helpers/open-question.jq, helpers/needs-maintainer.jq's `ask-no-hold`). The
# label blocks the merge. #3476 put a request for a bench session that needs the merge first under
# that heading, and so held the merge the session was waiting for. The drain is right to count
# every ask heading (a question it missed could merge unruled; a wrong add costs a glance, #1792),
# so the wrong add is stopped here, before it: a section with no question mark in it is refused,
# and the message names the two places such text goes.
#
#   - A task for a person that needs the merge first goes under `## After this merges`, which no
#     reader of open-question.jq counts.
#   - A note that asks nothing now opens its section with "Nothing." on a line of its own.
#
# The section and the test are open-question.jq's (`oq_unasked_sections`), so this and the drain
# read the same text. It reads text, not the tree. Exit 0 clean; exit 1 naming the line of each
# heading whose section asks nothing; exit 2 for a usage error.
#
# BUGS
#   - A question mark is the test, not a question. "Approve the wording." is a real ask and is
#     refused until it is written as one ("Do you approve the wording?"), and a `?` inside a code
#     span passes a section that asks nothing.
#   - Only the body is read. An ask in a comment is not checked, and the drain counts it the same.

set -e
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="$(basename "$0")"

check() {
    jq -R -s -r "$(cat "$here/open-question.jq")"'
        oq_unasked_sections
        | if length == 0 then empty
          else (.[] | "line \(.heading): a `What I need from you` section with no question in it"), halt_error(1) end' 2>/dev/null || {
        status=$?
        [ "$status" -eq 1 ] || { echo "$me: jq failed ($status)" >&2; exit 2; }
        return 1
    }
}

refuse() {
    cat >&2 <<'MSG'
ask-section-check: the merge drain reads a `## What I need from you` section as a question for
calef and adds `needs-architect`, which blocks the merge. A section with no question in it holds
the merge for nothing. Either ask the question (with a question mark), or move the text:
  - a task for a person once this has merged goes under `## After this merges`;
  - a note that asks nothing now opens its section with "Nothing." on a line of its own.
MSG
}

case "${1:-}" in
"")
    out="$(check)" && exit 0
    printf '%s\n' "$out" >&2
    refuse
    exit 1
    ;;
--selftest)
    fail=0
    expect() { # <name> <want: 0 or 1> <text>
        if printf '%b' "$3" | check >/dev/null; then got=0; else got=1; fi
        [ "$got" = "$2" ] || { echo "$me: selftest: $1: exit $got, wanted $2" >&2; fail=1; }
    }
    expect "no section" 0 'Summary.\n\n## What this changes\n\nA thing.\n'
    expect "a question" 0 '## What I need from you\n\nMay it merge with option A?\n'
    expect "Nothing." 0 '## What I need from you\n\nNothing.\n\nOne name stays provisional.\n'
    expect "**None.**" 0 '### What I need from you\n\n**None.**\n'
    expect "the post-merge heading" 0 '## After this merges\n\nOne bench session, once this merges.\n'
    expect "a post-merge task under the ask (#3476)" 1 '## What I need from you\n\nOne bench session at the board once this merges.\n\n1. Boot it.\n'
    expect "a question after the section does not count" 1 '## What I need from you\n\nA bench run.\n\n## Other\n\nWhy?\n'
    expect "a #### inside stays in the section" 0 '## What I need from you\n\nTwo forks.\n\n#### Fork 1\n\nA or B?\n'
    expect "the second of two sections" 1 '## What I need from you\n\nA or B?\n\n### What I need from you\n\nA key.\n'
    expect "CRLF line ends" 1 '## What I need from you\r\n\r\nOne bench session.\r\n'
    expect "a heading quoted mid-line" 0 'The drain reads `## What I need from you` headings.\n'
    [ "$fail" -eq 0 ] || exit 1
    echo "ask-section-check: 11 bodies, each refused or passed as expected"
    ;;
*)
    echo "usage: $me [--selftest] < body" >&2
    exit 2
    ;;
esac
