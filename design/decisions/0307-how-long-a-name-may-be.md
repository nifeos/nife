---
status: AMENDED
raised: 2026-10-17
decided: 2026-10-18
ratified_by: calef
---

# 307. How long a name may be: one bound for programs, packages and the files they travel in

*(Section number provisional until the merge queue lands it.)*

calef, 2026-10-17 (UTC): *"32 bytes is probably too short for packages anyways."* A program, a
package and the files they travel in share one name bound, and both formats that carry a name store
it length-prefixed. This changes formats that nife's kernel and tools, `jig` and basalt's published
packages share, so it was ruled before any code.

## The rules

1. **A package's file name, `<name>-<version>-<architecture>.jig.tar.zst`, is at most 252 bytes as
   a whole** (calef: *"252 seems fine for now."*). Each field is bounded by the file name, not given
   its own limit. A program name, which is its own file under `bin/`, is at most the same 252.
2. **The bound is the smallest name limit among the filesystems nife installs packages onto.** A
   filesystem nife only reads, such as ISO 9660, does not count. Today that set is redoxfs, whose
   `DIR_ENTRY_MAX_LENGTH` is 252: a 4-byte node pointer and 252 name bytes make its 256-byte entry.
   Each writable filesystem crate exports its maximum name length, and a test fails if any is below
   the bound. The bound rises only when that test allows it.
3. **Names are variable width** (calef: *"variable-width"*): a name costs what it uses.
   - A `nifefs` entry is a one-byte length, the name, then `start_block` and `len` as `u32`s.
     Entries are walked, not indexed. The header's `count` keeps a ceiling, so a hostile header is
     refused before anything is multiplied by it, and the directory is sized in bytes.
     `ENTRIES_IN_FIRST_BLOCK` becomes the entries that end inside block 0, found while walking,
     which is what the EL0 block driver in `crates/virtio` reads.
   - A package is §197 (a package is one archive file)'s tar, a jig package. A member name is a
     path, held to ustar's fields until pax `path` support lands (the amendment's rule 1). The
     package's name, version and architecture are lines of its `package_measurements` member.
4. **One constant, in a crate both formats import** (calef: *"Yes"*). `package_archive` does not
   depend on `nifefs`, so neither owns it; the crate holding it is named by the build lane. Every
   ceiling that holds a program or package name reads it: `HOLDER_MAX`, the timetable's `NAME_MAX`,
   swish's installed path and `Label`, and `STEM_LEN`. basalt's `helpers/packages.py` reads nife's
   value instead of restating it. The log registration's 32-byte `NAME_MAX` is a wire field and
   widens with its protocol. `documentation`'s 72-byte path is not a package name and stays.
5. **`jig install` refuses a package name longer than `jig remove` can carry.** The per-file grant
   name, `filesystem_protocol::grant::MAX_NAME`, is 16 bytes because it rides in two `START` words,
   and lifting it is its own milestone. Until that milestone lands, install checks the same 16
   bytes, so nothing is installed that remove cannot remove. All six of basalt's packages fit; the
   longest is `hackbench`, 9 bytes.
6. **The format changes visibly, and `jig` refuses the old one.** `nifefs` is `CRKR0003`, for §58
   (a wider archive name)'s reason: a reader striding the old layout would find a plausible name
   at the wrong offset. A package becomes §197's zstd-compressed tar, which no `NIFEPKG1` reader
   mistakes for its own. Once the migration's step 3 lands, `jig` refuses `NIFEPKG1` as an unknown
   format. No installed system holds a `NIFEPKG1` package.

## The migration, in order

nife and basalt pin each other. nife's CI installs basalt's packages: `script/ci-build`'s rows run
`cargo xtask basalt-fetch` and `jig install ripgrep` from the release `xtask/basalt-pins.index`
pins, and in CI a failed fetch fails the row. basalt builds its packages with the pinned nife's
`cargo xtask package`. So:

1. nife, one pull request. The shared constant and rule 2's test. `nifefs` at `CRKR0003`, with
   every reader in the same change (kernel, `virtio`, `sealed_pair`'s scan, xtask, the fuzz targets
   and the Kani harnesses), since images regenerate from the tree. `package_archive` writes
   §197's tar, `cargo xtask package` compresses it with zstd, and the readers accept `NIFEPKG1` too. The `NIFEPKG1` reader exists only to break the
   cycle: nife's CI must still install basalt's pinned packages until basalt has rebuilt them with
   this commit. It is not compatibility for users. Rule 5's check lands here too.
