# Naming rules ratified for a class of names

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the rulings
calef made for a whole class of names at once, with his words, and the list or the definition that
says which names each covers. `script/names` reads it (`helpers/name_rules.py`), so a covered name
counts as ratified and `script/names <name>` cites the rule. The directory `design/naming/` and this
file's stem are provisional names; naming is an architect's.*

A rule ratifies the names that follow it, today and later. A name that does not follow it is pulled
aside and keeps its own status, so a rule never ratifies a name nobody looked at by accident of
where it sits. Each section below says what it pulls aside and how that is decided.

## Rule 1: an appendix directory takes its parent's stem, and an appendix file a slug of its heading

Applies: appendix-stems
Ratified: 2026-10-17 (UTC)

The proposal, put to calef in the maintainer session of 2026-10-17 (UTC), and his reply:

> An appendix directory takes its parent's stem, and an appendix file takes a slug of the heading it
> was split from. Names that follow this are ratified by the rule. A name the parent's next rename
> changes follows automatically.

calef: "Ratified."

What it covers. A directory under `design/`, `notes/` or `briefs/` beside a document of its own
stem (`notes/stack/` beside `notes/stack.md`, the siting default of §212 (a prose budget)). Also each
document inside it whose stem is made of the words of one heading of that parent. Measured on
2026-10-17: 103 such directories holding 507 documents, 164 of which follow the rule.

What it pulls aside, leaving it provisional.

- A stem that matches no heading of its parent. The check is mechanical (`_matches_heading`): every
  word of the stem, articles aside, must be a word of one heading. A stem with a word no heading
  holds was not derived from a heading. The check and the word rules together pull aside 343 of the 507.
- A stem that breaks a word rule a script can see (`_word_rule`): not lowercase and hyphenated,
  claims the Unix service model (a trailing `-d`, or the word the naming check refuses), or uses a British spelling `helpers/house_style.py`
  lists. The acronym test and the noun test need a reader and are not checked.
- A stem opening with a digit (a numbered or dated record), everything under `design/roadmap/`, and a
  directory named for an index (`notes/README/`).

Not covered: `design/fatal-risks`, `notes/coes`

