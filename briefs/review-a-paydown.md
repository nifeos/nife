**Provisional name.** Review one paydown pull request, a diff that moves, cuts or rewrites comments
and prose without changing what the code does. You are a fresh agent, or the lane itself checking
its own diff before it pushes. Do the work; do not ask questions.

Why a brief. Milestone 929 (comments in the Linux style across first-party Rust) ran many paydown
lanes on 2026-10-23 (UTC). Fifteen of fifteen independent reviews came back MERGE AFTER FIXES. The fixes fell into
the six classes below, every one of them. A reviewer that knows the classes looks for them first.

## Read the diff, not the report

An independent reviewer reads only the diff and the files it touches:

    gh pr diff <N>

Do not read the lane's report or its pull request body before your verdict. The report says what the
lane meant to do, and a reviewer who reads it checks the diff against the intention instead of
against the code. That independence is what found every fix in the run above. A lane checking
itself has no such independence, so its self-check is a first pass and never the review.

## The checklist, in order of harm

1. A `Context:` line states the lock and context the code really holds. Open the function and
   trace each lock it takes or is called under. A `Context:` line naming the wrong lock is the
   worst defect in the class: a reader who trusts it can take the locks in the wrong order and
   deadlock. Check also that the line appears only where code holds locks or runs in interrupt
   context.
2. Every `§N` and milestone gloss names the right target. Open
   `design/decisions/<NNNN>-*.md` or `design/roadmap/<NNNN>-*.md` and compare the gloss with its H1.
   The ratchet checks that a gloss is grounded in some target's words. It cannot see that the
   number cites the wrong section, or the wrong side of a fork.
3. A fact is moved, not deleted. For every removed comment line that states a fact (an ordering,
   a bound, a hardware quirk, a reason), find the fact again in the item's doc comment or the note
   it links. A fact with no new home is a finding, whatever the cap says.
4. A long block is shortened, not split. A blank line or a stray `//` that cuts one block in two
   to get each half under the cap is a defect: the reader sees the same essay, and the ratchet is
   fooled. The fix is to move the design into the note.
5. Every doc comment sits on the item it describes. A paste that lands one line high or low
   documents the neighbor instead. Read each moved `///` block with the line after it.
6. A `BUGS` section stays beside the feature. `AGENTS.md` wants an honest `BUGS` section in the
   code, so every item that had a `# BUGS` section keeps one there, one short line per original
   entry, with any detail in the note. These are findings: an entry that lives only in a note, a
   section that lost entries, and entries merged into one line.

Beyond the six, check the house rules any paydown can break:

- §267 (a comment states the constraint as it is now): no history in a comment.
- A `// SAFETY:` comment is kept whole.
- A `Name:` block is left as `design/naming.md` shapes it.
- The diff changes no token outside comments.

## The verdict

End with one line, then the findings:

    VERDICT: MERGE | MERGE AFTER FIXES | HOLD

- MERGE: no finding.
- MERGE AFTER FIXES: each finding names a file, a line and the fix, and the lane can make it without
  a decision.
- HOLD: a finding needs an architect (a design fork, a name, a wire format), or the diff changes
  code. Say which.

A finding the lane will not fix in this pull request gets a home before the merge: a proposal under
`design/roadmap/proposals/` or a `BUGS` entry, as the developer-lane skill requires.

## BUGS

- The six classes come from fifteen reviews of one milestone. A paydown of another kind (a rename
  sweep, a note split) will have classes of its own, and this list will not find them.
- Item 1 rests on the reviewer tracing locks by reading. Nothing mechanical checks a `Context:`
  line against the code, so a wrong one that a reviewer misses ships.