2. basalt. Bump the nife pin to step 1's merge commit, cited as `nifeos/nife@<sha>`. Rebuild
   every package, rewrite every recipe's digest table with `--update-digests` (milestone 917
   (basalt's packages build reproducibly)), drop the hand-copied `NAME_LEN`, and publish a new
   Release. Every digest changes, and the pull request says why.
3. nife. Copy basalt's new index lines over `xtask/basalt-pins.index` and delete the `NIFEPKG1`
   reader in the same pull request, so the window closes in the change that ends its reason.
4. basalt. Re-pin nife to step 3.

basalt's earlier `rolling-*` Releases stay published and unchanged.

## Amendment, 2026-10-18: a package is a jig package, a zstd-compressed tar

calef, 2026-10-18 (02:16 to 02:35 UTC), on the measurements in
[notes/packages/container-format-measurements.md](../../notes/packages/container-format-measurements.md):
*"tar with a manifest"*, and on compression, *"Yes"*. The format is a jig package, named for its
client as RPM and apk are (*"Yes"*), in a file `<name>-<version>-<architecture>.jig.tar.zst`.

1. The container is ustar of regular files, read by upstream `tar-no-std` unpatched (*"Yes"*),
   with `package_archive`'s Kani harnesses proving properties through its public API. The build
   and the installer refuse a member path over 255 bytes split at a `/`, or a last component over
   100. That is rule 1's recorded exception, as rule 5's 16 bytes is, until pax `path` support
   lands as its own milestone. That milestone offers `tar-no-std` a patch first, and forks or
   writes a reader only if it is refused or Kani cannot prove it.
2. The first member is `package_measurements`, pairing with the boot archive's
   `program_measurements` (*"Yes"*). Its first line names the format and its version, for example
   `jig-package 1`, then identity lines, then one `<path> sha256:<hex>` line per member: the
   catalog-line shape `measured_boot` parses (calef: *"I believe we already debated the
   catalog-line format and aligned on what we have."*). Nothing else identifies a jig package: no
   pax global header and no zstd skippable frame (*"Yes"*).
3. The file that travels is that tar compressed whole with zstd. `jig` decompresses it at
   install. The compressor's version, level and flags are pinned, because the same input at the
   same level gave different bytes from zstd 1.5.5 and 1.5.7, and milestone 917 (basalt's packages
   build reproducibly) needs identical bytes.
4. §195 (a reviewed recipe vouches for a package)'s digest covers the compressed file as
   fetched, and `jig` checks it before decompressing.
5. The installer reads the uncompressed tar once. `system_initializer`'s activation step parses
   it at install, and `place_program` writes each program to `packages/<name>/<version>/<program>`.
   At spawn the progenitor reads that placed file and looks its digest up in the activation set
   (§208 (installing a package is granting it)), never the tar.
6. The installer checks a digest of the uncompressed tar. The recipe and the index carry it
   too, and the activation step checks it before parsing, never trusting
   `jig`'s check (*"Yes"*). The fetcher and the granter are separate parties, so the granter
   verifies what it grants, as containerd checks an image's uncompressed `diff_ids`. And it does
   not move with the compressor, as Nix signs `NarHash` and RPM 4.16 carries `PAYLOADDIGESTALT`.

zstd over gzip and xz, which stock macOS and Ubuntu read unaided, was kept. calef: *"It seems like we have a much smaller sample size than the distributions that moved to
zstd. And they moved there for a reason."* On basalt's 18 packages, level 19 gives 35.1% of their
uncompressed size.

basalt's package blobs gain an OCI registry location once nife's OCI registry client exists
(calef: *"We will be delivering container serving on nife"*, then *"Yes"* to this order). That
client is milestone 621 (a Linux persona server, confined)'s OCI import shim, or whichever
container-serving milestone builds it first.

- The registry is one more listed location per entry under §250 (an image names its
  distribution's package index), with GitHub Releases as the fallback. The TUF index stays on its
  host, pinned to ISRG Root X1.
- A blob is trusted by its §195 digest, which is its OCI layer digest unchanged
  ([notes/packages/oci-artifacts-measurements.md](../../notes/packages/oci-artifacts-measurements.md)).
- A third-party container registry is an added source, checked against §306 (the web's authority
  list is a package and a capability)'s list under its rule 5.

Why: nife carries an OCI client for container serving regardless, so this costs little, and every
`jig install` exercises the container path, as nife's CI installing ripgrep from basalt exercises
packages. JSON is not a cost, since TUF brings it. What remains is a token request before each
anonymous ghcr.io pull and one or two more connections.

Still calef's, waiting on milestone 621: which root authenticates a registry location, since §250's
clause 11 takes it from the image's roots and ghcr.io chains to Sectigo R46.

## Why these

One file name, not three fields. Bounding each of name, version and architecture at 255 would
permit a 775-byte file name that no filesystem in this tree or on Linux can hold. The real limit
would be the filesystem's anyway, met at install as an `EINVAL`. Bounding the file name is one fact
checked in one place: the file fits. That is also the only bound Debian has in practice, and it
comes from Linux's `NAME_MAX`, not from Debian's packaging rules (see prior art).

252, not Debian's 255. A 253-byte file name would be valid in the format and unstorable on
nife's own disk. Patching the vendored redoxfs to 255 would make its directory entry 259 bytes,
change its on-disk format and stop it reading upstream's images. Declaring 255 in the format and
enforcing 252 at install is two numbers for one fact. Rule 2's test is what lets the bound move when
the set of writable filesystems changes.

Variable width. Fixed 252-byte fields cost every entry of every image and archive the full
width; a length prefix costs what a name uses.

Measured on 2026-10-17 (UTC) against the 131 programs that the three architectures' initial
ramdisks pack between them, mean 13.3 bytes, longest 27. They are every `[[bin]]` in `components/`
and `fixtures/`, plus the `std`, farm, basalt and file-server programs `xtask/src/archive.rs` adds. For packages, the 18 `.nifepkg` assets on basalt's latest release
(`rolling-2026-10-17-155eba5`), 2 to 5 members each, longest file name 30 bytes.

nifefs directory (512-byte blocks; a variable entry is a one-byte length, the name, then the
two `u32`s, 9 bytes plus the name):

| layout | directory bytes for 131 files | blocks | entries in block 0 | files in today's 14 blocks |
|---|---|---|---|---|
| fixed 32 (today) | 5,252 | 11 | 12 | 178 |
| fixed 252 | 34,072 | 67 | 1 | 27 |
| fixed 255 | 34,465 | 68 | 1 | 27 |
| variable, `u8` length | 2,931 | 6 | 22 | about 320 at today's mean, 27 at worst |
| variable, every acronym expanded | 3,095 | 7 | 22 | about 300 |

Holding today's `MAX_FILES` of 178 at a fixed 252 takes 91 directory blocks, 46 KB.

Package header and table of contents (a variable field is a `u16` length and its bytes):

| layout | header + table, 2 members | 3 members | 5 members | cost on the smallest package (21,292 bytes) |
|---|---|---|---|---|
| fixed 32 (today) | 256 | 328 | 472 | |
| fixed 255 | 1,371 | 1,666 | 2,256 | +6.3% |
| variable | 142 to 153 | 204 to 222 | 319 to 320 | smaller than today |

Variable width is smaller than today's fixed 32 in every measured case, and fixed 252 is 6.5
times the directory for nothing. It is the more work of the two, so effort did not choose it.

What else was considered for the migration: one atomic nife change with no `NIFEPKG1` reader,
letting the basalt fetch rows fail until basalt republishes. Refused, because it turns a gate off for
a cross-repository round trip, and the failure it would hide is the one this migration can cause.

## The formats this replaces

Before this decision is built, two on-disk formats carry a name in a fixed 32-byte NUL-padded field,
and smaller ceilings sit beside them. Every one, found by search on 2026-10-17 (UTC):

| where | bound | what it bounds | kind |
|---|---|---|---|
| `crates/nifefs` `NAME_LEN`, magic `CRKR0002` | 32 | an archive entry, so every program the kernel or the progenitor spawns by name | on-disk, fixed; `ENTRY_LEN = 40`, `MAX_FILES = 178` in 14 directory blocks |
| `crates/package_archive` `NAME_LEN`, magic `NIFEPKG1` | 32 | package name, version, architecture, and every member | on-disk, fixed; header 112 bytes, member entry 72 |
| `package_archive::PROGRAM_PREFIX` | 28 | a packaged program, since its member is `bin/<program>` in a 32-byte field | derived |
| `package_archive::STEM_LEN`, `swish` `bare.rs` `PATH_MAX` (108), its `Label` (32) | 98, 108, 32 | the `name-version-arch` stem and `/packages/<n>/<v>/<a>` | runtime buffers |
| `filesystem_protocol::grant::MAX_NAME`, `grant_plan` `MAX_FILE_NAME` | 16 | a per-file grant's name, and so `jig remove <package>` and the Remove and Vouch messages | wire: two `START` words |
| `grant_plan::spawnproto` `HOLDER_MAX`, `system_log_protocol` `control::NAME_MAX`, `components/src/timetable.rs` `NAME_MAX` | 32 each | a NameTaken holder, a log registration, an installed program in store mode | wire or runtime |
| `crates/documentation` `PATH_MAX`, magic `CRKRMAN1` | 72 | a manual page's path | on-disk |
| basalt `helpers/packages.py` `NAME_LEN` | 32 | `doc/` member names, a hand copy of nife's value | basalt's own check |
| `vendor/redoxfs` `DIR_ENTRY_MAX_LENGTH` | 252 | a file name on nife's general filesystem | on-disk: a 4-byte node pointer and 252 name bytes make a 256-byte entry |
| `redoxfs_server` `NAME_MAX`, std's `nife` overlay | 4096 (one page) | a name or path on the filesystem request path | runtime |
| `filesystem_protocol::dirent` | 255 | a READDIR record's one-byte length | wire |

`activation_set` and `manifest_note` assume no name width (`manifest_note`'s 8-byte field holds only
the note owner `nife`). U-Boot's 32-byte image name and GPT's 36-unit label are foreign formats and
do not move.

## What it unblocks

Measured with a flat rule that spells out every acronym in a name (`abi`, `cpu`, `elf`, `fs`, `gpu`,
`http`, `os`, `pci`, `tls`, `uefi`, `usb`, `uuid`; `mdr` as `markdown_renderer`). The list was taken
from the names' own tokens; foreign program names (`rg`, `mkfs`, `ps`) keep their spelling, as the
acronym test already says. The flat rule is decided separately; this records what it costs.

| set | names | over 32 today | over 32 expanded | over 28 expanded (`bin/` today) | longest expanded |
|---|---|---|---|---|---|
| programs the images pack | 131 | 0 | 4 | 6 | 41, `pinned_transport_layer_security_exerciser` |
| every program in the tree | 132 | 1 | 7 | 9 | 44, `unified_extensible_firmware_interface_loader` |
| crates (Cargo's own limit, not nifefs's) | 124 | 2 | 8 | 12 | 44 |
| packages | 38 | 0 | 0 | 0 | 17, `markdown-renderer` |

The four image programs it unblocks: `usb_keyboard_driver` (36), `current_cpu_reader` (38),
`os_primitives_benchmarker` (39) and `pinned_tls_exerciser` (41). `gpu_driver` and `uuid` land at
29 to 32, which fits the archive and not a package's `bin/` member. Nothing expanded passes crates.io's 64.

## Prior art

Read on 2026-10-17 (UTC) from each project's own source, not recalled.

| ecosystem | bound | what is bounded | source |
|---|---|---|---|
| Debian Policy | none (at least 2 characters) | package, version, architecture | Debian Policy sections 5.6.1 and 5.6.7 |
| dpkg | none | `pkg_name_is_invalid` checks emptiness, first character and charset only | `lib/dpkg/parsehelp.c` |
| dak (ftp-master) | none | `re_field_package` is unbounded | `daklib/regexes.py` |
| lintian | 80, a warning | the whole `.deb` file name, with the longest architecture substituted (`package-has-long-file-name`) | `lib/Lintian/Check/Archive/File/Name/Length.pm` |
| crates.io | 64 characters; version 150 | name | `crates_io_validation/src/lib.rs` |
| npm | 214 | name, for new packages | `validate-npm-package-name` `lib/index.js` |
| RubyGems.org | 255 | name | `app/models/rubygem.rb` |
| Nix | 211 | store path name | `libstore/include/nix/store/path.hh` (`MaxNameLen`) |
| PyPI, pacman, RPM, Go modules | none found | name | warehouse `models.py`; `pkgname.sh.in`; `parsePreamble.cc`; `module/module.go` |
| Linux, ext4, Btrfs | 255 bytes | a file name | `NAME_MAX`, `EXT4_NAME_LEN`, `btrfs_tree.h` |
| APFS | 255 characters, not bytes | a file name | measured on macOS: 255 ASCII accepted, 256 refused, 255 three-byte characters (765 bytes) accepted |
| redoxfs upstream | 252 bytes | a file name | `src/lib.rs` `DIR_ENTRY_MAX_LENGTH` |

So Debian's 255 is a filesystem's limit and not a packaging rule; the only packaging check on the
file name is lintian's warning at 80.

What Debian actually uses, from sid main amd64's `Packages.xz` fetched 2026-10-17 19:36 (UTC),
76,655 binary packages:

| field | median | p99 | p99.9 | longest |
|---|---|---|---|---|
| package name | 17 | 40 | 49 | 75 |
| `.deb` file name | 36 | 66 | 77 | 102 |
| version | 8 | 28 | 37 | 52 |

Four binary package names pass 64, so crates.io's cap would refuse real Debian packages; none passes
128. 252 leaves Debian's longest file name more than twice its length in headroom. Not checked:
Homebrew, Alpine's apk and abuild, Cargo's local `restricted_names.rs` (searched, not audited), and
Fedora's packaging guidelines as distinct from `rpm` itself.

## BUGS

- The measurement counts names in the tree on 2026-10-17 (UTC). The per-architecture archives pack
  subsets of the 131, so the directory figures are an upper bound for any one image.
- APFS's limit was measured on one macOS host; no Apple document stating it was found.
- Rule 5 makes `jig install` refuse names `jig remove` cannot carry, which caps an installable
  package name at 16 bytes until the grant milestone lands.
