# helpers/queue-eligible.jq: what "eligible for the merge queue" means, in one place.
#
# Two scripts armed auto-merge in 2026-09 (`merge-drain.sh` on a schedule as `nife-smelter[bot]`,
# `queue-hold.sh` when a person releases a hold), and until the 2026-09-24 security audit each
# carried its own copy of this predicate, and both copies were wrong in the same way: "open, not a draft, against main"
# admitted a pull request from ANY fork. The ruleset on `main` requires zero approving reviews, so a
# stranger whose checks went green was one drain pass from merged, by automation, with nobody having
# read the diff. The audit report has the whole path (partsfoundry/record:design/audit-reports/, 2026-09-24).
#
# `isCrossRepository == false` is the fix: every lane in this project pushes its branch to this
# repository (AGENTS.md: the pushed branch is the lane ledger), so a head that lives in another
# repository is by construction not a lane, and arming it is a person's decision. A collaborator
# working from a fork arms their own pull request by hand.
#
# `jq` cannot compose `-f` with an inline program, so the consumers splice this file's text in front
# of theirs: jq -r "$(cat helpers/queue-eligible.jq)" '[ .[] | eligible | ... ]'. A missing file
# leaves `eligible` undefined, jq refuses the program, and the consumer's `|| echo '[]'` arms
# nothing, which is the direction to fail in. helpers/queue-eligible-selftest.sh checks the
# predicate against fixtures and checks that both consumers still splice it, and script/lint-fleet runs
# that self-test.
#
# The `--json` list a consumer passes to `gh pr list` MUST include isCrossRepository, isDraft and
# baseRefName; a missing field reads as null, `null == false` is false, and everything is refused.
def eligible:
  select(.isDraft == false)
  | select(.baseRefName == "main")
  | select(.isCrossRepository == false);
