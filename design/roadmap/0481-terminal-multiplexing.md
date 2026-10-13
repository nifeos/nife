---
status: SUPERSEDED
raised: 2026-09-20
superseded_by: 806
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 481. Real terminal multiplexing

The work is milestone 806 (concurrent login sessions), and §253 (concurrent login sessions) records
calef's rulings on it. It is wider than multiplexing a terminal: a greeter per console, authority
granted at login, memory from a broker and the owner's login all ride with it. The refusal is kept
below because it is the argument 806 answers.

Refused by milestone 49 (design/roadmap/0049-users-and-attribution.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '49. Users, login, and attribution: what identity is for once it stops being authority', under
`## Follow-on`:

> Real terminal multiplexing. `login` hands the terminal to the first successful caller and
> refuses the rest with a dedicated code until `LOGOUT`, deliberately, because the narrow shape
> commits to nothing the wider one would later have to unwind.
>
> -- design/roadmap/0049-users-and-attribution.md

## Why it is here rather than only there

`login` hands the terminal to the first successful caller and refuses the rest with a dedicated code
until `LOGOUT`. That is a narrow shape chosen on purpose: it commits to nothing that a wider one
would later have to unwind, and the refusal it returns is explicit rather than a hang or a race.

## The condition that reopened it

The condition, met on 2026-10-06 (UTC) when calef asked for more than one person on the machine at
once:


- **Condition.** Somebody needing two sessions at once. The refusal's argument is about commitment
  rather than difficulty: the narrow shape can grow into the wide one, and the wide one cannot be
  un-shipped, which is this tree's standing test for how much deliberation a decision deserves.

## Index row

One terminal, one session, and an explicit refusal for the second caller. The refusal is about what
a wider shape would commit to rather than about the work, which makes the condition a user rather
than a design.
