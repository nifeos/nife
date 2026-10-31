# §34's extended-attribute layer

The decision in full behind [§34](../0034-redoxfs-primary.md)'s "the xattr fork is closed, the
layer" (2026-07-31). It binds the RedoxFS server while RedoxFS holds the launch role.

§34's 2026-07-30 amendment left the mechanism open. **Decided: the FS server implements extended
attributes as a layer, not as an extension to RedoxFS's on-disk format.**

The argument that decides it is reversibility, and it outranks the others. `fs_proto` is the
contract; whether an attribute lives in a node's on-disk structure or in a store the server manages
is invisible above that boundary. So choosing the layer does not foreclose the format extension, if
attributes later prove central enough to justify diverging from a pinned upstream, or if upstream
accepts the change, the implementation moves and no client changes. That makes this a low-regret
decision rather than a bet, which is the right shape for a mechanism nobody has exercised yet.

The supporting reasons, in order:

1. It is crash-atomic, and a measurement showed it rather than an assumption. `fs.tx(|tx| …)`
   groups arbitrary mutations into one commit, so a file write and its attribute write land
   together, and a delete removes both or neither. This was the check that had to pass before the layer was viable at all.
2. **Attributes key on `TreePtr<Node>`, not on a path**, because that is what the FS server already
   works in (`handles: Vec<Option<TreePtr<Node>>>`). Rename is therefore free and correct: a
   rename changes a directory entry, not the node, so attributes follow the file without any code
   knowing that a rename happened. AppleDouble sidecars get exactly this wrong, and so would any
   path-keyed store. This correctness property is only available inside the FS server, which is an
   argument for the layer rather than merely a consolation for not forking the format.
3. It decouples the requirement from the filesystem choice. A layer works over any backing
   filesystem, so if RedoxFS is ever replaced the attributes come along.
4. It preserves §34's upstream reason. The pin stays at 0.9.1 with a two-import divergence patch
   rather than a format fork every future bump would pay for.
5. Nothing can bypass it, because all access goes through `fs_proto`. On Linux this style of layer
   is worthless (anything can open the file directly), and here it is authoritative. A capability
   property doing real work.

Three things to get right when building it, recorded now because each is a way to get it subtly
wrong:

- The attribute store must be invisible to enumeration. A directory listing must not show it, or
  it becomes part of the namespace clients can name.
- Deletion must be in the same transaction as the file's, or a deleted file leaks its attributes
  and a later node reusing that pointer inherits them. That is a correctness bug wearing a
  housekeeping costume.
- Recovery sees it. `redoxfs_host extract` and upstream's FUSE mount will show the store as
  ordinary data. That is acceptable and arguably good (the attributes come out with the backup), but
  it must be a decision rather than a surprise, and `notes/host-recovery.md` should say so.

What this refused: extending the on-disk format. Correct, and atomic by construction, and it costs
two of §34's four reasons: RedoxFS would stop being the filesystem Redox actually runs, and every pin
bump would pay for the divergence. Reversibility makes that cost avoidable rather than merely
deferred. calef's objection to "patching it in" was right, and the answer is not to patch it in.
