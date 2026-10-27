# Shared-page audit: findings 1 and 2, the filesystem's shared frame

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/shared-page-audit.md](../shared-page-audit.md), the tree's second security
audit (2026-08-04). The parent states the lens and its four questions: (a) the window, (b) what can
land in it, (c) the corrupted state, (d) whether it is reachable. Its honest summary has every
finding's disposition.

## 1. The FS service's one shared frame is mapped read-write into every client the boot ever wired

(a) The window. `kernel/src/user/fs_service.rs`'s `ensure()` allocates one frame,
`FILE_SHARED`, on the first call and hands that same physical address to every later caller:
`spawn_fs_client`, `start_granted`, `start_granted_dir`, `start_granted_set`, `narrow_dir`,
`start_file_sink`, `start_sink_verify` and `start_std`. Each maps it `Flags::user_data()`
(read-write) at `FILE_VA_CLIENT` (`0x60_0000`), or at `FS_PAGE_STD` for a std program. The FS server
maps the same frame at `FILE_PAGE`.

So the number of processes holding a writable mapping of the file page grows with every FS client a
boot wires, and never shrinks: a caretaker `serve`s forever and never exits.

(b) What can land in it. The property that makes one frame sound is stated in
`fs_file_caretaker.rs`'s module comment:

> One frame, three parties, and that is sound because every request on both sides is a blocking
> `CALL`: the client is parked inside its call for the whole time the caretaker is using the page.

That argument is correct for a chain (one client, one caretaker, one server) and says nothing
about a fan-out. A second FS client is not inside anybody's call; it is runnable, and it holds
the same page read-write.

This is not a hypothetical the audit invented. `fs_service.rs`'s own `wait_for_caretaker` exists
because exactly this went wrong once, at startup, and its comment is the precedent:

> a confined program that already exists writes its own first name over that page, and the FS
> server resolves whatever it finds there.

The fix taken then was ordering (drain the handshakes before the client exists), which closes
the startup case and not the steady-state one.

(c) The corrupted state. A client `A` sends `OPEN` with the name staged at offset 0 and blocks.
Client `B`, runnable, writes a different name over offset 0. The FS server reads the page after the
message arrives and opens `B`'s name, returning the handle to `A`. `A` now holds a handle to a file
it never named and, if `A` is behind a caretaker, one outside the namespace that caretaker exists to
enforce. The same substitution works on `CREATE`, `UNLINK`, `RMDIR`, `MKDIR`, `OPENDIR` and both
halves of `RENAME`, and on the *data* of a `WRITE`.

(d) Reachable? Not on 2026-08-04; reachable since 2026-09-27. In the interactive
boot three processes now map the file page, and the audit's first named event has happened.
`crates/system_initializer` grants the shell `(SH_FS_VA, g.fs_page, MAP_RW)`. It keeps its own copy
for the life of the boot (milestone 31 (A capability shell) phase 3, 2026-08-17). And it maps the
page into both the `fs_subtree_caretaker` and the program behind a directory grant.

What still closed the hole then was that those three are never runnable at once on the same page.
The shell is parked in `receive` on the spawned program's stream for the whole time that program
exists. The program is inside a blocking `CALL` whenever the caretaker is forwarding. And the
caretaker touches the page exactly once at startup and then only relays handles. Init itself never
writes it at all, worth stating because it holds the capability: it maps the frame into children
and does not speak `filesystem_contract`.
In the kernel test suite several caretaker chains do coexist on the one frame, but each is blocked on
`receive_cap` between tests, and the confined clients `exit()` after reporting.

The remaining opening is a runnable third party, and it opened on 2026-09-27.
The open question was whether, once init could build a caretaker per grant, "a runnable shell
holding the page coexists with a caretaker chain using it". Init can, since 2026-08-17, and the coexistence became runnable with milestone 205 (how a foreign program is told what to do). The prompt builds an
`fs_nameset_caretaker` on this one frame. The shell writes through it while draining a redirected
job (`std_grep needle docs > out.txt`). A write between the caretaker's
re-staging and the server's read opens a name the set never approved. The case is
`notes/a-set-grant-at-the-prompt.md`; milestone 599 (a frame per
filesystem client channel)'s production pool closes it. The second event is unchanged. A confined
program granted an untyped budget could retype a second `ThreadControlBlock` into its own address space. It could
then scribble the page from a helper thread while its main thread is parked in `CALL`. Today's
confined programs are granted two rendezvouses and no budget, so they cannot.

Disposition: proposed as a milestone. See "What wants a lane" in the parent. The fix is a frame per
client channel rather than a frame per service, which is a wiring change across `fs_service.rs` and
the four programs that name `FILE_VA_CLIENT`, plus a witness test with two live clients. It is too
large for an audit lane and too specific to leave as prose.

## 2. `fs_nameset_caretaker` checks a name and forwards it without re-staging it

(a) The window. `components/src/fs_nameset_caretaker.rs`'s serve loop:

```rust
if filtered && v.takes_name() && v.operand == verb::Operand::Name {
    let mut buf = [0u8; grant::MAX_NAME];
    let n = name_at(0, len, &mut buf);          // read the name OUT of the shared page
    if !nameset::contains(set, &buf[..n]) {     // check it against the granted set
        reply(reply_slot, reply_err(ENOENT));
        continue;
    }
}
...
let r = forward(fs::req(code, server_handle, n), second);  // the FS server reads the page AGAIN
```

The caretaker copies the name into a stack buffer, decides on the copy, and then forwards a request
carrying only the *length*. The FS server does its own read of the same page
(`redoxfs_server.rs:254`, `unsafe { file_page(len) }`) and resolves whatever is there then. Two reads,
one check, and the checked bytes are never written back.

`RENAME` has the same shape twice, and the destination half is the load-bearing one: the caretaker's
own comment says renaming a matched name onto an unmatched one "would destroy a name this capability
was never granted, which is an escape even though nothing was opened."

(b) What can land in it. Only another writer of the frame, which is finding 1. The confined
client is parked in its `CALL` and the caretaker is parked in `forward`.

(c) The corrupted state. The FS server acts on a name the set filter never saw, and the handle
comes back to the caretaker, which installs it in the client's table. The namespace the capability
designates is no longer the set.

(d) Reachable? No, for finding 1's reason and no other. The two are one bug seen from two sides,
and they are recorded separately because they have different fixes and the second is cheap.

Disposition: fixed in this lane. The caretaker now writes the checked bytes back into the page
before forwarding, so the bytes the FS server reads are the bytes the filter approved. On the honest
path this is a byte-identical rewrite and the existing glob-grant and directory-capability tests
prove it did not change behavior.

And the honest limit, stated where the fix is: re-staging narrows the window from "the whole
check-to-forward span" to "the caretaker's store until the FS server's load". It does not close it,
because a third writer of the frame can still land in the smaller window. Only finding 1's fix
closes it. This is a hardening with a named residue, not a repair.

`fs_file_caretaker` does not need this and cannot have the bug: it answers `OPEN` locally
(comparing the asked name against the granted one and returning `grant::HANDLE`) and never forwards
a name at all. `fs_subtree_caretaker` cannot have it either, for the reason in the parent's section on the structural fact that saves most of the tree.
