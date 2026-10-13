---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: what-dot-dot-does-at-a-mount-point
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 683. What `..` does at a mount point

*Title and slug are drafts.*
Raised 2026-09-26 by the lane for milestone 154 (a process that holds two directory capabilities).
The decision record for calef's ruling, "the prompt shows one tree" (pull request #1380, not yet
on `main`), and says outright that what `..` does at a
mount name was not part of the question. The shell has to do something, so it does the recommended
thing below, and this asks calef to confirm or change it. Name provisional: this file's stem is a
lane's coinage.

## The premise, checked

The ruling as the maintainer first relayed it to the lane included "`cd ..` from a mount point goes
to its parent in the one tree, as Unix does." The written record in #1380 leaves it open. The lane
built the relayed version and flags the gap rather than treating either text as settled.

## The options

| | behavior at `/media/usb` | cost |
|---|---|---|
| A | `cd ..` goes to `/media`, in the first tree. **Built.** | none: it is path arithmetic on what `pwd` prints |
| B | `cd ..` refuses at the mounted tree's own root, as §126 (a real, single, moving cwd) refused `..` at either labeled root | about ten lines: refuse an `Up` step whose result leaves the mount |
| C | `cd ..` goes to `/` of the second tree's own parent, whatever that is | not meaningful here: the mounted tree has no parent the shell holds |

## Recommendation: A

It is what the path the user sees says: `pwd` prints `/media/usb`, so `..` names `/media`. Unix and
Plan 9 agree (Plan 9's `..` is lexical, cleaned from the path string, which is exactly this). §126's
reason for refusing was that hitting a root meant hitting the edge of a narrow grant, and silence
could hide a wrong belief about how much the process holds. That does not apply here: the parent of
a mount point is inside the first tree, which the shell holds, so no capability edge is crossed or
hidden. `..` past `/` still refuses with nothing sent.

Would we choose A if both cost the same? Yes; the cost difference is ten lines and is not the reason.

## How reversible

Fully. One interactive shell's `..` arithmetic; no wire format, no program written against it, and
no boot shell holds a second tree yet (design/roadmap/0660-a-second-filesystem-mounts-in-the-boot-shell.md).

## Index row

`..` at a mount point: the shell goes to the mount point's parent, as Unix and Plan 9 do; the written ruling
left it open, so calef confirms or picks the §126-style refusal.
