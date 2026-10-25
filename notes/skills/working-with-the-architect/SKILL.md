---
name: working-with-the-architect
description: >-
  How an architect and the agents working for one share a session in nife: load at the start of any
  top-level session, before putting a question or a ruling to an architect, before writing a report
  to one, and before deciding whether a change is small enough to make yourself (it never is: every
  edit, push or CI fix goes to a lane). Holds how to bring a decision (plain background first, one
  decision at a time, recommendation first), how to report (short, outcome first, faithful about
  failures), that the main session stays free for the conversation, how to refer to calef
  (they/them), that a record states a decision and its date without quoting the decider, which
  machine names may appear in public text, and what needs an architect versus
  what agents merge on standing authority. Points to the decisions and maintainer skills and §88
  rather than restating them.
---

# Working with the architect

*Name: provisional, minted 2026-10-25 (UTC) by lane working-with-the-architect. Considered
`working-with-calef`, refused because the role can have more than one holder (ARCHITECTS.md). No
architect has ruled on it.*

The main session is an architect's conversation. These rules keep it that way, and keep an
architect's attention on the questions only an architect can answer. Each rule with a fuller home
elsewhere points there.

## The main session stays free; lanes do the work

Working in lanes is a standing practice (2026-10-25 UTC). A top-level session briefs, gates,
records and reports. Every edit, push or CI fix goes to a lane, however small: "too small for
a lane" is the failure, not an exception. [The maintainer skill](../maintainer/SKILL.md) holds the
rule and the roles behind it.

Start lanes when an architect asks for work, not on your own initiative. Lanes cost tokens, and the
person paying decides how many run.

## Bring one decision at a time, background first

Explain the thing in plain terms before the options, ask one question at a time, and put the
recommendation first. The [decisions skill](../decisions/SKILL.md) holds the format, beside the
seven questions a fork must answer before it arrives and the `## What I need from you` comment that
puts every question in the architect's queue.

## Report short, outcome first, and faithfully

Lead with what happened: what landed, what is held, what failed. Then the asks, verbatim, so an
architect can answer without opening a diff. Leave out the narration of how you got there.

Report a failure as a failure. A gate you did not run is not a pass; a run you did not read is not
finished. AGENTS.md's "Correct yourself loudly" applies to reports too, and the
[developer lane](../developer-lane/SKILL.md) holds a lane's report and handoff.

## What needs an architect, and what agents merge

Agent review is enough to approve and merge a pull request through the merge queue (2026-10-24
UTC). Do not ask an architect to approve each one.

Escalate what the standing rules say to escalate: an edit to a `design/decisions/` section, the
syscall surface, a new dependency, a name, a departure from a brief. [§88
(`needs-architect` as a required check)](../../../design/decisions/0088-needs-architect-as-a-check.md)
is the gate, and the [decisions skill](../decisions/SKILL.md) holds the label, the hold and the ask.

## Words

- Refer to calef as they/them (2026-10-25 UTC).
- Spell out contractions in prose written for nife: "do not", "it is", "cannot" (2026-10-25 UTC).
  This covers notes, roadmap blocks, decisions, pull request bodies and commit messages.
- A record states the decision and its date. It does not quote the decider or name them as its
  authority (2026-10-25 UTC). A decision stands on its reasons, and a quotation reads as if the
  person deciding were the reason. Where context helps, paraphrase it, with the date.

## Machine names in public text

The lab machines argon, xenon and radon are public (2026-10-24 UTC). Those three may appear
anywhere in nife: bench directories, milestone names, doc
comments. Do not scrub or flag them.

Every other machine name stays out of new public text: this repository, its pull requests, issues
and commit messages. Write instructions for any reader ("a macOS host", "your machine"), never for
one person's desk. A historical record (a bench log, a quoted ruling, a dated "run on" line) keeps
the name it already carries; do not scrub history. Addresses, serial numbers, outlets, home
directory paths and photos stay out too. In public text, say only that unit-specific details are
kept off-tree by the maintainers.
