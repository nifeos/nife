# A package's container: four candidates, measured

*An appendix to [`notes/packages.md`](../packages.md). Measured 2026-10-17 (UTC) by lane
`lane/package-format-measurements`, before §307 (how long a name may be)'s NIFEPKG2 migration is
built, because calef asked whether nife should take an existing format instead. This note measures
and does not choose. The scripts and the Rust harnesses are in
[container-format-measurements/](container-format-measurements/); `materialize.sh` copies the
harnesses out of the tree to build them. Name: provisional (2026-10-17).*

The four candidates:

1. NIFEPKG2, the variable-width layout §307 weighed against tar. After the magic come name, version and architecture,
   each a `u16` length and bytes, then a `u32` count and a `u32` reserved word. One table entry per
   member follows (a `u16` length, the path, `u32` offset, `u32` length, the 32-byte SHA-256), and
   members are 8-byte aligned.
2. NIFEPKG2 compressed whole with Zstandard (zstd) for transit.
3. Plain tar, ustar headers, with a first member `MANIFEST` holding `name`, `version`,
   `architecture` and one `member <sha256 hex> <path>` line per member.
4. The same tar, zstd-compressed, which is Arch Linux's `.pkg.tar.zst` model.

The model measured against. A package travels from basalt, compressed or not. `jig` decompresses
at install time in user space, where an allocator exists, and the installed form is uncompressed.
The loader (`system_initializer`, no allocator) reads only uncompressed bytes. §195 (a reviewed
recipe vouches for a package)'s digest covers the file as fetched.

## What the numbers are

- Sizes are bytes of files this lane built from the members of basalt's release
  `rolling-2026-10-17-155eba5` (six packages, three architectures, 18 assets). Only the `NIFEPKG1`
  column is a file basalt published. Every other column re-encodes the same member bytes.
  `scripts/build.py` writes them, and `measure-sizes.sh` reproduces the table from the release tag.
- The NIFEPKG2 encoding is this lane's reading of §307, since its crate is not built yet. It
  reproduces §307's own header-plus-table figures exactly: 142 to 153 bytes for 2 members, 204 to
  222 for 3, 319 to 320 for 5.
- Reader code size is `.text` in bytes on `aarch64-unknown-none-softfloat`, nightly-2026-10-17,
  fat link-time optimization (LTO), one codegen unit, overflow checks on, minus an empty baseline
  (88 bytes at `opt-level=3`, 80 at `s`; `.rodata` likewise, whose baseline is the 4,096-byte input buffer). Every reader does the same work on black-boxed input:
  parse, read the three identity fields, read every member's name, bytes and digest, and find one
  member by name. SHA-256 is left out of all of them, since every candidate checks the same
  digests with `measured_boot`'s implementation. `measure-code-size.sh` reproduces it.
- **Only the NIFEPKG1 reader is the tree's code.** The NIFEPKG2 and tar readers are prototypes
  this lane wrote to have something to measure (`readers/`), checked against the 18 packages and
  three synthetic ones and nothing more. Moving the same source from the binary crate into a
  library crate moved the NIFEPKG2 figure by 80 bytes and the tar figure by 728, so read any
  difference under about 1 KB as noise.

## Download size

zstd at level 19, every package (bytes):

