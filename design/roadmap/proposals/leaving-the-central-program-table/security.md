# Leaving the central program table: what each owner grant risks

[The proposal](../leaving-the-central-program-table.md) holds the end state. This appendix takes
each capability the ruling of 2026-11-02 (UTC) opens to installed bytes, says from the code what
happens with two holders, and says what a malicious package can do with it. It then recommends the
confirmation, the limits and the way back. Where the call is close it recommends the stricter
option: loosening later stays compatible, and tightening later would break what owners granted.

## Each grant, read from the code

| grant | what it is | two holders today | class |
|---|---|---|---|
| reboot object | `REBOOT` at slot 13; holding it is the whole authority (§251 (restarting the machine is a kernel object)) | no limit: `Wants::of` in `crates/system_initializer/src/spawn_loop.rs` places it for every job that declares it | shareable |
| sync-only capability | a client window bound by `fs::BIND_SYNC` at slot 14 (`bind_sync_only`) | bounded only by the file service's client windows | shareable |
| catalog | a read-only copy of the image's package catalog in the job's own page (`copy_text` in `activation.rs`); knowledge, not authority, since the progenitor checks every install against its own copy | each job gets its own copy | shareable |
| activation directory | a read-only directory window on `activation/` | bounded by windows; nothing written | shareable, discloses what is installed |
| index store | a directory window on `activation/indexes`, read only or read and write (`bound_index_store` in `client_windows.rs`) | no lock: two writers race on the same files | shared and mutable; high |
| installer endpoint | install, remove, rollback, approve and revoke on the progenitor (§270 (a package manager holds an installer endpoint, not the spawn endpoint)) | up to four holders (`INSTALLER_HOLDERS`), served one request at a time; rows carry the manager's name and `installed_programs::can_edit` keeps one manager off another's rows | owner-equivalent (below) |
| console input | the boot terminal badged `STDIN_BADGE` at slot 29, answering only `OPERATION_READ_LINE_BYTES` | exclusive by structure: the shell is parked on the job, and the terminal keeps one `pending_stdin`, so a second reader gets `BAD_REQUEST` (`components/src/line_editor.rs`) | exclusive; reads what the owner types |
| display session | the gpu, `display_terminal` and the keyboard, lent to the session program (`build_graphical_terminal_session` in `children.rs`) | exclusive only by structure: the shell waits on the session, and `&` is refused for it. The progenitor keeps no live-session flag, so a second request would start `gpu_driver` again on the same capabilities | exclusive; the screen and every key |
| root resolver zone | a resolver badge for any name at all | bounded by `GRANTS_MAXIMUM` live badges | shareable; already an owner grant (`grant <digest> zone .`) |

Two facts in that table matter more than the rest.

The installer endpoint is the owner's authority in all but name. On the console's own endpoint an
install carries an `answered console` trailer, and the progenitor believes it: it writes every ask
of the package's notes as granted (`program_asks::asked_grants`, in `activation.rs`). The yes the
owner gave lives in `jig`'s prompt, not in the progenitor. So a holder that lies installs anything
a catalog or index vouches for, with every grant those programs ask. Once `reboot`, the installer
and the display become askable, that includes them.

The index store decides what the installer accepts. Milestone 801 (packages over the internet)
installs a package "on the copy's word when the image's catalog does not vouch" (its ruling I2),
and the copy is the file in `activation/indexes`. Whoever writes that file names the digests the
progenitor will install by name.

## What a malicious repository can still do

The owner added the repository, installed its program and said yes. Each case assumes the program
is hostile.

