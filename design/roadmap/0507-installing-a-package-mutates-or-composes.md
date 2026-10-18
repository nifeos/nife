---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: installing-a-package-mutates-or-composes
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 507. Installing a package: mutate shared directories, compose a view, or only widen what can be spawned

The decision is §208 (installing a package is granting it, and the installed programs are versioned), at
[design/decisions/0208-installing-is-granting.md](../decisions/0208-installing-is-granting.md):
calef chose A3, with rollback (2026-09-23). Installing records that a package exists, its digest
and manifest become spawnable, its data is a read-only directory a session binds by name, and the
table of entries is versioned so an installed-programs table rolls back as a whole. §208 carries why A1 and A2
lost, what A3 obligates, and the one section it reopens.

The text below is the scoping proposal of milestone 198 (a package manager, and the trivial
install), kept as its author wrote it: the options, the costs and the two namespaces are the
argument the decision was made against. How installation reaches a running system is the contract
every package, every program author and every future installer is written against, so it is
irreversible in AGENTS.md's sense. DECISIONS §157 (a trivial install is a web page, a USB drive, and
packages over the internet) defines a trivial install as growing by installing packages over the
internet, which is installing onto a running system.

## What is being decided

What the act of installing *does*: which shared state changes, who holds the authority to change it,
and how it is undone. Milestone 47's conclusion is that installing a program is granting it into a
namespace; `design/haiku-bfs-and-packages.md` notes Haiku reached a similar shape (activate, do not
install) for atomicity rather than authority. This proposal checks both against what the tree
actually has.

## There are two namespaces, and every option must say what it does to each

Reading the code rather than the phrase found that "a namespace" is two different things here:

1. The program namespace: which names a shell will spawn. Today it is **sealed at boot.** The
   shell resolves a name through the closed `grant_plan::Prog` enum (`PROG_COUNT` = 13), sends its
   integer id over `spawnproto`, and the progenitor indexes a `[Option<Elf>; PROG_COUNT]` table it
   filled once, at boot, from the measured archive (`crates/system_initializer/src/lib.rs`,
   `boot`). Milestone 150, in flight as PR #968, generates that table from one declaration, which
   removes the hand-maintenance and leaves the seal: the set is still fixed when the image is built.
2. The file namespace: directories a session can reach. Built out of granted directory
   capabilities plus `bind`, which is "a small fixed table, up to four entries, mapping a name to a
   `(Which, Cwd)` position" (`grant_plan::nav::Bindings`, milestone 47). It names positions; it does
   not union directories.

**Two facts found on the way that any runtime option must answer, and neither was in the brief:**

- **The spawner cannot read anything installed after boot.** The progenitor gives away the file
  service "as soon as the shell holds it" (`system_initializer` module doc, "What it gives away once
  the system is up"). A program installed onto RedoxFS is bytes nothing that builds processes can
  read. The tree has solved this shape once: `login` needs `fs_subtree_caretaker`'s image, so the
  progenitor hands it "a copy of exactly the one program it needs instead, as a blob" (milestone 233).
- **The progenitor refuses what the measurement table does not name.** "One rule: the progenitor
  runs nothing it cannot vouch for" (milestone 104). An installed package is, by construction, not in
  a table the kernel vouched for at build time. That is its own fork:
  DECISIONS §195.

## Options

| | What installing does | Program namespace | File namespace | Undo | Prior art (read) |
|---|---|---|---|---|---|
| **A1. Mutate shared directories** | Write the package's files into shared directories on RedoxFS | A writable table of spawnable names the spawner re-reads | Files appear in place | A per-package file list; no atomicity across files unless RedoxFS provides it (**not checked**) | apk: installs change the shared filesystem in place (read: hydrogen18.com's apk post). The tree already does this on the host: `cargo xtask manual` writes `doc/<bundle>/` and a `doc/bundles` index into the RedoxFS image |
| **A2. Compose a read-only view** | Store the package whole and read-only; a composer presents the union of activated packages | The composer's view is what the spawner reads | A userspace server speaking `filesystem_contract` over the union | Keep the previous installed-programs table and select it | Haiku packagefs: "a virtually extracted union of the contents of all packages"; `activated-packages` file; the boot loader offers an old state (read: `haiku/docs/develop/packages/Infrastructure.rst`). Nix: a profile is a symlink to a generation, flipped atomically; rollback repoints it (read: `nix.dev/.../package-management/profiles`). OSTree: read-only `/usr`, boot the previous entry (read: `ostreedev.github.io/ostree/introduction/`) |
| **A3. Grant, do not place** | Record that the package exists: its digest and manifest become spawnable; its data is a read-only directory a session binds by name | Widened by one entry (name, digest, manifest) | `bind <pkgdir> <name>`; no union, one directory per package | Remove the entry; nothing was written into shared space | Fuchsia: components are resolved from packages by URL, and "some component resolvers are limited to base packages" (read: `fuchsia.dev/.../verified_execution`). Genode/Sculpt: after download "a configuration dialog ... define[s] the interplay of the new component with the system", routing each service (read: `genode.org/documentation/articles/sculpt-25-10`) |

