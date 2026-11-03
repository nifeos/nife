#!/bin/sh
# helpers/fuzz-daily-publish.sh: write the daily report of the continuous fuzz run.
#
#     helpers/fuzz-daily-publish.sh [--publish | --dry-run]
#
# Reads `$NIFE_FUZZ_HOME/state/windows.tsv` (helpers/continuous-fuzz.sh) and writes, under
# `$NIFE_FUZZ_HOME/out/<utc date>/`:
#
#   report.md      counter totals: windows, seeds run by driver, windows by status, the newest
#                  window's time, and the count and age in days of held findings
#   clean-ranges.tsv   driver, first seed, seeds run, commit, for every clean window. A seed replays
#                  the same calls, so a clean range is its own corpus
#
# **What never leaves.** A held finding is an escape, a crash or hang reached from user mode, or
# anything untriaged, and every window that is not clean is one. This script reads the held directory
# only to count its files and take their ages. It never opens one, never copies one, and the report
# names no seed, driver or commit of a held window. After a held replay the supervisor starts the
# slot's next window one seed past the held seed, so that window's first seed would locate it. The
# table therefore leaves out every range of a slot from that slot's first held window onward, read
# from windows.tsv alone; ranges of other slots, and of the same slot before it, are published
# (design/roadmap/proposals/the-clean-range-table-does-not-locate-a-held-seed.md). A held window is
# any window whose status is neither `ok` nor `infrastructure`, so a status this script does not
# know withholds its slot too. The seed totals in report.md still count every clean window.
#
# BUGS: nothing brings a withheld slot's ranges back. They should return once the held finding turns
# public, and the milestone that publishes it
# (design/roadmap/proposals/a-held-finding-turns-public-after-its-fix-reruns-clean.md) defines the
# signal; until it does, a slot that ever held a finding publishes no further ranges. That fails safe.
#
# **Where it goes.** Ruled 2026-11-01 (UTC): one GitHub release per day on nifeos/nife, tagged
# `fuzz-YYYY-MM-DD`, with report.md and clean-ranges.tsv as assets
# (design/roadmap/proposals/where-the-daily-fuzz-output-goes.md). Without a flag the script stops at
# the local files. `--dry-run` also prints the tag and the asset list. `--publish` runs
# `gh release create` and refuses unless NIFE_FUZZ_PUBLISH=1 is set. It also refuses, by name and
# before any `gh` call, a missing GH_TOKEN, a missing or unparseable GH_TOKEN_EXPIRES_AT (UTC,
# `YYYY-MM-DDTHH:MM:SSZ`), and a token that has expired or expires within `safety_margin_seconds`.
# The mint helper's token file supplies both variables (nifeos/fuzz-host, docs/fuzz-token.md). A
# refusal never prints the token. The credential is a one-hour
# installation token of the `nife-fuzz` App, minted from the App's root-only key (Contents write and
# Repository security advisories write on nifeos/nife alone; notes/fuzzing/the-fuzz-app.md). Main's
# ruleset refuses a push without a pull request, and this script never calls `git push`. Findings
# start as private security advisories, filed by a service on the host, not by this script
# (design/roadmap/proposals/fuzz-findings-without-a-person.md).
#
# Name: provisional, minted 2026-11-01 (UTC) by lane/continuous-fuzzing-host. calef has not ruled on it.

set -eu

home="${NIFE_FUZZ_HOME:-$HOME/fuzz}"
state="$home/state"
today="$(date -u +%Y-%m-%d)"
out="$home/out/$today"
windows="$state/windows.tsv"

mkdir -p "$out" "$state"
[ -f "$windows" ] || : > "$windows"

# Columns: utc, driver, slot, first, ran, ms, status, commit. Rows are in the order the windows
# ended, so a slot's rows after its first held one are the windows that ran after it.
awk -F'\t' '
    $7 != "ok" && $7 != "infrastructure" { held[$3] = 1 }
    $7 == "ok" && !($3 in held) { print $2 "\t" $4 "\t" $5 "\t" $8 }
' "$windows" > "$out/clean-ranges.tsv"

