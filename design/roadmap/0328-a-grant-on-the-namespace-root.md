---
status: BUILT
raised: 2026-09-03
built: 2026-10-27
milestone_dependencies: none
decision_dependencies: 173
machine_requirements: none
specific_machine: none
needs_person: no
---
# 328. A grant on the root of the shell's namespace

Filed 2026-09-03 as an unnumbered proposal by the milestone 247 sweep, from milestone 31's block;
numbered 2026-09-19 by milestone 433.

**Built 2026-10-27 (UTC).** §173 (narrowing the root of the shell's namespace) ruled option A, and
this milestone builds it. `filesystem_contract::fs::NARROW`, verb 23, takes a held directory handle
and a rights mask, mints a second handle through `Rights::attenuate`, and resolves no name. The
server's `narrow` calls `Rights::attenuate` and refuses a short intersection with `EPERM`, as
`OPENDIR` does; nothing restates the arithmetic. A caretaker or the progenitor's bound channel that
holds an empty directory name sends `NARROW` on the root where it would have sent `OPENDIR`, and the
shell's `dir_grant` hands the root over as that empty name. A name filter refuses `NARROW` at the
directory it filters (`Verb::reissues_handle`), since the new handle would carry no filter. So `rm
gate.txt` at the top prompt is granted `REMOVE` alone, as `rm rmtree/rm-solo` is. The shell's root
does not move.

Gates: `filesystem_contract` and `redoxfs_server` host tests (the narrowed handle can remove and
cannot look, open or widen), and `script/swish-check` on every architecture it covers, where
`caps rm gate.txt`, `rm -v gate.txt` and `wc gate.txt` replace the old refusal.

The name `NARROW` is provisional (an architect's to ratify).

**In brief.** A subtree caretaker attenuates by performing one `OPENDIR` *into* the directory it was
granted. The root of the shell's namespace has no name to descend into, so a caretaker holding an
empty name sends `NARROW` on the handle it already holds. `rm gate.txt` and `rm rmtree/rm-solo` are
both granted `REMOVE` alone, whatever the depth of the file.

## Why this matters

Milestone 31's claim is that typing a name is the grant. The top of a namespace is where anyone
starts, so a grant that worked one level down and failed at the top would have been the first hole
a newcomer met. `NARROW` closes it with the proved `Rights::attenuate`, a new row in
`filesystem_contract::verb::TABLE`, and no change to what any relative path means.

`notes/dir-capability.md` carries the sibling limitation: a grant more than one level down is a
chain of caretakers and is still a refusal.

## Index row

A grant on the root of the shell's namespace is narrowed by `fs::NARROW`, verb 23 on
`filesystem_contract`: a held directory handle and a rights mask in, a second handle out through
`Rights::attenuate`, no name resolved. `rm gate.txt` at the top prompt is granted `REMOVE` alone, as
`rm rmtree/rm-solo` is, and `script/swish-check` gates both on every architecture it covers.