What the tree already does in the analogous case.

- Documentation is A1 on the host today (the store is written into the image) and A3-shaped
  at query time: `apropos` merges per-bundle index shards, and a viewer is granted exactly one
  page, never the store (milestone 40, phase 2). The shard merge is milestone 40's own design for
  "installing a component makes its documentation searchable without a reindex pass".
- Namespaces are composed in the **client** (milestone 47, "Absolute paths: Plan 9's answer"), and
  §50 chose `bind` over stored paths. A2's union server would be the first place the tree composes
  a namespace in a *server* rather than in the client.
- A program's authority is its grant, not its location. Haiku's own limit applies verbatim
  (`design/haiku-bfs-and-packages.md`): its activation "gives atomicity and rollback, not
  confinement." So A2 buys nothing on authority that A3 does not, and A3 is the only one of the three
  where installing is literally a grant.

## Costs, measured or bounded by the code

- A1: a writable spawn table and a way for the spawner to read installed bytes. Also a removal
  story nobody has: milestone 150's block notes removal "has no page at all today" even in-tree.
- A2: a new server (a union over read-only package stores), plus the spawner reading through it.
  The largest of the three by construction; no measurement possible before it exists.
- A3: milestone 47's `PATH` lane priced the program half: a `NAME_BIT` (provisional) carrying a
  length-prefixed name over further `SEND`s, "the same shape `DIRECTORY_GRANT_BIT`'s 'expect two more SENDs'
  already establishes", plus a manifest encoding crate. The file half needs `Bindings` to hold more
  than four entries, or packages to share one bound store directory. Both are changes two programs
  agree on (the shell and the progenitor), which is why this proposal gives no winner.

## Reversibility, and who has acted on it

No one has installed a nife package. But milestone 150 is changing the program namespace's
declaration right now (PR #968): it generates the spawnable table from `Cargo.toml` `[[bin]]`
blocks and pins "the thirteen shipped wire ids". Any of A1 to A3 extends what 150 builds; none
contradicts it. The pinned wire ids are the thing to watch, because a name-carrying spawn request is
the alternative to an id, and an installer would add a second way to name a program beside the id
150 fixes.

## The §92 test

A1 is the cheapest to describe and not the cheapest to build here, because it needs a writable spawn
table, a removal list and an atomicity answer. A2 is the most expensive. A3 is between. If A3 is
preferred because it is cheaper than A2, that is effort, and should be said. The non-effort case
for A3 is that it is the only option in which installing changes only what may be granted, which is
milestone 47 (navigation and naming)'s claim made literal. The non-effort case for A2 is Haiku's and Nix's atomic rollback of
a whole system state, which A3 gets only if the table of entries is itself versioned.

## If calef says no to all three

*(calef chose A3; see §208. The section is the proposal's own reasoning about the stakes, which is
what made the cost of a refusal legible.)*

Nothing is installed at runtime; packages are build inputs to an image, and a new program arrives by
reflashing. That is the first slice, and it is a complete answer for us as builders. It is not an
answer for a customer who wants `git` without rebuilding their system. *(Under DECISIONS §157 that
first slice is not the answer, and a "no" here also stops rung 3; milestone 198 carries the
rescoping.)*

## Index row

What the act of installing *does*: which shared state changes, who holds the authority to change it,
and how it is undone.