| package | NIFEPKG1 (published) | NIFEPKG2 | NIFEPKG2.zst | tar | tar.zst |
|---|---|---|---|---|---|
| demos aarch64 | 82,732 | 82,628 | 4,805 | 87,040 | 4,964 |
| demos riscv64 | 21,452 | 21,348 | 4,855 | 26,112 | 5,013 |
| demos x86_64 | 21,292 | 21,188 | 4,796 | 25,600 | 4,951 |
| hackbench aarch64 | 274,427 | 274,323 | 87,045 | 278,016 | 87,052 |
| hackbench riscv64 | 200,227 | 200,123 | 87,369 | 203,776 | 87,530 |
| hackbench x86_64 | 196,595 | 196,491 | 81,810 | 200,192 | 81,951 |
| ioping aarch64 | 313,459 | 313,347 | 97,992 | 316,928 | 98,197 |
| ioping riscv64 | 234,043 | 233,931 | 98,390 | 237,056 | 98,692 |
| ioping x86_64 | 243,843 | 243,731 | 93,485 | 247,296 | 93,788 |
| ripgrep aarch64 | 2,995,791 | 2,995,639 | 863,368 | 3,001,856 | 863,591 |
| ripgrep riscv64 | 2,827,927 | 2,827,775 | 924,555 | 2,833,920 | 924,966 |
| ripgrep x86_64 | 2,983,559 | 2,983,407 | 873,036 | 2,989,568 | 873,106 |
| rmle aarch64 | 99,108 | 98,988 | 13,478 | 103,424 | 13,715 |
| rmle riscv64 | 37,828 | 37,708 | 14,163 | 41,984 | 14,358 |
| rmle x86_64 | 37,812 | 37,692 | 13,089 | 41,984 | 13,279 |
| sqlite aarch64 | 1,300,376 | 1,300,280 | 568,298 | 1,303,552 | 568,446 |
| sqlite riscv64 | 1,021,792 | 1,021,696 | 556,151 | 1,025,024 | 556,335 |
| sqlite x86_64 | 1,279,896 | 1,279,792 | 580,332 | 1,283,072 | 580,410 |
| all six, aarch64 | 5,065,893 | 5,065,205 | 1,634,986 | 5,090,816 | 1,635,965 |
| all six, riscv64 | 4,343,269 | 4,342,581 | 1,685,483 | 4,367,872 | 1,686,894 |
| all six, x86_64 | 4,762,997 | 4,762,301 | 1,646,548 | 4,787,712 | 1,647,485 |
| all 18 | 14,172,159 | 14,170,087 | 4,967,017 | 14,246,400 | 4,970,344 |

All 18, by zstd level:

| form | uncompressed | level 3 | level 19 | level 20 (`--ultra`) |
|---|---|---|---|---|
| NIFEPKG2 | 14,170,087 | 5,693,496 | 4,967,017 | 4,967,097 |
| tar | 14,246,400 | 5,693,018 | 4,970,344 | 4,970,260 |
| tar padded to 10,240-byte records | 14,356,480 | | | |
| NIFEPKG1, for reference | 14,172,159 | | 4,967,023 | |

- zstd at level 19 takes all 18 to 35.1% of their uncompressed size; level 3 to 40.2%. Level 20
  buys nothing over 19 at these sizes.
- Uncompressed, tar is 0.54% larger in total (76,313 bytes) and 20.8% larger on the smallest
  package (`demos` x86_64, 25,600 against 21,188). Compressed, the difference is 3,327 bytes in
  total (0.07%) and 155 bytes on that package (3.2%): zstd removes tar's zero padding.
- Where tar's bytes go: a 512-byte header per member, padding of each member to 512, the
  `MANIFEST` member's own header and padded body, and 1,024 zero bytes at the end. NIFEPKG2 pads
  members to 8.
- Not apples to apples: NIFEPKG2 carries the digests as 32 binary bytes, the tar manifest as 64 hex
  characters. Both carry the same facts.
- The release's packages carry no member name over 30 bytes, so every tar here is pure ustar. A
  §307-length name changes that (below).

## Reader code size

| reader | what it is | `.text`, opt 3 | `.text`, opt `s` | `.rodata`, opt 3 |
|---|---|---|---|---|
| `package_archive` (NIFEPKG1) | the tree's crate today | 2,368 | 2,252 | 256 |
| NIFEPKG2 | prototype | 2,904 | 2,112 | 256 |
| tar, ustar and pax `path`, with the manifest | prototype | 8,372 | 6,368 | 320 |
| tar, ustar only, with the manifest | prototype | 7,340 | 5,456 | 296 |
| `tar-no-std` 0.5.0, with the same manifest parser | taken crate plus prototype | 7,404 | 6,584 | 301 |
| the text manifest parser alone | prototype | 3,352 | 2,928 | 288 |
| `ruzstd` 0.9.1 decoder, for `jig` | taken crate | 56,880 | 25,804 | 7,906 (694 at `s`) |

- The two NIFEPKG readers are the same size to within the noise. Every tar reader is two and a half
  to three and a half times either, and roughly 3 KB of that is parsing the text manifest. A binary manifest would
  move that number; it would also stop `tar xOf pkg MANIFEST` from being readable.
- pax `path` support costs about 1 KB over ustar alone in the prototype.
- The decompressor is the decoder half of `ruzstd` with `default-features = false` (no checksum
  verification, no `std`) on `static-alloc`'s bump allocator (its cost is in the baseline subtracted), measured on the same bare-metal target, not on
  `jig`'s own `aarch64-unknown-nife`. It is 6.8 to 24 times the size of the readers here at
  `opt-level=3`, and it lives in `jig`, not in the loader.

### The decompressor: `ruzstd`