Those two are records directories (calef, 2026-09-29 and #1921), which §212 separates from appendix
directories, so their names and the nine fatal-risks evidence appendices stay provisional.

When the parent is renamed. The directory follows its parent's stem and each file follows the
heading it came from, so the rename moves them together and no second ruling is needed.

## Rule 2: a test fixture program is named for its role in its test, as a noun phrase

Applies: fixture-roles
Ratified: 2026-10-17 (UTC)

The proposal and his reply:

> A test fixture program is named for the role it plays in its test, as a noun phrase. Names that
> follow this are ratified by the rule.

calef: "Yes"

What it covers. The 24 programs under `fixtures/src/` listed below. The list is explicit because
"names a role" is a judgment a pattern would guess at. A new fixture is covered by adding a bullet
here in the commit that adds it, and until then it is provisional like any other new name.

Covered:

```
- `carried_capability_client`
- `carried_capability_server`
- `confined_syscall_fuzzer`
- `console_test_client`
- `current_cpu_reader`
- `cycle_counter_reader`
- `delegation_granter`
- `delegation_receiver`
- `file_sink`
- `file_source`
- `frame_revoker`
- `image_self_checker`
- `memory_region_depleter`
- `name_resolver_test_client`
- `network_echo_client`
- `notification_binder`
- `page_frame_consumer`
- `page_frame_producer`
- `rendezvous_minter`
- `rendezvous_peer`
- `scratch_release_witness`
- `scratch_window_exerciser`
- `sink_transcript_writer`
- `unreachable_network_witness`
```

What it pulls aside, leaving it provisional. `alto`, `tenor`, `greeting`, `hello`, `noteless`,
`outlaw`, `chatty` and `flaky`. None names a role in its test. `alto` and `tenor` are two voices of a
duet, and `greeting`, `hello` and `noteless` say what they print. `outlaw`, `chatty` and `flaky` carry
rulings of their own that no rename has applied (`kernel_test_subject`, `swap_boundary_witness` and
`first_attempt_crasher`).

## Rule 3: Appendix stems kept as they stand, 2026-10-18

Applies: listed-stems
Ratified: 2026-10-18 (UTC)

calef, 2026-10-18 (UTC), on reading every remaining appendix stem against its title one by one: "Approve all". Of 126 stems, 114 were kept as they stand and 12 were renamed. The renames are recorded in their directories' READMEs.

What it covers. The 114 documents listed below, in 39 directories. Each is named for its title, and each is ratified as it is. A name that is not on the list is not covered, and a new appendix stem is provisional until a ruling or a rule reaches it.

Covered:

```
- design/naming: `command-line-rulings`, `convention-rulings`, `documents-numbers-and-gates`, `programs-scripts-and-directories`, `provenance-limits`, `rename-traps`, `rename-what-moves`, `rename-where-names-hide`, `spelled-out-rulings`
- notes/citations: `census-and-backfill`
- notes/crates-io-on-nife: `probe-results`
- notes/delegated-review: `the-findings`, `the-method`
- notes/documentation: `resolved-bugs`, `search-limitations`
- notes/entropy: `jh7110-trng`
- notes/footprint-perturbation: `executed-footprint`
- notes/framebuffer-contract: `flushed-hang`
- notes/frames: `run-fork`
- notes/glyphs: `retro-fonts`, `terminal-and-proof`
- notes/incremental-path: `sources`
- notes/isa-discovery: `counter-rate-in-userspace`
- notes/job-mix: `null-syscall-hvf-full-mix`, `null-syscall-under-load`
- notes/kani-upstream: `issue-2402-comment`, `kani-lib-link-order`
- notes/kernel-proofs: `riscv64-from-an-aarch64-host`
- notes/load-sensitive-assertions: `caretaker-teardown-wait`, `confirmation-run`, `first-loaded-acceptance-run`, `global-baselines-and-the-drift-law`, `inbound-eintr`, `miss-taxonomy-and-clockless-loops`, `the-icount-claims`, `unowned-reds`
- notes/merge-queue: `merge-methods-and-holds`, `prose-only`
- notes/mutation-testing: `a-memory-bound-per-mutant`, `baseline-survivors-grant-plan-to-swish`, `documentation`, `job-mix-and-jh7110-entropy`, `machine-discovery`, `measured-boot`, `new-crate-backlog`, `uefi-loader`, `video-terminal`
- notes/net: `memory-and-reclamation`, `prior-art-and-the-contract`, `std-tcp-listener`, `the-outbound-gates`
- notes/packages: `jig`, `measurements`, `over-the-internet`, `several-programs`, `the-index-format`, `vouching`
- notes/pipes: `second-stream`, `second-stream-fork`
- notes/process-view: `the-machine-and-your-share`
- notes/project-metrics: `ledger`
- notes/redleaf: `sources`
- notes/register-of-measures: `cache-experiments`, `code-proofs-and-coverage`, `file-size`, `function-length`, `gated-dated-and-owed-rows`, `landed-each-week`, `project-cost`, `prose-budget`, `ratchet-debt`, `reading-the-weekly-series`, `records-by-status`, `unsafe-series`, `which-model-wrote-it`
- notes/releasable-crates-2026-10-07: `inventory`
- notes/scheduled-execution: `archive-narrowing`, `calendar-and-wall-clock`, `calendar-grammar-g5`, `one-image-per-entry`
- notes/serial-less-output: `the-xenon-grid`
- notes/shared-page-audit: `compositor-findings`, `console-and-virtio-findings`, `filesystem-findings`
- notes/smb: `adapter-as-it-stood`, `remaining-work`, `testing`, `throughput`, `wire-decisions`
- notes/stack: `progenitor-stack`
- notes/std: `allocation`, `file-times`, `fs-descent`, `threads`
- notes/stranger-test: `instrument`
- notes/unsafe-obligations: `ceilings`
- notes/untested-error-paths: `tables`
- notes/verification: `calendar-proofs`, `core-harnesses`, `elf-parser-wall`, `glob-proofs`, `paging-blocks`, `parser-harnesses`
- notes/verus: `sources`
- notes/visionfive2: `boots-7-and-8`, `boots-9-and-10`, `hart-roster-stops`, `on-board-test-suite`
- notes/x86-port: `acpi-and-pci`, `interrupts-and-the-fine-map`, `ring-3`, `scheduler`, `user-mode-runtime`, `what-does-not-fit`
```

`script/names` fails when an entry names a file that does not exist, so a later rename of a listed stem must edit this list in the same commit.

## Review order: names that cross a package boundary come first

calef, 2026-10-17 (UTC), at the maintainer session: names crossing a crate boundary are reviewed
before names internal to one crate. Internal names still need ratifying later; the order is a
priority and not an exemption.

`script/names --unratified` ranks the items and modules tier that way, grouped by defining crate.
Crossing is a stated heuristic (`helpers/name_crossings.py` says what it counts and misses) and
only `--unratified` pays for it, about 8 s. A program is a `[[bin]]`; a file another program
includes through `mod` or `#[path]` is a module of it.

## nife's wire-contract crates end in `_contract`

calef, 2026-10-18 (UTC), on being asked "Adopt `_contract`?": "Yes". He had weighed `_handshake`,
`_agreement` and `_compact`, and the prior art: Singularity's channel contracts, Hubris `-api`,
Genode sessions and Fuchsia's `fidl_`.

The reason is that his 2026-09-13 rule spells out network standards, so external standards keep
producing `*_protocol` names (`network_time_protocol` is NTPv4's own), and those collide with nife's
contract suffix. The tree already says "wire contract" and "service contract" for these crates.

Nineteen crates took the suffix; `network_time_protocol` keeps its word, as the standard's name.
`supervision_protocol` waits for its split into `supervision_contract` and `process_builder`.
`script/lint` check 3 holds the spelling and lists those two exceptions by name.

## A ruled name is not asked again, and its rename has a deadline

A block reading `provisional, and ruled: calef ruled **`x`** on <date>` records a settled name.
Its rename is not performed. `script/names --unratified` prints these first, under "ruled, rename
owed", with the target and the age, and never in the plain provisional list. They still count in the
worklist total. A `ruled the stem` block is not a rename and stays plain provisional.

`script/names --check` fails when one is more than 45 days old. The limit is a measure, not a
principle. The oldest owed rename was 35 days old on 2026-10-18 (UTC), so a shorter limit would
have failed on day one and blocked every unrelated merge behind a backlog nobody can clear at once.
Forty-five days passes a backlog being worked and fails a ruling nobody acted on. The parse and the limit
are `helpers/name_ruled.py`.

## Rename-owed exceptions

A rename that cannot be performed yet (it waits on another milestone) is excused by one bullet of
the form below. It expires on its end date, and the check then fails again. A bullet names the
reason, so an exception is never silent.

- (none)
