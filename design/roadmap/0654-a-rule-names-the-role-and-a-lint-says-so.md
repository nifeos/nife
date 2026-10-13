---
status: NOT-STARTED
raised: 2026-09-25
promoted_from: a-rule-names-the-role-and-a-lint-says-so
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 654. A present-tense rule names the architect role, and a lint says so

*Title and slug are drafts.*
calef, 2026-09-25 UTC: *"I want to strike all assumptions from the tree that I am the only
architect so that when we have another we need not scour."* A lint that stops new ones is wanted
only if it can be made precise, and it cannot yet. **Name provisional**: this file's stem is a lane's coinage.

A lint in `script/lint` is reversible in AGENTS.md's sense: nothing outside this
repository reads it.

## What the lint would refuse

A sentence, outside `design/decisions/` and the dated records, that names a listed architect
(read from `ARCHITECTS.md`, never spelled) as the one who decides, in the present tense: `<name>'s
call`, `names are <name>'s`, `is <name>'s to ratify`, `until <name> rules`, `does not need <name>`.
The census lane's rewrite script already encodes those shapes and the guards that keep records out.
A match inside a quotation or backticks is skipped. So is a match with a date within about twenty
characters, or with a past-tense verb (`was <name>'s call`).

## Why it was not built in that lane: measured false positives

After the sweep, the same shapes with the guards on and no other exclusion still match **60 lines**,
and all 60 are correct as written. They are not architect-role sentences. They are ownership:

- calef's machines and bench (`xenon`'s firmware settings, "the machines are calef's", a disk
  wipe on a board on his desk);
- the organization and its credentials (the App, `TOOLCHAIN_BUMP_PAT`, secret names);
- repository settings (making a check required is one checkbox an organization owner clicks);
- his network, his router, and attributions such as "the governing constraint is calef's".

A lint that fires 60 times on a clean tree is a lint people learn to suppress. The rewrite script
excluded these with a keyword list (`machine`, `firmware`, `secret`, `owner`, `App is`, ...). That was
fine for one reviewed pass and would be wrong as a gate, because the next ownership sentence that
uses a new word fails somebody's build.

## Options

1. A ratchet. Count the matches and fail when the count rises, the way the prose ratchet does.
   Cheap and honest about the 60, but it cannot tell an ownership sentence from a new rule, so a
   legitimate new sentence about calef's bench raises the count and fails.
2. A marker at the ownership sentence. Every one of the 60 gains a short tag saying it is about
   ownership rather than the role, and the lint fails on any untagged match. Precise, and rung three
   at the thing itself, at the cost of 60 edits and a new convention (whose spelling is an
   architect's call).
3. Separate the two facts first. Ownership that is not the role (who owns the machines, who
   administers the organization) gets its own list beside `ARCHITECTS.md`, and the lint reads both.
   A match naming someone on the ownership list inside an ownership sentence still needs option 2's
   judgment, so this does not remove the marker; it only names the second role.

Recommendation: option 2, because it is the only one that fails on exactly the defect and
nothing else. Option 1 is less work and would still be chosen by nobody if the two cost the same.

## Index row

Sentences in the tree still assume calef is the only architect. Proposed: a lint that refuses a present-tense rule naming a listed architect as the one who decides, once it can be made precise.
