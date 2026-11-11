# The owner's console: vouching for a build, and who else may run new code

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*Built 2026-09-26 (UTC) by the lane `milestone/198-owner-console`, on DECISIONS §221 (the boot
prompt is the owner's console). Names provisional. An appendix to [packages.md](../packages.md).*

The boot prompt, the shell on the console before any login, is the machine owner's. Whoever holds
it is the owner, as with single-user mode elsewhere. Two things follow, and both are here: the
owner can vouch for bytes no package carries, and the owner decides which users' sessions may run
bytes nobody vouched for.

## Vouching for a build

A fresh build is unvouched. At the boot prompt it runs on §219 (how the shell names an installed
program to the spawner)'s gate D2, with what its line
delegates plus the read-only clock and configuration pages ([running-unvouched.md](running-unvouched.md)).
`vouch <path>` records its digest in a new activation generation, so the same bytes then run
vouched:

```
$ vouch installed/unvouched
  vouched; generation 3 is live
$ caps installed/unvouched
  installed/unvouched would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    provenance: vouched by the owner in activation generation 3 (digest sha256:e6fd81d1...)
$ jig rollback
  rolled back; generation 2 is live
$ caps installed/unvouched
  ...
    provenance: unvouched (digest sha256:e6fd81d1...)
```

The shell sends the file as frames, the way it sends an image to run, and then the name to record
it under: the path's last component, at most sixteen bytes (`grant_plan::vouched_name`). The
progenitor hashes its own copy, checks that it parses as an executable, and writes the next
generation with an entry `<name> owner <digest>` (`installed_programs::OWNER` in the package column).
It commits exactly as an install does, so the only change that makes the vouch live is the rename
of `current`. The bytes are not copied anywhere: a digest vouches for itself wherever the file is.
A later vouch or install of the same name replaces the entry, which is what an edit loop wants.

A vouch is a generation, as the proposal it came from recommended
(`vouch-for-a-local-build`, promoted into milestone 198 (a package manager)'s block), so `jig rollback` undoes it
and nothing else has to know about a second table.

**Who may vouch** is whoever holds the spawn endpoint, the one door to the progenitor's activation
verbs. That is the boot prompt and nothing else. A session `login` builds holds no spawn endpoint,
and its directory is confined to its own subtree, so it can neither ask nor write `activation/` by
hand.

### What proves it

`script/swish-check` runs the transcript above on every architecture, with the D2 run of the same
bytes before it and after it. The census the program prints is what moves: `slots held: 0 1 2`
unvouched, `slots held: 0` vouched (the installed manifest holds only the output), and `0 1 2` again
after the rollback. Falsified once on aarch64: recording the digest with one bit flipped left both
the `caps` line and the census red, because the vouch no longer named those bytes.

## Who else may run new native code

A session `login` builds gets D2's capability only if its identity is on the owner's list:
`may-run-unvouched` at the root of the file service (`login_contract::RUN_UNVOUCHED_LIST`), one
identity per line, `#` for a comment. There is no list on a fresh machine, and no list lists
nobody. The owner writes it at the boot prompt:

```
$ echo alice >> may-run-unvouched
```

`login` reads it on every login, after the secret is checked and the session is built, so an edit
holds from the next login on. A session cannot reach the file: it is confined to its own subtree.

The proof rides logins `system_tests/src/user/login_tests.rs` already makes. A listed `alice` gets the
capability, and it reaches the endpoint `login` was given and cannot be passed on. With no list,
`alice` gets none. With a list naming only `bob`, he gets one and `alice` does not. Falsified
once on aarch64: with the list check answering yes for everyone, the no-list login failed.

No login of its own, on purpose: a first version added two, and the aarch64 suite's `std_net`
test then failed to load a program for want of frames (riscv64 hung the same way). Each client run
keeps a little memory for the rest of the suite, which has no margin left.

## BUGS

- A vouched build holds the manifest its own ELF note carries, or `uptime`'s if it carries none
  (milestone 597 (a program carries its manifest in an ELF note)). So vouching can widen a build
  to whatever its note asks, the network included, and the owner's vouch is the only check.
- Every activation verb, `vouch` included, is open to whoever holds the spawn endpoint. The boot
  prompt is the only holder, so that is the owner. A session given one would be the owner too;
  before any is, the verbs need a presentation of their own (`grant_plan::spawnproto`'s BUGS).
- A vouch of a name an installed package already uses replaces that package's entry, as an
  install of a newer version would. The package's bytes stay under `packages/`, and a rollback
  brings its entry back.
- Rollback is by number. After a vouch and its rollback, the next edit is numbered past the
  vouch's generation, and rolling that edit back lands on the vouch. `script/swish-check`'s second
  boot shows it: its removal is generation 4 and its rollback lands on 3.
- The list names identities by their string. Renaming an identity, which nothing does yet, would
  need the list edited too.
- Taking a name off the list holds from that identity's next login. A session already built keeps
  its copy: nothing here revokes a delegated capability.
- `login` reads the list through the file page every session shares. That is sound while the
  terminal rule keeps one session live at a time, the assumption every client of that page makes.
- No session `login` builds holds a spawn endpoint, so a listed session's capability is delivered
  and proven, not used.
