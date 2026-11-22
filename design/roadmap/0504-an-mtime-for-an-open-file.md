---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: an-mtime-for-an-open-file
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 504. An mtime for an open file, not only for a name

Promoted from the proposal `an-mtime-for-an-open-file`, filed 2026-09-19, on calef's instruction of
2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited
except for this paragraph: the argument is its author's and promotion is not the moment to improve
it. Written by the milestone 64 (enough std to run somebody else's crate) lane
(`milestone/64-std-filesystem-shim`), from milestone 64's block.

Every option that closes this puts something on the file contract, which two
programs agree on (`crates/file_system_contract`, the FS server, every caretaker and the `std` PAL),
and so is an architect's. Declining is also an answer, and the PAL already implements it.

In brief. The three mtime verbs milestone 47 (navigation and naming) added for `touch` (`GETMTIME`,
`SETMTIME`, `SETMTIME_AT`, DECISIONS §112 (touch's two behaviors need two rights)) all take a **name
under a directory handle**. `std` asks for a modification time in two shapes: by path
(`std::fs::metadata(p).modified()`, `std::fs::set_times(p, ..)`), which is bound, and through an
open file (`File::metadata().modified()`, `File::set_times`, `File::set_modified`), which is refused
with `Unsupported` because a handle has no name to ask by. This asks whether the contract should
answer the second shape too.

## Why this matters

Less than it sounds, and the honest ranking is the first thing to say. The path shape is the
common one: milestone 64's probe list ranked `Metadata::modified` at 19 with 5 crates behind it and
`File::set_times` at 28 with 2. Every walker (`walkdir`, `ignore`, so `ripgrep`) reaches
metadata through `DirEntry::metadata`, which is by path and works. What still refuses:

- `File::metadata().modified()`, which programs reach for after `File::open` when they want the
  size and the time of the thing they are about to read. A caller can call `std::fs::metadata` on
  the same path instead, and that is what the refusal's message says.
- `File::set_modified` / `File::set_times`, the stable (1.75) API that `tar`-shaped extractors
  and `filetime`'s std path use to restore a timestamp after writing a file's contents. There is no
  path-shaped workaround that is equally correct, because the path may have been renamed since.

The PAL could make both work today without asking anyone, by remembering the name a `File` was
opened by and asking with that. **It refuses to**, and that is the part a reader should weigh: the
answer would describe whatever holds the name *now*. After a rename or an unlink it reports another
file's time or a `NotFound` for a file the caller is holding open, and `set_times` would stamp the
wrong file and report success. Unix's `fstat` and `futimens` act on the inode, which is what a caller
means. That reasoning is recorded at `Mtime` in `patches/std-nife/overlay/std/src/sys/fs/nife.rs`.

## The options

(a) Put the mtime in `FSTAT`'s second reply word. The server's `reply` already sends two words
and fills the second with 0, so this is additive on the wire: an old client ignores it. It closes the
read half only. Cost: one line in the server, one in the PAL, and every caretaker taught to relay the
second word. `components/src/fs_file_caretaker.rs`'s `reply` sends a hard-coded 0 there today, so
a program behind a per-file grant would read an mtime of 0 rather than a refusal, which is the
fabricated-answer shape this proposal exists to avoid. Risk: `FSTAT` is also the PAL's
reachability probe, and a reply shape change there is the one place a mistake would make every
`std` program think it has no filesystem.

(b) Handle-taking twins of the three verbs (`FGETMTIME`, `FSETMTIME`, `FSETMTIME_AT`, names
provisional). Closes both halves, and it is the shape POSIX kept (`stat`/`fstat`, `utimensat`/
`futimens`). **The setter runs straight into §112**, which is the finding that makes this a real
fork rather than a binding: a file handle is hard-attenuated to `READ | WRITE` the moment it is
opened (`Server::open_file_at`). So it can never carry `dir::SET_MODIFICATION_TIME_AT`, and the arbitrary-time
authority has nothing to be checked against. Either file handles grow a `SET_MODIFICATION_TIME_AT` bit (a rights
change on every open, and the caretakers' `POLICY` table learns a new row per verb). Or
`FSETMTIME_AT` is refused on every handle and only `FSETMTIME` (the server's own "now") is offered,
which is not what `File::set_modified(t)` asks for.

(c) Decline, and keep the refusals. What the PAL does today. `File::metadata().modified()` and
`File::set_times` answer `Unsupported` with a message naming the path-shaped call. Nothing is lost
that a caller cannot get another way except restoring a timestamp through a handle.

## The seven questions

1. Considered and lost: re-resolving the opened name inside the PAL (lost on correctness, above);
   a stat-by-name verb carrying size and mtime together (does not help an open file, which is the
   whole gap).
2. What the tree does in the analogous case: extended attributes are handle-taking (`GETXATTR`
   on an open file), so the contract already answers "a fact about the file I hold" by handle; the
   mtime verbs are the exception, deliberately, because `touch` never opens what it acts on.
3. Prior art: POSIX `fstat`/`futimens`; WASI's `fd_filestat_get`/`fd_filestat_set_times`, which
   `cap-std` binds to, are handle-first. Recalled rather than re-read for this proposal.
4. Is the premise true? Checked: `file_system_contract::file_system::GETMTIME`'s doc says name-taking on
   purpose, `redoxfs_server`'s dispatch reads the name from the page, and the per-file caretaker
   refuses all three with `ENOTDIR` (`file_system_contract::grant::POLICY`).
5. Cost, measured: (a) is a line each in the server and the PAL plus a relay in every caretaker. (b) is three
   opcodes, three verb-table rows, three caretaker policy rows, server methods and host tests, plus
   the `SET_MODIFICATION_TIME_AT` question, which is the expensive part and is not code.
6. **Reversibility**: (c) is fully reversible. (a) and (b) are wire changes and are not.
7. Same cost? If (b) cost the same as (c), (b)'s getter would still win and its setter would
   still stall on `SET_MODIFICATION_TIME_AT`. The recommendation below is not about effort.

## Recommendation, and what it is not

None made, because this is a wire format. If a nudge helps: (a) is the cheap, additive read half
and could be taken alone, and the setter should wait for a customer that restores timestamps
(`tar`, a backup restore), because §112's rights question deserves a real use case.

What is blocked until it is answered: nothing on the customer path. Milestone 121 (`ripgrep`)
reaches metadata by path. Milestone 64 is BUILT without it; the refusals are its `BUGS` entry.

## Index row

The three mtime verbs milestone 47 added for `touch` (`GETMTIME`, `SETMTIME`, `SETMTIME_AT`,
DECISIONS §112) all take a name under a directory handle.
