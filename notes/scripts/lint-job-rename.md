# The lint job, and renaming a required check

An appendix to [`../scripts.md`](../scripts.md), holding the rename of a required check. Name:
provisional; calef names files.

The lint job is named `lint` (calef, 2026-10-08 UTC: "Go ahead with lint"). It runs every check in
`script/lint`, then `script/lint-fleet`, so a red `clippy` was usually a citation, decision or
roadmap check. A required check is matched by name, so a rename goes in three steps, each safe
alone. First, `ci.yml` gains `lint` and keeps a `clippy` job that only re-reports `lint`'s verdict.
Second, once every open pull request has run that workflow, ruleset 19596094 requires `lint` in
place of `clippy`. Third, the `clippy` job is deleted.

Outstanding: the first step is done and the second is not. The ruleset still requires `clippy`, so
the relay job in `ci.yml` stays until the maintainer runs the second step:

```sh
gh pr list --state open --json number,isDraft,statusCheckRollup --jq '.[] | select(.isDraft | not)
  | select([.statusCheckRollup[].name] | index("lint") | not) | .number'  # must print nothing
gh api repos/nifeos/nife/rulesets/19596094 --jq '{rules: [.rules[]
  | if .type == "required_status_checks" then .parameters.required_status_checks
    |= map(if .context == "clippy" then {context: "lint"} else . end) else . end]}' \
  | gh api -X PUT repos/nifeos/nife/rulesets/19596094 --input -
```
