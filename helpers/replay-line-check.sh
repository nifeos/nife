#!/bin/sh
#
# helpers/replay-line-check.sh: refuse public text that carries a reproducer.
#
#     helpers/replay-line-check.sh < message-or-body
#     helpers/replay-line-check.sh --range BASE..TIP      each commit message in the range
#     helpers/replay-line-check.sh --selftest
#
# Name: provisional, minted 2026-11-03 (UTC) by lane/fix-lanes-read-the-advisory.
#
# A seed, a crash input and a replay line stay out of public text until the fix for the finding is
# on main (fuzz-findings-without-a-person, step 5). The fuzz drivers print a seed in exactly two
# forms in this tree, and a person pastes one of them into a commit message or a pull request body
# without thinking. This refuses both:
#
#   1. A replay line: a line whose first words are `replay:`, in any case, after any quoting,
#      list or indentation marks (`>`, `*`, `-`, a backtick, a quote). The drivers print
#      `replay: <assignment>`.
#   2. A seed assignment: `NIFE_`, a name, then `SEED` or `SEEDS`, then `=`, anywhere in a line,
#      so `export NIFE_X_SEEDS=...` and a quoted one are caught. The assignment form with a star in
#      the name, which prose uses to name the family, is not an assignment and passes. To mention
#      the variable in a message, leave out the equals sign.
#
# It reads text, not the tree: notes and code that document the forms are not its business. A
# message that trips it is rewritten to name the invariant that broke and nothing that reproduces it.
# Exit 0 clean; exit 1 with the offending line numbers (never the lines: the check must not copy a
# seed into a log); exit 2 for a usage error.

set -e
tmp="$(mktemp "${TMPDIR:-/tmp}/replay-line-check.XXXXXX")"
trap 'rm -f "$tmp"' EXIT

# One scan: reads standard input, prints "line N: <rule>" per offense, no text.
scan() {
    awk '
        { line = $0 }
        tolower(line) ~ /^[ \t>*`"'"'"'-]*replay:/ { printf "line %d: a replay line\n", NR; bad = 1 }
        line ~ /NIFE_[A-Za-z0-9_]*SEEDS?[ \t]*=/ { printf "line %d: a seed assignment\n", NR; bad = 1 }
        END { exit bad ? 1 : 0 }
    '
}

if [ "${1:-}" = "--selftest" ]; then
    exec "$(dirname "$0")/replay-line-check-selftest.sh"
fi

if [ "${1:-}" = "--range" ]; then
    [ -n "${2:-}" ] || { echo "replay-line-check: --range wants BASE..TIP" >&2; exit 2; }
    status=0
    for c in $(git rev-list --no-merges "$2"); do
        if ! git log -n 1 --format=%B "$c" | scan > "$tmp" 2>&1; then
            echo "replay-line-check: commit $(git rev-parse --short "$c") carries a reproducer:" >&2
            sed 's/^/  /' "$tmp" >&2
            status=1
        fi
    done
    exit "$status"
fi

[ "$#" -eq 0 ] || { echo "usage: replay-line-check.sh [--range BASE..TIP | --selftest] < text" >&2; exit 2; }
if scan >"$tmp" 2>&1; then
    exit 0
fi
echo "replay-line-check: the text carries a reproducer:" >&2
sed 's/^/  /' "$tmp" >&2
exit 1
