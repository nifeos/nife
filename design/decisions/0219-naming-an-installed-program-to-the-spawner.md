---
status: DECIDED
raised: 2026-09-24
decided: 2026-09-26
ratified_by: calef
---

# 219. How the shell names an installed program to the spawner

calef's question, 2026-09-26 (UTC): *"What approach allows a developer to build on nife and then
run what they build? What about running a script they write?"* A, B and C answer only "run
something installed". Option D answers both.

## The ruling

calef, 2026-09-26 (UTC, about 01:45 to 02:00Z): *"D with D2 seems right."* And on what an unvouched
child may hold: *"Yes, allow the clock and config pages."*

- Option D. The shell sends the executable's bytes as frames it owns. The progenitor hashes its
  own copy. A digest in the activation set is vouched and runs with that entry's manifest. A miss
  gets only what the caller delegated.
- Gate D2. Running unvouched bytes requires a capability the owner gives a session (Fuchsia's
  `vmex` shape), visible in `caps`.
- An unvouched child may be given the read-only clock page and the configuration page. Never the
  process domain, the network, entropy, the file service or anything else from the progenitor's
  own table.

The consequence, in calef's words: *"if we established multiple users and not every user got a
shell that could launch unvouched programs, then those users would not be able to launch unvouched
programs. We could constrain which users could do such things on nife."* The capability is handed
out per session, by whatever builds sessions (`login`, milestone 233 (`login` dies on every boot)).

Milestone 104 (the measurement continues past init)'s rule is restated by this ruling. "The
progenitor runs nothing it cannot vouch for" becomes "the progenitor grants nothing of its own to
what it cannot vouch for", and running what it cannot vouch for takes D2's capability.

### Recorded limitations

These are the maintainer's, raised with the ruling. They are not part of it.

1. D2 constrains new native code, not new behavior. A vouched interpreter runs a script as input,
   so a session that holds an interpreter can do anything the interpreter can, with no D2
   capability. A user who must run no new code must also not be given an interpreter.
2. The capability is transferable unless it is granted without the right to pass it on. In this
   kernel that is a capability without `GRANT` (`abi::rights::GRANT`): `SEND_CAP` and `CAP_INSERT`
   both refuse to move one, as §12 (call/reply IPC)'s reply capability relies on. The maintainer recommends granting
   it without `GRANT` by default, pending an architect; that is reversible. The tree shows one
   consequence: a capability without `GRANT` cannot ride on a spawn request either. So under that
   default, D2's capability is a progenitor endpoint the session invokes, held `WRITE` only, not a
   token it sends along. The implementing lane proposes which shape is built.

## What is being decided

§208 (installing a package is granting it, and the activation set is versioned) ruled that
installing makes a package's digest and manifest *spawnable*. It also named the cost: "naming a
program that was not there at boot is a change two programs agree on." This section is that change.
One question: when a person runs a program the boot image did not carry, what travels from the shell
to the process that builds it? The program may be installed, or it may be one they just built.

Today a closed enum answers it. The shell resolves a name through `grant_plan::Prog` and sends its
integer id as word 0 of a `spawnproto` request (`crates/grant_plan/src/spawnproto.rs`). The
progenitor indexes `progs[p.id()]`, a table it filled once at boot from the measured archive
(`crates/system_initializer/src/lib.rs`, `spawn_service`). An installed program has no variant, no
id, and no row. Neither does a binary a developer just linked.

## Is the premise true? Three corrections to the record first

- The progenitor keeps the file service for the life of the boot. §208 and milestone 507
  (installing a package: mutate, compose, or widen what can be spawned) both say "the spawner gives
  the file service away", and `system_initializer`'s module documentation says so too. The code
  stopped doing that in milestone 31 (a capability shell) phase 3 (2026-08-17). It kept
  `WRITE | GRANT` on the RedoxFS service so the progenitor could build `fs_subtree_caretaker`s
  (`Channels.fs`, and the comment "The filesystem stays"). So the spawner *can* read an
  installed program today. What it cannot do is be asked for one.
- `PROG_COUNT` is 14, not 13. Both earlier records quote 13.
- The slot figure this section first quoted, fifteen of sixteen, is stale. The table is 24 slots
  and the measured peak is 23 of 24 since milestone 590 (the booted system starts its network
  stack), in #1290, `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`.

## What the tree has to carry bytes, measured

Can a file capability or a memory object carry an executable? The tree answers both halves.

- There is no file capability to pass. A RedoxFS file handle is an integer local to one
  session with the file server (`filesystem_protocol::fs::OPEN` returns it). The only file-shaped
  capability is a `fs_subtree_caretaker` endpoint, and only the progenitor can build one. The
  shell's file-service endpoint carries no `GRANT`, which is exactly why `DIR_BIT` carries data
  rather than a capability.
