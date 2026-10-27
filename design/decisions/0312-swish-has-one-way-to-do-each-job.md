---
status: DECIDED
raised: 2026-10-27
decided: 2026-10-27
ratified_by: calef
---

# 312. swish has one way to do each job in its language

*(Section number provisional until the merge queue lands it.)*

Ruled 2026-10-27 (UTC), while ruling §160 (what a subshell copies). That ruling needed to know
whether `( ... )` keeps its Unix spelling, and the answer turned on a rule that milestone 47
(navigation and naming) states for the shell's commands and that nobody had stated for its
language. This section states it. It amends the language half of the rule in milestone 47's "Why it
matters"; the commands half stands unchanged.

## What is being decided

Milestone 47 set one bar for the whole shell: gratuitous divergence from Unix taxes every user
forever, so keep what Unix does unless the capability model forces a change. For commands (`cd`,
`pwd`, `ls`, `mkdir`, `rm`) that bar is right and stays. For the language around the commands (how a line groups, scopes, substitutes, tests, quotes and
sequences) it backfires. "Survive unchanged" invites every Unix form, and Unix has several forms
for each job. The question is what bar governs the language.

## The rule

- Commands. Keep the Unix command names and behavior wherever they can work honestly. Unchanged
  from milestone 47.
- Operators everyone types stay. `|`, the redirections, `&&`, `||` and `$( )` keep Unix
  spelling. Each is also the one way swish has to do its job.
- The language has one way per job. Grouping, scoping, substitution, testing and the other jobs
  a line can do each have one form, as fish does. A form that would be a second way to do a job
  already covered is not added.
- A new form carries its own case. A pull request that adds a language form shows, in its body,
  that the form is not a second way to do something already covered, naming the job it does and
  the form that did the job before, if any. A form that fails the showing is not added.
- Unix scripts are not an aim. Running Unix scripts unchanged is not something swish is for.
  It belongs to a separate POSIX layer (§10 (process model: capability-based, microkernel), §31
  (the foreign-language seam)), which can speak Unix's forms without swish carrying them.

## What this rules on the spot

- `( ... )` does not keep its Unix meaning: the job it did is two jobs (§160 (what a subshell
  copies)), and each now has its own form. It becomes a refusal that points at both.
- A form that is already in the language and is a second way to do a job is a finding, not a
  deletion. Removing one takes its own ruling.

## Prior art

fish, which this rule follows. fish later added `&&`, `||` and `$( )` under user demand, which is
why those stay here from the start. §160 weighed Nushell, Elvish and rc, which also give a scoped block one
spelling.

## How reversible it is

A rule about what to add is cheap to revise. What it protects is not: a language form lands in
every script anyone writes (the same cost §160 names for a spelling), so the change that adds a form owes the showing, not a later cleanup.
