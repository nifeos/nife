# P1: before the tag

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

## P1: before the first tag

4. The shipped text names a person, quotes rulings, and cites numbers that mean nothing outside
   this tree.
   Finding: after the move deletes the `Name:` blocks, the staged `src/lib.rs` and `src/tests.rs`
   still carry 12 lines that name the architect (one quoting a ruling word for word). They also carry 29 milestone citations, 3 section (`§`) citations and 9 paths into nife (`notes/capability-manifest-note.md`,
   `AGENTS.md rule 7`, `crates/elf`, `script/lint`). The module documentation's first sentence
   ends in one. The move runbook's rule is that the crate never mentions nife, and the project's
   rule for records is that a document states the rule and not who decided it or when.
   Evidence: dry run `citations` (four lines) and `standalone` (lines 6 to 9); staged `src/lib.rs`
   lines 3, 19, 54, 105, 126, 271, 961, 969, 1084, 1093, 1305 and `src/tests.rs` line 331.
   Change: rewrite each passage to state the fact (the layout, the limit, the refusal) with no
   citation, name, date or quotation. The reasons that matter to a stranger (why the layout has no
   byte for `reboot`, why version 3 exists beside version 2, why 256 verbs) move into the README or a `docs/` page in the new repository. They appear in the crate's own words. The history stays in nife's notes and git. The lint ceiling of 37 for this crate then drops to the literals of item 10.
   Door: two-way. Effort: M.

5. Stale documentation a stranger would act on.
   Finding: `VERSION`'s comment says it is "the only one it reads" while `decode` reads versions 1,
   2 and 3 (line 114). The docs describe `Error::WrongLength` for version 1 only (line 185), and the decoder returns it for every version. `decode`'s comment describes version 1 and version 2 and not
   version 3 (lines 389 to 411). The module's BUGS entry says nothing writes a note object for a
   foreign build (line 12), while `xtask/src/foreign_note.rs` does exactly that from `Note::of`.
   `verification.rs` says the `INTERFACES` harnesses stay in `lib.rs` (line 4); that note type
   moved to `interfaces_note` and `lib.rs` has no harness. `check_zones` and `check_serviced`
   promise "the first byte at fault" (lines 673, 701) and report the zone's length byte for a bad
   letter inside the zone; `decode` reports `FLAG_LETTERS` for a bad letter anywhere in the list
   (line 436). A version 3 descriptor cut before its service byte is `BadField(at)` where every
   other short descriptor is `WrongLength` (line 724, and the test at `tests.rs` line 498 pins it).
   Change: correct each sentence; either make the offsets exact or say "the field at fault"; and
   either return `WrongLength` for the cut version 3 descriptor or document the exception at
   `Error::BadField`.
   Door: two-way (the error variant change is observable but nothing outside the tree matches on
   it yet). Effort: S.

6. Private names break the spelled-out rule and the option vocabulary.
   Finding: `ARG`, `MEM_MINIMUM`, `MEM_MAXIMUM`, `DIR`, `CONFIG`, `decode_arg`, `decode_mem`,
   `decode_dir` and the bindings `arg`, `mem`, `dir`, `args` are 15 hits in the template's
   `spelled_out.py` at a ceiling of 0. `FLAG_COUNT` and `FLAG_LETTERS` hold option letters, and
   the vocabulary rule says an option is never a flag. `RUNTIME` and `decode_runtime` read the
   field named `startup`. Template v7, the newest release, does not run these checks yet; the
   next release does, and the crate meets them then unless this lands first.
   Evidence: `src/lib.rs` lines 194 to 213, 428 to 429, 442, 477, 489, 517; `src/tests.rs` lines
   189, 196, 211.
   Change: `PROGRAM_ARGUMENT`, `MEMORY_MINIMUM_PAGES`, `MEMORY_MAXIMUM_PAGES` (the unit, since
   they are page counts), `DIRECTORY`, `CONFIGURATION`, `STARTUP`, `OPTION_COUNT`,
   `OPTION_LETTERS`, and the functions and bindings to match. Private, so no user sees it.
   Door: two-way. Effort: S.

