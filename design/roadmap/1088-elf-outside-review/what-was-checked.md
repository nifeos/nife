# What was checked, and how

An appendix to [the milestone](../1088-elf-outside-review.md).

Verified firsthand, on a fresh depth-50 clone at `cf1328af6`, on a Linux x86_64 host with the
repository's pinned nightly (rustc 1.102.0-nightly) and Kani 0.68.0:

- `cargo test -p elf --all-targets`: 47 unit tests (25 in `tests.rs`, 22 in `note.rs`) and the 2
  integration tests of `tests/fuzz_seed.rs` pass. `cargo clippy -p elf --all-targets` is clean.
  `cargo doc -p elf --no-deps` with warnings denied is clean; the two doctests pass.
- `cargo check -p elf` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`: all three build.
- `cargo kani -p elf`: 8 of 8 harnesses verify. Each of the 8 falsification patches, applied in turn
  to the clone, fails exactly its own harness: the four in `verification.rs` at the overflow or
  assertion their headers name, the four in `note.rs` the same.
- `cargo package -p elf --list`: it warns that the manifest has no description, documentation,
  homepage or repository, and the listing holds the manifest, the sources and the eight patches, and
  no README, license file or changelog.
- `script/crate-review-packet --no-rustdoc elf`: no `unsafe`, no dependencies, two workspace-inherited
  keys, no `rust-version`, 20 lines citing a path in this tree, 2 citing a section, 7 citing a
  milestone, 3 naming the architect, and 3 `include_bytes!` reading a file outside the crate.
- The mutation census as recorded (not run; the window rule forbids it here): the latest row for
  `elf` reads 222 mutants tested, 178 caught, 0 missed, 4 timed out, 40 unviable, no real holes.
  Earlier triage brought six machine-check survivors to zero; the four timeouts have no triage line.
- The crates.io API, for the name `elf` and for the prior-art crates in the table below.
- Every caller's use of the API (`grep` over the tree): `Elf::parse`, `entry`, `segments`, the
  three `Segment` predicates, `page_range`, `vaddr`, `paddr`, `memsz`, `data`, `Elf::note`,
  `NoteSegments::from_head`, `NoteSearch`, `NATIVE_MACHINE`, `KNOWN_MACHINES`,
  `FOREIGN_MACHINES`, `PF_R`, `PF_W`, `PF_X`, `Error`.

Inferred from reading, not run. First, the behavior on a target whose `usize` is narrower than 64
bits (item 7); the machine has no 32-bit target and the review added none. Second, the behavior of
`Segment::page_range` on a page size of zero or a non-power-of-two (item 5). A debug build would
panic on the underflow or the division, and the review did not write a test to show it. Third,
whether the crate builds at Rust 1.88 (the version the other packages state). Fourth, the LLVM and
Linux behaviors cited under prior art, which come from recall rather than a re-read where the text says so.

Not run: a mutation census, a fuzz campaign (the review read the `elf_parse` target and did not run it),
`script/crate-standalone`, the template's workflows (no repository exists), `cargo semver-checks`
(no baseline), Miri, and the stranger test (no README to follow). The reviewer pushed, tagged,
opened and commented nothing.

Effort sizes: XS is under an hour for a lane, S half a day, M a day or two, L more. A door is
one-way when somebody outside this repository acts on it after a release: a public name, a published
behavior, a published number. Before the first release every door below is two-way, and the word
"one-way" marks what a release would close.
