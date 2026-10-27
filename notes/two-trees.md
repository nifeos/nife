# A shell holding two trees (milestone 154's live half)

*Provisional name for this note, like everything new in it; naming is calef's.*

Milestone 154 (a process that holds two directory capabilities) proved the mechanism first: one
confined program, two `fs_subtree_caretaker`s, both subtrees reachable and neither able to see the
other's (`system_tests/src/user/multi_dir_namespace_tests.rs`, the `fs_test_client` witness). This note is
the half after that: the real `swish` builtins and the per-command grant planner working across two
trees, presented the way calef ruled.

The pure half is `crates/grant_plan` (`Holdings::anchor`, `locate`, `place`, and a `which` on
`FileGrant` and `DirectoryGrant`). The requests are `components/src/swish/navigation.rs` (`Nav`, `Tree`, the
`two_trees` witness). The guest proof is
`a_shell_holding_two_trees_moves_between_them_and_crosses_neither`.

## One tree, with the second mounted in it

calef, 2026-09-26: *"One tree with other trees mounted at names in it."* The user sees one root. A
second tree appears at a mount point, a `/media/<label>` convention or wherever the owner binds it,
like a Unix mount or a Plan 9 `bind`. Which capability serves a name is bookkeeping the shell keeps
to itself, and `pwd` prints the mount path. `cd ..` from a mount point goes to its parent, which
the ruling left open (below).

That ruling replaced an earlier cut of this work that gave each tree a label and printed `/b/logs`,
following §126 (a real, single, moving cwd). §126's single moving position survives; its labels do
not. The ruling's decision record is pull request #1380, and it leaves what `..` does at a mount
point open: the shell goes to the parent, and design/roadmap/0683-what-dot-dot-does-at-a-mount-point.md
asks calef to confirm.

So the shell's position is one path, `Holdings::cwd`, and a second tree is only a mount point,
`MountedTree::mount`. Everything that resolves a path does it the same way:

1. `Holdings::anchor` says where the steps start: where the shell stands for a relative path, a
   bound name's position when the first component is bound, the root otherwise.
2. `Cwd::apply` applies the rest. This is where `..` is answered, so `..` from `/media/usb` is
   `/media`, and `..` past `/` refuses with nothing sent, as it always did.
3. `Holdings::locate` says which tree serves the result: the second, from its own root, for a path
   through the mount point; the first, unchanged, for anything else.

`plan_path`, `walk_steps` and the planner's `designate` all do exactly this. Before, there were
three resolvers, and `designate` walked an absolute token literally, so `wc /recent/x` ignored a
bind that `ls /recent` honored. One resolver closes that too.

A listing of the mount point's parent names the mount point, whether or not the first tree has a
directory of that name. A mount hides a real directory at the same path, as a Unix mount does.

## A handle means nothing without its endpoint

Two caretakers number their handles independently, so handle 3 from one tree and handle 3 from the
other are different directories. A request that sends one on the other's endpoint names the wrong
directory rather than failing. So a `Walk` carries the `Tree` (slot and rights) it walked in, and
every request on its handles goes to `name_call_in(w.tree, ...)`.

A walk always opens from its tree's own root, one `OPENDIR` per level. That costs a few more
requests on a relative `cd` than popping a stack did, and it is what makes a path through a mount
point open on the right endpoint without a second code path.

The system's own files are pinned to the first tree wherever the shell stands. `apropos` opens the
manual store there, and `caps <image>` reads the activation table there.

## EXAMPLES

In the two-tree witness, the first tree is the fixture's `sub` and the second its `other`, mounted
at `/second`. Each line goes through the prompt's own `builtin`:

```
pwd                       /
ls /                      inner, ..., second         (no secret)
cd /second                pwd is /second
ls                        secret                     (no inner)
cd ..                     pwd is /
cd second                 pwd is /second
cd ../..                  refused: at your root, and pwd is still /second
wc < /second/secret       planned in the second tree, opened there, reads the secret's body
rm /second/secret         planned, then refused at delivery (see BUGS)
bind /second bee          ls /bee lists the second tree
bind / second             refused: that name leads to where a tree is mounted
cd                        pwd is /
```

