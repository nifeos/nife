#!/bin/sh
# helpers/fuzz-daily-publish.sh: write the daily report of the continuous fuzz run.
#
#     helpers/fuzz-daily-publish.sh [--publish | --dry-run]
#
# Reads `$NIFE_FUZZ_HOME/state/windows.tsv` (helpers/continuous-fuzz.sh) and writes, under
# `$NIFE_FUZZ_HOME/out/<utc date>/`:
#
#   report.md      counter totals: windows, seeds run by driver, windows by status, the newest
#                  window's time, the count and age in days of held findings, and one `Filer:` line
#                  (below)
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
# (design/roadmap/1016-the-clean-range-table-does-not-locate-a-held-seed.md). A held window is
# any window whose status is neither `ok` nor `infrastructure`, so a status this script does not
# know withholds its slot too. The seed totals in report.md still count every clean window.
#
# **The Filer line.** The filer (`fuzz-filer` in nifeos/fuzz-host) is a separate service, and a
# disabled timer once left every other check green. The line gives host-local counts and times only:
# held logs, causes, filed, parked, unfiled (causes that are not parked budget or hang causes and
# have no advisory yet), the filer ledger's newest timestamp, whether fuzz-filer.timer is enabled and
# when it last ran. The counts come from `fuzz-filer run --dry-run`, which reads no token and sends
# nothing; only its closing tally is kept, and no other line of its output is copied. A value the host
# cannot read is `unknown`, which helpers/fuzz-heartbeat.py fails on. NIFE_FUZZ_FILER,
# NIFE_FUZZ_FILER_LEDGER and NIFE_FUZZ_SYSTEMCTL replace the program, the ledger path and systemctl
# for the selftest.
#
# BUGS: nothing brings a withheld slot's ranges back. A person publishes a finding's advisory and the
# host is not told (design/roadmap/1010-a-held-finding-turns-public-after-its-fix-reruns-clean.md,
# refused 2026-11-04), so a slot that ever held a finding publishes no further ranges. That fails safe.
#
# **Where it goes.** Ruled 2026-11-01 (UTC): one GitHub release per day on nifeos/nife, tagged
# `fuzz-YYYY-MM-DD`, with report.md and clean-ranges.tsv as assets
# (design/roadmap/1026-where-the-daily-fuzz-output-goes.md). Without a flag the script stops at
# the local files. `--dry-run` also prints the tag and the asset list. `--publish` runs
# `gh release create` and refuses unless NIFE_FUZZ_PUBLISH=1 is set. It also refuses, by name and
# before any `gh` call, a missing GH_TOKEN, a missing or unparseable GH_TOKEN_EXPIRES_AT (UTC,
# `YYYY-MM-DDTHH:MM:SSZ`), and a token that has expired or expires within `safety_margin_seconds`.
# The mint helper's token file supplies both variables (nifeos/fuzz-host, docs/fuzz-token.md).
#
# **A second run on the same day.** Before creating the release, `--publish` reads it with
# `gh release view`. If the day's release already holds exactly report.md and clean-ranges.tsv, the
# run logs that and exits 0: the release keeps the day's first report, and this run's report stays in
# the local directory. A release with any other set of assets (one missing after a failed upload, or
# one added by hand) is refused with exit 1 and left alone; the script never deletes, replaces or adds
# an asset, so a person decides. Assets are compared by name, not content, because every run writes a
# newer report than the last. A `gh release view` failure other than `release not found` is refused
# too, since it cannot tell whether the release exists. A
# refusal never prints the token. The credential is a one-hour
# installation token of the `nife-fuzz` App, minted from the App's root-only key (Contents write and
# Repository security advisories write on nifeos/nife alone; notes/fuzzing/the-fuzz-app.md). Main's
# ruleset refuses a push without a pull request, and this script never calls `git push`. Findings
# start as private security advisories, filed by a service on the host, not by this script
# (design/roadmap/1015-fuzz-findings-without-a-person.md).
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

