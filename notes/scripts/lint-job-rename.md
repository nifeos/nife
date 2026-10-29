# The lint job, and renaming a required check

An appendix to [`../scripts.md`](../scripts.md), holding the rename of a required check. Name:
provisional; calef names files.

The lint job is named `lint` (calef, 2026-10-08 UTC: "Go ahead with lint"). It runs every check in
`script/lint`, then `script/lint-fleet`, so a red `clippy` was usually a citation, decision or
roadmap check. Ruleset 19596094 requires `lint` directly, and the relay job that re-reported
`lint`'s verdict under the old name is gone. No step is outstanding.

A required check is matched by name, so a rename goes in three steps, each safe alone. First,
`ci.yml` gains the new name and keeps a job under the old one that only re-reports the new name's
verdict. Second, once every open pull request has run that workflow, the ruleset requires the new
name in place of the old. Third, the relay job is deleted. That is how the tree moved from
`clippy` to `lint` in October 2026.