- There is no multi-page memory object either. The kernel's object types are the rendezvous,
  the address space, the thread and the one-page `PageFrame` (`crates/abi`, `objtype` and
  `page_frame`). Nothing like Fuchsia's VMO exists. An executable of N pages is N frame
  capabilities, and a `SEND_CAP` carries one.
- `build_child` needs a parsed `elf::Elf` over bytes in the builder's own address space
  (`crates/supervision_protocol`). It copies every segment page into a fresh frame from the build
  untyped (`fill_and_map`), mapped through a scratch VA that is never reused. So the builder must be
  able to read the bytes. It never maps the source pages into the child.

So "pass the file" reduces to one of two shapes. The shell can name a file and let the progenitor
read it with its own authority, which is A with a path. Or the shell can read the bytes into frames
from its own budget and delegate the frames. D is the second.

## The options

| | What travels | Who builds the process | Wire change | Measured cost |
|---|---|---|---|---|
| **A. A name on the spawn request** | A new `spawnproto` flag (`NAME_BIT`, provisional) saying "two more `SEND`s carry a name of at most 32 bytes" | The progenitor. It looks the name up in the activation table on RedoxFS and reads the package with the file service it already holds. It re-checks the member digest and builds with `build_child` as for any program | Yes: the shell and the progenitor | No new permanent slot (the file service is already held). Reading `uptime` costs 22 pages (89,168 bytes stripped), two 64 KiB file reads. The progenitor pays, against `INIT_OWN_PAGES` = 128 and a job region of 40 |
| **B. A launcher in the sealed namespace** | Nothing new: `package` (provisional) is one more `Prog` row, declared the way milestone 150 (adding a program should not need eight hand-maintained lists) declares every row | The launcher, from bytes it read, with a `--mem` grant, as `login` builds sessions (milestone 233 (`login` dies on every boot)) | No | No argument vector exists (milestone 205 (how a foreign program is told what to do)), so the shell cannot say which package. The child gets at most what the launcher holds, so one manifest caps every installed program. Deaths go to the launcher, not `job_undertaker`, so `^C` and pipelines need re-plumbing |
| **C. Ids assigned at install** | The same integer word, now `>= PROG_COUNT` for an installed program | The progenitor, indexing the activation table | Shape unchanged, meaning changed | The shell must read the activation table to resolve a name. An id means a different program after a rollback, so a spawn in flight across one runs whatever the new set put there. It reuses the wire ids milestone 150 pinned |
| **D. The executable's bytes, as frames the caller owns** | A new flag (`IMAGE_BIT`, provisional) on word 2; word 0 carries the byte length instead of a program id. One `SEND_CAP` per page follows, then the grants as today | The progenitor. It maps each frame through `build_child`'s never-reused scratch VA and deletes the capability before taking the next. It hashes its own copy and looks the digest up. A hit is vouched, with that entry's manifest. A miss is unvouched, with only what the caller delegated | Yes: one bit and a new meaning for word 0 | One transient slot per frame, none permanent. The caller pays the staging pages (22 for `uptime`). One message per page, so 22 more `SEND_CAP`s for `uptime` than A's two `SEND`s. The child is built from the caller's job untyped, as an interruptible job already is |

**A becomes a lookup in front of D.** Under D the shell resolves a name to a package, reads the
member into frames, and sends the frames. No name crosses the wire and no id does either. The
progenitor recognizes an installed program by its digest in the activation set, so C's rollback race
cannot happen: a digest cannot mean a different program.

## How D serves the developer, and the script

A developer's build. The developer links a binary into their own directory, then runs it by
path. The shell reads it through the file service it already holds, into frames split from its own
budget, and sends them with whatever the line grants: output, input, a directory, `--mem`. The
digest is in no table, so the child runs with exactly those grants and nothing the progenitor holds.
The same request is what a build tool needs to run a test binary it just linked. So D is the shape
milestone 172 (a capability-native subprocess) needs before `cargo` can hold a spawn endpoint.

A script. An interpreter is a program like any other, vouched or not, and the script is its
input. The tree already has the grant for that: `FileSpec::Required { writable: false }`, one
read-only file named on the line (`grant_plan`). So `interp build.nsh` works once an interpreter
exists, and the script's authority is the interpreter's, narrowed by the line. What it cannot yet do
is hear its own arguments: `build.nsh release` needs a string, and the ABI carries one `u64` and a
flag mask. §170 (how a foreign program is told what to do) ruled a byte argv on 2026-09-26, and
milestone 205 builds it.

`#!` belongs in the shell. Resolving `#!/pkg/interp` means reading text and looking up a name.
In the spawner that puts a parser and a name table inside the process that decides what runs, and it
turns D back into A. In the shell it is a read of the first line, then an ordinary run of the
interpreter with the script as its file grant. Plan 9 and Linux do it in `exec` because `exec` takes
a path; D takes bytes.

## Authority: what an unvouched child may hold

