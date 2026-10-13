---
status: DECIDED
raised: 2026-09-19
decided: 2026-09-20
ratified_by: calef
---

# 197. A package is one archive file, named and vouched for by its recipe

calef, 2026-09-20 (05:14 UTC), after comparing the container against what apt,
Homebrew, Alpine, Haiku and Nix actually ship: *"C2 seems like the right shape given the
comparisons."* *(Section number provisional until the merge queue lands it.)*

The ruling. A package is **one archive file per package**, the shape apt (`.deb`), Homebrew (a
bottle), Alpine (`.apk`) and Haiku (`.hpkg`) all use. It is identified by name and version, and
DECISIONS §195 (a reviewed recipe vouches for a package) decides whether its bytes may run: the
recipe carries the digest, per source, and the machine's owner may overrule. **Homebrew is the
worked example of exactly this pairing**, a tar bottle whose SHA-256 lives in a human-reviewed
formula, which is what made C2 a live option rather than the heavyweight one.

What it costs, stated because it is the reason the other two existed. A container is bytes a
target must parse, and parsing bytes we did not write is on this tree's hostile-input path: the
reader owes a fuzz target and the Kani treatment `crates/nifefs` and `crates/elf` already carry.
That cost is now accepted rather than avoided.

What it buys, and why the alternatives lost. C1 (members of the boot archive) is what the tree
does today and is not a format at all; it cannot serve DECISIONS §159 (lab machines upgrade like
user machines), because under it installing is something the *build* does and every upgrade is a new
image. C3 (content-addressed blobs) avoids the parser and makes identity and integrity one fact, and
it loses the thing a person and a repository both want: `rg 14.1` as a name. §195 already answered
"who says these bytes are rg 14.1" with a reviewed recipe, so C3's advantage was smaller than it
looked when the options were written.

## Still calef's, and narrowed by this ruling

- Where a program's manifest travels. Ruled 2026-09-26: M2, recorded in the next section.
- The digest's shape. Ruled 2026-10-07: C, a plain SHA-256 labeled `sha256:`, recorded in the
  section after the manifest's.
- Activation (what installing *does*) is untouched by this and is its own ruling. It has
  since been ruled: §208 (installing a package is granting it), 2026-09-23.

## Where the manifest travels: M2, ruled 2026-09-26

calef, 2026-09-26 (UTC), on the evidence in draft pull request #1319 (the price of an ELF-note
manifest): *"M2 is right."* A program's manifest travels inside its executable, as an ELF note
found through a `PT_NOTE` program header. Under §219 (how the shell names an installed program to
the spawner) option D, the unit the system handles is the executable's bytes. The manifest rides in
the only thing that is hashed, handed over and run, so it cannot drift from the code or be separated
from it. That holds for an installed program, a fresh build and a foreign one alike. M1's one
remaining advantage, changing a manifest without relinking, is an effort argument, and #1319
recorded it as one.

What #1319 measured, on patagonia with Kani 0.67.0 and rust-lld 23.1.1:

- `crates/elf` stays program-headers-only. The reader is 113 code lines with three Kani harnesses
  (about 6.4 s together), and the `elf_parse` fuzz target now calls it. `Elf::parse` is unchanged,
  and the kernel never calls the reader.
- `crates/user_mode_runtime/link.ld` discards `*(.note*)` today. Two lines give aarch64, riscv64
  and x86_64 each a `PT_NOTE`.
- The note survives `llvm-strip --strip-all` and `llvm-objcopy --strip-sections` on all three.
- A foreign build links a note object through `-Clink-arg`, with no change to its source.
- `objcopy --add-section` does not work. It adds a section with no program header, which a
  program-header reader cannot see.

Prior art on the same side: Apple's entitlements, carried in the code signature; freedesktop's UAPI.8
package-metadata note; FreeBSD's `elfctl` feature-control note. Fuchsia and Android chose a sibling
file, because their unit is the package. nife's unit is the executable, which runs unpackaged.