held_count=0
oldest_days=0
if [ -d "$state/held" ]; then
    # Count and age only; the files stay closed.
    held_count="$(find "$state/held" -type f | wc -l | tr -d ' ')"
    if [ "$held_count" -gt 0 ]; then
        oldest="$(find "$state/held" -type f -printf '%T@\n' | sort -n | head -1 | cut -d. -f1)"
        oldest_days=$((($(date +%s) - oldest) / 86400))
    fi
fi

{
    echo "# Continuous fuzzing, daily report for $today (UTC)"
    echo
    echo "| driver | windows | seeds run |"
    echo "|---|---:|---:|"
    for d in kernel-thread confined; do
        awk -F'\t' -v d="$d" '$2 == d && $7 == "ok" { w++; s += $5 } END { printf "| %s | %d | %d |\n", d, w, s }' "$windows"
    done
    echo
    echo "Windows by status:"
    echo
    echo '```'
    awk -F'\t' '{ n[$7]++ } END { for (k in n) printf "%s %d\n", k, n[k] }' "$windows" | sort
    echo '```'
    echo
    echo "Newest window: $(tail -1 "$windows" | cut -f1)"
    echo
    echo "Held findings: $held_count, oldest $oldest_days days. Their inputs are not published until the fix merges."
} > "$out/report.md"

echo "fuzz-daily-publish: wrote $out"

tag="fuzz-$today"
repo=nifeos/nife

# A token with less than this left is refused: the mint helper renews every 20 minutes, so a token
# this close to its end means the renewal has stopped.
safety_margin_seconds=300

# Seconds since the epoch for `YYYY-MM-DDTHH:MM:SSZ`, or nothing and status 1. GNU date first, then BSD.
epoch_of() {
    case "$1" in
        [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z) ;;
        *) return 1 ;;
    esac
    date -u -d "$1" +%s 2>/dev/null || date -u -j -f '%Y-%m-%dT%H:%M:%SZ' "$1" +%s 2>/dev/null
}

# Refuses, naming the cause and never the token, unless GH_TOKEN is set and has time left.
require_fresh_token() {
    if [ -z "${GH_TOKEN:-}" ]; then
        echo "fuzz-daily-publish: no publish token: GH_TOKEN is not set; see fuzz-token" >&2
        return 1
    fi
    if [ -z "${GH_TOKEN_EXPIRES_AT:-}" ]; then
        echo "fuzz-daily-publish: publish token has no expiry: GH_TOKEN_EXPIRES_AT is not set; see fuzz-token" >&2
        return 1
    fi
    if ! expires="$(epoch_of "$GH_TOKEN_EXPIRES_AT")"; then
        echo "fuzz-daily-publish: publish token expiry is unreadable: GH_TOKEN_EXPIRES_AT is not YYYY-MM-DDTHH:MM:SSZ; see fuzz-token" >&2
        return 1
    fi
    left=$((expires - $(date +%s)))
    if [ "$left" -le 0 ]; then
        echo "fuzz-daily-publish: publish token expired at $GH_TOKEN_EXPIRES_AT; see fuzz-token" >&2
        return 1
    fi
    if [ "$left" -lt "$safety_margin_seconds" ]; then
        echo "fuzz-daily-publish: publish token expires at $GH_TOKEN_EXPIRES_AT, within the ${safety_margin_seconds} second safety margin; see fuzz-token" >&2
        return 1
    fi
}

publish_output() {
    if [ "${NIFE_FUZZ_PUBLISH:-}" != 1 ]; then
        echo "fuzz-daily-publish: NIFE_FUZZ_PUBLISH=1 is not set; nothing published" >&2
        return 1
    fi
    require_fresh_token || return 1
    gh release create "$tag" --repo "$repo" --title "Continuous fuzzing, $today (UTC)" \
        --notes-file "$out/report.md" "$out/report.md" "$out/clean-ranges.tsv"
}

case "${1:-}" in
    --publish) publish_output ;;
    --dry-run)
        echo "fuzz-daily-publish: would create release $tag on $repo with assets:"
        echo "  $out/report.md"
        echo "  $out/clean-ranges.tsv"
        ;;
esac
