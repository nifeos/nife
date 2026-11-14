# P1: before the tag

An appendix to [the milestone](../1094-grant-plan-outside-review.md).

## P1: before the first release

4. The tokenizer drops a seventeenth token on the floor. DONE (`a_seventeenth_token_is_refused_and_never_dropped`, `the_earliest_unplaceable_token_is_named_when_the_line_is_also_too_long`; `parse_run` records the first token the buffer had no room for as `unexpected`, and `tokenize` keeps its signature).
   Finding: `tokenize` writes into a caller's array and stops when it is full ("Tokens past
   `out.len()` are dropped"), and `parse_run` gives it sixteen slots. The crate's own rule, stated
   at `Refusal::Unexpected` and again at `ParsedInvocation::unexpected`, is that the crate refuses a token nothing can
   place "so authority the user thought they were granting never silently evaporates".
   Past the sixteenth token it does evaporate. No shipped manifest reaches it: `--mem` is the only
   token that spends two slots and no shipped program declares both a memory grant and a positional.
   With the catalog as it is, the crate would always have refused the dropped token anyway.
   `plan_against_with` takes any manifest, so a caller's manifest declaring both reaches it. The
   line `prog --mem 1` repeated seven times, then `a b`, plans `a` and loses `b`.
   Evidence: `src/parsing.rs` lines 14 to 44 and 308 to 310.
   Change: `tokenize` reports that the line had more (a `Result`, or a returned `(filled, rest)`),
   and `parse_run` records the first dropped token in `unexpected`. `each_word` and `argv` already
   walk the whole line one token at a time and stay unaffected. One test with seventeen tokens pins
   the refusal.
   Door: two-way. Effort: S.

5. There is no proof, and the two falsification patches are not against proofs.
   Finding: the crate has no Kani harness. `falsifications/` holds two patches that break two host
   tests in `job_windows.rs`, each with the header the template's `falsify` step expects. Whether
   that step pairs a patch with a host test or demands a harness was not verified. The review
   packages of the other crates pair each harness with one patch. The logic that most wants a
   machine-checked claim stays bounded, allocation-free and small. `word::read` never panics, and a
   quoted word's text is a sub-slice of the token. `tokenize` yields only non-empty sub-slices of
   the line, and the count never exceeds the array. `NameSet::insert` and `keep_smallest` keep the
   `ends` monotone, with `name_bytes() <= NAME_BYTES`, `n <= MAXIMUM_NAMES`, no name twice, and a
   batch that is a prefix of the byte order. `Cwd::apply` is all or nothing. `line::split` never
   indexes past the line for any input. `Escalation` reaches `Forcible` within `COOP_GRACE_TICKS +
   1` ticks of the first interrupt, and never twice. `Windows::take` never returns a held or a bound
   window, and the two patches already exist there.
   Evidence: `falsifications/*.patch`; the absence of `kani` anywhere under `crates/grant_plan`.
   Change: add one harness per claim above, each arriving with its falsification patch, in the
   module's `verification.rs` as `file_allocation_table` does it. The `job_windows` pair becomes a harness
   over any sequence of `take`, `reaped`, `release` and `set_bound`, and its two patches move to it.
   Door: two-way. Effort: M.

