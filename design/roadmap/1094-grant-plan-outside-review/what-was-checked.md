# What was checked, and how

An appendix to [the milestone](../1094-grant-plan-outside-review.md).

Verified firsthand, on a fresh `--depth 50` clone of `nifeos/nife` at the head of `main`, with the
tree's pinned nightly toolchain, on one machine:

- `cargo test -p grant_plan`: 252 unit tests and 7 documentation tests pass in under a second.
- `cargo clippy -p grant_plan --all-targets`: clean.
- `cargo build -p grant_plan` for `x86_64-unknown-none` and `riscv64imac-unknown-none-elf`:
  clean. The crate is `#![no_std]` with no `alloc`, and its tests are `no_std` too (byte buffers in
  place of `String`).
- `cargo doc -p grant_plan --no-deps` with `RUSTDOCFLAGS="-D warnings"`: **fails** on one private
  intra-doc link (item 2).
- `cargo package --list`: ships `src/` and the manifest only. No README, no license files, no
  changelog.
- The template's `standalone.py --manifest-dir crates/grant_plan`: 13 findings (items 1 and 3).
- The template's `spelled_out.py` over `src/`: about 60 identifiers carrying `prog`, `pos`, `dir`,
  `arg`, `mem`, `op`, `args`; six of them public (item 7). `register_names.py`: four `len` fields
  with no unit (item 6).
- `grep` counts over `src/`: 376 `milestone` mentions, 173 `§` citations, 81 `notes/` paths, 24
  `components/` paths, 86 lines naming the architect, 201 `Name:` blocks (144 provisional, 64
  ratified). The externalization lint (`helpers/lint_externalized_crates.py --count`) reports 103
  consumer-name hits for this crate, the largest waiver on its list.
- No `unsafe` anywhere in the crate. No Kani harness anywhere in the crate. The two files under
  `falsifications/` are patches against **host tests** in `job_windows.rs`, not against proofs.
- Type sizes, by a temporary `size_of` test since reverted: `Endowment` 2,344 bytes, `Streams`
  1,216, `Holdings` 1,008, `DirectoryGrant` 696, `Expansion` 568, `NameSet` 544, `FileGrant` 400,
  `Name` 253, `ParsedInvocation` 184, `Cwd` 144, `Line` 128, `Manifest` 80, `Refusal` 24.
- Which of the crate's items the rest of the tree uses, by `grep` over every other crate: `Manifest`
  (84 uses), `Program` (52), `NO_NOTE_MANIFEST` (42), `parse` (22), `MAXIMUM_VERB` (22), then the
  slot constants and `spawnproto`. Thirteen workspace crates and programs link it; the kernel links
  it for `argv`, `RUN_UNVOUCHED_SLOT`, `Source` and one `Refusal`.
- crates.io download figures for the prior-art crates, from the crates.io API.

Read, not run: the mutation record. The baseline census counted 489 mutants for this crate at 93.7
percent caught with 76 missed, every one since closed or argued equivalent (notes/mutation-testing);
two later inflow triages report 36/33/3 and 7/7/0 (keys/killed/equivalent). The record holds no full run
since the split of the crate root into modules (§272). The brief forbids a census outside the
08:00 to 13:00 UTC window, and this review ran none.

Not run: a stranger test, coverage, `cargo semver-checks` (no release to compare), a fuzz campaign
(the tree's fuzz workspace links this crate only through `capability_manifest_note`'s target), the
template's `check`, `proofs` and `weekly` workflows (no repository exists), and `script/names` for
this crate. That script answers for names, not crates. `script/crate-review-packet` was not run
either.

Unverified, from reading alone: three questions. Does nife's own `script/lint` run rustdoc with
warnings denied? Item 2 says what the template runs. Does the template's `falsify` job pair a patch
with a host test or only with a Kani harness (item 5)? Does the shell ever type a line with more
than sixteen tokens at a program whose manifest declares both `--mem` and a positional (item 4)?
