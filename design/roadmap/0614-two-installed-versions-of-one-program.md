---
status: BUILT
raised: 2026-09-27
built: 2026-10-19
---
# 614. Two installed versions of one program, each runnable, and a caller granted the one it needs

Minted 2026-09-27 (UTC) by a maintainer-delegated lane, from calef's ask the same day: *"One
limitation of our model is that we cannot support multiple versions of the same binary on a system.
This is important to developers that might run different versions of their frameworks because
different packages have different dependencies on upgrading a dependency. We should mint a milestone
to address that."* *(Number provisional until the merge queue lands it. Title and slug are drafts.)*

Built 2026-09-29 (UTC) on `milestone/614-every-version-live`, against the five rulings below.
`crates/installed_programs` carries the ruled shape: rows `<digest> <program> <version> <package>`
keyed on the digest, and the default pointer inside the generation file. Install appends and moves
the pointer, so the bare word still means the newest install. Removal follows ruling 5: bare remove
takes every live version and the pointer; `package@version` removes one, moving the pointer to the
sole survivor and refusing, candidates named by the shell from the live table, when several remain.
Rollback is unchanged. The shell resolves the bare word through the pointer, and consults the
nearest version-set file at or above the working directory (asdf-style; built as `versions`,
renamed `.jig-versions` by §258 (names for two installed versions of one program)). It prints the
divergence notice on the spawn line when what ran differs from what the set specifies, and takes
`program@version` as an explicit ask. `script/swish-check`
types it on all three architectures: the second version of `greeting` installs beside the first,
each runs by path, the set selects 0.1.0 for the bare word, and a set naming an uninstalled
version produces the notice.

## Why two versions cannot coexist today

(The state that made the milestone, kept as minted; the paragraph above is what changed.)

The bytes already can. Install writes a program to `packages/<name>/<version>/<program>`
([notes/packages.md](../../notes/packages.md)), and remove leaves them there. What cannot hold two
is the table that says what may run:

- A generation of the activation set, per §208 (installing a package is granting it), is `<program> <package>
  <digest>` lines with no version field. `activation_set::with_entry` keeps one entry per program
  name and replaces it in place, which is how an upgrade works.
- Running by path is not a way round it. §219 (how the shell names an installed program to the
  spawner) sends bytes, and the progenitor runs them only if their digest is in the live
  generation (`activation_set::lookup_digest`). The old version's digest left the table when the new
  one replaced it, so `packages/uptime/0.1.0/uptime` gets `SPAWN_UNVOUCHED` once 0.2.0 is installed. The one exception is a session
  holding the capability to run unvouched bytes, a developer's escape hatch rather than an
  install.
- §229 (a bare name reaches an installed program) resolves a bare word through one entry per name,
  with no search order, by calef's ruling. Pull request #1374 (milestone 47, a bare word runs an
  installed program) adds the refusal that another package may not take a name already provided.
- Until §220 (signed builds) is built, a new version of any package is a new image catalog, and
  so a new boot slot: §241 (a threadbare base), "What is being decided", item 2.

Libraries are not the problem here. There is no dynamic loader and no shared library
([notes/abi.md](../../notes/abi.md)), so each program links its own dependencies at build. Where
nife's "framework" versions collide is in programs and the services they talk to.

## Prior art

Read 2026-09-27 from each project's own documentation, except where marked.

