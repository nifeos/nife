# P1: before the tag

An appendix to [the milestone](../1092-glob-outside-review.md).

## P1: before the first tag

#### P1-1. One test function fails the template's question gate

`fn naive(pattern, name, j, i) -> bool` (`src/lib.rs` line 580) returns a `bool` and is not named like a
question. `check.yml` runs `question_shape` at a ceiling of 0, so the first pull request in the new
repository is red. Change: `naive_matches` or `exhaustive_matches`. Two-way. S.

#### P1-2. The falsification patches and their records are written for the tree's paths

All six patches diff `a/crates/byte_glob/src/lib.rs`, and the six `Falsification: replayable` records
name `crates/byte_glob/falsifications/<name>.patch`. The template's `falsify.py` accepts only
`falsifications/<module.path>.<harness>.patch` (its `CONVENTIONAL` regex) and applies with `git apply
-p1` at the crate root, so each would fail as "no longer applies", which the gate counts as a failure
and never a skip. The hunk headers are also about 150 lines stale; they apply at an offset today
and a rewrite is the moment to regenerate them. Whether the move lane's tooling already rewrites
paths stays unverified. Change: rewrite the six paths and the six records at the move, regenerate the
hunks, and replay each once. Two-way. S.

#### P1-3. The inline proof module will be mutated in the new repository

`#[cfg(kani)] mod verification` is inline in `src/lib.rs` (line 881). In nife, `.cargo/mutants.toml`
excludes `verification::` by regex; that file does not travel. cargo-mutants in the repository will
mutate harness bodies that no host test can reach, and the weekly mutation gate will fail on them, as
it did for `e1000e`. Change: split the file as the naming page prescribes for `file_allocation_table`
(`src/tests.rs` for the host tests, `src/verification.rs` for the proofs, `src/lib.rs` for the
matcher) and exclude `src/verification.rs` by glob in the repository's `.cargo/mutants.toml`, as
`generational_table` does. The split also takes the 1,047-line file to about 370 lines of shipped
code. Two-way. S.

#### P1-4. The crate does not forbid `unsafe`

`src/lib.rs` has no `unsafe` and does not say so. `#![forbid(unsafe_code)]` makes the claim one the
compiler keeps, and it is the shortcut the template's fuzz opt-in scan accepts. Change: add it under
`#![no_std]` (line 26). Two-way. S.

#### P1-5. The one caveat the record says is in the crate docs is not there

`notes/glob.md` and `notes/glob-scope.md` both say that the matcher's blindness to `/` is "written
down instead, in the crate docs where a reader meets the function". `src/lib.rs` never mentions a
separator or a path. A caller who passes `matches(b"*.txt", b"dir/notes.txt")` gets `true` and
nothing warned them. Change: one sentence in the module doc and one in `matches`, and the README
line in P0-6. Two-way. S.

#### P1-6. The module doc opens for nife's reader, not the crate's

The first paragraph of `src/lib.rs` explains that one matcher "serves both the shell's expansion and
its grant" and that the authors wrote it "because it is on the verification path (§46 (thin primitives or whole subsystems; we write everything in between) ...)". A stranger
needs the first paragraph to say what the crate is: an `fnmatch(3)`-style matcher over bytes for one
file name, with no allocation, no `unsafe`, a cost bound computable in advance, and six Kani proofs.
The grant argument belongs in the README's "How it differs" section, in the crate's own words.
Change: rewrite the two opening paragraphs; keep the syntax table. Two-way, with P0-4. S.

#### P1-7. The names of the two length parameters do not carry their unit

`step_bound(pattern_len: usize, name_len: usize)` (`src/lib.rs` line 236). The naming page says a length
carries its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`) and that the rules apply to
parameters; the rules accept `len` only as the standard library's method spelling, which is why
`spelled_out` passes it. Change: `pattern_bytes` and `name_bytes`. A parameter name is not part of the
API's compatibility surface, so this is two-way, and it sits here rather than under P2 only
because it is the one naming item the gates do not catch. S.
