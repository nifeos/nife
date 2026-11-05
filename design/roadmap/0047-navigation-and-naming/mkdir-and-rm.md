# Milestone 47 appendix: `mkdir`, `rm`, `rmdir` and `rm -r`

An appendix to [milestone 47 (navigation and naming)](../0047-navigation-and-naming.md). The page is
complete without it. It holds how `mkdir`, `rm`, `RMDIR` and `rm -r` work and why.

## `mkdir` and `rm`

`mkdir` is the same verb family as descending: it mints a directory node and hands back a
capability to it, exactly as `CREATE` returns a file handle. `mkdir` is descend-with-creation.

`rm` is where Unix conflated two operations. Unix's `rm` unlinks a name. The data survives while
anyone holds a descriptor, and the blocks survive after that. So it cannot promise what people mean
when they delete something sensitive (and `shred` only pretends to on a copy-on-write filesystem
like ours). The two operations are separate:

- Unlink: remove a name from a directory; existing capability holders keep reading. These are
  Unix's semantics, and genuinely useful (atomic replace and the temp-file idiom both depend on it).
- Revoke: the object dies and *every* capability to it goes stale.

`rm` is `UNLINK`. Revocation is not exotic here: §13 (capability revocation and untyped
reclamation) revokes frames, §16 (object revocation) revokes objects, and generational names
(`crates/generational_table`) make a stale capability fail safely rather than point somewhere wrong. It is not
offered for files, because the FS server validates handles against a table that is per session, and
the server does not track all outstanding sessions.

A directory capability carries separable rights to enumerate, open (read versus write), create and
remove, per §47 (a directory capability carries six rights). A program handed a directory to write
logs into cannot thereby delete what is there. `FileSpec` makes the same split for files: the
manifest declares direction, and the human designates the file without typing a mode.

One safety property falls out free. `rm -rf /` is bounded by what your directory capability
reaches, structurally. A shell rooted at a subtree cannot recursively delete the system, because no
capability naming those files exists in it. Not a guard rail, not a confirmation prompt, not a check
that could be wrong: there is nothing to name.

## `rmdir` and `rm -r`: Unix already made the safe choice

§49 (removal is a directory operation) is the decision, and notes/rm.md documents the commands.

`rmdir(2)` removes only an empty directory, and that is the whole safety property. The recursion
in `rm -r` lives in userspace, as a loop of individually safe single-step operations: walk, unlink
files, remove empty directories bottom-up. No single call in the contract can take a subtree away.

So `RMDIR` requires `REMOVE` on the parent and refuses a non-empty directory with `ENOTEMPTY`. It is
not revocation, for the reason in §48 (navigation is the shell rebinding what it holds): the handle
table is per server, so handles cannot be invalidated for clients the server cannot enumerate.

The recursion is bounded by construction, which Unix cannot say. `rm -r` needs `ENUMERATE` to see,
`DESCEND` to recurse and `REMOVE` to delete, *at every level*, so the walk stops exactly where the
capabilities stop. Unix bounds `rm -rf /` with a permission check per file, which is a check that can
be wrong and famously has been.

`rm` is a program, not a builtin, and that is Unix's shape. `cd`, `pwd` and `ls` are builtins
because the shell is rebinding what it already holds; `rm -r` is a destructive loop. A builtin would
run with the shell's entire endowment, while a program takes an explicit attenuated grant. So
`caps rm -r logs/` prints the subtree at risk before anything happens, and a bug in the recursion
can only reach what it was handed. Globbing has the same shape
([touch-and-globbing.md](touch-and-globbing.md)): attenuate, then hand over.

`-f` keeps Unix's semantics: "attempt to remove the files without prompting for confirmation,
regardless of the file's permissions. If the file does not exist, do not display a diagnostic
message or modify the exit status." Its value is idempotency: `rm -f maybe-there` succeeding is what
makes a script re-runnable, and "absence is the desired state" is not a lie about failure, so it
does not offend §42 (a filesystem declares what it offers).

Reporting is Unix's too, per `rm(1)`: silence on success, and `-v` exists because the default
prints nothing ("be verbose when deleting files, showing them as they are removed"). Failure is a
diagnostic plus exit status: "exits 0 if all of the named files or file hierarchies were removed…
If an error occurs, rm exits with a value >0." So a partial `rm -r` says what it could not do, exits
non-zero, and says nothing about what it did.

An interrupted `rm -r` leaves a partial tree. There is no transaction spanning requests. One would
mean the server holding a transaction open across receives, which conflicts with the
serve-loop-runs-one-request-to-completion property §47 relies on for concurrency atomicity. Partial,
with failures reported and a non-zero exit, is the answer, and it is what Unix does.

`rm(1)` says "it is an error to attempt to remove the files `/`, `.` or `..`". That is a literal
special-case guard for `/` in the utility, the kind of check that could be wrong. nife needs no such
case: a shell holding a subtree cannot name the root. And `rm` on a directory stays a refusal
(`EISDIR`) rather than a silent escalation to recursive removal, which is Unix's behavior and right
for the same reason `rmdir` is empty-only.