- Nix. Every build lands at `/nix/store/<hash>-<name>-<version>`, so versions never collide. A
  profile generation is a tree of links into the store; the bare command is whatever the current
  generation links, and rollback swaps generations
  ([store paths](https://nix.dev/manual/nix/stable/store/store-path.html),
  [profiles](https://nix.dev/manual/nix/stable/package-management/profiles.html)). Guix is the same
  design (from memory). nife's versioned activation set is already Nix's generations; what it
  lacks is Nix's store, where many versions stay live.
- Homebrew. Each version is a keg at `Cellar/<formula>/<version>`; `opt/<formula>` and `brew
  link` point at the active one. A second major version is a second formula, `python@3.12`, left
  unlinked and reached by its `opt/` path ([FAQ](https://docs.brew.sh/FAQ),
  [Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)).
- Debian alternatives. A generic name links through `/etc/alternatives/` to one registered
  alternative, highest priority wins unless an administrator sets one
  ([update-alternatives(1)](https://manpages.debian.org/bookworm/dpkg/update-alternatives.1.en.html)).
  Versioned package names such as `python3.11` beside `python3.12` carry the rest (from memory).
- Fuchsia. A package is identified by the Merkle root of its metadata.
  `fuchsia-pkg://<repo>/<package>?hash=<root>#meta/<component>.cm` pins exact bytes; omit `hash`
  and the resolver picks the current revision
  ([packages](https://fuchsia.dev/fuchsia-src/concepts/packages/package),
  [component identifiers](https://fuchsia.dev/fuchsia-src/concepts/components/v2/identifiers)).
- asdf and rustup. Versions install side by side behind a shim or proxy. asdf reads
  `.tool-versions`, walking up from the working directory
  ([asdf](https://asdf-vm.com/manage/versions.html)). rustup takes `+toolchain`, then an
  environment variable, then a directory override, then `rust-toolchain.toml` walking up, then the
  default ([overrides](https://rust-lang.github.io/rustup/overrides.html)). Both are search orders.
- Capability routing. A Genode child asks for its binary as a ROM session by label, and the
  parent's `<route>` decides which file answers, per child, so two children asking for one name
  can get two files
  ([init](https://genode.org/documentation/genode-foundations/26.05/system_configuration/The_init_component.html)).
  Plan 9's `bind` puts a different binary at `/bin/prog` in each process's own namespace
  ([bind(1)](https://9p.io/magic/man2html/1/bind)). Neither has a global answer to "which
  version": the caller holds what its parent gave it.

## Options

| option | what it is | verdict |
| --- | --- | --- |
| A. The version goes in the name | `python@3.12`, Debian's `python3.11`: a second package with a second program name | Refused as the answer, kept as a fallback. It is what calef called the limitation; `uptime` at two versions becomes two programs |
| B. One active version, switched | Debian alternatives, `brew link`: many installed, one live at a time, chosen system-wide | Refused. Two consumers needing different versions at once is the whole ask |
| C. A file in the working directory picks the version | asdf's `.tool-versions`, rustup's `rust-toolchain.toml`, both behind a shim | Refused. It is a search order keyed on ambient state, which §229 ruled out, and the directory rather than a grant decides what runs |
| D. The table admits every installed version | Nix's and Fuchsia's arrangement: a version is identified by its content, and many can be live | **Recommended, as the table half.** Install of a new version appends; the bare name keeps one entry, the default |
| E. The caller is granted the version it needs | Genode routes a child's binary by label; Plan 9 binds a per-process `/bin`; Fuchsia routes a capability to a component pinned by hash | **Recommended, as the selection half.** In a capability system "which version" is "which file the caller holds", and §219 already made the spawner send bytes rather than a name |

**The recommendation is D with E.** The table stops forgetting a version when another is installed.
The bare word still means one version, the default, so §229 is untouched. A specific version is
reached by path, or by a directory capability that a project, a session or a package's recipe is
granted, in which `uptime` means 0.1.0. No search order and no ambient lookup is added: the
answer falls out of the model rather than fighting it. D alone would leave a developer typing store
paths. E alone cannot work, because the progenitor would refuse the old version's digest.

Would it still be chosen at equal cost? Yes. A is cheaper and is refused on what it does to names,
not on effort.

## Rulings

All five questions below were ruled by calef on 2026-09-29 (UTC), in session with the maintainer;
the reasoning is kept to one line each so a reader can check the rule against its reason.

1. A grant names a version both ways, and the digest is authoritative. The version string is the
   upstream developer's claim (they know they shipped 0.1.0); the digest is the packager's
   attestation, minted when the package is built. The tree already has those roles: §195 (a reviewed
   recipe vouches for a package) is the vouching, the archive carries the member digest, installing
   verifies, the table enforces.
   A string alone would let a rebuilt 0.1.0 masquerade; a digest alone would put digests in every
   diagnostic.
2. Rows key on the digest; `program`, `version` and `package` are label columns. A rebuild claiming
   a version string already live is a second row, visible, never a silent replacement. The default
   pointer lives inside the generation file, so a rollback restores the table and the default
   together and `current` stays the one commit point. No architecture column: the digest is of
   target-specific bytes, so builds for two ISAs never collide in one table. No install datetime:
   the generation index is a total order with no clock in it, and `package` writes an install event
   to §242 (a system log) for timeline reconstruction.
3. The bare word means the newest install: every install of a program moves its default pointer.
   This is today's implicit rule written down, so nothing a user does today changes meaning, and
   §229 stands, one answer per name with no search order. A `package default` command, if ever
   wanted, is a table edit that moves the pointer and needs no format change.
4. The per-project selection is a directory capability named a version set (name provisional). Its
   content is a committed file of `<program> <version>` lines: version strings, because people
   write it, resolved to digests at activation through the live table, so enforcement stays
   digest-authoritative (ruling 1 one layer down). The shell consults the nearest version set at or
   above the working directory, asdf-style, on calef's usability ruling; a version set can only
   select among installed versions, so a cloned repository can ask but cannot install or run
   uninstalled bytes. Two guards: when the version that ran differs from the version the set
   specifies, the spawn line says both (`uptime 0.2.0 (repo specifies 0.1.0)`, reworded by §258); and an explicit
   override exists, by path or by an explicit version-qualified ask. An installing dev tool is
   follow-on work, not this milestone; the capability, the file and the tool are three provisional
   names.
5. `jig remove uptime` removes every live version of the package, its rows and its programs'
   pointers: the verb's object is the package (calef, 2026-10-14 (UTC), on milestone 919 (a
   package carries several programs): "jig remove should work on packages, not programs."), and
   the version-qualified form removes one. Removing the version that holds a program's pointer
   moves it to the sole remaining version, and refuses, naming the candidates, when several remain,
   because no ordering among live versions exists to pick with. Rollback is unchanged (§208): a
   generation is one snapshot of rows and pointer, and bytes are never deleted, so what it
   restores is still on disk. Removal is not dependency-aware; see BUGS.

## Dependencies

- Milestone 198 (a package manager, and the trivial install): rung 3a only, install, remove,
  rollback and the activation set this changes. Rung 3a is built (2026-10-05; the evidence is
  notes/packages.md and the install, remove and rollback gates on all three architectures). 198 is
  PARTIAL as the umbrella over 801 and 802, which this does not need. The frontmatter dropped it
  on 2026-10-06 (UTC) under calef's rule in notes/roadmap.md: depend on part of a milestone, and
  you split it.
- Milestone 47 (navigation and naming), pull request #1374: the bare-name resolution and install
  refusals this must keep.
- The proposal
  [0666-install-time-signature-verification.md](0666-install-time-signature-verification.md)
  (§220). Not a hard dependency, since two versions can be proved with two catalog entries, but
  without it a developer cannot install a second version without a new boot slot.

## What this unblocks

- A developer running two projects that need different versions of one program or service at once,
  which is calef's case.
- Staged upgrades: install the new version beside the old, move one consumer, then the rest. §208's
  live-swap intent (*"packages that can live load to replace an older version and then roll back
  if it breaks"*) wants the old
  version still runnable while the new one is tried.

## Done means

- Host tests in `activation_set`: installing 0.2.0 over 0.1.0 leaves both digests live; the bare
  name resolves to whichever version the ruled default names; removing one version leaves the
  other; a rollback restores the whole set, as `a_rollback_restores_the_whole_set` does today.
- Bare removal: `jig remove <package>` takes every live version and the pointer, with one
  test at two versions live proving both rows go and a rollback brings both back.
- A `script/swish-check` transcript on aarch64, riscv64 and x86_64 per §19 (architectural parity is
  a tenet). It installs two versions of one fixture, runs each by path and by the ruled grant in
  one session, and shows the bare word running the default.
- The names and the format above ratified by an architect, recorded in `design/decisions/` by the
  integrator.

## BUGS

- **Install writes no event to the system log.** Ruling 2's timeline reconstruction is served by
  §242 (a system log), and its service is not built: nothing on the install path can append to it,
  so the generation index remains the only install history. The trigger that makes the gap real is
  §242's building lane landing the capability an install would hold; revisit there, and do not
  build a second log to fill it.
- **A remove operand shares the sixteen bytes a packed name carries** with its version
  (`file_system_contract::grant::MAX_NAME`). A package whose name plus `@` plus the version exceeds
  sixteen bytes cannot be removed by the qualified form; it is still removable bare, which
  removes every version. Recorded 2026-09-29 when the qualified form was spelled. If a real
  package hits it, the wire grows a second packed name rather than widening the first.
- Services, not only programs, will want two versions at once (two versions of a file service
  behind two projects). This milestone covers programs a spawner runs. A running service at two
  versions is the same table change plus two endpoints, and is left to the building lane to
  scope or split.
- Removal is not dependency-aware (ruled 2026-09-29): nothing in the system records
  package-to-package dependency, so `jig remove` cannot warn about dependents. With no dynamic
  loader, what looks like a dependency is a grant, and removing a package narrows what may be
  spawned next without revoking anything already running. The trigger that makes the question real
  is the services milestone above, where "who holds a grant from this" becomes askable; revisit
  there.
- No ban semantic is built (ruled 2026-09-29). The eventual shape is settled: a denial outranks a
  row at the spawn check. It is not minted now because a ban needs an authority behind it (§220
  (signed builds), or a package source worth revoking from). Its near-term trigger is the
  version-set tool's installer meeting a version a user has decided is bad. The ruled format does
  not preclude it: a denial can join as a third line kind beside rows and defaults, checked at the
  same choke point.
- **Corrected 2026-10-02 UTC: the fetch failure was this milestone's defect, not a flake.** The
  2026-09-30 diagnosis called it the second-fetch flake; every leg of this branch that reached
  `package install greeting` failed the same way (five of five runs, through 2026-10-01), and the
  cross-PR merge-queue runs it cited batched this pull request. The cause: adding
  `greeting-0.2.0.recipe.toml` put two `greeting` stems in the image's catalog, and
  `package_archive::catalogued_stem` took the first line, which recipe filenames ordered as 0.2.0
  (`-` sorts before `.`). The gate's source serves only 0.1.0, so the guest's GET was a 404. Now a
  bare fetch of a name cataloged at several versions is refused as `Ambiguous` before the network,
  and `jig install greeting@0.1.0` (the qualified spelling) picks one. The refusal names no
  candidates, because the shell does not read the catalog; a reader who wants them has the
  recipes. Revisit when §220 (signed builds) makes catalog versions something a person adds.

## Follow-on

Added by the build lane, 2026-09-29.

- **Done.** The transcript bullet of Done means, by merge-queue run 39267144306 at `42836328b`:
  job 118109695594 (aarch64, riscv64) and job 118109695819 (x86_64). The gate wants every phrase
  inside its own line's answer, in order, and skips none of these lines in CI. A local
  `NIFE_SHOW_TRANSCRIPT=1` run at that commit shows the answers on all three legs.
- **Done.** Ratification, by calef on 2026-10-07 (UTC), recorded in §258 (names for two installed
  versions of one program). Ratified as they stood: `program@version`, the `as` recipe key, the `default` line kind,
  `NO_VERSION`, `versions_of` and `without_version`. Renamed: the `versions` file is
  `.jig-versions` and its module `swish::jig_versions`, `package_archive::StemMiss` is
  `CatalogMiss`, `activation_set::Error::Ambiguous` is `Error::SeveralVersions`, the fixture
  `greeting_two` is `greeting_v2`, and the divergence notice reads `uptime 0.2.0 (.jig-versions
  asks for 0.1.0)`. Every name this milestone shipped is now ruled. The section holds the
  refusals.
- **Done.** Carried by pull request #1443, in a comment of 2026-10-03 00:24 UTC. The
  `package install <package>@<version>` spelling: ratified 2026-10-03 (calef), the same `@` as
  `remove`. Refusing a bare `package install <name>` that matches more than one cataloged version
  as ambiguous: accepted 2026-10-03 (calef). `StemMiss` was not covered and stays provisional.

## Index row

Two versions of one program could not be installed together: the activation set held one entry per
program name, and the progenitor refused bytes whose digest had left it. An upgrade made the old
version unrunnable even by path. Built 2026-09-29 as ruled (see the paragraph under the title):
the Nix and Fuchsia table, every installed version live and identified by content. The selection
is capability-shaped: a caller is granted the version it needs, and the bare word keeps one
default. Ruled 2026-09-29:
both names with the digest authoritative, digest-keyed rows with the default pointer in the
generation file, the bare word means the newest install, and the selection is a version set.