filer_bin="${NIFE_FUZZ_FILER:-/usr/local/bin/fuzz-filer}"
filer_ledger="${NIFE_FUZZ_FILER_LEDGER:-/var/lib/fuzz-filer/ledger.tsv}"
systemctl_bin="${NIFE_FUZZ_SYSTEMCTL:-systemctl}"

# The filer's closing tally, "N held logs, N causes, N filed before, N parked, ..., none to file" or
# ", N to file". Everything else the dry run prints names causes and is dropped here.
tally="$("$filer_bin" run --dry-run 2>&1 | grep -E '^fuzz-filer: [0-9]+ held logs, ' | tail -1 || true)"
f_held=unknown f_causes=unknown f_filed=unknown f_parked=unknown f_unfiled=unknown
case "$tally" in
    *"held logs, "*" causes, "*" filed before, "*" parked, "*)
        counts="$(printf '%s\n' "$tally" | sed -E 's/^fuzz-filer: ([0-9]+) held logs, ([0-9]+) causes, ([0-9]+) filed before, ([0-9]+) parked, .*/\1 \2 \3 \4/')"
        # Not `set --`: the script's own arguments are read again below.
        IFS=' ' read -r a b c d extra <<COUNTS
$counts
COUNTS
        if [ -n "$d" ] && [ -z "$extra" ]; then
            f_held=$a f_causes=$b f_filed=$c f_parked=$d
            case "$tally" in
                *", none to file"*) f_unfiled=0 ;;
                *) n="$(printf '%s\n' "$tally" | sed -nE 's/.*, ([0-9]+) to file.*/\1/p')"
                   [ -n "$n" ] && f_unfiled=$n ;;
            esac
        fi
        ;;
esac

# Newest ledger line's timestamp, the first field of the last line that is not a comment.
f_ledger=none
if [ -e "$filer_ledger" ]; then
    f_ledger=unknown
    if [ -r "$filer_ledger" ]; then
        f_ledger="$(awk -F'\t' '/^#/ || NF == 0 { next } { t = $1 } END { print t }' "$filer_ledger")"
        [ -n "$f_ledger" ] || f_ledger=none
    fi
fi

# The timer's state and last start, from systemd. GNU date reads systemd's UTC timestamp.
f_enabled="$("$systemctl_bin" is-enabled fuzz-filer.timer 2>/dev/null || true)"
case "$f_enabled" in enabled) f_enabled=yes ;; "") f_enabled=unknown ;; *) f_enabled=no ;; esac
f_last="$("$systemctl_bin" show fuzz-filer.timer --property=LastTriggerUSec --timestamp=utc 2>/dev/null \
    | sed -n 's/^LastTriggerUSec=//p' || true)"
case "$f_last" in
    "" ) f_last=unknown ;;
    n/a|0) f_last=never ;;
    *) f_last="$(date -u -d "$f_last" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo unknown)" ;;
esac

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
    echo
    echo "Filer: held=$f_held causes=$f_causes filed=$f_filed parked=$f_parked unfiled=$f_unfiled ledger-newest=$f_ledger timer-enabled=$f_enabled timer-last-run=$f_last"
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
    view_err="$(mktemp "${TMPDIR:-/tmp}/fuzz-daily-publish.XXXXXX")"
    if names="$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name' 2>"$view_err")"; then
        rm -f "$view_err"
        names="$(printf '%s\n' "$names" | sed '/^$/d' | sort | tr '\n' ' ')"
        if [ "$names" = "clean-ranges.tsv report.md " ]; then
            echo "fuzz-daily-publish: release $tag already exists on $repo with report.md and clean-ranges.tsv; nothing published, the release keeps the day's first report and this run's stays in $out"
            return 0
        fi
        echo "fuzz-daily-publish: release $tag already exists on $repo with assets '${names% }', want 'clean-ranges.tsv report.md'; refusing to change it, a person decides" >&2
        return 1
    fi
    reason="$(head -1 "$view_err")"
    rm -f "$view_err"
    if [ "$reason" != "release not found" ]; then
        echo "fuzz-daily-publish: cannot tell whether release $tag exists on $repo ($reason); nothing published" >&2
        return 1
    fi
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
