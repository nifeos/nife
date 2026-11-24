#!/bin/sh
#
# helpers/scope-merge-base.sh: resolve the correct base to diff a pull request's (or a merge
# group's) OWN changes against, printing the merge-base sha on stdout.
#
#     helpers/scope-merge-base.sh [<base-ref>] [<fallback-base-sha>]
#         prints the merge-base sha (HEAD vs "origin/<base-ref>") on stdout, exit 0.
#         if <base-ref> is empty, or "origin/<base-ref>" does not exist locally, falls back to
#         <fallback-base-sha>.
#         prints nothing and exits 1 if neither resolves to a commit (the caller decides its own
#         further fallback, same as before this script existed: a plain `push` event has no
#         base-ref and no fallback sha either, and every caller already falls back further, to
#         `git merge-base HEAD origin/main`, in that case).
#
# # Why this exists
#
# `.github/workflows/architect-label.yml` and nine scope-check steps in `.github/workflows/ci.yml`
# (each heavy job's own "does this change need the thing this job builds" step; since 2026-10-05
# those are one prose-only classification in the `gate` job of ci.yml and of verify.yml, plus
# watchdog's own step, all of which call this script) used to diff
# `github.event.pull_request.base.sha` (or, in ci.yml, `|| github.event.merge_group.base_sha`)
# straight against HEAD. That field is a snapshot GitHub took when the pull request's base last
# changed FOR THIS PULL REQUEST (opened, or last synchronized); it does not track the base branch's
# current tip, so it goes stale the moment another pull request merges into the base branch without
# any push to this one. #1416 was flagged needs-architect for `dependency: xtask/Cargo.toml: toml`:
# that dependency is #1405's, added to `main` after #1416 branched, and the payload's cached
# `base.sha` still pointed at #1414's merge, one commit behind #1405's on `main`. Diffing against
# that stale commit read #1405's own addition as though #1416 had introduced it. ci.yml's
# scope-check steps read the field the identical way and share the same exposure (not yet observed
# to misfire there, only found by inspection when fixing architect-label.yml; fixed here in the same
# pass rather than left for a second one, since it is the same bug with the same fix).
#
# The fix: never trust the payload's cached sha as the primary source when a base-ref is known.
# `actions/checkout` with `fetch-depth: 0` already fetches every branch fresh in the same job, so
# `origin/<base-ref>` is the base branch's actual current tip, at the moment this job runs, for
# free. Take the merge base of THAT against HEAD (what a three-dot diff computes) rather than a
# two-dot diff against a sha that may already be behind it. A pull request's own diff is only ever
# the commits reachable from HEAD and not from that merge base; a base branch that has advanced
# past the fork point changes what the merge base IS, never what "the pull request's own changes"
# means.
#
# `<base-ref>` is empty for a `merge_group` event (whose own `base_sha` is this script's fallback
# argument there, unchanged from before: a merge group's synthetic base is short-lived enough that
# the staleness this script exists to fix is not the risk there) and for a plain `push` (no
# pull_request and no merge_group context at all: both arguments come up empty, this script prints
# nothing, and the caller's own further fallback runs exactly as before this script existed).
#
# Pure git plumbing, no `gh`, no network beyond what the caller already fetched: cheap enough for
# `helpers/scope-merge-base-selftest.sh` to build a throwaway repo and run this against it
# directly, no subprocess mocking required.
#
# Name: provisional, minted by this lane, 2026-09-27 (renamed from the single-caller
# `architect-label-diff.sh` once `ci.yml` became a second caller); naming is an architect's call
# (partsfoundry/record:design/naming.md).

set -eu

base_ref="${1:-}"
fallback="${2:-}"

candidate=""
if [ -n "$base_ref" ]; then
	candidate="origin/$base_ref"
	if ! git rev-parse --verify --quiet "$candidate" >/dev/null 2>&1; then
		candidate=""
	fi
fi
[ -n "$candidate" ] || candidate="$fallback"

if [ -z "$candidate" ] || ! git rev-parse --verify --quiet "$candidate" >/dev/null 2>&1; then
	exit 1
fi

git merge-base HEAD "$candidate"
