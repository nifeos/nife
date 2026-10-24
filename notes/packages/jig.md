# `jig`, the package manager

The appendix to [notes/packages.md](../packages.md) for milestone 809 (the package client becomes a
program), built 2026-10-10 (UTC). `jig` installs, removes and rolls back packages and lists what the
image vouches for. The progenitor stays the installer; `jig` asks it.

## EXAMPLES

From `script/swish-check`'s first boot, on all three architectures:

```
$ jig list
  packages this image vouches for (its catalog; no index yet):
  greeting 0.1.0
  ...
$ cd downloads
$ jig install ./noteless.nifepkg
  installed; generation 1 is live
$ cd /
$ jig install nosuch
  refused: this image's catalog names no such package, so nothing was fetched
$ jig install greeting@0.1.0
  fetched and installed; generation 2 is live
$ installed/jig rollback
  this jig holds no installer endpoint; only the owner's console grants one
```

From the second boot, after `update` (milestone 801 (packages over the internet), 2026-10-12 UTC),
riscv64's transcript:

```
$ jig update
  index updated: 4 packages for riscv64, from basalt.test over TLS
$ jig list
  packages the index offers (the copy jig update wrote):
  greeting 0.1.0
  rebound 0.1.0
  uptime 0.1.0
  greeting 0.3.0
$ jig install uptime
  passed over https://packages.basalt.test:8443/rolling/targets/uptime-0.1.0-riscv64.nifepkg: not resolved: NotFound
  refused: the index does not vouch for the bytes that arrived; nothing was sent to the installer
$ jig install greeting@0.3.0
  passed over https://packages.basalt.test:8443/rolling/targets/greeting-0.3.0-riscv64.nifepkg: not resolved: NotFound
  fetched greeting-0.3.0-riscv64 from basalt.test over TLS
  fetched and installed; generation 6 is live
```

`jig list -u`, typed after that `update` and before `greeting` 0.3.0 is installed, compares the
live generation, which holds `greeting` 0.1.0 and 0.2.0, with the copy:

```
$ jig list -u
  installed packages the index offers newer (the copy jig update wrote):
  greeting 0.2.0 -> 0.3.0
$ jig list --upgradable
  refused: jig spells that list -u; it takes no long form
```

Newer is Debian's order, dpkg's `verrevcmp` (calef, 2026-10-18 UTC: "Debian dpkg order"), in
`package_index::version`: `1.10` is newer than `1.9`, and `1.0~rc1` older than `1.0`. A version
dpkg would refuse or warn about (an empty one, one not starting with a digit, or one with a
character outside `0-9 A-Z a-z . + ~`) refuses an index at `update`, and an installed row's at
`list -u`, naming it.

The image's catalog names no `greeting` 0.3.0, so the installer took it on the index copy's word.
The listed location is passed over because this prompt cannot resolve it; the repository's own
copy is the fallback.

The verbs are apt's where apt has one; [the record](../../design/naming/command-line-rulings.md#jig-takes-apts-verbs)
has each spelling. A word with a `/` is a file, so a package file in this directory is `./<file>`.
It is installed from the directory it is in, because a word is designated at the current directory,
and the root cannot be narrowed to one name.

## What it holds

`caps jig install greeting@0.1.0` prints it. A `std` program, so the slots are the `std` layout's:

| slot | what | from |
|---|---|---|
| 2, 3 | the network stack, and its own region for each socket's page | `Manifest::network` |
| 8 | its words | `ProgramArgumentDeclaration::Words` |
| 15 | the installer endpoint, `WRITE` | `Manifest::installer`, §270 (a package manager holds an installer endpoint, not the spawn endpoint) |
| 16 | the endpoint its answers come back on, `READ` | the same |
| 17, and a page | a copy of the image's catalog | `Manifest::catalog` |
| 4 | the file a word names, read-only | the line, for `install <path>` |
| 4 | the index store, `activation/indexes`: read and write for `update`, read for `list` and `install <name>` | `Manifest::indexes`, on a line that names no directory |
| 4 | `activation/`, read only, for `list -u`: the live generation and the index copy beneath it | `Manifest::activation_directory`, in the index store's place |
| 18, and a page | the image's index sources: where its index is, and the root its TLS must chain to | `Manifest::indexes` |
| 5, 6 | the clock and entropy, for TLS | `update` and `install` |

