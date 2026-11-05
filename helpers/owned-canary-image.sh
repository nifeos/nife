#!/bin/sh
# Run the owned image probe for each named check, print the output (and append it to the job
# summary when GITHUB_STEP_SUMMARY is set), and exit 1 when any probe exits non-zero or prints a
# FINDING line.
#
#     helpers/owned-canary-image.sh tools toolchain apt-proxy pip
#
# Why a script: the step used to set `bad=1` inside a `{ ...; } | tee` group. A pipeline stage runs
# in a subshell, so the assignment never reached the step's final `test "$bad" = 0`, and a run that
# printed FINDING lines concluded success. Here the probes write to a file, no variable crosses a
# pipe, and the exit status is read from the shell that holds it. The FINDING scan is a second,
# independent test of the same fact. helpers/owned-canary-image-selftest.sh proves both ways.
#
# OWNED_IMAGE_PROBE names the probe to run, for the selftest; it defaults to
# helpers/owned-image-probe.sh. Run from the repository root.
#
# Name: provisional, minted by lane/canary-finding-fails-job.

set -u

probe=${OWNED_IMAGE_PROBE:-helpers/owned-image-probe.sh}
out=$(mktemp) || exit 2
trap 'rm -f "$out"' EXIT

bad=0
{
    echo "### image: canary-owned"
    echo '```'
    for check in "$@"; do
        echo "-- $check"
        "$probe" "$check" || bad=1
    done
    echo '```'
} >"$out" 2>&1

cat "$out"
if test -n "${GITHUB_STEP_SUMMARY:-}"; then
    cat "$out" >>"$GITHUB_STEP_SUMMARY"
fi
if grep -q '^FINDING' "$out"; then
    bad=1
fi
exit "$bad"
