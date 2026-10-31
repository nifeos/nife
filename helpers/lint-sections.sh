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
lane_pids=""
cleanup() {
    # Only the lane subshells this script started, by their own pids.
    for p in $lane_pids; do kill "$p" 2>/dev/null || :; done
    rm -rf "$dir"
}
trap cleanup EXIT HUP INT TERM
awk -v mode=split -v dir="$dir" -f "$root/helpers/lint-no-cargo.awk" "$lint_self" || exit 1

if [ "$mode" = no-cargo ] || [ "$mode" = quick ]; then
    LINT_NO_CARGO=1
    export LINT_NO_CARGO
fi
LINT_SECTION=1
export LINT_SECTION

# How many sections may run at once. See "Running sections side by side" in the header.
jobs=${NIFE_LINT_JOBS:-}
if [ -z "$jobs" ]; then
    cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)
    jobs=$((cpus / 4))
    [ "$jobs" -le 8 ] || jobs=8
fi
case $jobs in
'' | *[!0-9]* | 0) jobs=1 ;;
esac

# Where the per-section rows go (see the header). The directory is created once, here: a failed
# creation empties the file name, and every per-section write then skips, so the loop below never
# repeats the decision.
lint_times_default="${HOME:-}/.local/share/nife/lint-section-times.csv"
lint_times_file=${NIFE_LINT_TIMES:-$lint_times_default}
if ! mkdir -p "$(dirname "$lint_times_file")" 2>/dev/null; then
    lint_times_file=""
fi

tab="$(printf '\t')"

# The plan: the sections this run covers, in order, one `id lane waits name` line each. `no-cargo`
# drops the cargo sections, `quick` keeps the sections that carry a lint-quick marker, and `clippy`
# stops after the section that carries the `--clippy` cut, which is where `script/lint --clippy`
# stopped.
: >"$dir/plan"
while IFS="$tab" read -r id keep lane waits quick tdir name <&3; do
    if { [ "$mode" = no-cargo ] || [ "$mode" = quick ]; } && [ "$keep" = 0 ]; then
        continue
    fi
    if [ "$mode" = quick ] && [ "$quick" != 1 ]; then
        continue
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$lane" "$waits" "$tdir" "$name" >>"$dir/plan"
    if [ "$mode" = clippy ] && grep -q '"--clippy"' "$dir/$id"; then
        break
    fi
done 3<"$dir/index"

# Run one section in a fresh shell and record how it ended in $dir/done.<id>: exit status, whole
# seconds of wall time, CPU seconds the section's processes used (user plus system, from `times`
# in a subshell that has run nothing else), and the UTC time it ended. The file appears whole or
# not at all, so a reader that sees it sees all of it.
run_section() {
    id=$1  # $2: the section's own build directory name, or -
    cat "$dir/preamble" "$dir/$id" >"$dir/run.$id"
    start=$(date +%s)
    (
        # A section with its own build directory gets it when the run has the cores for several
        # cargo builds at once (see the header); otherwise every section shares target/.
        if [ "$jobs" -ge 4 ] && [ "$2" != - ]; then
            CARGO_TARGET_DIR="$root/target/lint-lanes/$2"
            export CARGO_TARGET_DIR
        fi
        sh -c 'lint_tmp=$1; shift; . "$lint_tmp"' "$lint_self" "$dir/run.$id" 4>&-
        status=$?
        times >"$dir/times.$id" 2>/dev/null || :
        exit "$status"
    )
    status=$?
    secs=$(($(date +%s) - start))
    cpu=$(sed -n 2p "$dir/times.$id" 2>/dev/null | awk '{
        for (i = 1; i <= 2; i++) { split($i, a, "m"); t += a[1] * 60 + a[2] }
        printf "%.1f", t }')
    printf '%s %s %s %s\n' "$status" "$secs" "${cpu:-0.0}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        >"$dir/done.$id.tmp"
    mv "$dir/done.$id.tmp" "$dir/done.$id"
}

ran=0
failed=""
times=""
wall_start=$(date +%s)

# The pass marker the pre-push hook reads (helpers/pre-push-lint-cache.sh), keyed before the first
# section starts, in the background so the key costs the run nothing. A run of every section, or of
# the no-cargo sections, that passes whole on a tree that did not move leaves the marker, so a push
# that follows skips a run it would repeat. `quick` and `clippy` cover fewer sections than the hook
# does and leave none.
marker_key_before=""
# Only for this tree's own script/lint: a fixture lint script (the selftests') passing says nothing
# about the tree and must never leave a marker.
key_pid=""
case "$mode" in
all | no-cargo)
    if [ "$lint_self" = "$root/script/lint" ]; then
        "$root/helpers/pre-push-lint-cache.sh" key >"$dir/key.before" 2>/dev/null &
        key_pid=$!
    fi
    ;;
esac