Only the owner's console grants the index store, as it grants the installer, and no note may ask for
it. The image's index sources are a measured archive entry (`package_index::SOURCES_ENTRY`), so
basalt can name another index without rebuilding `jig`.

**Each line holds what its verb needs and no more** (option V2, milestone 281 (`watch` holds
exactly what `ps` holds)'s test): `list` the catalog alone, `remove` and `rollback` the installer
alone, `install` all three. `grant_plan::narrow_by_verb` decides it from `grant_plan::Program::verbs`.
The shell narrows for `caps` and the progenitor for the endowment, each reading the line's second
word. The progenitor reads it off the argv page and refuses the spawn if the child's copy says
otherwise.

## `update` and the index copy

The image names its index's addresses and the root their TLS must chain to (`package_index::Sources`,
at `std_runtime_contract::INDEX_SOURCES_PAGE`). This is item 6 of milestone 809. `update` fetches the
index from the first address that answers, pinned to that root, and keeps this architecture's lines.
It writes them to `package_index::IMAGE_INDEX_COPY` in the index store, the directory the owner's
console grants at `std_runtime_contract::FS_DIR_SLOT`.

After that, a name is looked up in the copy first. The package is fetched from the entry's locations,
or from the repository's own `targets/`, over HTTPS, and judged by the entry's digest. The progenitor
installs it on the same copy's word (ruling I2). Before any `update`, a name is looked up in the
image's catalog and fetched from the LAN package source, as since milestone 198 (a package
manager)'s rung 3a. A command word with a `/` in it is a file, by the prompt's rule for a command
word (§219 (how the shell names an installed program to the spawner)).

## How an install travels

`jig` reads the package into one run of its own pages: from the file, or from the package source
over plain HTTP (`http_response`). A fetched package must then be the one asked for, and the catalog
must vouch for its digest (`package_archive::installable_as`), or nothing is sent.

Then `spawnproto`'s activation request goes out on the installer endpoint, unchanged from the
builtin's: three words, one `READ` frame per page, one answer. The frames are slices of the run
(`abi::page_frame::SLICE`), so the buffer and the frames are the same memory. The progenitor copies
each page before it hashes, checks its copy against its own catalog, places the program and writes
a generation. The row records `jig` in its manager column.

The run needs room for a package beside the heap, so a program declaring the installer is built in a
region one package larger, from the progenitor's image pool (`grant_plan::named_std_region_pages`).

## What moved

- Out of the shell: `Command::Package`, `PackageVerb`, `package_verb`, `PACKAGE_USAGE` and the
  builtin. `vouch` stays a builtin, on the spawn endpoint, which the installer endpoint is not.
- Out of the progenitor: the fetch, `Activation::Fetch` and its three statuses (holes in the wire
  now), and `http_response`, with `init`'s package exception for it.
- Into the installed-programs table: a manager column (`installed_programs::Entry::manager`). A row from before it
  reads as `-`.
- Into the progenitor's stack: two pages, to fourteen (`kernel::user::PROGENITOR_STACK_PAGES` has the
  measurements).

## A rehearsal with basalt's sources

No check boots the production sources, so the bench evening's step 3 (`notes/xenon-bench-2026-10-18.md`)
is rehearsed by hand under QEMU's x86_64 UEFI machine, from an installed disk, against the live
`basalt.nifeos.org`. Under slirp the lease's name server forwards to the host's, so here the
index host is resolved by name.

