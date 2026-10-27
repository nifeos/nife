# Who may write the activation set: a proposal, answered

*Status: ANSWERED 2026-09-26 (UTC) by calef, as DECISIONS §221 (the boot prompt is the owner's
console): the boot prompt is the owner's root console, so option A holds and the hole below is
closed by definition. Options B and C are not built. Kept as the record of the question. Written
2026-09-26 by milestone 198 (a package manager) rung 3a's installer lane, which built everything
around this question and stopped at it. Name: provisional.*

## What is being decided

Since 2026-09-26 the progenitor installs, removes and rolls back packages
([packages.md](packages.md), "Installing on the target"), and it is the only process that does so on
purpose. It is not the only one that *can*. The boot prompt's shell can create or edit
`activation/<n>` and `current` directly, add the digest of any bytes it likes, and then run them as
vouched. DECISIONS §219 (how the shell names an installed program to the spawner) ruled that running
unvouched bytes takes a capability (gate D2). A session that can write the table holds that
capability without being given it.

D2 is built (2026-09-26, [packages/running-unvouched.md](packages/running-unvouched.md)), and the
boot prompt is given it by one call in `crates/system_initializer`, marked provisional. That line
is option A below, taken for now so the gate could be proven at the prompt. Option B deletes it.

The question: **what stops a session writing `activation/`**, when the progenitor must still write it?

## Is the premise true? Checked, and it is narrower than it was recorded

- Who holds the root. The kernel grants the progenitor `WRITE | GRANT` on the RedoxFS service
  (`kernel/src/user.rs`, slot 5). The progenitor hands the same endpoint, narrowed to `WRITE`, to the
  boot shell (`crates/system_initializer`, slot 4), to `identity_provisioner`, to `login`, and to
  every caretaker it builds. A session `login` establishes gets a fresh `fs_subtree_caretaker` over
  its own subtree (`components/src/login.rs`), so it cannot name `activation/`. The exposure is
  the boot prompt, the machine owner's console, and nothing a user logs into.
- The server cannot tell its clients apart. `redoxfs_server` serves one endpoint with one handle
  table (`serve`, `receive_capability(FILE)`: "we never learn who they are"). A handle is an integer in that
  shared table, so a holder of the endpoint can also use any handle another holder opened. A rule
  enforced *in the server* therefore needs the server to learn which endpoint a request came on.
- No existing caretaker can narrow the shell's root. Milestone 31 (a capability
  shell) recorded it: "a grant on the root of the shell's namespace cannot be narrowed at all,
  because a caretaker descends into a name and the root has none." `fs_nameset_caretaker` filters
  an allow-list, which would hide every name created at the root after it started.

## What the tree already does in the analogous case

The attribute store. `redoxfs_server`'s `check_component` refuses `.nife-attrs` in every directory,
to every client, and `READDIR` does not list it (milestone 57 (partitioning and formatting a real drive)). That is a name no client may reach
as bytes, enforced where no client can route around it. It works because nothing but the server
writes the store. Here the progenitor, a client, must write the table.

## Prior art, recalled rather than read

Nix's multi-user mode keeps `/nix/store` writable only by one privileged service process; users
ask it over a socket, and root can still bypass it. Fuchsia's `blobfs` is written by the package resolver and
system updater, and a component sees a read-only view. Both put the write in one privileged process
and give sessions a request, which is what this tree now does. Neither makes the most privileged
session unable to write; Nix's root can.

## The options

### A. Record it: the boot prompt is the owner's console, and holds D2 implicitly

Zero cost. It contradicts §219's ruling for exactly one session, and silently: "running unvouched
bytes needs a capability" becomes false at the only prompt most people use. If chosen, D2's `BUGS`
should say so where §219 is read.

### B. A root caretaker in front of the boot shell, with `activation` reserved

The shell's slot 4 becomes a caretaker's endpoint instead of the server's, as a `login` session's
already is. The caretaker binds the root without a descent. It answers `ENOENT` for the reserved
name on every name-taking verb (`OPEN`, `CREATE`, `OPENDIR`, `MKDIR`, `UNLINK`, `RMDIR`, both halves
of `RENAME`, the mtime and xattr verbs) and leaves it out of `READDIR`. That is
`fs_nameset_caretaker`'s filter with the set inverted. The progenitor refuses a directory grant
naming it. It also ends the shared-handle exposure for the shell, since a caretaker keeps its own
handle table. Userspace only. It costs one more IPC round trip per shell file operation, at 350 ns a
round trip (`notes/benchmarks.md`); the file operations themselves were not measured here. It adds
one boot process. And its filter must cover every name-taking verb, the forget-a-verb surface
`fs_nameset_caretaker`'s header warns of.

### C. The server reserves the name, and serves it on a second, privileged endpoint

The `.nife-attrs` rule, extended. One rule in one place, covering every client including a buggy
caretaker. It needs the server to know which endpoint a request arrived on, and it cannot today: one
receive loop, one endpoint, no badges. A second receive loop is a second thread over one `Server`,
and badges are a kernel change. Either is a restructure well beyond this lane.

### D. Chroot the boot shell

Put it under an existing `fs_subtree_caretaker`, with `activation/` outside. That changes what `/`
means at the owner's console, every path a person types, and the names the progenitor resolves
directory grants against. Refused: it moves the whole namespace to hide one directory.

### E. Authenticate the table

Sign it with a key the shell cannot read. The key has to survive a reboot, so it lives on the same
disk the shell can read. Refused: the same problem one level down.

## Recommendation

**B**, if the boot prompt should not hold D2 implicitly; **A**, written down where §219 is read, if
the owner's console is meant to be root. Both are reversible: nothing outside this repository reads
the shell's slot 4 or the caretaker's wire.

Would we still choose B over C at equal cost? Not clearly. C is one rule in one place and B is a
filter per verb. B is recommended because C needs endpoint identity in the server, a second thread
or a kernel change. That part of the recommendation is about effort, and should be weighed as
effort.

## What I need from you

1. Is the boot prompt the owner's root console, allowed to vouch its own bytes? If yes, take A:
   the provisional grant becomes the rule, nothing else built. If no, go to 2, and the grant goes.
2. B or C? B can be built by a lane with no kernel change. C needs a decision on how the server
   learns which endpoint a request came on.

Nothing is blocked by this. The installer, rollback and removal work either way. What waited was the
day an installed program's manifest travels with its package (§197 (a package is one archive file)).
Then a self-vouched entry could claim more than `uptime`'s manifest. That day is milestone 597 (a
program carries its manifest in an ELF note), built 2026-09-26. A vouched program is now endowed
from its own note, so the boot prompt can vouch bytes whose note asks for the network. The fork
above is unchanged; its cost of waiting is not.