- With `reboot`: restart the machine at any time it runs, and skip the sync, losing what the
  device had not flushed. RedoxFS keeps a prefix of the workload at every cut (milestone 37 (prove
  RedoxFS's crash consistency)), so the loss is the recent suffix, not the filesystem. A program run
  once does this once. A program that runs at every boot loops the machine: a lockout.
- With `sync`: make the file server flush. Cheap denial of service at worst.
- With the catalog or the activation directory: learn what the image carries and what the owner
  installed. Disclosure, nothing more.
- With the index store, read and write: rewrite the index copy so the next install by name fetches
  its bytes. That is a supply-chain substitution against every later `jig install`.
- With the installer on the console's form: install anything and grant it anything. Remove the
  owner's own rows, which `can_edit` lets any manager undo, along with rows from before the manager
  column. Approve a file that delegates the same to another program. A console takeover.
- With console input: read every line the owner types while it runs in the foreground, a
  password included, and keep asking. It cannot read once it exits or runs in the background
  (`SpawnLoop::console_input` gives a quiet job none).
- With the display session: own the screen and every key until it ends. It can draw a fake shell
  prompt and record what the owner types into it. On the no-keyboard arm it reads the boot
  discipline raw and can switch its mode under the shell, which `components/src/graphical_terminal.rs`
  records as a disruption.

## The three classes

Ruled 2026-11-02 (UTC). Each class says who may hold it and how the owner grants it. The classes
set policy; the runtime guards in the proposal's BUGS stay separate work, each tied to its
migration step.

1. Shareable: `reboot`, `sync`, the catalog, the activation directory and zones. Several holders
   may run at once. Any installed bytes may hold them on an owner's line. The confirmation names each in plain words.
2. Exclusive: console input and the display session. At most one holder at a time: the planner
   refuses a second and names the first holder. Any installed bytes may hold them on an owner's
   line, but only in the foreground of the owner's console. Console input gains the `req.badge == 0` check the installer already has
   (`spawn_loop.rs` places stdin for any std job that declares it, with no badge check). The
   display gains a live-session flag that refuses a second session with `SPAWN_NO_DISPLAY`.
3. Console class: the installer endpoint and the index store's write right. An installed program
   holds them only in §270's delegated form, bounded by an approval: its lines read
   `grant <digest> installer within <approval>` and `grant <digest> indexes rw within <approval>`, and the progenitor serves it the delegated badge
   of §270, never the console's endpoint. It may not send `Approve`, `Revoke` or `Vouch`, and
   the progenitor checks that every install it asks lies within the approval (`approval_gate`).
   A delegated holder may not install a package that asks for an exclusive or console-class
   grant, whatever its approval allows, so it cannot grant the installer onward. Milestone 911 (scripted installs grant only what an approval file allows)'s
   `installer` allow key keeps serving the owner's own `jig install --approved` at the console. Its index store is its own directory, `activation/indexes/<package>`, so it cannot
   poison `jig`'s copy, and the progenitor installs on a copy's word only for the manager whose
   copy it is.

So the image's `jig` stays the one program that answers `answered console`, because the archive
vouches for its bytes and its prompt. A replacement manager does everything `jig` does for packages
the owner approved, and nothing beyond.

Why not refuse the console class to installed bytes outright, as before the ruling? Because the
ruling's goal includes a third-party package manager, and an approval-bounded one serves that goal
with no new authority. Why not give it the console's form? Because then the owner's yes would rest
on bytes nobody reviewed, and the owner could not tell an honest prompt from a forged one. Moving
from the bounded form to the console's form later stays possible. The reverse move would break
every owner's grant.

No recorded service (milestone 910 (a package declares a service)) may hold `reboot`, an
exclusive grant or a console-class grant. A service runs at every boot with nobody watching, which
is exactly the reboot loop and the silent keylogger above. 910 is unbuilt; its block should carry
this as a refusal.

## What the owner must see before granting

`jig install` prints one block per program and asks once (`say_asks`, `owner_says_yes`). The root
zone already prints in capitals. The end state keeps one prompt per package and changes three
things.

- Each ask prints its consequence, not its field name. Shareable asks get one line. Exclusive and
  console-class asks print in capitals with the sentence above: "MAY RESTART THIS MACHINE AT ANY
  TIME", "READS EVERYTHING YOU TYPE WHILE IT RUNS", "TAKES THE SCREEN AND KEYBOARD",
  "MAY INSTALL AND REMOVE PACKAGES WITHIN APPROVAL <name>".
- An exclusive or console-class ask needs the owner to type the program's name, not `y`. A habit of
  typing `y` should not hand over the keyboard.
- The prompt names the repository the bytes came from, and whether it is the image's own source.

Example:

```text
$ jig install acme-power
  acme-reboot asks for:
    MAY RESTART THIS MACHINE AT ANY TIME, WITH OR WITHOUT SAVING YOUR FILES
    flushes the writable filesystem
  from the repository acme (added by you), not from this image's source
  type acme-reboot to grant these and install, or anything else to stop:
```

## The way back

The built-in stays the default and the fallback in three ways, and none of them needs the
replacement's cooperation.

- The image's programs hold their grants through the archive. No install takes them away.
- The word `reboot` always finds the built-in. A replacement answers only to its own name or path
  ([the command-name appendix](command-names.md)), so it never shadows the image's program.
- `jig rollback` and `jig remove` restore an earlier generation or drop the rows, and their grants
  go with them. The image's `jig` runs from the archive, so a bad replacement manager cannot remove
  the tool that removes it.

One way to lose the machine remains: a replacement that runs at boot. The console is a boot program,
not a grant, so no grant replaces it, and the service refusal above keeps `reboot` out of anything
that starts unattended. If the owner can still reach the prompt, rollback reverts any grant.

## Not verified

- What `gpu_driver` and `display_terminal` do when started a second time on the same capabilities;
  the code has no guard, and nothing tests it.
- Whether two writers on `activation/indexes` can leave a torn file; no lock exists, and the
  file server's behavior under two writers was not read.
- Whether an owner can tell a fake prompt drawn on the display from the real one. No secure-attention
  key exists; a key the display session cannot intercept would close it, and nothing proposes one.