6. Limits that do not carry their unit, and four `len` fields.
   Finding: the units rule says a length or a limit ends with its unit (`MAXIMUM_NAME_BYTES`, never
   `MAXIMUM_NAME_LEN`). The release review reopens a name ratified before that rule. Eight public
   limits are in bytes with no unit: `MAXIMUM_FILE_NAME` (ratified; `src/lexing.rs` line 16),
   `WORDS_NAME` (line 25), `expand::MAXIMUM_NAME` (ratified; `src/expand.rs` line 16),
   `nav::MAXIMUM_NAME` (ratified; `src/nav.rs` line 25), `nav::MAXIMUM_DIRECTORY_NAME` (line 34).
   The list goes on with `nav::RENDER_MAXIMUM` (ratified; line 40), `spawnproto::HOLDER_MAXIMUM` (ratified;
   `src/spawnproto.rs` line 492) and `MAXIMUM_VERB` (ratified; `src/verbs.rs` line 100).
   `expand::NAME_BYTES` already carries its unit. The template's `register_names.py` refuses the
   four private fields `len` (`src/expand.rs` line 42, `src/manifest.rs` line 125,
   `src/spawnproto.rs` line 630) and `name_len` (`src/nav.rs` line 496) at a ceiling of 0. The
   `standalone` job is red on them. `MAXIMUM_NAMES`, `MAXIMUM_DEPTH`, `MAXIMUM_BINDS`,
   `MAXIMUM_STAGES`, `MAXIMUM_POSITIONALS`, `MAXIMUM_WINDOWS`, `MAXIMUM_DUE` and
   `MAXIMUM_SHORT_OPTIONS` count things and are right as they are.
   Evidence: the lines above; `design/naming.md`, "Units"; `register_names.py` output.
   Change: the third decision below. The fields take `length_bytes` (`len` is the standard
   library's spelling for a method, and these are fields of a packed value, not methods).
   Door: one-way at the release (public names). Effort: S once ruled.

7. Public names that need a decoder.
   Finding: `design/naming.md` spells out `prog`, `pos`, `opt`, `proto`, `dir` and the rest, and
   names "abbreviations that need a decoder" as the first failure mode. The public offenders are
   these. The modules `nav` (`src/lib.rs` line 47) and `spawnproto` (line 49). The module `rmopt`
   (`src/command.rs` line 287). The function `spawnproto::prog_id` (`src/spawnproto.rs` line 976).
   The field and method `BindEntry::pos` (`src/nav.rs` lines 497 and 507). The constant
   `COOP_GRACE_TICKS` (`src/escalation.rs` line 383), where `tick` is also on the vocabulary page's
   list of words whose kernel meaning differs. The assertion constant `THE_RIGHTS_BITS_SHARE_NO_BIT`
   (`src/manifest.rs` line 402), where the rule is `_DO_NOT_OVERLAP` with no leading `THE_`, and
   `.cargo/mutants.toml` names it. The module `job_page_frame`, where `frame` is on the same list.
   `Cwd` and `argv` are terms of art a reader knows (`getcwd`, `argv`), and `Program::from_id`
   keeps `id`, which the rulings accept. `same_bytes` (`src/program/catalog.rs` line 143) is already
   flagged in its own `Name:` block. The private `is_op`, `is_diag_op`, `prog`, `pos`, `dir`,
   `arg`, `mem_pages` and `args` bindings are the maintainer's to sweep.
   Evidence: `spelled_out.py` output; `design/naming.md`, "Acronyms and abbreviations" and
   "Vocabulary".
   Change: the second decision below covers the public ones. A sweep lane takes the private ones.
   Door: one-way at the release for the public names; two-way for the private ones. Effort: S.

8. British spellings in doc comments.
   Finding: the British form of "recognize" (`src/line.rs` line 224), the British form of "behavior" (`src/nav.rs` line 139), the British form of "honors"
   (`src/spawnproto.rs` line 105), the British form of "catalog" (line 386) and the British form of "neighbor" (`src/program/tests.rs`
   line 114). The template's `british_spelling.py` runs in the `standalone` job.
   Evidence: the lines above.
   Change: the American forms.
   Door: two-way. Effort: XS.

9. The documentation assumes a reader inside this tree.
   Finding: the doc comments cite 376 milestones and 173 decision sections by number. They point at
   81 `notes/` and 24 `components/` paths and name the architect on 86 lines. They quote the
   architect's words in several places (`MountedTree`, `WordGrant`, `SYNC_SLOT`,
   `UNVOUCHED_MANIFEST`, `Activation::Remove`, `ActivationStatus::NameTaken`, `TwoRoots`, and the
   `Name:` blocks that record a ruling as a quotation). A stranger can follow none of the citations
   and should not meet a person's name in an API document. The tree's own rule is that a record
   states the decision and its date and never quotes the decider. The crate also opts out of
   `missing_docs` with a worklist of 22 items (`src/lib.rs` lines 18 to 20; `notes/doc-coverage.md`
   line 123). So `Escalation::new`, `NameSet::empty`, `len`, `is_empty`, `contains`, `Name::as_bytes` and the
   public fields of `Endowment`, `Manifest` and `Streams` without a comment ship undocumented. The
   201 `Name:` blocks go at the move under the ruling that a crate repository carries no provenance
   record; the move commit strips them.
   Evidence: the counts above, from `grep`; `standalone.py`: 174 section citation lines, 338
   milestone citation lines, 201 provenance block lines, 86 architect's-name lines. Change: at the
   move, strip the `Name:` blocks. Replace each citation with the sentence it stood for or with a
   link into the crate's own `docs/`. Remove every quotation of a person. Close the 22-item worklist
   so the crate root can drop `#![allow(missing_docs)]`. The README states the grammar
   (`notes/grant-expression/grant-plan-crate.md` already has it) and one example that runs `parse`,
   `line::split` and `plan` end to end, which `test_support::plan_line` is.
   Door: two-way. Effort: M.

