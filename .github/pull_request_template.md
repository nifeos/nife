**Lane:** <branch or milestone>, written by an agent; the author GitHub shows is the person running it.

<!--
Delete the line above only if a human wrote this pull request. AGENTS.md requires it first thing in
the body of anything an agent writes, because every artifact here carries calef's name whether he
wrote it or not, and a reader cannot otherwise tell an architect's voice from a lane's.
-->

## What changed, and why

<!-- The diff shows what. Say why, and say what you decided against. -->

## Gates

<!--
`script/ci-build` with no arguments runs every check a pull request must pass, cheapest first, so a
formatting slip costs twenty seconds rather than the whole run. Paste the result, or name what you
ran and what you skipped and why. "Not run" is an answer; a missing answer is not.

The checks are NOT listed here on purpose. `script/ci-build --list` prints them, and this template
carried a list that said "five" while the script ran six, because a set written down twice rots in
one of the two copies. That is the defect milestone 286 removed; do not put it back.
-->

- [ ] `script/ci-build`

## Identified work

<!--
AGENTS.md: work this change found but is not doing leaves in one of exactly two shapes, and "worth
doing someday" is neither. Either a proposed milestone (the integrator mints the number at merge),
or a `BUGS` entry written where a reader meets the feature. Say "none" if there is none.
-->

## Needs an architect

<!--
Delete this section unless something here is an architect's call: a design fork, the system call
surface, a new dependency, a name, or anything two programs agree on. If it is, add the
`needs-architect` label and say what the ask is, answerable without reading the diff, including what
happens if the answer is no. -->
