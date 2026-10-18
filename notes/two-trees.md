# A shell holding two trees (milestone 154's live half)

*Provisional name for this note, like everything new in it; naming is calef's.*

Milestone 154 (a process that holds two directory capabilities) proved the mechanism first: one
confined program, two `fs_subtree_caretaker`s, both subtrees reachable and neither able to see the
other's (`system_tests/src/user/multi_dir_namespace_tests.rs`, the `fs_test_client` witness). This note is
the half after that: the real `swish` builtins and the per-command grant planner working across two
trees, presented the way calef ruled.

The pure half is `crates/grant_plan` (`Holdings::anchor`, `locate`, `place`, and a `which` on
`FileGrant` and `DirGrant`). The requests are `components/src/swish/navigation.rs` (`Nav`, `Tree`, the
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
`SecondDir::mount`. Everything that resolves a path does it the same way:

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

How an interactive shell learns it holds a second tree is decided, as the transport and not yet the
policy. Init puts the endpoint at a named slot, the shell probes it at `_start`, and a shared
constant holds the mount path and rights. It lands with the first real second filesystem, and so do
two behaviors calef set: a dead mount answers Gone and disappears, and a shell inside it goes home
with a message. The automatic mount name and who receives a new device are deferred decisions.
All of it is design/roadmap/0660-a-second-filesystem-mounts-in-the-boot-shell.md.

## BUGS

- `rm` under a mount point is refused at delivery. The progenitor builds a directory grant's
  caretaker from the one filesystem it holds, and the spawn words do not say which tree, so sending
  them would hand the program a same-named directory in the first tree. It waits on the proposal.
- `spawnproto::SECOND_DIRECTORY_GRANT_BIT` has no emitter and no decoder in init. A second directory grant for one
  spawned program needs a manifest declaring two directory operands, and none does. §170 (how a
  foreign program is told what to do), ruled 2026-09-26 to grant a named file or directory per the
  manifest, makes the manifest the only source of one.
- A path through a mount point is bounded by the one tree's depth, eight components counting the
  mount point's own, not by the second tree's.
- `grant_plan::plan` and the three functions under it take an `~1 KiB` `Holdings` by value, so a
  debug build keeps a copy in each frame. The globbing witness ran within bytes of its stack and
  now has a seventh page; passing `&Holdings` is the smaller frame that should replace that page.
