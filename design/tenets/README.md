# Tenets: the appendices to AGENTS.md

`nifeos/nife:AGENTS.md` carries the rules. These files carry the reasons: the measurements,
the anecdotes, the failures that produced a rule and the dates they were learned. Every one of them
is linked from the rule it explains, and a reader who only needs to act never has to open one.

*Name: ratified 2026-09-24 (calef, `design/naming.md`'s "Where a document goes"). A tenet gets
cited on its own, by the code and the notes that live under it, rather than only as `AGENTS.md`'s
footnote, so it belongs in `design/` beside the option space and the decisions. Refused `notes/` (a
note records what exists and what building it taught us; a tenet is an argument for how to work,
which is what `design/` holds). Also refused the reason first offered for `design/`, that thirteen
rows would crowd the glossary index. `script/lint` reads `notes/*.md` without recursing, so a
`notes/tenets/` subdirectory would have cost that index nothing; the argument was never real.
`project-history.md` arrived after that ruling and was named on its own: Name: ratified 2026-09-24
(calef, approving #1189). `measure-first.md` was named the same way: Name: ratified 2026-09-24
(calef, approving #1182).*

| appendix | what it explains |
|---|---|
| [three-principles.md](three-principles.md) | the customer ranking function, the method as a result, and the stranger test |
| [mechanisms-not-memory.md](mechanisms-not-memory.md) | the ladder's rungs, the evening that produced it, and what rung zero cost |
| [elegance-over-convenience.md](elegance-over-convenience.md) | why an argument from implementation cost is the weakest one available here |
| [reversibility.md](reversibility.md) | why each category is irreversible, and the two failures that shaped the rule |
| [measure-first.md](measure-first.md) | why measuring is the action when the problem is not understood, and the case that overruled both sides |
| [documentation-standard.md](documentation-standard.md) | why FreeBSD's Handbook and man pages are the standard, and what each part buys |
| [lane-count.md](lane-count.md) | the measurement that retired queue depth, and the memory and disk ceilings |
| [roles-and-the-queue.md](roles-and-the-queue.md) | the night that named the roles, and why each holds the authority it holds |
| [routing-work-and-decisions.md](routing-work-and-decisions.md) | why a report is not a record, and what the label and the ask are for |
| [shared-state.md](shared-state.md) | the 2026-07-30 collisions, the nife-dev link, and the branch that held a finding |
| [working-with-calef.md](working-with-calef.md) | the seven questions with their worked examples, and the anecdotes behind the conduct rules |
| [codebase-rules.md](codebase-rules.md) | what each of the seven codebase rules buys, with its examples |
| [git-in-a-worktree.md](git-in-a-worktree.md) | why commit-early and curate-later are not opposites, and the worktree hazards behind each rule |
| [project-history.md](project-history.md) | the two renames, the symlink, and the pivot from learning project to demonstrator |
