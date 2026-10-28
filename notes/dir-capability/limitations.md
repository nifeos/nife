# The directory capability: the BUGS list in full

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, after
`notes/soak/limitations.md` and `notes/frames/limitations.md`.*

An appendix to [notes/dir-capability.md](../dir-capability.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). The main page lists each limitation in a line; this is each one in
full. Known limitations, next to the feature rather than only in a tracker.

- ~~`std::fs::rename` is still `Unsupported`.~~ Closed. `rename` is bound in
  `patches/std-nife/overlay/std/src/sys/fs/nife.rs` on `RENAME`, with `REMOVE` asked on the source
  and `CREATE` on the destination; notes/crates-io-on-nife.md records milestone 64 (enough `std` to
  run somebody else's crate) binding it.
  Milestone 55 (Time Machine) depends on it (`fruit:posix_rename` in the reference
  Samba config).
- ~~`UNLINK` does not exist.~~ Built by the commands lane, along with the unlink/revoke split the
  roadmap argues for: see [shell-navigation.md](../shell-navigation.md). `REMOVE` now gates two
  verbs. That lane found two things and did not fix them. There was no `RMDIR`, so `MKDIR` could make
  a directory this contract could not remove. `RMDIR` exists now (`filesystem_contract`'s verb 13;
  corrected 2026-10-11 UTC). And no verb reports what rights a handle carries, so a program handed a
  directory capability must be told out of band or discover by probing.
- A directory cannot be moved between directories (`EINVAL`). The argument is in
  [rename.md](rename.md). It is a real restriction against POSIX, and it is declared rather than
  silently approximated.
- `READDIR`'s cursor is an index, and the directory is re-read per call. A name added or removed
  between two calls of one enumeration can be seen twice or missed. That is readdir's usual caveat.
  Fixing it means a snapshot per client, and this table is not per client.
- A client of a dead caretaker blocks forever, exactly as a client of a dead FS server does (§27
  (the filesystem service)).
  §26 (the fault endpoint) is the mechanism that would turn that into a message a
  supervisor can act on.
  Wiring the FS service into a supervision tree belongs to milestone 23 (a capability-routed
  component OS with live replacement).
- `fs_subtree_caretaker` holds at most 16 handles per client (`EMFILE` past that). It has no heap
  and runs on the single stack page `run` maps. A growable table would need an allocator, and a
  4 KiB local would overflow the stack on the first request. Sixteen is well past what the attacker
  or any `cd`/`ls` sequence needs.
- A single-name grant is still the directory the name is in, which is wider than the name. The
  globbing lane closed that for a *pattern* operand: a nameset caretaker
  (`components/src/fs_nameset_caretaker.rs`) serves only the names that matched (see
  [glob-grant.md](../glob-grant.md)). It is still open for a literal one, because a set of exactly
  one has no wiring behind it today.
- A grant on the root of a shell's namespace narrows with `NARROW` (milestone 328 (a grant on the root of the shell's namespace), §173 (narrowing the root of the shell's namespace)). The root
  has no name to descend into, so the caretaker holds an empty directory name and narrows the
  handle it already has. `rm gate.txt` at the top prompt is granted `REMOVE` alone, as `rm
  rmtree/rm-solo` is. A word-hearing program's grant at the root is still refused
  (`words_grant`).
- A grant whose directory is not there reads as "the progenitor is out of memory" at the prompt.
  The caretaker answers `DESCENT_REFUSED`, and the progenitor does the right thing with it: nothing
  is spawned, and the region goes back. But the only word the progenitor can put on the result
  endpoint is `spawnproto::SPAWN_FAILED`, and the shell renders that word as the job pool's
  sentence (`swish::spawn_failure_sentence`). A program the image does not carry answers with a
  word of its own, `spawnproto::SPAWN_NOT_IN_IMAGE`, and gets its own sentence.
  So `rm nosuchdir/x` says something true about the outcome and false about the cause. Fixing it is
  a second sentinel on that endpoint, which is two programs agreeing on a format for one diagnostic.
  It was left rather than taken. The honest sentence is worth having, and it is not worth widening
  the wire for on its own. Whatever else next needs a reason on that endpoint should carry this too.
- Init builds one caretaker per grant, so a grant more than one level down is not delivered either.
  That shape is a *chain* of caretakers, each an ordinary FS client above and an ordinary FS server
  below. DECISIONS §92 (a caretaker is supervised by the client it serves) names it as the case
  supervision was chosen to make free. Nothing builds the
  chain today; `rm a/b/c.txt` is a refusal at the prompt.
- The rights are not printed by `caps`. §42 (a filesystem declares what it offers) says the rights
  *are* the discovery mechanism for what
  a mount offers, and they are introspectable in principle. `caps rm -r logs` says which of the two
  capabilities is being handed over in a sentence ("...and everything under it: -r grants the
  walk") rather than as a rights mask. So a reader learns the shape of the grant and not its bits.
  (§27's amendment records an older refusal, from before the boot had a filesystem at all.)
- A wrong row in `filesystem_contract::verb` is wrong in three programs at once (milestone 61 (the
  caretakers: one verb table)). The
  mitigation is that it is pure data in a host-testable crate, so the tests and Kani can reach it,
  which a hand-written match in a `no_std` binary could not. The row that decides a security
  property rather than a formatting one is `takes_name()`, because it is what `fs_nameset_caretaker`
  filters on. A name-taking verb whose row said otherwise would walk straight past the filter. It is
  pinned by a host test that spells the expected list out rather than deriving it from the field it
  is checking.
- ~~The caretakers answer `EOPNOTSUPP` to the extended-attribute verbs.~~ Closed by milestone 61;
  see [xattr.md](../xattr.md) for which caretaker refuses a write through a read-only grant and
  which leaves it to the server.
