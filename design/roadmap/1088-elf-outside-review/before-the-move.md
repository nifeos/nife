# P0: before the move

An appendix to [the milestone](../1088-elf-outside-review.md).

### Shipped text cites this tree's paths, milestones, sections and the architect

Finding: `src/error.rs` lines 70 to 71 explains `WritableAndExecutable` by `paging::Flags` and
`notes/elf/parser.md`. `src/error.rs` lines 75 to 76 explains `SegmentsOverlap` by
`crates/user_mode_runtime/link.ld`. `src/segment.rs` lines 26 to 31 sends the reader to `kernel/src/user.rs`,
`kernel/link-x86_64.ld`, milestone 200 (A virtual address and a physical address stop being the same type) and `notes/elf.md`. `src/lib.rs` lines 7 and 19 to 25 cite two notes, §14 (the project's direction: a verified-Rust capability microkernel that runs real workloads)
and milestone 18 (verify the capability core, then spread inward). `src/machine.rs` lines 1 to 2, 6, 14 and 40 say what "nife runs". `src/machine.rs` lines 46 to 49 record a
provisional name in prose with the architect's name, outside a `Name:` block, so the move's
stripping will not remove it. `src/note.rs` lines 3 to 5 cite milestone 597 (A program carries its manifest in an ELF note), §197 (A package is one archive file, named and vouched for by its recipe) and the architect.
`src/note.rs` lines 244 and 271 give `b"nife"` as the example owner. The Kani harness at `src/note.rs` line 355
searches for `b"nife"`. `src/parse.rs` lines 259 to 261 speak of "halting the kernel". The packet counts 29
such lines across `src/`. A stranger can follow none of them, and a crate that has left must not
reference back into this repository.

Change: rewrite each in the crate's own words. The loader-policy rationale (why W^X, why one segment
per page, why 64 headers, why the machine stays fixed) becomes a "What it refuses, and why" section of
the README, phrased for any loader. The doc comments keep one sentence each and link the README
section. The example owner becomes a neutral one (`b"GNU"` is the one every reader knows). `Name:`
blocks go in the move commit, as ruled for every extracted crate. Door: two-way. Effort: M.

### The fuzz seed test reads files outside the crate, and the fuzz target lives in this tree

Finding: `tests/fuzz_seed.rs` lines 18 to 20 `include_bytes!` three seeds from
`../../../fuzz/seeds/elf_parse/`, and `fuzz/fuzz_targets/elf_parse.rs` with its `fuzz/Cargo.toml`
entry are in this repository's fuzz workspace. The target searches for owner `b"nife"` and type 1.
Packaged on its own the test fails to compile. This is the shape milestone 1017 (the contract crates leave in this order) recorded for
`capability_manifest_note`: the seeds and the target leave with the crate.

Change: the new repository carries `fuzz/` with the target and `fuzz/seeds/elf_parse/`. The test
reads `../fuzz/seeds/` or a copy under `tests/fixtures/`. The owner and type in the target are a
constant of the target with a comment, not a project's name. This repository's `fuzz/Cargo.toml`
drops the `elf_parse` target, and `fuzz/seeds/README.md` loses its row. The template's `fuzz.yml`
runs it weekly only if the repository opts in; the crate has no `unsafe`, so it may. Door: two-way.
Effort: S.

### The manifest and the repository files a stranger needs

Finding: `Cargo.toml` has no `description`, `repository`, `keywords`, `categories`, `readme` or
`rust-version`. The manifest inherits `license` and `lints` from the workspace (lines 6 and 8-9). The
comment on `publish = false` (line 5) sends the reader to `crates/publish-allowlist.txt`, a file the
new repository will not have. There is no README, `LICENSE-MIT`, `LICENSE-APACHE`, `CHANGELOG.md`,
`SECURITY.md` or `CONTRIBUTING.md`. `cargo package --list` warns about the metadata.

Change: write the inherited keys as literals, as `jh7110_entropy` did. Take the scaffold's files
(`README-crate.md`, `CHANGELOG.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, `_typos.toml`,
`deny.toml`, `rustfmt.toml`, the caller workflows, `docs/stranger-test.md`). State `rust-version` as
the other packages do (1.88) after a build at that version confirms it (the crate uses edition 2024,
`let else` and `div_ceil`, so 1.85 is the floor by reading; not verified). Replace the `publish =
false` comment with one sentence about the publishing hold. The README states the scope in its first
paragraph: static, little-endian ELF64 `ET_EXEC` for one machine, program headers only, with a
validating parse and a note lookup. Door: two-way. Effort: S.

### A public function and three public constants name the project or fix the machine to the build

Finding: `machine_no_nife_build_accepts` (`src/machine.rs` line 80) carries the project's name in a
public identifier. `KNOWN_MACHINES`, `NATIVE_MACHINE` and `FOREIGN_MACHINES`
(`src/machine.rs` lines 19, 38 and 50) are a test-support surface for this tree's three architectures, exported
because `tests/fuzz_seed.rs` and `system_tests` iterate them (`KNOWN_MACHINES` 3 uses,
`FOREIGN_MACHINES` 1, `NATIVE_MACHINE` 11, all in tests and forgeries). A crate that has left names
no project, and a published crate that lists "the machines some build accepts" lists one
organization's targets.

Change: the shape is decision D1's. Whatever D1 rules, `machine_no_nife_build_accepts` leaves the
public surface before the move (it has two callers, both tests in this crate), and the three
constants either move behind `cfg(test)` and into `system_tests`' own forgery helper, or stay public
under D1's vocabulary. Door: one-way at release, two-way now. Effort: S for the function, M with the
callers if the constants move.