## What is left, and when

The boot shell holds one second tree: the disk tree, the NVMe disk as one read-only file, at
`/disk` (milestone 834 (ioping on nife and Linux)). The progenitor places its endpoint at `spawnproto::MOUNTED_TREE_SLOT`, the
shell probes that slot at `_start`, and `spawnproto` holds the mount name and rights. A directory
grant under `/disk` is named through the mount (`disk/<dir>`), and the progenitor builds it against
the disk tree. What is left for a filesystem that can come and go: a dead mount answers Gone and
disappears, and a shell inside it goes home with a message (calef, 2026-09-26). The automatic mount
name and who receives a new device are deferred decisions. All of it is
design/roadmap/0660-a-second-filesystem-mounts-in-the-boot-shell.md.

## The disk tree's server

`kernel/src/user/disk_tree_service.rs` starts the server behind `/disk` (calef, 2026-10-18 21:36
UTC, "Yes, read-only at prompt"). It serves the whole NVMe namespace as one read-only file with no
cache, so a program at the prompt measures the device. It is the NVMe server's second client beside
the filesystem server, kept apart by its own transfer window (calef, 2026-10-18 23:06 UTC,
"Per-client window").

- Slot 0, the NVMe request endpoint (WRITE), badged with `DISK_TREE_WINDOW`.
- Slot 1, its own request endpoint (READ), and slot 2, a report endpoint (WRITE).
- Mapped read-only at the block channel's address: that window's pages. The controller fills them
  by DMA, and the server only copies out of them.
- Mapped read/write directly above: `fs::CLIENT_WINDOWS` client windows.

Window 0 is the file service's own window 0 frame, not the pool's first window. The boot shell holds
both trees and maps one page for both, which is sound because it has one `CALL` in flight. The pool's
first window is never mapped. The progenitor is granted the request endpoint and the pool, as it is
the filesystem server's, and slices one window per client.

## The globbing witness's stack, measured

The globbing witness in `system_tests/src/user/glob_grant_tests.rs` runs the shell with seven stack
pages, and each step to that number is a measurement. Two pages overflowed by 256 bytes. That showed
up as a data abort on the shell's own `sp`, then as the 60 s lost-wakeup watchdog, because the test
was still waiting for a report from a process that had died. Four pages fit the one-tree shell.

The cost is a name set traveling by value through frames a debug build does not collapse: the
expander holds one, `Expansion` carries one into `plan`, `designate` returns one, and the returned
`Endowment` carries one more. The same measurement set `nameset::MAX_NAMES`: sixteen names did not
fit in four pages, and the answer was smaller frames rather than a bigger stack.

Seven came with this milestone, and it is a number rather than a smaller frame. Six pages ran within
a few dozen bytes of the guard, and a `Holdings` that learned where a second tree is mounted crossed
it, with a data abort on `sp` inside `nav::path`, under `designate`. The smaller frame is known, and
the `Holdings` entry under BUGS below records it.

## BUGS

- `rm` under a mount point is refused at delivery. A program that hears words is granted its
  directory under `/disk` by name, but `rm`'s grant is not routed that way yet, and the disk tree
  would refuse every removal anyway (`EROFS`).
- `spawnproto::SECOND_DIRECTORY_GRANT_BIT` has no emitter and no decoder in init. A second directory grant for one
  spawned program needs a manifest declaring two directory operands, and none does. §170 (how a
  foreign program is told what to do), ruled 2026-09-26 to grant a named file or directory per the
  manifest, makes the manifest the only source of one.
- A path through a mount point is bounded by the one tree's depth, eight components counting the
  mount point's own, not by the second tree's.
- `grant_plan::plan` and the three functions under it take an `~1 KiB` `Holdings` by value, so a
  debug build keeps a copy in each frame. The globbing witness ran within bytes of its stack and
  now has a seventh page; passing `&Holdings` is the smaller frame that should replace that page.
