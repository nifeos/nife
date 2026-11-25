#!/bin/sh
#
# Selftest for script/claim's refusals, run by script/lint-fleet. Every case here must be refused before
# script/claim touches git, the disk or the network. `git` and `gh` are stubs on PATH that record
# being called and fail, so a refusal that regresses fails this test instead of making a real claim
# (the first falsification of this file, run without the stubs, opened draft #1898 and pushed its
# branch, 2026-10-10 UTC). The refusals are the point: a claim that
# accepts an unclassified lane looks the same as one that cannot (calef's ruling R1 on #1894,
# 2026-10-10 UTC: exactly one of --debt-paydown or --new-work; nifeos/record:notes/debt-paydown.md).
#
# Name: provisional, 2026-10-10 (UTC), after the other helpers/*-selftest.sh.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
claim="$root/script/claim"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export NIFE_WORKTREES="$work/worktrees"
real_git=$(command -v git)
mkdir "$work/bin"
for tool in git gh; do
    printf '#!/bin/sh\necho "%s $*" >> "%s/called"\nexit 97\n' "$tool" "$work" > "$work/bin/$tool"
    chmod +x "$work/bin/$tool"
done
PATH="$work/bin:$PATH"
branch="maintainer/claim-selftest-$$"

fail() { echo "claim-selftest: $*" >&2; exit 1; }

# expect NAME WANT_EXIT WANT_TEXT ARGS...: run script/claim, check its exit and its output.
expect() {
    name=$1 want=$2 text=$3
    shift 3
    set +e
    out=$(sh "$claim" "$@" 2>&1)
    got=$?
    set -e
    [ ! -e "$work/called" ] || fail "$name: reached $(cat "$work/called") before refusing"
    [ "$got" -eq "$want" ] || fail "$name: exit $got, wanted $want ($out)"
    printf '%s' "$out" | grep -q -- "$text" || fail "$name: output lacks '$text' ($out)"
}

expect "no classification" 2 "add --debt-paydown or --new-work" "$branch"
expect "both classifications" 2 "both given" "$branch" --debt-paydown --new-work
expect "both, other order" 2 "both given" --new-work "$branch" --debt-paydown
expect "help names both" 0 "(--debt-paydown | --new-work)" --help
# An issue that is not N or owner/repo#N, or an issue with --no-claim, is refused before gh runs.
expect "issue not a number" 2 "takes N or owner/repo#N" "$branch" --new-work --issue 12x
expect "issue, three slashes" 2 "takes N or owner/repo#N" "$branch" --new-work --issue a/b/c#1
expect "issue without a draft" 2 "with --no-claim" "$branch" --new-work --issue 12 --no-claim
# Classified, but the name is a milestone near-miss: the shape refusal still fires.
expect "classified, bad name" 1 "does not parse" milestone-1-x --debt-paydown

# A milestone number an open pull request holds is refused before git is touched. `gh pr list`
# answers from $work/prs; every other gh call, and every git call, still records and fails. #1901's
# shape: branch cut as 868, title renumbered to milestone 871 (a sixth outsider pass attacks the
# confinement claim), so 871 is held and 868 is not.
export NIFE_SMELTER=
printf '#!/bin/sh\nif [ "$1 $2" = "pr list" ]; then cat "%s/prs"; exit 0; fi\necho "gh $*" >> "%s/called"\nexit 97\n' \
    "$work" "$work" > "$work/bin/gh"
printf '1901\tmilestone/868-a-sixth-pass\tmilestone 871 (a sixth outsider pass attacks the confinement claim)\n1897\tmilestone/592-uart-drain\tdrain the UART\n' \
    > "$work/prs"
expect "held by a title" 1 "#1901 milestone 871" milestone/871-another-thing --new-work
expect "held by a branch" 1 "#1897 drain the UART" milestone/0592-a-thing --new-work
# A free number gets past the check, and the next thing it reaches is git (the stub's exit 97).
set +e
out=$(sh "$claim" milestone/868-a-new-thing --new-work 2>&1)
set -e
grep -q '^git ' "$work/called" || fail "free number: never reached git ($out)"
printf '%s' "$out" | grep -q "already held" && fail "free number: refused ($out)"
rm -f "$work/called"

# A relative --worktree means the caller's directory, wherever the script lives. git answers
# `fetch` and records the path `worktree add` was handed, then fails, so nothing is created.
printf '#!/bin/sh\ncase "$3 $4" in "fetch origin") exit 0 ;; "worktree add") echo "$5" > "%s/added" ;; esac\necho "git $*" >> "%s/called"\nexit 97\n' \
    "$work" "$work" > "$work/bin/git"
mkdir -p "$work/caller"
set +e
out=$(cd "$work/caller" && sh "$claim" "maintainer/claim-selftest-rel-$$" --new-work --worktree ../lanes/rel 2>&1)
set -e
[ "$(cat "$work/added" 2>/dev/null)" = "$work/caller/../lanes/rel" ] \
    || fail "relative --worktree: git was handed '$(cat "$work/added" 2>/dev/null)', wanted it under $work/caller ($out)"
rm -f "$work/called" "$work/added"

# An issue someone holds is refused before git runs. The stub gh answers the three reads
# helpers/issue_claim.py makes (the issue, assigned to someone; no comments; no pull request) and
# records anything else, such as a claim comment, which must never be posted here.
printf '#!/bin/sh\ncase "$*" in\n"api repos/o/r/issues/12") echo '"'"'{"state":"open","assignees":[{"login":"someone"}],"labels":[]}'"'"'; exit 0 ;;\n"api --paginate"*) exit 0 ;;\n"api graphql"*) echo '"'"'{"data":{"repository":{"issue":{"closedByPullRequestsReferences":{"nodes":[]}}}}}'"'"'; exit 0 ;;\nesac\necho "gh $*" >> "%s/called"\nexit 97\n' \
    "$work" > "$work/bin/gh"
printf '#!/bin/sh\necho "git $*" >> "%s/called"\nexit 97\n' "$work" > "$work/bin/git"
expect "issue held by an assignee" 1 "assigned to @someone" "maintainer/claim-selftest-issue-$$" --new-work --issue o/r#12

"$real_git" -C "$root" rev-parse --verify --quiet "refs/heads/$branch" >/dev/null \
    && fail "a refused claim created branch $branch"
[ ! -e "$NIFE_WORKTREES" ] || fail "a refused claim created $NIFE_WORKTREES"
echo "claim-selftest: ok"