10. Refusal sentences spell numbers that live in constants, one of them in another crate.
    Finding: `Refusal::message` writes "252 bytes a file, 16 a directory, 8 deep", "at most 8, 512
    bytes of name", "at most 4", "at most 8" and "4,080 bytes" by hand; `nav::Refused::message`
    and `BindRefused::message` write "252", "8", "16" and "4". The tests pin the sentences exactly,
    which catches a drift of the sentence and not a drift of the constant: raise
    `MAXIMUM_FILE_NAME` and every test still passes while the prompt lies. `ScopeTooDeep` says
    "at most 3 deep" for `swish::scope::MAXIMUM_DEPTH`, a constant of a downstream crate this one
    cannot see.
    Evidence: `src/refusal.rs` lines 245 to 253, 290, 324, 329 and 352; `src/nav.rs` lines 71 to
    76 and 535 to 537.
    Change: without `alloc` the crate cannot format the sentence at runtime, so pin it the other way: a
    host test that formats each constant with `std` and asserts the sentence contains it. For
    `ScopeTooDeep`, either the shell supplies the number (the message becomes the fixed half, as
    the others are) or the depth constant moves here where the refusal is.
    Door: two-way. Effort: S.

11. Two-kilobyte values travel by value through the planning API.
    Finding: `plan`, `plan_stage`, `plan_against` and `plan_against_with` take `Holdings` (1,008
    bytes), `Expansion` (568) and `Streams` (1,216) by value and return `Endowment` (2,344). The
    crate records two stack overflows caused by exactly this shape (`src/command.rs` lines 389 to
    392; `src/expand.rs` lines 539 to 541) and works around them inside (`Holdings::anchor` writes
    through an out-parameter). The sizes are a fact of the `no_std`, no-`alloc` design, and the
    crate says so at `Sink`, `Source` and `Diagnostics`; what it does not do is keep them off the
    call path or pin them.
    Evidence: the signatures at `src/planning.rs` lines 26, 64, 94 and 106; the sizes measured
    above.
    Change: take `&Holdings`, `&Expansion` and `&Streams`; add a `size_of` test that pins
    `Endowment`, `Holdings`, `NameSet` and `Cwd`, so a field added later shows its cost in a
    failing test rather than in a shell that overflowed. The README states the sizes.
    Door: two-way before the release (an API change). Effort: S.

12. Narrow length fields rest on a limit nothing asserts.
    Finding: `Name::len` is a `u8` and `Name::new` writes `name.len() as u8`; `NameSet::ends` are
    `u16`; `Cwd::lens` and `BindEntry::name_len` are `u8`. Each is right because
    `MAXIMUM_FILE_NAME` is 252 and `NAME_BYTES` is 512, and nothing says so where the cast is. A
    raised limit would truncate silently, which is the one thing this crate promises never to do
    with a name.
    Evidence: `src/expand.rs` lines 42, 68, 94 and 179; `src/nav.rs` lines 213, 271 and 586.
    Change: a `const` assertion beside each limit (`MAXIMUM_FILE_NAME <= u8::MAX as usize`,
    `NAME_BYTES <= u16::MAX as usize`, `MAXIMUM_DIRECTORY_NAME <= u8::MAX as usize`), or
    `u8::try_from` where a `const` cannot reach.
    Door: two-way. Effort: XS.

13. A manifest can name a widening option it does not declare, and the planner narrows silently.
    Finding: `DirectoryGrantDeclaration::Required { subtree_option }` names a letter; if that
    letter is not among the manifest's `short_options`, `plan_directory_grant` takes the narrow
    reading and says nothing. The comment calls it "a bug in the manifest", which is where it
    should fail.
    Evidence: `src/planning.rs` lines 309 to 316; `src/manifest.rs` line 475.
    Change: a `const fn Manifest::is_well_formed` that `programs!` asserts for every row and that
    `capability_manifest_note`'s decoder calls, so a manifest whose `subtree_option` lacks a declaration
    never reaches the planner. The planner's arm then stays as the safe reading for a manifest that
    arrived some other way.
    Door: two-way. Effort: S.

14. The mutation record predates the module split, and the gate wants a green full run.
    Finding: the last full count for this crate is the baseline (489 mutants, 93.7 percent, 76
    missed, since closed); the two triages after it cover inflow only. The template's release gate
    refuses a release without a green `mutation.yml` run at the tagged commit. `.cargo/mutants.toml`
    carries five exclusions for this crate, one naming `THE_RIGHTS_BITS_SHARE_NO_BIT` (item 7).
    Evidence: the baseline census note under `notes/mutation-testing/`, this crate's row;
    `.cargo/mutants.toml` lines
    303 to 311.
    Change: after the split (item 1), one full `cargo mutants` run over the crate that leaves,
    inside the 08:00 to 13:00 UTC window, with its exclusions carried into the repository's
    `.github/mutants-exempts` under their new names.
    Door: two-way. Effort: S.
