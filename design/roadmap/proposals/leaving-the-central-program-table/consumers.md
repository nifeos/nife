# Leaving the central program table: every consumer, and what it needs instead

[The proposal](../leaving-the-central-program-table.md) holds the end state. This appendix lists
every place outside the table's own module that reads `Program`, `PROGRAM_COUNT`, `Program::ALL`,
`Program::from_id`, `Program::from_name`, `Program::verbs`, `Program::resolver_zone`, a wire id or
a named variant. The search was `grep` over every `.rs` file outside `target/`, for each of those
symbols and for `Program::<Variant>`. A file and a symbol stand in for line numbers (§277 (cite a
heading or a symbol)).

## What each consumer reads, grouped by what it needs

Five needs cover every consumer. The last column says which replacement serves it in the
[end state](end-state.md).

| need | consumers | replacement |
|---|---|---|
| A. a name to a manifest, for planning | `grant_plan::plan_stage` (`Program::from_name`, then `.manifest()`); `crates/swish/src/words.rs`'s `expansion`; `crates/swish/src/preview.rs` (`caps`), which narrows `e.program.manifest()` by `e.program.verbs()`; `components/src/swish/running.rs`'s `hold_pages`; `spawnproto::held_shares` | the manifest the program's own note declares, read by the shell from the base list or from the file, and carried on the `Endowment` |
| B. the names that exist, for resolution and completion | `components/src/swish/prompt.rs` (command completion sweeps `Program::ALL`); `components/src/swish/capabilities.rs`'s `bare`; `crates/system_initializer/src/activation.rs`'s install check (`image`, the §229 (a bare name reaches an installed program) refusal); `crates/timetable/src/plan_text.rs`'s `saw` | the base list's names beside the live installed-programs table |
| C. a wire id to loaded bytes | `crates/system_initializer/src/boot_steps.rs`'s `Spawnable` (`[Option<Elf>; PROGRAM_COUNT]`, `absent`, `refused`); `spawn_loop.rs`'s request reader (`Program::from_id(prog_id(w0))`) and `archive_name`; `system_tests/src/user/pipeline_service.rs`'s stand-in spawner | a name sent on the wire, looked up in the archive the progenitor already measured |
| D. a pinned grant tied to one program | `spawn_loop.rs`'s `Wants::of` and the resolver zone (`Program::resolver_zone`); `narrow_by_verb` at the progenitor (`resolve`); the graphics wiring checks on `Program::GraphicalTerminal` in `running.rs` and `spawn_loop.rs` | a small pinned table, keyed by name, for `reboot`, `jig` and, unless ruled otherwise, `graphical_terminal` |
| E. a sweep for build and test checks | `xtask/src/archive.rs` (`check_declared_programs`, the `packaged_only` refusal, `the_declared_programs_agree_with_grant_plan_and_the_boot_list`); `xtask/src/swish_check/script/tests.rs` (every row has a script line); `system_tests/src/user/measured_boot_tests.rs` (every row measures); `crates/swish/src/preview/tests.rs` and `sentences/tests.rs`; `crates/capability_manifest_note/src/tests.rs` (`every_compiled_in_manifest_round_trips`) | a sweep over the `[[bin]]` blocks that carry a note, or over a fixture's own constant |

## Notes on particular consumers

### The progenitor's array

`Spawnable::progs` is `[Option<elf::Elf<'static>>; grant_plan::PROGRAM_COUNT]`, filled by
`measure_spawnable` from `Program::from_id(id).and_then(archive_name)` and `measured(fs, table,
name)`. `archive_name` is `Some(p.name())` and nothing more. So the id indexes a cache of parsed
ELFs, and the archive name is the real key. The replacement needs a lookup by name in the measured
archive, plus the `absent` and `refused` answers it already gives. It does not need an id.

### The endowment's stand-in row

`Endowment` carries `program: Program` (`crates/grant_plan/src/command.rs`). An image has no row, so
`plan_against` files it under `grant_plan::IMAGE_ROW`, which is `Program::Uptime`. The doc comment
on `IMAGE_ROW` in `image.rs` calls this a foot gun, and `spawnproto`'s `BUGS` records the fix: an
endowment that holds its manifest. Every consumer in need A that reads `e.program.manifest()` reads
the wrong manifest for an image. Three of them guard against it by hand: `components/src/timetable.rs`
(`fire_row`), `crates/timetable/src/plan_text.rs` and `crates/timetable/src/admission.rs` each carry
a comment saying not to read the row. Carrying the manifest removes need A's dependence on the
row in one step, with no wire change.

### The verb table and the resolver zone

`Program::verbs` answers `verbs::JIG_VERBS` for `jig` and `&[]` for every other row.
`Program::resolver_zone` answers the root zone for `jig` and `None` for the rest. `narrow_by_verb`
runs in three places with one input: the shell's `hold_pages` and a second call in `running.rs`,
the preview, and the progenitor's `resolve`. A note can already carry a verb table, as note type
`VERBS` (`capability_manifest_note::encode_verbs`). But `endowed_image` in
`crates/system_initializer/src/image_request.rs` refuses installed bytes that carry one. Its comment
gives the reason: endowing from the manifest alone would hand every line every verb's grants. So the
verb narrowing has a wire format and no installed-path reader. A zone for an installed program
comes from the note's zones and the owner's `grant` lines (`granted_zones`), never from the table.

### The shell's words that name a row

`crates/swish/src/sentences.rs`'s outcome renderer matches `Program::LeastAuthorityDemo`,
`Program::MemoryGrantDepleter` and `Program::GraphicalTerminal` to print a sentence for a program
that answers in a register. The fallback is silence, and `every_program_that_answers_in_words_renders_its_answer`
fails for a words program with no arm. With no table, the renderer needs either a generic sentence
for an answer in a register or the sentence carried by the program. A generic "answered N" line is
the smallest change; the two demonstration sentences move to the fixtures' own transcripts.

### The note that copies a row

Three programs build their note from a row today: `components/src/least_authority_demo.rs` carries
`Program::LeastAuthorityDemo.manifest()`, `jig/src/main.rs` carries `Program::Jig.manifest()` with
the pinned fields cleared, and `fixtures/src/greeting.rs` borrows `Program::Uptime.manifest()` as a
base. Tests in `crates/program_asks` and `xtask/src/base_image.rs` do the same. Each of those
becomes a manifest constant in the program's own source.

### The wire id outside the shell and the progenitor

`crates/timetable/src/plan_text.rs` keeps a plan as a bit mask `1 << p.id()` and compares an
archive's names against it in `saw`. The search found no place that writes that mask to disk or
sends it to another program. If none exists, the mask is a local set and can key on names. The
[unverified appendix](unverified.md) records that this was not traced to every reader.

## What no consumer needs

No consumer needs a closed enum for exhaustiveness except the two the table itself generates:
`Program::manifest`'s arms (`E0004` forces one per row) and `sentences.rs`'s match, which falls back
to a wildcard. No consumer needs the numeric order of ids. No consumer outside the shell and the
progenitor needs the wire id, apart from the timetable's local mask and the two test sweeps.
