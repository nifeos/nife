---
status: NOT-STARTED
raised: 2026-10-12
milestone_dependencies: 835, 809, 810
decision_dependencies: 297, 195, 197, 250
machine_requirements: none
specific_machine: none
needs_person: no
---
# 906. ioping is packaged in basalt, and its test installs it with `jig`

*(The title and slug are provisional.)*

calef, 2026-10-12 (UTC): *"They should all be installed via jig through basalt. I missed that for
our benchmarking tools, so I created debt by prioritizing them first."* This is that debt for
ioping. It is the sibling of milestone 810 (`ripgrep` is packaged in basalt and installed with
`jig`). It is also the sibling milestone 897 (SQLite is packaged in basalt, and its tests run in
basalt's package build) named for ioping.

## What exists, checked 2026-10-12 (UTC)

- `helpers/build-ioping.sh` fetches ioping 1.3's one C file, `ioping.c` at the `v1.3` tag, pinned
  by SHA-256, compiles it with `helpers/c-library-cflags.sh`, and links it through
  `helpers/build-c-program.sh` into `target/ioping/<triple>/ioping` for the three targets.
- `xtask` packs that file into the archive when it is on disk (`xtask/src/farm/programs.rs`).
- `system_tests/src/user/c_program_tests.rs` runs it in
  `unmodified_ioping_times_reads_in_a_granted_directory`, and skips when the helper has not run.
  Nothing in CI runs the helper.
- Milestone 834 (ioping on nife and Linux, storage latency one request at a time) runs the same
  program on xenon, and from 2026-10-12 its exit needs the package this milestone builds.

## The work

1. A recipe in `nifeos/basalt` naming ioping 1.3, its source URL and the SHA-256 the helper pins.
   The build refuses a download that does not match.
2. basalt's package build turns the helper into a build job. At the nife commit `pins.toml`
   names, it compiles `ioping.c` with that commit's C library flags. It links the result the way
   `helpers/build-c-program.sh` does, once for each architecture (§19 (architectural parity is a
   tenet; the targets are aarch64, riscv64, and x86_64)).
3. The manifest note the helper writes today, written by basalt's build instead, through the note
   writer milestone 810 builds.
4. A package archive per architecture (§197 (a package is one archive file, named and vouched for
   by its recipe)), kind `optional`. The recipe vouches for it (§195 (a reviewed recipe vouches for
   a package, and the machine's owner may overrule it)). It is published to basalt's index
   (§250 (an image names its distribution's package index, at `basalt.nifeos.org`, and a package's
   bytes may live anywhere)).
5. The test moves behind `jig install ioping`: the run in `c_program_tests.rs` installs the package
   first and times reads with the installed program, or the test moves into basalt's package build
   as milestone 897 moves SQLite's. Which of the two is this milestone's first finding.
6. When the package is the only source, `helpers/build-ioping.sh` and the archive's packing of
   `target/ioping/` leave this tree.

## Exit

On aarch64, riscv64 and x86_64: a nife at basalt's pinned commit runs `jig install ioping` against
basalt's index, the archive's digest matches the recipe, and the installed `ioping` times reads in
a granted directory as the helper-built one does today. A recipe whose checksum disagrees with the
download fails basalt's build.

Reuse: the helper's pinned fetch and flags, `helpers/build-c-program.sh`'s link, and milestone
810's recipe, build job and publishing, all moved rather than rewritten.

## BUGS

- `helpers/build-ioping.sh`'s header names `system_tests/src/user/ioping_tests.rs`, which does not
  exist; the test is in `c_program_tests.rs`. Left for whoever next edits the helper.
- basalt has no package build yet. This waits on milestone 810 for the first one.

## Index row

ioping built by basalt from a pinned source and installed on nife by `jig`, so the storage-latency
benchmark milestone 834 runs is a package and not a helper-built binary.