```sh
cargo xtask std-exerciser
NIFE_INDEX_SOURCES=basalt cargo xtask install-boot     # install-boot: PASS
# then boot target/nife-install.img with the stick detached, and type at `$`:
#   jig update; jig install ioping; cd packages; ioping -v
```

On 2026-10-24 (UTC), against `rolling-2026-10-24-87a3b29`:

```text
$ jig update
    index updated: 6 packages for x86_64, from basalt.nifeos.org over TLS
$ jig install ioping
    fetched ioping-1.3-x86_64 from release-assets.githubusercontent.com via github.com over TLS
  fetched and installed; generation 1 is live
$ cd packages
$ ioping -v
  ioping 1.3
```

The Release hop works only while GitHub's `302` head fits `http_response::REDIRECT_HEAD_MAX`,
32 KiB. It measured 10,806 bytes on 2026-10-24, most of it one `content-security-policy` line.
When it does not fit, `jig` prints `passed over https://github.com:443/...: Http(HeadTooLong)` and
installs from the Pages copy. The ignored test `a_release_download_is_e46_then_one_redirect_to_x1`
(`pinned_tls_client/tests/peer.rs`) checks the same hop from the host.

## BUGS

- GitHub sets the size of its redirect's head, and no gate sees it grow, because a check may not
  depend on a host outside the runner. If it passes 32 KiB, `jig` passes over the Release with
  `Http(HeadTooLong)` and installs from the Pages copy, uncounted. Running
  `cargo test --test peer -- --ignored` in `pinned_tls_client` by hand is the check.

- **`jig` is inside the trusted base for installs** until the index carries a signature the
  progenitor checks (option I3 in milestone 809's block). The progenitor installs whatever the index
  copy vouches for, and `jig` writes the copy from a reply it parses first, so a defect in `jig` or in
  the index host's certificate becomes a defect in what runs. An owner's yes that `jig` collects for
  a grant (milestone 892 (a std program at the prompt holds the network and a name resolver)) is inside the same base. A compromised `jig` already holds the network,
  so that adds only a grant that outlives it.
- Before any `update`, `list` reads the image's catalog, and a name is fetched over plain HTTP
  from the LAN source. The catalog and the index copy are each one page, so `xtask` refuses a
  catalog of a page or more and `update` refuses an index whose lines for this architecture are.
- An image names the test peer and root (`xtask`'s `image_index_sources`), reached at 10.0.2.9 by
  the sources' address, unless it is built with `NIFE_INDEX_SOURCES=basalt`, which packs
  `package_index::BASALT_SOURCES` (`basalt.nifeos.org` pinned to ISRG Root X1). No check boots
  the production text, since a check may not depend on a host outside the runner.
- A listed location is fetched through `pinned_tls_client::listed`: the root the index names for it
  among those the image carries, then at most one redirect. The redirect is held to the
  public-address check and to the index's root, per the third amendment of §250 (an image names its
  distribution's package index). Under the test sources no listed location resolves at the prompt,
  because slirp asks the host's resolver, which knows no `.test` name, so the boot check never
  fetches from one. It does require that the name reaches the name server
  (`resolver_refusal_failure` in `xtask/src/swish_check/boot.rs`), and `package_fetch_exerciser`
  proves the fetch, calling the same function.
- `update` keeps no record of which address answered, and `install` tries the image's addresses
  in order again.
- The index store needs a file server that binds grants itself; on one that does not, `jig` is
  not granted it and says so.
- Its own refusals name no generation, and a `remove <package>@<version>` it cannot pick names no
  candidates. Only `list -u` holds a view of `activation/`.
- `list -u` compares versions only: an entry whose digest changed at the same version is not
  reported.
- Before `update`, the package source is the runners' fixture peer, plain HTTP; outside QEMU there
  is none.
- Nothing gates its dependencies to the public contracts the block names; `jig/Cargo.toml` lists
  them with that argument.
- A copy run by path carries a note that asks for less than the image's `jig` holds, so it lists
  nothing and installs nothing. That is the point of the `installed/jig` line, and also all a copy
  can do.
