---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The clean-range table does not locate a held seed

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), reading
`helpers/continuous-fuzz.sh` and `helpers/fuzz-daily-publish.sh` against step 5 of the ruling
recorded in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md).

Reuse: the existing `awk` filter in `helpers/fuzz-daily-publish.sh` grows one condition; nothing
else fits.

## The defect

After a held window with a replay line, the supervisor starts the slot's next window one seed past
the failing one. The daily table leaves out the held window's own row, but it publishes the next
window's first seed. That seed, less one, is the held seed. So once the daily publish runs,
`clean-ranges.tsv` names a reproducer before its fix is on main, which step 5 forbids. Nothing has
leaked, because nobody has switched publishing on.

## What it builds

The table leaves out every range of a slot from that slot's first held window onward, reading
`windows.tsv` alone. It still covers every other slot, and the count of seeds run stays the same.

The ranges should return once the held finding turns public. Nothing builds that yet, because no
milestone defines the signal that a finding has turned public. The milestone
[a held finding turns public after its fix reruns clean](a-held-finding-turns-public-after-its-fix-reruns-clean.md)
publishes the advisory and is where that signal belongs. Until then a slot that ever held a finding
publishes no more ranges, which fails safe.

## How it is checked

`helpers/fuzz-daily-publish-selftest.sh` plants a `windows.tsv` with a held window followed by clean
ones, runs the publish script with `--dry-run`, and fails if any published first seed minus one, or
any range end, falls on the held seed. It also fails if a slot with no held window loses a range.

## Status

The withholding is built in `helpers/fuzz-daily-publish.sh`, and its `BUGS` note records that the
ranges do not return. The return is not built; it waits on the signal above.