| | |
|---|---|
| crate | `ruzstd` 0.9.1, released 2026-10-11, a pure-Rust zstd decoder (and an encoder this would not use) |
| license | MIT |
| repository | github.com/KillingSpark/zstd-rs: last push 2026-10-16, 11 open issues, 30 contributors, not archived |
| dependencies with default features off | none (`twox-hash` arrives only with the `hash` feature, which verifies the frame checksum) |
| source | 12,642 lines; 40 lines containing `unsafe` outside its `tests/` directory |
| hardening | its 2026-10-11 release asserts that a block cannot decode to more than the frame allows; a further fix of the same kind (#128, 2026-10-16, a per-block limit on literals) is on its main branch and not in any release |
| checked here | decoded all 54 compressed files in this note back to their exact bytes; ripgrep's 3 MB tar in 10.5 ms on the development Mac |

The C library's Rust binding (`zstd`, libzstd under BSD-3-Clause) was not built: it needs a C
cross toolchain for nife, which is its own cost.

## Is there a `no_std` tar reader to take (AGENTS.md rule 6)

Three crates claim `no_std` tar reading. Only one is allocation-free.

| crate | version, license | allocation-free | verdict for the loader |
|---|---|---|---|
| `tar-no-std` | 0.5.0 (2026-08-13), MIT | yes, borrows from the buffer | the one candidate |
| `tar-core` | 0.1.0, MIT OR Apache-2.0 | no: `extern crate alloc` is unconditional and `parse.rs` uses it | needs an allocator |
| `libarchive_oxide-core` | 0.2.0, MIT OR Apache-2.0 | no: `format/tar.rs` uses `alloc` | needs an allocator |

`tar-no-std` 0.5.0: github.com/phip1611/tar-no-std, 822,791 downloads, last push 2026-10-01, one
open issue, 8 contributors. 1,885 lines with its tests; 4 `unsafe` blocks outside them; dependencies
`bitflags`, `log` and `num-traits`.

Under 120 s of cargo-fuzz (one worker, AddressSanitizer, `-max_len=131072`, seeded with four
of the tars above): 16,753,330 runs, 228 coverage edges, no crash. The harness also asserts that
every member's data slice lies inside the input, and that held. For comparison, `ruzstd` under the
same 120 s ran 1,309,427 times over 1,056 edges with no crash, but libFuzzer's length limit had
only grown to 17,389 bytes, so large frames went unexplored.

**What it cannot read.** It reads names only from the ustar header, up to 256 bytes as prefix plus
name, and skips pax records. A member whose name fits only in a pax `path` record is reported
under the header's fallback name, silently. Measured on a member named `bin/` plus 252 bytes, which
§307 allows: GNU tar 1.35 and macOS's bsdtar both list the 256-byte name; `tar-no-std` listed a
100-byte name with no `bin/`. Its own documentation names the limit ("PAX-only paths or file sizes"
do not work).

`package_archive` carries two Kani harnesses, and rule 6 takes a crate in their place only when it
carries proofs of the same properties. Taking `tar-no-std` waits on those proofs, ours contributed
upstream if it lacks them, and on a fix for the pax-path gap above.

## Stock tools

Measured 2026-10-17 (UTC) on macOS 26.6.2 (`/usr/bin/tar` is bsdtar 3.5.3 on libarchive 3.7.4)
and in two Ubuntu 24.04 containers (Docker's `ubuntu:24.04` and nife's own runner image), GNU tar
1.35:

| | macOS, Homebrew zstd 1.5.7 on `PATH` | macOS, `/usr/bin:/bin` only | Ubuntu 24.04 image | Ubuntu 24.04 after `apt install zstd` (1.5.5) |
|---|---|---|---|---|
| `tar tvf` the tar | lists all members | lists all members | lists all members | lists all members |
| `tar xOf pkg.tar MANIFEST` | prints it | prints it | prints it | prints it |
| `tar tf` the tar.zst | lists (bsdtar runs the external `zstd`) | refused: "unable to run program zstd -d -qq" | refused: "zstd: Cannot exec" | lists (GNU tar picks zstd from the suffix) |
| `tar tf` NIFEPKG2 | "Unrecognized archive format" | same | "This does not look like a tar archive" | same |
| `zstd -d` NIFEPKG2.zst | yields the NIFEPKG2 bytes, which no stock tool lists | no `zstd` | no `zstd` | yields them, byte-identical |

- Plain tar is readable by both systems' stock tools out of the box. tar.zst needs a `zstd`
  program on either one: stock macOS has none (`/usr/bin/zstd` does not exist), and neither Ubuntu
  image ships one. Full desktop and server installs were not checked.
- NIFEPKG2, compressed or not, is opaque to every stock tool; `strings` and a hex dump are what is
  left.

## What each format forces

- **Alignment.** tar puts every header and every member's bytes on a 512-byte boundary; NIFEPKG2
  on 8. Both are at least the 8 an ELF wants. The cost is in the size tables above. CLI tar also
  pads the file to 10,240-byte records by default, another 110,080 bytes across the 18 packages;
  readers accept a file without it.
- **Reading a member at its offset.** Both uncompressed forms allow it: a member's bytes are
  contiguous at a known offset, and every reader here returns a borrowed slice. Finding that offset
  means walking: NIFEPKG2 walks a contiguous table (variable width, §307), tar walks one 512-byte
  header per member through the file. At 2 to 5 members both are a handful of reads. The
  compressed forms allow neither; that is why the installed form is uncompressed.
- **Name length.** ustar holds a name as up to 155 bytes of prefix and 100 of name, split at a
  `/`. `bin/<program>` fits ustar while the program's name is at most 100 bytes. §307 allows 252,
  which needs a pax `path` record: one more 512-byte header and one 512-byte block, 1,024 bytes per
  such member (a synthetic two-member package went from 4,096 to 5,120 bytes), and a reader that
  parses pax records.
- **Two places for a name.** With pax, a tar member has a name in the pax record and a fallback in
  its header. Two readers that pick differently see two different packages, as `tar-no-std` and
  GNU tar did above. NIFEPKG has one place.
- **Fields NIFEPKG lacks.** A tar header carries mode, owner, group, modification time, user and
  group names and a type flag. A writer must fix each for a reproducible build. This lane wrote
  mtime 0, uid and gid 0, empty names, mode 0755 under `bin/` and 0644 elsewhere. A reader must
  decide what a type flag other than a regular file means. The prototype refuses
  one, and `tar-no-std` skips the entry.
- **Compression and the digest.** Today `package_archive::installable`, run by
  `system_initializer`, hashes the whole staged package and compares it with the catalog. §195's
  digest covers the file as fetched. If `jig` fetches a compressed file and stages the decompressed
  one, the bytes the loader hashes are not the bytes the digest covers. This comes with
  compression (candidates 2 and 4) and is the same for tar and NIFEPKG2.
- **Compression and reproducibility.** The same tar at the same level gave different bytes from
  zstd 1.5.7 (macOS, arm64) and zstd 1.5.5 (Linux, arm64), at levels 3 and 19, for all three
  files tried. Thread count did not matter (`-T0` and `-T1` agreed). A digest over a compressed
  file therefore pins the compressor's version as well as its level and flags, for milestone 917
  (basalt's packages build reproducibly).
- **The decoder's window.** zstd declares a window the decoder must be able to hold. Compressing a
  file, zstd shrinks it to the input (2.86 MiB for ripgrep's tar). Compressing from a pipe, as
  Arch's build servers do (`zstd -c -T0 --ultra -20 -` in devtools' `makepkg-x86_64.conf`), it
  declares 32 MiB. `ruzstd` accepts up to 128 MiB by default. On the development Mac its peak
  resident memory decoding ripgrep was 19.5 MB for the level-19 file and 19.7 MB for the 32 MiB
  window, so the declared window did not show. Those figures include the harness's input, output
  and comparison copies.

## Serving packages as OCI artifacts

Where a package is stored and how it is served, with the payload still one of the four files, is in
[oci-artifacts-measurements.md](oci-artifacts-measurements.md). In short, the package file becomes
one blob whose digest is §195's recipe digest unchanged. A client fetching by that digest needs a
token request, one redirect and no manifest, and ghcr.io's certificate chains to a root the image
does not carry.

## What this did not measure

- A real NIFEPKG2 crate, which nothing builds: §307 makes a package the tar of §197 (a package is one archive file). Both NIFEPKG2 and tar readers here
  are unreviewed prototypes, neither fuzzed nor checked by Kani.
- Code size on riscv64 or x86_64.
- `ruzstd` on `jig`'s real target, `jig`'s own size (so no "percent of `jig`"), and libzstd at all.
- Decompression time on nife or under QEMU.
- Other compressors (xz, gzip, lz4).
- Whether GitHub's release download applies its own transfer compression.
- Fuzzing past 120 s, with more than one worker, or of anything but `tar-no-std` and `ruzstd`.
- Full Linux desktop or server installs, and any BSD.