7. The tests and the public examples depend on nife's program catalog.
   Finding: `grant_plan::Program::Uptime`, `Rm`, `Jig` and `Reboot` appear 17 times across
   `src/tests.rs`, the `decode` and `encode_verbs` doctests and `tests/fuzz_seed.rs`. The catalog
   names nife programs, which the externalization lint counts, and a stranger reading
   `decode`'s example learns nothing about building a `Manifest` of their own. The test
   `every_compiled_in_manifest_round_trips` is a test of nife's catalog against this layout and
   belongs where the catalog lives.
   Evidence: `src/lib.rs` lines 395, 1165; `src/tests.rs` lines 39, 65, 85, 103, 130, 150, 173,
   248, 256; `tests/fuzz_seed.rs` line 63.
   Change: the examples and tests build manifests from `NO_NOTE_MANIFEST` and struct update
   syntax, with `ShortOptions::try_new(b"rfv")` where a test needs letters. The catalog round trip moves to `grant_plan` (or `system_tests`) in nife, which keeps the guard that a new program
   with an unspellable manifest fails before a build carries it. Whether `Program` leaves with
   `grant_plan` is milestone 1017's question, not this crate's; after this item the crate no
   longer cares.
   Door: two-way. Effort: S.

8. The fuzz target and its seeds stay behind, and the seed test breaks in the new repository.
   Finding: `tests/fuzz_seed.rs` reads `../../fuzz/seeds/capability_manifest_note_decode/` (line
   118) and fails when the directory is absent; the staged repository has no `fuzz/`. The dry run
   leaves `fuzz/fuzz_targets/capability_manifest_note_decode.rs` in nife because it links
   `grant_plan` and `domain_name_resolution_contract` directly; both are the crate's own
   dependencies, so nothing stops the target from moving. Milestone 1017's table says the seeds
   and target leave with the crate. The target's structure-aware mutator is the only thing that
   reaches the version 2 and 3 zone walk beyond Kani's 57-byte bound, so leaving it behind leaves
   the walk covered by hand-written tests alone.
   Evidence: `tests/fuzz_seed.rs` lines 115 to 119; dry run `fuzz` notes; `fuzz/Cargo.toml`
   lines 73, 170 to 171.
   Change: a `fuzz/` workspace in the crate with the target, its `Cargo.toml` and the 14 seed
   files; the seed test's path becomes `fuzz/seeds/...` from `CARGO_MANIFEST_DIR`; nife's fuzz
   workspace drops the target in the move pull request. The template's `fuzz.yml` runs it only if item 9 allows the opt-in.
   Door: two-way. Effort: S.

9. The template refuses the weekly fuzz opt-in because `carry!` spells `unsafe`.
   Finding: `unsafe_scan.py` counts the five `#[unsafe(link_section = ...)]` attributes inside
   the `carry!` and `carry_subtree_grants!` macro bodies as `unsafe` in shipped source (lines 994,
   999, 1005, 1011, 1120). The crate has no `unsafe` block, function or impl; the attribute is
   unsafe for the program that expands the macro, and that program is the one placing bytes in its
   own section. With the scan as it is, `public-crash-inputs: true` turns the fuzz job red, so the
   crate's fuzzing would run only on nife's private fuzz host.
   Evidence: the scan's output; template `README.md`, "Why only safe-Rust crates opt in".
   Change: two options. Keep the opt-in off and fuzz privately (the restrictive choice, and no
   change anywhere), or ask the template, through its own queue, to treat `#[unsafe(...)]`
   attribute syntax inside `macro_rules!` as the caller's `unsafe` and not the crate's. Recommend
   the first until the crate has a second reason to want public crash inputs; the finding is a
   panic in safe code either way.
   Door: two-way. Effort: XS here, S in the template.
