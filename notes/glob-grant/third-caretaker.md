# `fs_nameset_caretaker`, and why it is a third caretaker

*Name: provisional, minted 2026-10-11 (UTC) by lane/fourth-ten-longest-notes.*

An appendix to [`notes/glob-grant.md`](../glob-grant.md), which states the conclusion.

There were three candidate shapes, and this was a real fork rather than a formality.

- A generalization of `fs_file_caretaker`. The roadmap's phrasing invites it, and it does not
  work. `fs_file_caretaker` serves the *file* protocol: its client `OPEN`s a fixed handle, `CREATE`
  is `ENOTDIR`, and every directory verb falls through to `EBADF`. Teaching it a set means teaching
  it `READDIR`, `UNLINK` and `RMDIR`, which is not generalizing a file caretaker, it is writing a
  subtree caretaker and calling it a generalization.
- A mode on `fs_subtree_caretaker`. Tempting, because `fs_subtree_caretaker` already does the
  handle-namespace translation. Refused, and its own design property is the reason:
  `fs_subtree_caretaker` performs no rights checks at all, so there is no branch in it that can
  be wrong. A name filter is a check, and one that must be consulted on every name-taking verb
  (`OPEN`, `CREATE`, `OPENDIR`, `MKDIR`, `UNLINK`, `RMDIR`, and both halves of `RENAME`). A mode
  would trade that program's one strong property for a switch, and put a forget-a-verb surface in
  the caretaker that most deliberately has none.
- A third caretaker. Taken. It also has a structural reason and not only a stylistic one: the
  two grants have different shapes. One name rides in two `START` argument words; a set does not
  fit in any number of registers, so this program is started with a frame as well. Bolting that onto
  `fs_subtree_caretaker` would make every subtree grant carry machinery it does not use.

The honest cost is about thirty lines of handle table duplicated from `fs_subtree_caretaker`. That
is the price of keeping "this caretaker checks nothing" true of the one that says so.
