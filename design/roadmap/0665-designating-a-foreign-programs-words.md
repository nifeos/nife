---
status: BUILT
raised: 2026-09-26
built: 2026-09-28
promoted_from: designating-a-foreign-programs-words
---
# 665. Designating a foreign program's words

Status BUILT 2026-09-28: built by milestone 205 on PR #1402. *(Title and slug are drafts.)*

Raised by milestone 205 (how a foreign program is told what to do), which built §170 (how a foreign program is told what to do)'s clause 1: a `std` program at the
prompt hears its line as bytes. This is clauses 2 to 5, which turn the words that name something
into a grant. The slug and every name below are a lane's coinage.

## Ruled and built

calef ruled on 2026-09-27 at 06:27Z (UTC): "N1". A line that names no file grants nothing, and to
search here a person types `rg pattern .`. He ruled the mark at 06:35Z: there is none
([`0675-the-mark-on-a-foreign-programs-word.md`](0675-the-mark-on-a-foreign-programs-word.md)). Milestone 205
built the mechanism below the same day (#1402); its block records what was built and what differs.
This file stays until an integrator retires it.

## The one question that was calef's

**What does a line with no word naming a file grant?** `rg pattern` names nothing. With nothing
granted, `ripgrep` searches `.`, finds no directory, and says so.

| | a line naming nothing grants | cost |
|---|---|---|
| **N1** | nothing. To search here, say so: `rg pattern .` | Clause 5 read literally. One more word for the commonest `rg` line, and a `std` program that walks `.` by default fails loudly until the person learns it. |
| **N2** | the shell's current directory, read-only | What a person expects. It is a grant no word on the line designated, which is the first time the prompt would move authority nobody typed. `caps` would print it, as it prints the clock. |

**Recommendation: N1**, because designation is authorization everywhere else in this shell (`rm`,
`wc`, globbing) and N2 would be the one exception. N2 is reversible later; N1 is not a trap. calef
chose N1.

## The mechanism, which is reversible and which the lane would build

What the tree already does: `rm *.txt` is a set of names in one directory, served by
`fs_nameset_caretaker`. "A name that is not in the set does not exist here", with the filter at the
granted directory only, so a matched directory can be walked if the grant carries `DESCEND`
(`notes/glob-grant.md`). That is clause 2's shape for words: the directory the shell stands in, and
the names the line's words resolve to in it.

1. The planner stats each word that is not an option-shaped token (`-x`, `--y`) against the shell's
   current directory. A word that resolves is a designation. One that does not resolve is inert
   bytes: a pattern, a count, or a name the program will create (clause 3's case).
2. The designated names become one `DirGrant` at the current directory, delivered by a nameset
   caretaker at slot 4, with rights from the manifest: `READ | ENUMERATE | DESCEND` for read-only.
   The program's root is the shell's directory, so `rg foo src notes.txt` opens `src/...` and
   `notes.txt` by the paths it was given, and nothing else under that directory exists.
3. A manifest field (`words`, provisional) says read-only, read-write, or the file's directory.
   Boot-image programs declare it in `grant_plan`. An installed program's comes from its ELF note,
   which is blocked on that note's names (`a-program-carries-its-manifest-in-an-elf-note.md`).
4. An unvouched program is read-only whatever it declares (clause 4). A mark on a word widens that
   one word; its spelling is a naming decision, proposed in
   [`0675-the-mark-on-a-foreign-programs-word.md`](0675-the-mark-on-a-foreign-programs-word.md).

**Prior art.** The four systems `notes/foreign-program-arguments.md` read from source (Fuchsia,
Genode, seL4's `sel4utils`, Xous) all keep authority out of argv and hand it over separately, as a
namespace. None of the four derives a grant from the words themselves; that step is this tree's,
and it is what milestone 47 (navigation and naming) already does for `rm`.

What else was considered, and why it lost:

- A subtree caretaker at the current directory. It is cheaper, because the progenitor already
  builds one. But it grants every sibling of every named file, which is what `notes/glob-grant.md`
  calls the catastrophic over-grant.
- One caretaker per word. `std` has one directory slot. Several need milestone 154 (a process
  that holds two directory capabilities) in `std`'s PAL, and a program's paths would stop being
  relative to one root.
- A deeper word (`src/main.rs`). It grants its first component, `src`, as a walkable directory,
  so the nameset caretaker's filter still holds at the top. That grants more than the one file.
  Precision would need a filter at every level, which no caretaker has. The recommendation is to
  build the top-level form, record the over-grant in `BUGS`, and refuse nothing.

What it costs, measured from the tree:

- The caretaker exists: `components/src/fs_nameset_caretaker.rs`, 352 lines. The progenitor does
  not build it yet; only the kernel harness does (`fs_service::start_granted_set`).
- The shell refuses a set today ("a set of names is delivered by a nameset caretaker, and the
  progenitor builds the subtree one", `components/src/swish.rs`, `dir_grant`).
- It also refuses a grant at the namespace root, or more than one level down. So the progenitor's
  nameset path and those two limits are the build. They are the same work milestone 47's `rm *.txt`
  at the prompt is waiting on.

**Reversibility.** All of it is code. The manifest field's name and the mark's spelling are the only
parts two programs agree on, and both ship provisional.

## What is blocked until calef answers

Only which of N1 and N2 a word-less line gets. The mechanism does not depend on it.

## Follow-on

- **Milestone 672.** Milestone 672 (the argument page's layout). The argument page's layout, built provisionally by milestone 205 (how a foreign program is told what to do) and still awaiting an architect: `design/roadmap/0672-the-argument-page-layout.md`.
- **Milestone 675.** Milestone 675 (the mark on a foreign program's word). The mark on a foreign program's word, which calef refused on 2026-09-27: `design/roadmap/0675-the-mark-on-a-foreign-programs-word.md`.

## Index row

A foreign program at the prompt hears its line as bytes, and the words that name something must become a grant. Ruled by calef on 2026-09-27 (a line that names no file grants nothing) and built by milestone 205 (how a foreign program is told what to do) in #1402.