# What a finished section adds to the run: the time lines, the CSV row, the failure list.
account() {
    id=$1
    name=$2
    read -r status secs cpu stamp <"$dir/done.$id"
    ran=$((ran + 1))
    if [ "$status" -ne 0 ]; then
        failed="$failed  $name (exit $status)
"
    fi
    echo "lint-section-time: $name $secs"
    echo "lint-section-cpu: $name $cpu"
    times="$times$secs$tab$cpu$tab$name
"
    if [ -n "$lint_times_file" ]; then
        # The subshell keeps a redirect failure (a full disk, a file that lost its permission)
        # inside its own stderr: the row is skipped silently, and the verdict carries on.
        (printf '%s,%s,%s,"%s",%s,%s\n' "$stamp" \
            "$(hostname)" "$mode" \
            "$(printf '%s' "$name" | sed 's/"/""/g')" "$secs" "$status" \
            >>"$lint_times_file") 2>/dev/null || :
    fi
}

if [ "$jobs" -le 1 ]; then
    # One at a time, in order, each section's output straight to the terminal: exactly what this
    # script did before sections could overlap.
    while IFS="$tab" read -r id lane waits tdir name <&3; do
        run_section "$id" "$tdir"
        account "$id" "$name"
    done 3<"$dir/plan"
else
    # Side by side. Lanes run side by side and each runs its own sections in order; sections in no
    # lane are the pool and start in plan order. At most $jobs sections run at any moment, counted
    # by tokens in a FIFO. Output goes to a file per section and is replayed below in plan order,
    # so a log reads as it always has, with each section's lines together under its `==>` line.
    mkfifo "$dir/tokens"
    exec 4<>"$dir/tokens"
    n=0
    while [ "$n" -lt "$jobs" ]; do
        echo >&4
        n=$((n + 1))
    done

    # Wait for every earlier section of the lanes named in $1 (comma separated, or `-`).
    wait_for() {
        for lane_name in $(printf '%s' "$1" | tr , ' '); do
            [ "$lane_name" != - ] || continue
            [ -f "$dir/lane.$lane_name" ] || continue
            while read -r dep <&6; do
                [ "$dep" -lt "$2" ] || break
                while [ ! -e "$dir/done.$dep" ]; do sleep 0.1; done
            done 6<"$dir/lane.$lane_name"
        done
    }
    run_into_file() {
        wait_for "$2" "$1"
        run_section "$1" "$3" >"$dir/out.$1" 2>&1 </dev/null
    }

    : >"$dir/lane.-"
    while IFS="$tab" read -r id lane waits tdir name <&3; do
        printf '%s\n' "$id" >>"$dir/lane.$lane"
    done 3<"$dir/plan"

    # Each named lane, in the order its first section appears, so the lane that starts first
    # (the cargo sections, which hold the build lock and take longest) gets the first tokens.
    for lane_name in $(awk -F"$tab" '$2 != "-" && !seen[$2]++ { print $2 }' "$dir/plan"); do
        (
            while read -r id <&5; do
                waits=$(awk -F"$tab" -v id="$id" '$1 == id { print $3 }' "$dir/plan")
                tdir=$(awk -F"$tab" -v id="$id" '$1 == id { print $4 }' "$dir/plan")
                read -r _ <&4
                run_into_file "$id" "$waits" "$tdir"
                echo >&4
            done 5<"$dir/lane.$lane_name"
        ) &
        lane_pids="$lane_pids $!"
    done
    (
        while read -r id <&5; do
            waits=$(awk -F"$tab" -v id="$id" '$1 == id { print $3 }' "$dir/plan")
            tdir=$(awk -F"$tab" -v id="$id" '$1 == id { print $4 }' "$dir/plan")
            read -r _ <&4
            (
                run_into_file "$id" "$waits" "$tdir"
                echo >&4
            ) &
        done 5<"$dir/lane.-"
        wait
    ) &
    lane_pids="$lane_pids $!"

    # Replay. The section at the head of the plan has its `==>` line printed as soon as it has
    # one, so a run that hangs still names the section it hangs in, as the log readers expect
    # (script/ci-log-baseline takes the last `==>` line).
    while IFS="$tab" read -r id lane waits tdir name <&3; do
        while [ ! -s "$dir/out.$id" ] && [ ! -e "$dir/done.$id" ]; do sleep 0.1; done
        if [ -s "$dir/out.$id" ]; then
            head -n 1 "$dir/out.$id"
        fi
        while [ ! -e "$dir/done.$id" ]; do sleep 0.1; done
        tail -n +2 "$dir/out.$id"
        account "$id" "$name"
    done 3<"$dir/plan"
    wait
fi
wall=$(($(date +%s) - wall_start))

if [ "$mode" = clippy ] && [ "$ran" -gt 0 ]; then
    echo "==> clippy-only mode: the rest of lint still runs before any report or merge"
fi

if [ "$ran" -gt 0 ]; then
    echo "lint: seconds per section, slowest first ($ran sections; wall seconds, then CPU seconds)"
    printf '%s' "$times" | sort -rn | awk -F"$tab" '{ printf "%6d %7s  %s\n", $1, $2, $3 }'
    printf '%s' "$times" | awk -F"$tab" -v wall="$wall" -v jobs="$jobs" '
        { s += $1; c += $2 }
        END { printf "lint: %d s wall, %d s of sections added up, %.0f s CPU, up to %d at once\n",
              wall, s, c, jobs }'
fi

if [ -n "$failed" ]; then
    count="$(printf '%s' "$failed" | grep -c .)"
    echo "" >&2
    echo "lint: $count of $ran sections failed. Each one's own output is above its name's ==> line:" >&2
    printf '%s' "$failed" >&2
    exit 1
fi
echo "lint: all $ran sections passed"
# Reached only when every section passed. Any failure to key or to write skips the marker.
if [ -n "$key_pid" ]; then
    wait "$key_pid" 2>/dev/null || :
    marker_key_before=$(cat "$dir/key.before" 2>/dev/null) || marker_key_before=""
    marker_key_after=$("$root/helpers/pre-push-lint-cache.sh" key 2>/dev/null) || marker_key_after=""
    if [ -n "$marker_key_before" ] && [ "$marker_key_before" = "$marker_key_after" ]; then
        "$root/helpers/pre-push-lint-cache.sh" write "$marker_key_after" >/dev/null 2>&1 || :
    fi
fi
