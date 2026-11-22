# Ruled decisions

An appendix to [the milestone](../1094-grant-plan-outside-review.md). The decisions below are
the rules in force; none is open.

## Where the crate boundary falls

Background. `grant_plan` holds three things that are one crate today. The first is a parser and
planner: bytes in, a typed grant or a typed refusal out, against a manifest. Nothing in it needs
to know a program's name. The second is the vocabulary programs and their spawner share: the
`Manifest` struct and its declaration enums, the capability slot numbers (`DOMAIN_SLOT` and its
neighbors, `INTERFACE_ID` 17), and the verb narrowing. The third is nife's own table: 28 programs
with wire ids, their manifests, the spawn protocol's word layout, the progenitor's window pool and
the region budgets. The externalization rule says a crate that leaves may name no nife program; the
third part is a list of them. Milestone 1017 found the second part from the other side: programs
link the whole planner to read one slot number. The planner's output type, `Endowment`, carries a
`Program` today, so the first part cannot leave without the third unless `Endowment` changes.

Recommendation. Three parts. `grant_plan` leaves as the planner alone, with `Endowment` carrying
the `Manifest` its plan used (`plan_against_with` already takes one); callers that need
the row keep it beside the endowment, as they keep the `ParsedInvocation`. The manifest types, the
slot constants, `VERSION`, `INTERFACE_ID` and `verbs` become a small leaf crate that `grant_plan`
depends on and that a program can link without the planner; its name is yours. The program table,
`spawnproto`, `job_windows`, `job_page_frame`, `image` and the region budgets stay in nife, where
the two programs that speak that protocol live. The alternative, leaving everything together and
widening the lint's waiver, is less work this quarter and makes every later edit to the catalog a
cross-repository bump of a published crate, which is the coordination cost 1017 measured.

Prior art. `clap` ships its lexer as `clap_lex`, a crate with no knowledge of any command, and
builds the catalog on top. Fuchsia's "petals" kept interfaces in repositories apart from the
programs that implement them and rolled each by a pinned revision (1017's prior-art appendix).
Shill separated the contract language from the scripts.

Example. After the split, `swish` plans a line with
`grant_plan::plan_against_with(&run, manifest, holds, expanded, streams)` and sends
`spawnproto::request(program.id(), endowment.program_argument, ...)` from its own table; a
`std` program reads `DOMAIN_SLOT` from the leaf crate and links no parser.

Ruled 2026-11-14 (UTC): the three parts, as recommended, and the split is built.
`grant_plan` is the planner, and `Endowment::manifest` is the manifest its plan used. The leaf
crate is `crates/grant_contract`; its name is provisional, for calef to ratify. The catalog, `spawnproto`, `job_windows`, `job_page_frame`, `image` and
the region budgets are in `crates/program_table`, a provisional name, whose `plan` and
`plan_stage` return the row beside the endowment. No slot number, `INTERFACE_ID`, `VERSION` or word
layout changed.

## Public names that need a decoder

Background. The naming page spells out `prog`, `pos`, `opt`, `proto` and `dir` in every name this
tree authors, and lists `tick` and `frame` among words whose kernel meaning differs. Eight public
names in this crate carry one of them. Each is a path somebody outside will write after the first
release.

Recommendation. `nav` becomes `navigation`. `spawnproto` becomes `spawn_protocol` (it stays in nife
under decision 1, so this one is cheap). `rmopt` becomes `rm_options`. `prog_id` becomes
`program_id`. `BindEntry::pos` becomes `position`. `COOP_GRACE_TICKS` becomes
`COOPERATIVE_GRACE_POLLS` (the doc already says it counts watch-loop iterations and not time).
`THE_RIGHTS_BITS_SHARE_NO_BIT` becomes `RIGHTS_BITS_DO_NOT_OVERLAP` and the mutation exclusion moves
with it. `job_page_frame` becomes `job_page` or `job_channel_page` (it is one shared page of four
words, and `frame` is the kernel's word for something else). `Cwd` and `argv` stay as terms
of art. The ratified names stay closed: none of the eight has a ratification.

Prior art. The standard library writes `position`, `program` and `options` in full; Plan 9 and
POSIX write `argv` and `cwd` as they are.

Example. A reader of `grant_plan::navigation::Bindings::lookup(name).position()` knows what came
back without opening the module.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The eight renames hold, with `job_page_frame` becoming
`job_channel_page`, and no alias of the old spellings. `Cwd` and `argv` stay. Item 7 is the work.

## Limits carry their unit

Background. A limit is the largest size a value may take, and the naming rule says it ends with
its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`). Most of this crate's limits were
ratified before that rule, and a release review rechecks such a name. Seven public limits count
bytes with no unit, and four private fields named `len` or `name_len` fail the template's name
gate. After the first publish every public rename breaks a caller.

Rule, ruled 2026-11-19 (UTC) by the maintainer, provisional: a two-way door until the first publish.
`MAXIMUM_FILE_NAME_BYTES`; `MAXIMUM_NAME_BYTES` in `expand` and in `nav`;
`MAXIMUM_DIRECTORY_NAME_BYTES`; `MAXIMUM_RENDERED_PATH_BYTES`; `MAXIMUM_HOLDER_NAME_BYTES`;
`MAXIMUM_VERB_BYTES`; `MAXIMUM_WORDS_NAME_BYTES` kept as built; the four fields become
`length_bytes`. `file_system_contract::grant::MAXIMUM_NAME`, which two of these mirror, moves in the
same lane. A rename lane does the work, not the pull request that records the rule.

Prior art. This tree's `program_argument_contract::PAGE_BYTES` and `expansion::NAME_BYTES` carry their
unit; POSIX's `PATH_MAX` and `NAME_MAX` are the case the rule avoids, since neither says whether the
terminator counts. Where nife differs: the unit is in the name, not in a manual page.

Example. A test holds `Refusal::FileNotNameable`'s sentence against `MAXIMUM_FILE_NAME_BYTES`, and a
reader knows what 252 counts.

## The names `grant_contract` and `program_table`

Background. The split made two crates out of `grant_plan`. `grant_contract` holds what a program and
its spawner share: the manifest types, the capability slot numbers, `INTERFACE_ID` 17 and the verb
narrowing. It leaves for a repository of its own. `program_table` holds nife's table of 28
programs, the spawn protocol and the region budgets, and stays in nife. §323 (the binary interface,
and the names of the project's layers) settled that crate names stay descriptive and that the
page-contract crates take no common prefix.

Rule, ruled 2026-11-19 (UTC) by the architect: both names stand as built. `grant_contract` follows
the tree's `_contract` pattern for what two programs agree on, and `program_table` says what the
crate holds.

Prior art. `clap` ships its lexer as `clap_lex` and its derive as `clap_derive`, named for the part.
Where nife differs: the leaf carries no family prefix, per §323.

Example. A `std` program writes `grant_contract::DOMAIN_SLOT` and links no parser.

## Review clearance and the repositories

Background. Clearing a crate review is the go to create its repository under nifeos. The planner and
the leaf each leave at step 6 of milestone 1017 (the contract crates leave in this order).

Rule, ruled 2026-11-19 (UTC) by the maintainer, provisional: write both review packages now. The
architect then ratifies any provisional names from the packages and clears `grant_plan` and
`grant_contract` together, with no tag and no publication.

Prior art. In this tree `generational_table`, `block_roster` and `file_allocation_table` each moved
on a cleared review with no tag. Where nife differs: nothing outside nife pins either crate.

Example. Both packages exist, the architect clears them in one step, and `grant_contract` and
`grant_plan` get their repositories together, the planner depending on the leaf by version.