The progenitor endows five things from a program's own manifest rather than from the line: the
clock page, the process domain, the configuration page, entropy and the network (`wants_clock`
through `wants_network` in `spawn_service`). Under D those come only from a manifest the progenitor
found by digest. An unvouched binary's own declaration is self-asserted and confers nothing.

The ruling allows the clock and configuration pages, nothing else. A developer who needs the
network vouches for their build first (§195 (a reviewed recipe vouches for a package), clause 3).

`grant_plan` gains a manifest for "no manifest": output, input, one optional directory and `--mem`,
and nothing else. The shell binds a D line against it, so `caps ./a.out` previews exactly what will
be delegated. It adds one row, `provenance: unvouched (digest <hex>)`, or the source that vouched.
The shell computes that preview from the bytes it read. The progenitor hashes its own copy,
because the caller keeps write access to its frames and could change them after hashing.

Milestone 202 (every confinement test is a ritual until somebody breaks the confinement) gains one
claim, stated and falsified in its shape: an unvouched child holds no capability the caller did not
delegate, beyond the two pages the ruling allows. `unreachable_network_witness` (program id 15) is
the fixture's shape. The new test probes the process domain, entropy and the network, and must go
red when one of them is granted.

## Vouched, unvouched, and the trust root

The kernel's trust root does not change. It vouches for the progenitor and the measurement table,
and nothing about D touches either. Milestone 104's rule
changes, as the ruling records. That is a published claim, so it is the irreversible part of D.

Three ways to hold the gate were offered:

- **D1. Vouch, then run.** The developer records the digest in the owner's table first (§195
  clause 3, already ruled) and D refuses anything not found. Milestone 104's rule survives
  unchanged. Every rebuild needs a vouch, which is friction on every edit, compile and run.
- **D2. A capability to run unvouched bytes.** The progenitor accepts a miss only from a caller that
  delegates a run-unvouched capability the owner gave its session. This is Fuchsia's shape:
  `zx_vmo_replace_as_executable` needs a `vmex` resource, and `fuchsia.kernel.VmexResource` routes
  it like any capability. The authority to run new code is then visible in `caps`.
- **D3. Always allowed, with only the caller's grants.** Genode's and Redox's posture. Redox's own
  source calls its execute check advisory. Simplest, and it leaves no record that new code ran.

D2, the one ruled, is the only one that makes running new code an authority someone holds, which is
this system's whole claim about everything else.

## Prior art, read 2026-09-26

In [`prior-art.md`](0219-naming-an-installed-program-to-the-spawner/prior-art.md).

## The seven questions

1. What else was considered, and why did each lose? B cannot be built well until milestone 205
   lands, and it builds a second spawner, which §208 argued against. A and C serve installed
   programs only. D serves all three cases.
2. What does the tree already do? `DIR_BIT` is A's exact shape, data too big for a word. Milestone
   47 (navigation and naming)'s `PATH` lane priced `NAME_BIT` in those words and stopped at the wire
   change. `login` is B's shape. D's nearest analogue is the interruptible job, which already builds
   a child from an untyped the shell delegated. What D adds is the bytes.
3. Prior art outside the tree. Above, read this session.
4. Is the premise true? Checked above. Reach was not the blocker, and no file capability exists.
5. What does each cost, measured? The table. D's line count is not measured, because nothing is
   built. Its one-slot claim follows from receiving one frame at a time, and `script/swish-check`'s
   peak-slot line would confirm it.
6. How reversible, and who has acted on it? The wire is ours alone: the shell and the progenitor
   ship in one image. It is still a contract every future shell is written against, and milestone
   39 (repository structure for a loosely-coupled OS)'s split exists to allow a third-party one.
   D2's capability and milestone 104's restated rule are the irreversible parts.
7. Would we still choose it at equal cost? For D, yes. It keeps names out of the spawner (milestone
   31 (a capability shell: designation is authorization)), charges the caller for the pages, and
   serves every case with one request. None of that is about effort.

## The two questions A would bring with it, and D inherits

Both are reversible until something outside this repository reads them, so each carries a
recommendation. Both remain open; D's first build takes each provisionally.

- The activation table's shape. Recommended: text, one `<name> <package stem> <digest>` line per
  active program, a file per generation plus a one-line `current` naming the generation. That is
  `measured_boot`'s manifest shape with one column added, which the progenitor already parses.
  Under D the progenitor looks entries up by digest rather than name, from the same file.
- Where a program's manifest travels. Still §197 (a package is one archive file)'s open question.
  Under D both sides need it: the shell to bind the line, the progenitor to endow. A first cut can
  refuse an installed program whose manifest asks for more than `Prog::Uptime`'s, with a `BUGS`
  entry, rather than answer §197 by accident.

## What the ruling unblocks

Rung 3a of milestone 198 after "verified by digest": installing, running, surviving a reboot,
rolling back and removing. Also running anything a developer builds on nife, and milestone 172's
subprocess primitive. Scripts wait on milestone 205, which builds §170's argv.
