#!/bin/sh
#
# Run `rustup "$@"`, retrying a failed download with backoff, and fail loudly when every attempt has.
#
#     helpers/rustup-retry.sh toolchain install
#     helpers/rustup-retry.sh target add --toolchain "$channel" x86_64-unknown-none
#
# # Why this exists
#
# Every CI job fetches the pinned nightly from static.rust-lang.org, because no job caches it, and
# rustup makes one attempt at a connection. On 2026-10-18 a TCP connect to the CDN timed out once
# (30 s) in `script/bootstrap`, and a docs-only pull request went red for it. On 2026-10-08 the
# floating `nightly` manifest failed its checksum while the CDN was mid-publish, and that one ejected
# a pull request from the merge queue. Both cleared on the next try. partsfoundry/record:notes/ci-toolchain-download.md
# has the two weeks of counts this was measured against.
#
# # What it retries, and what it does not
#
# Only a failure whose output looks like a download: a connect or DNS error, a timeout, a checksum
# mismatch. Anything else (an unknown target, a typo in a component) fails on the first attempt with
# rustup's own message, because retrying a real error four times only hides it for two minutes.
#
# NIFE_RUSTUP_ATTEMPTS (default 4) bounds the attempts. NIFE_RUSTUP_BACKOFF (default 10) is the
# first wait in seconds, tripled after each failure: 10, 30, 90, which with rustup's 30-second
# connect timeout puts the worst case near five minutes, inside every job's budget.
# helpers/rustup-retry-selftest.sh, run by script/lint, holds all of this against a stub rustup.
#
# Name: provisional, minted 2026-10-19 (UTC) by maintainer/rustup-retry. calef has not ruled on it.

set -u

attempts=${NIFE_RUSTUP_ATTEMPTS:-4}
wait=${NIFE_RUSTUP_BACKOFF:-10}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

n=1
while :; do
    # rustup's output still streams to the job log; the copy is what the classifier and the final
    # message read. The exit status crosses the pipe through a file because POSIX sh has no
    # pipefail.
    { rustup "$@" 2>&1; echo $? > "$work/rc"; } | tee "$work/out"
    rc=$(cat "$work/rc")
    [ "$rc" -eq 0 ] && exit 0

    if ! grep -qiE 'could not download|error downloading|download failed|checksum failed|error sending request|tcp connect error|dns error|timed out|connection (reset|refused)' "$work/out"; then
        echo "rustup-retry: \`rustup $*\` failed (exit $rc) with an error that is not a download; not retrying" >&2
        exit "$rc"
    fi

    url=$(grep -oE "https?://[^' )]+" "$work/out" | head -1)
    [ -n "$url" ] || url=${RUSTUP_DIST_SERVER:-https://static.rust-lang.org}

    if [ "$n" -ge "$attempts" ]; then
        msg="rustup-retry: \`rustup $*\` failed $n of $attempts attempts fetching $url"
        echo "$msg" >&2
        # An annotation puts the line on the run's summary page, where a reader of a red check looks
        # first, instead of only at the bottom of a long step log.
        [ -n "${GITHUB_ACTIONS:-}" ] && echo "::error title=rust toolchain download::$msg"
        exit "$rc"
    fi

    echo "rustup-retry: attempt $n of $attempts failed fetching $url; retrying in ${wait}s" >&2
    sleep "$wait"
    wait=$((wait * 3))
    n=$((n + 1))
done
