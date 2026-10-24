# Bound grants in the progenitor: what the last piece of ruling D needs

*(Milestone 606 (a directory walk costs what it does on Linux), ruling D's last piece. Written
2026-09-27 by lane `milestone/606-progenitor`. The page's stem and every new name here are
provisional; calef names things.)*

calef's ruling D (2026-09-27 07:17Z, on #1387) ends with the progenitor choosing, per mount,
between a bound badge and a caretaker, from what the filesystem's package declares. The server
side is built (#1408): `subtree_scope`, `BIND` and `UNBIND`, and RedoxFS as the first eligible
server. This page is why the progenitor side is not built yet. It needs two answers that are
calef's, and one of them was already open before this milestone.

Status: ruled and built. calef ruled T1 at 2026-09-27 15:22Z and option 4 of milestone 599's pool
at 15:29Z, both on #1413. The pool landed as #1418; this page's last section says what was built
on top of it. The body above that is the proposal as it was written.

## Is the premise true?

Partly. Three things the brief assumed turned out otherwise when the tree was read (2026-09-27):

- There is no mount table. A boot has one file server, started by the kernel before the
  progenitor runs (`kernel/src/user.rs`, `fs_service::root_directory`), and the progenitor holds
  its one endpoint as `BootEndowment::fs_ep`. "Per mount" today means "for the one filesystem".
  The choice is still worth making per server, because milestone 140 (mount a drive this system did
  not create) will add a second server kind, and FAT will not be eligible.
- Nothing on the device reads a package declaration. `packages/*.package.toml` is read by
  `helpers/packages.py` on the host and by nothing else. What reaches the device is the package
  catalog (stem and digest lines) and each program's ELF manifest note (milestone 597 (a program carries its
  manifest in an ELF note)). So a
  declaration field needs a way onto the device, and that way is a format.
- The progenitor has no client windows. A bound badge is a window index (§230 (badged endpoint
  capabilities)), and a bound client reads and writes its own window. The production progenitor
  still gives every client window 0. Giving it a pool is milestone 599 (a frame per filesystem
  client channel)'s outstanding piece, and that piece is blocked on a fork of its own. The
  progenitor's 24-slot capability table cannot hold seven more frame capabilities, and the six ways
  around that are priced in `notes/a-frame-per-filesystem-client-channel.md`. Its short answer
  needs one new operation, and that is calef's call. A bound grant in the progenitor cannot be built before it.

## What is decided here, and what is not

Two questions are calef's, because each is a format two programs agree on.

### Question 1: how "this server is eligible" reaches the progenitor

| | where the fact lives on the device | what changes |
|---|---|---|
| T1. A second note in the server's ELF | `.note.nife.manifest`, a new note type beside milestone 597's manifest, descriptor one flags word | `manifest_note` gains a type; the progenitor reads the file server's image from the initrd and decodes it |
| T2. A column in the package catalog | `package_catalog`, a third field on the server's line | `package_archive`'s catalog format; `xtask` writes it from the package declaration |
| T3. A bit from the kernel | the progenitor's START words, set by the kernel that started the server | the kernel reads the note (T1) and passes the answer on; a new meaning for an argument word |

Recommendation: T1. The fact is about a binary, and it should travel with the binary, measured
with it, so it cannot describe a different build from the one that runs. Milestone 597 chose the
same answer for a program's grants. The package declaration then carries the same field for the
host side, and `script/lint` checks the two agree. It would also check that every crate declaring
it depends on `subtree_scope`. T2 puts a runtime fact in a file whose job is integrity, and T3
spends an argument word and a kernel change on something the progenitor can read for itself.

The field in the package declaration, in #1405's TOML form, provisionally:

```toml
subtree_grants = "subtree_scope"   # absent means "use a caretaker"
```

It is a string rather than a flag so that a future second scope crate would be a new value rather
than a second field. If #1405 has not landed when this is built, the field goes into both forms.

### Question 2: milestone 599's pool

Already open, and not this milestone's to answer. What this milestone adds to it is one more
reason the answer matters: a bound grant needs a client window of its own, where a caretaker
grant can go on sharing window 0.

## What is recommended, because it is reversible

These are code in the progenitor, and would be built as soon as both answers are in:

- Caretaker unless proven otherwise. The progenitor binds a badge only when the note says so
  and a window is free. Any failure (no note, a note it cannot decode, no free window, a `BIND`
  refused) falls back to the caretaker it builds today, and says so on the console once.
- A set of names is bound and still gets a caretaker. A badge is bound to one directory, all of
  it, and a set (`spawnproto::NAMESET_BIT`) is what narrows that directory to the names a line
  designated, so `fs_nameset_caretaker` serves the set over the bound channel, told with an empty
  name that the bound root is its directory (`client_windows::at_bound_root`).
- Bind before the child holds anything. The progenitor opens the directory on its own unbadged
  channel, sends `BIND`, and only then mints the badged endpoint into the child's table. A badge
  is open (the whole filesystem) until bound, so the order is the safety property.