Still open, and an architect's, because two programs agree on each of them: the note's owner string
(#1319 proposes `nife`; nothing has chosen it), its type number, and the descriptor's encoding. The
build is filed as the proposal
[`a-program-carries-its-manifest-in-an-elf-note`](../roadmap/0597-a-program-carries-its-manifest-in-an-elf-note.md),
since promoted to milestone 597 (a program carries its manifest in an ELF note).

Correction, 2026-09-26. The M2 row below says M2 "extends `crates/elf`, which parses program
headers only", and "reluctantly" rested on that. So did `notes/component-manifest.md`, which said
reading a note meant teaching the parser section headers. The premise joined two mechanisms. A
`PT_NOTE` is a program header, found in the table the crate already walks. A section-header reader
would in fact lose the note after `--strip-sections`. The row is left as calef ruled on it.

## The digest's shape: C, ruled 2026-10-07

calef, 2026-10-07 (UTC): *"C"*. A package's digest is a plain SHA-256 over the package file,
written with its algorithm as a label in the container-image convention, `sha256:<hex>`. A Merkle
root can then be added later under its own label (for example `merkle-sha256-8k:<hex>`), beside
the plain digest rather than as a migration.

Refused, with the reason for each:

- A, a plain SHA-256 unlabeled. It leaves a format change waiting for the day a demand-paging
  loader needs a Merkle root, and on that day every catalog and recipe already written is in
  the old format.
- B, a Merkle root now. It builds for a loader that does not exist (today's loader reads every
  binary whole), adds code and proofs to the trusted base, and stops a stranger checking a package
  with `sha256sum`.

Why it is cheap now: nothing is installed anywhere and no package has left this machine. After
milestone 801 (packages over the internet), published digests live in other people's catalogs.

What lane/digest-label built on it, 2026-10-07:

- One formatter and one parser, in `crates/measured_boot` beside the hash they label
  (`digest_text`, `parse_digest`; names provisional). Every text that carries a digest uses them:
  a recipe's `digest`, a catalog line, the measurement table the trust root is generated from
  (it shares the catalog's format and parser), an activation-set row and pointer, and the
  shell's `caps` provenance line. Bare hex is refused as `Unlabeled`, any other label as
  `UnknownLabel`, so an old line or a newer format is named rather than compared. Two Kani
  harnesses prove the round trip for every digest and that nothing but `sha256:` and 64 hex
  characters parses.
- The in-memory `Digest` stays 32 bytes with no algorithm in it. A Merkle root, when it comes,
  is a second fact beside the whole-file hash, checked differently, so it gets a type of its own;
  an enum with one variant now would let one reach a check that only knows how to hash a whole
  file. `measured_boot`'s `Digest` documentation carries the argument.
- The kernel's bare-hex diagnostics and `sealed_pair`'s, which a person compares against
  `shasum -a 256` and nothing parses, are unchanged.

## The proposal as calef ruled on it

## What is being decided

Three things, which are separable and should be ruled separately if calef prefers:

1. The container. What bytes a package is: members inside an archive the target already
   parses, one new archive file, or a content-addressed set of blobs named by digest.
2. Where a program's manifest lives. Beside the binary as its own member, or inside the ELF.
   `notes/component-manifest.md`'s `BUGS` already records this as an open wire-format question and
   declined to decide it "until a second supervisor or an out-of-tree component actually exists".
   A package is that trigger.
3. What the metadata must carry. Derived below from what the tree already needs, not invented.

## The constraint every option is checked against

nife cannot build software. A package is produced on a host (macOS or Linux) by a Rust
cross-toolchain and consumed by a target with no compiler, no allocator in the loader, and a
Kani-proven parser policy for bytes it did not write (`crates/elf`, `crates/nifefs`, the GPT and
device-tree parsers, all fuzzed by `script/fuzz`). So:

- **Per-architecture output is the normal case.** Three triples, three binaries. Genode keeps
  `bin/<arch>/<name>/<version>/` beside architecture-independent `raw` archives; Nix gives a
  cross-built output its own store path. Both read, cited below.
- **The target-side reader is hostile-input code on the boot or install path**, so every option's
  reader cost is a new fuzz target and, by this tree's habit, Kani harnesses.
- **Anything borrowed from a self-hosting system has to survive having no builder on the target.**
  Haiku's packages are routinely built on Haiku; its cross-building exists for bootstrap only
  (read: haiku-os.org, `docs/develop/packages/Bootstrapping.html`). Fuchsia builds everything on the
  host and ships assembled bundles (read: `fuchsia.dev/.../software_assembly/overview`), which makes
  it the closest match on this axis as well as on capabilities.

## What the metadata must carry, from what the tree already asks for

| Field | Who needs it today | Source |
|---|---|---|
| Name, at most 32 bytes | `nifefs::NAME_LEN` is 32; the longest archive entry today is 25 (`os_primitives_benchmarker`), measured from `target/initrd-riscv.img` | `crates/nifefs` |
| Architecture | Three triples; one ELF per triple | DECISIONS §19 |
| SHA-256 of each member | The progenitor refuses anything the measurement table does not vouch for | milestone 104, `measured_boot::verdict` |
| The program manifest (`grant_plan::Manifest`) or component requirements (`component_plan::Requirements`) | What a grant is checked against before spawn; today compiled in, keyed off the closed `Prog` enum | milestone 31, milestone 23, milestone 47's `PATH` section |
| `Requirements::pages` | "a property of the build, not of the contract", and "the strongest single argument for the wire format" | `notes/component-manifest.md` `BUGS` |
| License | "Each packaged program's licence is recorded where a reader meets the program" | DECISIONS §135 (running GPL software is aggregation), requirement 2 |
| Documentation bundle and index shard | "installed by the package that owns it"; the index is "a merge of shards, one per installed package" | milestone 40 |
| Version | Only when a package built at one commit meets a system built at another; see the sibling gate proposal | `design/what-a-distribution-packages.md` |
| Contract versions it speaks | Same trigger; the `fs_proto` first word is full, so this is a connect-time handshake and a separate protocol decision | same note |

The last two rows are the ones a whole-image first slice does not need, and the ones a format
chosen now must leave room for.

## Options

### Container

| | Shape | Target-side cost | Prior art (read) | Lost or kept because |
|---|---|---|---|---|
| **C1. Members of the boot archive** | A package is a set of named members (`rg`, `rg.manifest`, `rg.licence`) packed into the `nifefs` image beside everything else; the measurement table gets a line per member | **None new.** `nifefs` is already a flat name-to-bytes store, and `measured_boot`'s table is already generic over names | Milestone 47's `PATH` lane found this first ("`nifefs` needs no format change at all") | Kept as an option. Measured headroom: the archive holds **89 of `MAX_FILES` = 127** entries today (riscv64; aarch64 71, x86_64 87), and a `.manifest` suffix fits every current spawnable name but not the four longest fixture names (25 + 9 > 32). It cannot travel alone: a member exists only inside an image |
| **C2. One archive file per package** | Header, table of contents, members, attributes: the `.hpkg` or `.apk` shape | A new parser for hostile input, a fuzz target, and a place to store the file on the target | Haiku `.hpkg`: header, 64 KiB zlib-chunked heap, TOC, attributes (name, version, architecture, licenses, dependencies), addressed by name and version, **no signature in the package**; SHA-256 only in the repository index (read: `haiku-os.org/docs/develop/packages/FileFormat.html`). apk v2: three concatenated gzip streams, signature, control, data (read: hydrogen18.com; the Alpine wiki returned 403, so the signature detail is unverified) | Kept. The familiar shape; one file on the wire; identity is a name and a version, so two builds of `rg 14.1.1` are indistinguishable by name |
| **C3. Content-addressed** | A small metadata document listing each member's digest; members stored and fetched as blobs named by digest; the package's identity is the metadata's digest | A blob store on the target (a RedoxFS directory of hex-named files is enough to start) and a metadata parser | Fuchsia: `meta.far` lists paths to Merkle roots; identity is the Merkle root of `meta.far`; blobfs is write-once and verified on read (read: `fuchsia.dev/.../concepts/packages/package`, `.../filesystems/blobfs`). Nix: store path digest over the object graph; NAR is deterministic, three node types, no timestamps (read: `nix.dev/manual/nix/latest/store/store-path`, `.../protocols/nix-archive/`) | Kept. **The closest to what the tree already does**: `program_measurements` is literally a name-to-SHA-256 table, which is a C3 metadata document without the name for it |

**What the tree does in the analogous case, which is the likeliest decider.** Programs reach the
target today as C1: the host packs ELFs into `nifefs`, writes `program_measurements`, and the kernel
vouches for that table (`kernel/src/trust.rs`, DECISIONS §26's milestone 22 phase B record). The
only foreign program that has ever run here, `ripgrep`, reached it the same way:
`helpers/build-ripgrep.sh` fetches the crate, cross-builds three triples, and `xtask` packs the ELF
if it is on disk. And the documentation store is a host-built directory written into the RedoxFS
image (`cargo xtask manual`, `doc/<bundle>/`), which is C1's shape one filesystem over. **So C1 is
not a new format, it is the absence of one, and C3 is what the measurement table becomes if it is
allowed to travel.** C2 is the one with no analogue here.

### Where the manifest lives

| | Shape | Cost | Lost or kept because |
|---|---|---|---|
| **M1. A sibling member** | `rg.manifest` beside `rg`, measured like any member | A small encoding crate both sides share (rule 7; milestone 47 proposed `manifest_proto`, provisional) | Kept. Separable: a spawner can be handed a binary with somebody else's manifest, which is why the member must be measured, and why C3 (identity covers both) answers this better than C1 or C2 |
| **M2. An ELF note or section** | The manifest travels inside the binary | Extends `crates/elf`, which "parses program headers only and is a Kani-proven, hostile-input-hardened parser on the boot path" | Kept, reluctantly. One artifact, cannot be separated, and it is Fuchsia's opposite (`.cm` ships under `meta/`, read: `fuchsia.dev/.../component_manifests`) |

### The digest

SHA-256 is already hand-written in `crates/measured_boot`, tested against FIPS 180-4 vectors, and
in the kernel's trust root. Fuchsia's Merkle root is SHA-256 over 8 KiB blocks (read from a search
excerpt of `fuchsia.dev/.../merkleroot`; the page itself was not fetched). A Merkle tree buys
verification of part of a file without reading all of it, which matters for a 10.7 MB `rg` ELF paged
in on demand and not for one read whole into memory, which is what the loader does now. Whether to
take a plain digest or a Merkle root is part of this ruling.

## Costs, measured

- `nifefs` headroom: 38 entries and 7 name bytes beyond the longest spawnable name, from the
  archives in the main checkout's `target/` on 2026-09-19.
- The largest member a package would carry today: `rg`, 4.7 MB (aarch64) and 10.7 MB (riscv64), per
  `notes/ripgrep-on-nife.md`.
- A new parser's standing cost, by the tree's own precedent: `crates/nifefs` is 746 lines with a
  fuzz target (`fuzz/fuzz_targets/nifefs_roundtrip`) that found a bug "in under a minute".

## Reversibility, and who has acted on it

Nobody outside this repository has a nife package, so today every option is reversible. **The day
one package is fetched by somebody else, the format is fixed for everything that can read it.**
C1 is the exception: it never leaves an image, so it can change with the image, which is exactly why
it cannot be the format a third party ships.

## The §92 test

C1 is the cheapest by a wide margin, and it is cheap for a reason that is also its limit: it is not
a format anyone else can produce. **If C1 is chosen as the answer rather than as the first slice's
stopgap, that choice is about effort** and should be recorded as such. Between C2 and C3, cost is
similar (one parser each, C3 adds a blob store); the difference is identity by name versus identity
by content, which is a judgment, not an effort question.

## If calef says no to all three

The first slice still builds, because it needs no format: it composes whole images from source on
the host, which is C1 by default. Nothing can be installed onto a running system, and nothing leaves
the tree as a package. *(DECISIONS §157 (a trivial install is a web page, a USB drive, and packages over the internet) replaces that first slice; see milestone 198 (a package manager)'s "Rescoped 2026-09-19". Under §157 a "no" here also stops rung 3.)*
