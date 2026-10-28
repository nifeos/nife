#!/bin/sh
#
# helpers/lint-sections.sh: run script/lint's sections one at a time, keep going past a failure,
# and fail at the end if any section failed.
#
#     helpers/lint-sections.sh <lint script> all|no-cargo|clippy
#
# `script/lint` calls this; you do not. Each section is the preamble plus the text from its
# `echo "==> ` header to the next one, as helpers/lint-no-cargo.awk splits them, run in a fresh `sh`
# with $0 naming the lint script. A failing section prints what it always printed, the next section
# runs anyway, and a summary at the end names every section that failed.
#
# Why not stop at the first failure. On 2026-10-18 (UTC) 16 lanes ran lint 178 times and 74 runs
# failed. 25 of those failures were already in the tree at the run before, so a lane learned one
# problem per run. notes/lint-cost-2026-10-18.md has the counts.
#
# **The verdict is the fail-fast verdict.** A run passes exactly when every section passes, which
# is when the old single `set -e` script passed: a section that would have been skipped after an
# earlier failure can only add failures to a run that already fails. Each section runs in order,
# so a section that reads a file an earlier one wrote (the uefi_loader clippy passes read the
# kernels the section before them builds) still finds it. A section cannot see an earlier one's
# shell variables, so anything two sections share belongs in the preamble; a section that breaks
# that rule fails here, loudly, in every run. helpers/lint-sections-selftest.sh holds the verdict
# to fail-fast's across every combination of planted failures.
#
# `no-cargo` runs only the sections `--no-cargo` keeps, with LINT_NO_CARGO=1. `clippy` stops after
# the section that carries the `--clippy` cut, which is where `script/lint --clippy` stopped.
#
# This script times each section in whole seconds around its `sh -c`. As a section completes, one
# line names it and its seconds, so the cost appears where the run's own output already flows:
#
#     lint-section-time: clippy: host crates + xtask 187
#
# A section still running, or one the run never reached, shows as the absence of its line. The
# table at the end sorts the sections that ran slowest first, beside the failure summary and the
# verdict. The lines carry no `==>`, so the log readers that attribute a lint job by its last
# `==>` marker (script/ci-log-baseline, helpers/hang_evidence.py) see nothing new.
#
# One row per section also lands in a CSV outside this repository, on script/effort's precedent
# (the collector must write from a launchd run without touching a worktree). The default is
# ~/.local/share/nife/lint-section-times.csv; NIFE_LINT_TIMES overrides it. Columns, in order, with
# no header row, because two lint runs share this machine and a create-time header would race:
# date (UTC), host, mode, section, seconds, exit. The host is `hostname`'s output at write time,
# unquoted: a base-system hostname is letters, digits, hyphens and dots, so it never carries the
# comma the name can, and the name stays quoted CSV-style so a comma inside it reads as part of the
# name. helpers/lint-sections-selftest.sh plants a comma in a section name so the quoting stays
# provably correct. A machine adopts the logging by pulling main and running lint: there is no
# setup step, the CSV is per machine by design, and the host column keeps a row attributable when
# files from several machines are combined later. The whole path stays /bin/sh over base-system
# tools (`date`, `hostname`, `sed`), no python: a machine whose python is too old for lint's own
# sections must still log its section times. A directory that cannot be created, or a write that
# fails, is skipped silently: these rows are a measurement, and writing one must never change the
# verdict. helpers/lint-sections-selftest.sh holds all of this, including the unwritable case. The
# rows carry the per-section wall time that notes/lint-cost-2026-10-18.md does not; what they mean
# and where they should land once a week of them exists is that note's maintainer call, not this
# script's.
#
# Name: provisional, minted 2026-10-19 (UTC) by lane maintainer/lint-cost.

set -u
lint_self=$1
mode=$2
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$(mktemp -d "${TMPDIR:-/tmp}/lint-sections.XXXXXX")" || exit 1
trap 'rm -rf "$dir"' EXIT HUP INT TERM
awk -v mode=split -v dir="$dir" -f "$root/helpers/lint-no-cargo.awk" "$lint_self" || exit 1

if [ "$mode" = no-cargo ]; then
    LINT_NO_CARGO=1
    export LINT_NO_CARGO
fi
LINT_SECTION=1
export LINT_SECTION

# Where the per-section rows go (see the header). The directory is created once, here: a failed
# creation empties the file name, and every per-section write then skips, so the loop below never
# repeats the decision.
lint_times_default="${HOME:-}/.local/share/nife/lint-section-times.csv"
lint_times_file=${NIFE_LINT_TIMES:-$lint_times_default}
if ! mkdir -p "$(dirname "$lint_times_file")" 2>/dev/null; then
    lint_times_file=""
fi

tab="$(printf '\t')"
ran=0
failed=""
times=""
# The index is read on fd 3 so that a section reading stdin cannot eat it.
while IFS="$tab" read -r id keep name <&3; do
    if [ "$mode" = no-cargo ] && [ "$keep" = 0 ]; then
        continue
    fi
    cat "$dir/preamble" "$dir/$id" >"$dir/run"
    ran=$((ran + 1))
    start=$(date +%s)
    sh -c 'lint_tmp=$1; shift; . "$lint_tmp"' "$lint_self" "$dir/run"
    status=$?
    if [ "$status" -ne 0 ]; then
        failed="$failed  $name (exit $status)
"
    fi
    secs=$(($(date +%s) - start))
    echo "lint-section-time: $name $secs"
    times="$times$secs$tab$name
"
    if [ -n "$lint_times_file" ]; then
        # The subshell keeps a redirect failure (a full disk, a file that lost its permission)
        # inside its own stderr: the row is skipped silently, and the verdict carries on.
        (printf '%s,%s,%s,"%s",%s,%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
            "$(hostname)" "$mode" \
            "$(printf '%s' "$name" | sed 's/"/""/g')" "$secs" "$status" \
            >>"$lint_times_file") 2>/dev/null || :
    fi
    if [ "$mode" = clippy ] && grep -q '"--clippy"' "$dir/$id"; then
        echo "==> clippy-only mode: the rest of lint still runs before any report or merge"
        break
    fi
done 3<"$dir/index"

if [ "$ran" -gt 0 ]; then
    echo "lint: seconds per section, slowest first ($ran sections)"
    printf '%s' "$times" | sort -rn | awk -F"$tab" '{ printf "%6d  %s\n", $1, $2 }'
fi

if [ -n "$failed" ]; then
    count="$(printf '%s' "$failed" | grep -c .)"
    echo "" >&2
    echo "lint: $count of $ran sections failed. Each one's own output is above its name's ==> line:" >&2
    printf '%s' "$failed" >&2
    exit 1
fi
echo "lint: all $ran sections passed"
