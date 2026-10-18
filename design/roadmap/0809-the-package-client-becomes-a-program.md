---
status: PARTIAL
raised: 2026-10-06
promoted_from: the-package-client-becomes-a-program
milestone_dependencies: 205
decision_dependencies: 270
machine_requirements: none
specific_machine: none
needs_person: no
---
# 809. The package client becomes a program, `jig`, with the verbs an index needs

**PARTIAL, 2026-10-10 (UTC).** Items 1 to 5 and item 8's digest check are built, and pass
`script/swish-check` on aarch64, riscv64 and x86_64 under QEMU. `update` and the index copy are
built (milestone 801 (packages over the internet)). Left: `list -u`, item 7
(`add-repository`) and item 8's index half. The program is
[notes/packages/jig.md](../../notes/packages/jig.md).

*(calef, 2026-10-07 (UTC): "Promote jig to a milestone." `jig` is ratified; the title, slug and
verb spellings are drafts. Later rulings are in the work list and the forks table.)*

*Not a dependency of 198, the umbrella over 802; it uses rung 3a, built inside 198.*

calef ruled on 2026-10-06 (UTC) that the package client must be a program, not a shell builtin, and
named six things it must do.

The program is `jig`, the basalt package manager, ratified 2026-10-06 by calef: the guide that makes
flat-pack assembly come out the same every time. Refused: `package`, `pkg`, `bpm`, `knap` and
`flatpak`; `kit` and `cam` collide with shipped commands. When built, this record becomes the
`Name:` block in its module doc, where `script/names` reads it.

calef also ruled, the same day: *"We abbreviate on the command line."* Under it he ruled the verbs
on 2026-10-10 (UTC): *"Lets use apt's verbs."*, and the same day *"Change add-index to
add-repository."* [The record](../naming/command-line-rulings.md#jig-takes-apts-verbs)
has each spelling, whose it is, and the manuals read.

## What calef asked for

His wording is the intent.