- `UNBIND` at reap, and the window back after it. Revoke on the job's reap, then return the
  window. A window is never handed out again while its last holder can still run, which is the
  `BUGS` entry in `subtree_scope` that makes a revoked badge safe to reuse.
- Gated by `script/swish-check`, which boots the real progenitor, as milestone 599's own block
  says for its pool.

## The seven questions

1. Other options. T2 and T3 above, and each loses for the reason given. Keeping the caretaker
   everywhere loses the measured 0.16 ms per walk that ruling D was for.
2. This tree's analogous case. Milestone 597: a program's grant manifest travels as an ELF note
   and the progenitor decodes it. T1 is the same mechanism for a server.
3. Prior art, from memory and not read for this page. An ELF note describing a binary's
   properties is the GNU property note's job (`.note.gnu.property`, CET and BTI marks).
4. The premise. Checked above; three parts were not true as stated.
5. Cost. The walk through a bound grant: 0.347 to 0.355 ms, through a caretaker 0.508 to
   0.521 ms, Linux 0.41 to 0.42 ms (release kernel, HVF, #1408). A note is a few dozen bytes in
   one binary. The pool's costs are milestone 599's note's.
6. Reversibility. The note type and the catalog column are formats. Once an image carries the
   note, a progenitor that reads it is acting on it. The progenitor's behavior is code.
7. At equal cost. T1 still wins, for keeping the fact with the binary. The recommendation is not
   about effort.

## What was built (2026-09-27)

- `manifest_note::SUBTREE_GRANTS` (note type 2, provisional): a version word and a scope word, one
  scope (`subtree_scope`). `carry_subtree_grants!` places it; `redoxfs_server` carries it.
- `packages/redoxfs.package.toml` declares `subtree_grants = "subtree_scope"` (provisional field),
  and `helpers/packages.py` checks three things agree: a crate carrying the note depends on
  `subtree_scope`, its package declares the field, and a package declaring the field has a crate
  that carries the note. The selftest plants a violation of each.
- The progenitor reads the note from `redoxfs_server`'s measured bytes at boot and says which way
  it went. For a directory grant it binds the next window's badge to the named directory on its own
  channel, before the job holds anything, and hands the job the badged endpoint and a page of that
  window. It falls back to a caretaker on any failure. `script/swish-check` requires the
  bound-badge sentence, so a boot that lost the note fails the gate. It passes on all three
  architectures, and the `rm` lines run through the bound grant.
- `UNBIND` runs at the job's reap, when `job_undertaker`'s "reaped" message reaches the progenitor
  (milestone 685 (a job is finished when its memory is back)), and when a taken window's job never started. `Windows::take` skips a window
  still bound, so the unbind cannot drift back to reuse: the host test
  `take_never_hands_out_a_bound_window` fails if it does.


## The window pool: `grant_plan::job_windows`

The pool's bookkeeping is pure logic in `crates/grant_plan/src/job_windows.rs`, so its rule is
tested on the host. The progenitor holds the capabilities and does the binding.

A window shared by two live jobs is the shared-channel race the pool exists to close. It is worse
once the window is bound, because the older job's badge then resolves to the newer job's directory
(confinement claim 24 in [confinement-claims.md](confinement-claims.md)). So `Windows::take` hands
a window to a job only when the job that last held it has been reaped. The progenitor learns of a
reap from `job_undertaker`'s "reaped" message. That is option A of milestone 685 (a job is finished
when its memory is back), which calef ruled on 2026-10-06 (UTC).

A bound window is unbound at its holder's reap, never at its reuse (milestone 606 (a directory walk
costs what it does on Linux), ruling D). `Windows::take` skips a window whose bound bit is set, so
the progenitor's `UNBIND`, at `Windows::reaped` or `Windows::release`, must run before the window
can go to anyone else.

`ReapsDue` is the other use of the "reaped" message. It records the finished jobs whose whole answer
went to the shell's result endpoint and whose reap has not yet come. The shell reads such an answer
to the end before it sends another request. So when a later request finds a pool short, that job
has finished or is finishing, and waiting for its reap waits for an event that will come. A job whose
output went elsewhere, such as a pipeline stage with a sink, may still be waiting on a stage the
request in hand has not built. It is not recorded, because waiting for it could wait forever.

### BUGS

- **A window whose `UNBIND` fails is never handed out again.** Its bound bit stays set, and
  `Windows::take` skips it for the life of the boot. The progenitor is the only caller allowed
  `UNBIND` and names a window it bound, so the file server has no reason to refuse it. Leaking is
  the safe direction: the pool refuses spawns, and it never shares a window.
- **A window is only as free as the reaps that reach the progenitor.** A job that `job_undertaker`
  never reaps keeps its window for the life of the boot. A screen-narrowed job (DECISIONS §106 (the
  `terminal_sink_caretaker` narrowing)) is reaped by the shell, not the undertaker, so one behind a
  directory grant would leak its window. None can be today: a tail is screen-narrowed only when its
  manifest declares `writes_while_reading`, and no shipped manifest does.
