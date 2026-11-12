# What the review covered

An appendix to [the milestone](../1092-glob-outside-review.md).

Verified firsthand, in a fresh depth-50 clone on a Linux host, nightly cargo 1.102 from the tree's
pinned toolchain, every run under a 12 GiB memory cap:

- `cargo test -p glob`: 18 unit tests, 2 integration tests and 2 documentation tests pass. The
  exhaustive cross-check (2,657,200 pattern and name pairs against the naive matcher) and the
  100,000-byte backtracking test both run inside one second.
- `cargo clippy -p glob --all-targets -- -D warnings` and `cargo doc` with broken intra-doc links
  denied: clean. The crate builds for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`.
- `cargo kani -p glob` for `a_negated_class_is_the_exact_complement_of_the_class` (190 seconds) and
  `star_matches_anything_and_question_matches_one_byte` (2 seconds): both verify. With its
  falsification patch applied in the clone, the negated-class harness fails at its `assert_eq!`
  (212 seconds), and the review restored the clone. The other four harnesses and five patches were not
  run; the crate's record puts the six harnesses at about ten minutes together.
- Every falsification patch applies with `git apply --check -p1` at the tree root, each at an offset
  of about 150 lines from its hunk header.
- The template's helpers over the crate as it stands: `standalone.py` lists 13 findings (below).
  `spelled_out`, `unit_symbols`, `british_spelling`, `service_model` and `contract_names` judge 197
  names and refuse none. `question_shape` refuses one (P1-1). `unsafe_scan` finds no `unsafe`. Every
  comment block is within 40 lines, every function within 100 code lines, and the one source file
  under 2,000 lines.
- `script/crate-review-packet glob` and `helpers/name_release.py --draft glob`: the public API is
  one enum and six functions. No type from another crate appears in a signature, and there is no
  `unsafe` site. The crate has two dev-dependencies into the tree and three dependents
  (`grant_plan`, `pgrep`, `swish`). Six harnesses pair with six patches, and seven public names have
  no `Name:` block.
- The registry: crates.io reports `glob` 0.3.4 at rust-lang/glob with over 700 million downloads.
  `byte_glob`, `name_glob`, `shell_glob`, `filename_glob`, `component_glob` and `fnmatch` are free;
  `glob_match`, `glob_matcher`, `wildmatch` and `globset` belong to other crates.
- The recorded mutation results: the baseline run's 21 survivors were all dispositioned (14 killed,
  7 confirmed hangs, none deferred), and the first weekly census reports 100% caught. No census ran here.
- Every call site in the tree, read: `swish::words` asks `has_magic` before anything else;
  `grant_plan::expand::offer` matches with the default dot rule; `pgrep` matches state names with
  `Dot::Ordinary`.

Read and reasoned about, not run: the correctness of `match_steps`, `decode` and `class_at` on every
corner the tests name and on the ones they do not (the escaped range endpoint, a class ending in a
bare escape, `[]-a]`, `[a\-c]`). Also the soundness of the bound in `cost_bound` and the step
accounting.
The reviewer found no defect in the matcher.

Not run: the remaining four harnesses, the falsification sweep as a whole, a mutation census,
Miri, cargo-fuzz (no target exists), `cargo-semver-checks` (no baseline), the stranger test and any
template workflow (no repository exists). Unverified: whether any directory enumeration in nife
yields the entries `.` and `..` (P2-2).

Effort sizes: S is an hour or two for a lane, M a day, L more.