| Intent | Spelling (ruled 2026-10-10) | Built |
|---|---|---|
| List available packages, from the local copy of the index | `jig list` (apt's) | yes; the catalog before any `update` |
| Install a package from the index | `jig install <name>` (apt's) | yes; `jig` fetches |
| Install a local package, an archive already on the system | `jig install <path>` (apt's verb); a word with a `/` is a file (§219 (how the shell names an installed program to the spawner)) | yes |
| Add another index | `jig add-repository <url>` (nife's; apt has no verb) | no, item 7 |
| List what is outdated | `jig list -u` (the short flag apt also accepts) | no, item 6 |
| Update the local copy of the index | `jig update` (apt's) | yes (801) |
| Remove a program | `jig remove <program>` (apt's) | yes |
| Roll back a generation | `jig rollback` (nife's; apt has none) | yes |

`update` refreshes the index and installs nothing: apt(8), FreeBSD's pkg-update(8) and Homebrew's
manpage, read 2026-10-10 (UTC), agree. apt's `upgrade` is not built here; it is the spelling the
#1805 amendment below will use.

The outdated flag is `-u` alone (ruled 2026-10-17 UTC). Asked whether it was `--upgradable` or
`--upgradeable`, calef asked *"What if we just did -u?"*, then answered *"Yes"* to: *"`-u` only,
with a refusal that names `-u` when someone types `--upgradable` or `--upgradeable`."* Measured
2026-10-17 in apt's source (Debian/apt `apt-private/private-cmndline.cc:356-357`): apt accepts both
long spellings and `-u`, and `apt update` prints `apt list --upgradable`
(`apt-private/private-update.cc:159-160`). `-u` is ratified under "We abbreviate on the command
line."

*calef, 2026-10-07 (UTC), on #1805: `jig` must update base packages; only the slot reboots. See §159 (only a new kernel needs a reboot).*

## What moved, and what stays the progenitor's

The builtin's two reasons were checked first. The progenitor alone holds what installing needs,
which keeps the installer there, not the client in the shell. And a program could not be told which
package: stale since milestone 205 (how a foreign program is told what to do) built
`ArgSpec::Words` on 2026-09-27.

So the builtin left the shell and the fetch left the progenitor, in one change, so the owner never
had two clients for one store. Under §195 (a reviewed recipe vouches for a package) the digest
decides, so whoever carries the bytes need not be trusted. `jig` fetches them and sends an ordinary
install, and `http_response` left the progenitor's graph.

The progenitor keeps the installed-programs table and the one rename of `current` that changes what runs
(§208 (installing a package is granting it, and the installed programs are versioned)), the digest check
on its own copy, and the image's catalog. `vouch` stays a builtin; a `jig vouch` later is small and
reversible.

## The capabilities the program holds

| Capability | Verbs that use it | How it is granted | Exists today? |
|---|---|---|---|
| A client view of the network stack (`NETWORK_SLOT`; slots 2 and 3 for a `std` program) | `install <name>`, `update` | `Manifest::network` | yes (milestone 590 (the booted system starts its network stack)); for a `std` program at the prompt, since this block |
| The index directory, read and write | `list`, `list -u`, `update`, `add-repository`, `install <name>` | `Manifest::indexes`, a directory grant on `activation/indexes`, owner's console only | yes (801) |
| `activation/`, read only | `list -u` | a directory grant | as above |
| The file a line names | `install <path>` | `ArgSpec::Words` designation (§170 (how a foreign program is told what to do)) | yes |
| A request to the installer | `install`, `remove`, `rollback` | `Manifest::installer`, below | yes, since this block (§270) |
| The image's catalog, read only | `list`, `install <name>` | `Manifest::catalog` | yes, since this block |

The installer endpoint is the one new grant, recorded in §270 (a package manager holds an
installer endpoint, not the spawn endpoint). It is a copy of the spawn endpoint, badged for one job.
It serves install, remove and rollback, never `Vouch`, and only the owner's console grants it. The field the draft called `activation` is `installer`, and `catalog` came with it. The
program is `StartupContract::Std`, because `ArgSpec::Words` requires it.

## One program with verbs: ruled

calef, 2026-10-06 (UTC): *"One program."* That refuses V3. V1 against V2 is detail within it, and
this block keeps V2.

calef's principle, from §226 (`pidwait` takes tids): a program does one thing. Milestone 281
(`watch` holds exactly what `ps` holds) made that measurable: two programs are two programs when
they hold different authority. These verbs do. `list` needs one directory, `remove` only the
installer, and `update` the network and a write.

| Option | What it is | Verdict |
|---|---|---|
| V1. One program holding the union | Every line holds the network, the index, `activation/` and the installer | Refused. `jig list` would carry authority it never uses: 281's own counterfactual, where `ps` would always carry a clock it rarely needs |
| V2. One program, granted by verb | The manifest maps each verb to its grants, and the planner reads the first word | Recommended |
| V3. Split by authority | A reader (`list`, `list -u`), an index keeper (`update`, `add-repository`) and an installer client (`install`, `remove`, `rollback`) | Refused by calef's ruling |

V2 has a precedent: `rm -r` hands over more than `rm` through `subtree_option`, and a verb table is
that idea keyed by a word. V1 was cheaper, so V2 was not about effort. The table's encoding was the
lane's: sorted (verb, grants) pairs, a note type of its own (`manifest_note::VERBS`).

## Who writes the index copy: ruled, `jig`

calef, 2026-10-06 (UTC), choosing I2: *"I would think jig."* Once milestone 801 (packages over the internet)'s index replaces
the image's catalog, whoever writes the index copy decides which digests the progenitor accepts.

| Option | What it is | Cost |
|---|---|---|
| I1. The progenitor fetches the index | `jig update` is one more activation request | TLS and an index parser in the most trusted process (§196 (nife carries TLS)) |
| I2. The program fetches and writes the copy | The progenitor trusts the copy in a directory only the owner's console grants | A client defect becomes a defect in what runs; provenance becomes "the owner's client said basalt vouched" |
| I3. A signature over the index | The progenitor verifies the copy against a key in the image | A long-lived key, which §195 clause 4 deferred and named irreversible |

I2 widens nothing, since the owner may already vouch for any bytes (§195 clause 3, §221). Until
I3, which waits on a key-custody ruling, `jig` is inside the trusted base for installs.

## Many indexes per machine: ruled

calef, 2026-10-06 (UTC): *"A machine may write many indexes."* So a machine holds several, as apt's
`sources.list` does; details below.

§250 (an image names its distribution's package index) fixes only an image's one index, basalt's;
an index the owner adds is the machine's. §195 clause 2 scopes trust per source, and §196 clause 4
holds roots per source.

Its four details are all ruled:

1. How an added index is authenticated: ruled 2026-10-17 (UTC). calef: *"Just a URL."*
   `jig add-repository <url>` takes only a URL, and a refusal names the root the server needed.
   He first ruled that the index is checked against the roots the image pins for basalt (ISRG Root
   X1 today). §306 (the web's authority list is a package and a capability), decided 2026-10-18,
   replaces that set. An added index is trusted at the channel as a website is, by the list's
   authorities; basalt's own channel never depends on the list.
2. The same name in two indexes: ruled 2026-10-17 (UTC). calef chose *"A"*: a bare name offered by
   two indexes is refused as `Ambiguous`, as milestone 614 (two installed versions of one program, each runnable) does for versions, and an
   index-qualified name picks one. First-index-wins is refused as the dependency-confusion shape.
   The qualifier is `jig install --from <index> <name>`, with the index's URL exactly as added. The
   spelling is provisional (`Name: provisional` in
   [design/naming/README.md](../naming/README.md), tracked by `script/names --unratified`). It is
   a flag because §219 reads a word containing `/` as a file, and `@` already means a version.
3. Provenance: ruled 2026-10-17 (UTC). Asked *"Record each row's vouching index by its URL?"*, calef
   answered *"Yes"*. Each installed-programs table row records the URL of the index that vouched
   for it, exactly as added, in place of `OWNER` alone, so a rollback can say what it is undoing.
4. An added index that says it moved: ruled 2026-10-17 (UTC). calef chose *"B"*: `jig` does not
   follow an added index's "moved to" field. It reports it and names the `jig add-repository <new
   url>` command. basalt's own index keeps following §250's field.

## Other package managers, against the same contracts

`jig` uses only public contracts: `crates/package_archive` (§197 (a package is one archive file)),
the index format, `spawnproto`'s activation request (§208) and `crates/installed_programs`. A second
manager on those crates can do what `jig` can, once the owner's console grants it the installer
endpoint.

calef, 2026-10-06 (UTC), accepted how managers coexist: *"Yes."* Only `jig` holds the endpoint by
default; other tools stay user-local, like `pipx`. Each installed-programs row records which manager
installed it, and the progenitor refuses a manager's remove or rollback of another's row.

## How this splits against milestones 801 and 802

Milestone 801 built the index format (`crates/package_index`) and `package_fetch_exerciser`; `jig`
adopts both at item 6. Milestone 802 (the trivial install) depends on this block so its stranger
meets a program, not a builtin.

## The work, in the order it can land

1. The installer capability: the badged endpoint, the slot and the `Manifest` field, granted only
   by the owner's console. Falsify it: a program without the grant is refused, and a `login`
   session cannot grant it.
2. The program, with `install` (both forms), `remove` and `rollback`, ported from `swish::package`.
   The builtin goes in the same change. `script/swish-check`'s transcripts and `notes/packages.md`
   change from builtin to program.
3. The fetch moves into the program, and `Activation::Fetch` and `http_response` leave the
   progenitor. The gate that fetches `greeting` today passes unchanged in what a person types.
   Per calef's ruling on #1796 fork 4 (2026-10-07), the same change deletes `system_initializer`'s
   `http_response` exception in `packages/init.package.toml`.
4. Grants by verb (V2), or V1 with a `BUGS` line if V2 is refused.
5. `list`, reading the image's catalog until an index exists, and saying so.
6. `update` and `list -u` over the LAN fixture source, plain HTTP, as rung 3a, with
   `jig` writing the index copy through `package_index`.
7. `add-repository`, with the many-indexes details above. Unblocked.
8. Before installing, `jig` checks three things. The archive's digest matches the index (§250 (an
   image names its distribution's package index)). The index's needs match the archive's metadata
   (§197 (a package is one archive file)). The metadata matches the manifests in its ELF notes,
   recomputed after download. A mismatch refuses (calef, 2026-10-07, "Yes"). The needs
   come from [#1797's fork 1 ruling](https://github.com/nifeos/nife/pull/1797#issuecomment-6029565630):
   written only in program manifests, computed by the build into metadata and index. These checks
   keep resolution correct and records honest; they are not the security boundary, which stays at
   spawn, where a program gets only what its manifest names and the grant plan approves (§208).

Items 1, 2, 3 and 5 wait on nothing. Every item is proved on aarch64, riscv64 and x86_64 by the
same gate (rule 5).

Reuse: all inside the tree (`package_archive`, `http_response`, `socket_contract`'s fixture peer,
`installed_programs`, `spawnproto`'s activation request, swish's frame sender, milestone 205's argv and
designation). Writing the program was forced: no existing client speaks this capability ABI. The
apt, pkg and Homebrew manuals §46 (thin primitives or whole subsystems) owed were read 2026-10-10;
[the vocabulary record](../naming/command-line-rulings.md#jig-takes-apts-verbs) cites them.

## What was built

Built 2026-10-10 (UTC), #1903, as [notes/packages/jig.md](../../notes/packages/jig.md) describes,
on all three architectures under QEMU. `install` (both forms), `remove` and `rollback` pass
`script/swish-check` with the builtin gone, and `greeting` is fetched by `jig`. The progenitor has
no `http_response` or `Activation::Fetch`, and a program without the installer grant is refused.
The gate's typed text changed: `package` became `jig`, file installs are typed from inside
`downloads` since the root cannot be narrowed, and `jig`'s own refusals name no generation. Three
lines are new: `caps jig install ...`, `jig list`, and `installed/jig rollback`, the same bytes run
unvouched and refused the installer. The progenitor's stack went from twelve pages to fourteen,
measured.

## Follow-on

- **Outstanding.** `list -u`, which must read `activation/`; the index store grant does
  not reach it.
- **Outstanding.** Item 7, `add-repository`: unblocked lane work, since all four many-indexes
  details are ruled. Checked 2026-10-17 against the Many indexes section.
- **Outstanding.** Item 8's index half, the needs against metadata and manifests. Checked
  2026-10-10: the catalog carries digests only, so there is nothing to compare until item 6.
- **Milestone 892.** Milestone 892 (a std program at the prompt holds the network):
  `design/roadmap/0892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md`. Item 2 is
  built here for `jig`; 1, 3 and 4 remain, and `jig` needs them to resolve `basalt.nifeos.org` for
  milestone 810 (`ripgrep` is packaged in basalt).
- **Milestone 696.** Milestone 696 (the spawn service runs outside `boot`'s frame),
  `design/roadmap/0696-the-spawn-service-runs-outside-boots-frame.md`, is the trim that would let
  the progenitor's stack come back down from the fourteen pages this block raised it to.

## Forks

| Fork | State | Blocks |
|---|---|---|
| The name | ratified 2026-10-06: `jig` | nothing |
| One program or several | ruled 2026-10-06: one program; V2 kept, and built | nothing |
| Who writes the index copy | ruled 2026-10-06: `jig` (I2); I3 the follow-on | nothing |
| Many indexes | ruled 2026-10-06: a machine may write many; all four details ruled 2026-10-17 | nothing |
| A row records its manager (plan fork 3) | ruled 2026-10-06: yes; `jig` alone holds the endpoint by default. Built before item 1 | nothing |
| Who fetches | ruled 2026-10-07, #1796 fork 4: `jig` | nothing |
| Install-time checks | ruled 2026-10-07: item 8's three | nothing |
| Needs against what is installed | open, #1797 fork 3 | nothing yet |
| The installer endpoint (plan fork 1) | ruled 2026-10-10, calef: "Yes"; §270 | nothing |
| The verb table's encoding (plan fork 2) | the lane's, approved 2026-10-10 on #1903 with calef's change: sorted (verb, grants) pairs, variable length, at most 256 | nothing |
| Verb spellings (plan fork 4) | ruled 2026-10-10, calef: "Lets use apt's verbs."; `add-repository`, same day | nothing |
| 801's items 1 and 2 here (plan fork 5) | moot: 801 built them; `jig` adopts them at item 6 | nothing |
| The outdated flag | ruled 2026-10-17, calef: "Yes"; `-u` only, `--upgradable` and `--upgradeable` refused with a message naming `-u` | nothing |
| How a manifest names a fixed directory | decided by 801's lane 2026-10-12, reversible: `Manifest::indexes` | nothing |

## BUGS

- Retiring `Activation::Fetch` changed a wire format that only this tree speaks.
- Until I3, `jig` is inside the trusted base for installs, and the installed-programs table's provenance for
  an index install is `jig`'s claim.
- `jig`'s own refusals name no generation, and a `remove <package>@<version>` that cannot pick a
  default names no candidates: it holds no view of `activation/` until item 6 grants one.
- A package file at the root of the shell's namespace cannot be installed by path from the root,
  because the root cannot be narrowed to one name; `cd` to its directory first.
- An installer holder is built from the image pool, so a file run by its path in the same pipeline
  waits for it or is refused. The shell refuses no such pipeline; §270's BUGS has the limits.

## Index row

The package client was a shell builtin, and the progenitor fetched packages, so the most trusted
process parsed HTTP. It is now one program, `jig`, with apt's verbs where apt has one, an installer
endpoint only the owner's console grants (§270), and grants chosen by verb. It fetches, so
`Activation::Fetch` and `init`'s `http_response` exception are gone. `update` and the index copy
are built (milestone 801); `list -u`, `add-repository` and item 8's index half are left.
