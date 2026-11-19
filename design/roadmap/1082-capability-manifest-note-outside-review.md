---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1082. `capability_manifest_note`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/capability_manifest_note`, done as a stranger meeting the
crate for the first time would, on a fresh shallow clone of `nifeos/nife` at the head of `main`
(the merge of #3514). It changes nothing; it lists what the next lanes change before the crate
moves to `nifeos/capability_manifest_note` and what can wait until after.

There is no review package for this crate in `notes/crate-reviews/`, so this block has no settled
baseline to stand on. It treats as settled, and does not reopen, what the crate's own text records
as ratified. That is the crate name, the owner string `nife`, the note type numbers `1`, `2` and
`3`, the version 1 layout and its field order, and the version 2 layout and number. It also covers
`MAXIMUM_VERBS`, `MAXIMUM_ZONES` and `ZONE_MAXIMUM` as values, `MAXIMUM_DECLARED_SHORT_OPTIONS`
and `INTERFACE_ID` 7. Everything else in the crate is marked provisional by its own `Name:` blocks.
The crate is on `crates/externalization-list.txt`, in step 6 of milestone 1017 (the contract
crates leave in this order): after `machine_statistics_contract`,
`domain_name_resolution_contract` and `grant_plan`.

Reuse: `script/crate-move --dry-run`, the template `nifeos/crate-workflows` and its scaffold and
helpers, Kani, cargo-fuzz, the crate's own encoders as seed writers. No new tool.

## Finding status

- P0-1, the manifest cannot stand alone: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P0-2, there is no README: Open. The README is in the scaffold's sections ([#3643](https://github.com/nifeos/nife/pull/3643)); the
  stranger run is recorded in `docs/stranger-test.md`, which the template's scaffold supplies, in
  the move pull request.
- P0-3, the encoders accept manifests the decoder refuses: Done, [#3565](https://github.com/nifeos/nife/pull/3565), [#3643](https://github.com/nifeos/nife/pull/3643).
- P1-4, the shipped text names a person and cites the tree: Done, [#3643](https://github.com/nifeos/nife/pull/3643). Only the `Name:`
  blocks remain, and the move strips them.
- P1-5, stale documentation: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P1-6, private names break the spelled-out rule: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P1-7, the tests and examples depend on nife's catalog: Done, [#3643](https://github.com/nifeos/nife/pull/3643), [#3727](https://github.com/nifeos/nife/pull/3727).
  `every_compiled_in_manifest_round_trips` runs in `crates/program_table`, which holds the catalog.
- P1-8, the fuzz target and seeds stay behind: Open. `tests/fuzz_seed.rs` still reads
  `../../fuzz/seeds/` and the target is in nife's fuzz workspace; the move pull request gives the
  crate a `fuzz/` workspace with the target and its 14 seeds, reads the seeds from
  `CARGO_MANIFEST_DIR`, and drops the target from nife's.
- P1-9, the template refuses the fuzz opt-in over `carry!`: Done (maintainer, two-way), [#3643](https://github.com/nifeos/nife/pull/3643).
  The weekly opt-in stays off, the crate is fuzzed on the project's own host, and the template's
  scan does not change.
- P2-10, `Error` is not `Display` or `Error`: Done, [#3565](https://github.com/nifeos/nife/pull/3565), [#3643](https://github.com/nifeos/nife/pull/3643).
- P2-11, `Zones` lacks `len`, and the views differ on lifetimes: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P2-12, the proofs cover version 1 only: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P2-13, the note header is written four times: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P2-14, the `interfaces_note` dev-dependency: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- P2-15, `std` inside a `no_std` crate: Done, [#3643](https://github.com/nifeos/nife/pull/3643).
- The wire literal `nife`: Done (ruled 2026-11-14 UTC, option A), [#3575](https://github.com/nifeos/nife/pull/3575).
- The `_LEN` family: Open. Built as the `_BYTES` names ([#3643](https://github.com/nifeos/nife/pull/3643)), provisional; ratification is
  under Open decisions.
- `service` as a yes-or-no function: Open. Built as `asks_service` ([#3643](https://github.com/nifeos/nife/pull/3643)), provisional;
  ratification is under Open decisions.
- `VERSION` as the name of version 1: Open. Built as `BARE_VERSION` ([#3643](https://github.com/nifeos/nife/pull/3643)), provisional;
  ratification is under Open decisions.
- BUGS, the two statics `carry!` emits: Open. `NIFE_MANIFEST_NOTE` and `NIFE_SUBTREE_GRANTS_NOTE`
  are identifiers, not wire literals, so a rename lane gives them names that do not spell nife
  before the move; the names are two-way, and the maintainer rules them provisionally.
  `globally_unique_identifier_partition_table`'s `NIFE_DATA` and `NIFE_BOOT` are that crate's own.
- The move: Open. No review package exists under `notes/crate-reviews/`, and
  nifeos/capability_manifest_note does not exist; a package lane comes first, then the clearance
  under Open decisions.

## Open decisions

### Ratify the eleven `_BYTES` names

Background: `capability_manifest_note` writes and reads the ELF note a program carries to ask for
authority. Its public byte lengths and the functions that compute them were named `_LEN`. The units
rule says a length carries its unit, and gives `MAXIMUM_NAME_BYTES, never MAXIMUM_NAME_LEN` as its
own example, so the crate's names now end in `_BYTES`: `DESCRIPTOR_BYTES`, `NOTE_BYTES`,
`SUBTREE_GRANTS_BYTES`, `SUBTREE_GRANTS_NOTE_BYTES`, `VERB_RECORD_BYTES`, `MAXIMUM_ZONE_BYTES`,
`zoned_descriptor_bytes`, `serviced_descriptor_bytes`, `zoned_note_bytes`, `serviced_note_bytes`
and `verbs_descriptor_bytes`. The rule decided the suffix. The choice the rule did not make is the
word `descriptor` in the middle of the functions: `zoned_len` did not say what it measured, and
`zoned_note_len` beside it measured something else. Callers in nife (`program_asks`,
`system_initializer`, `jig`, `xtask`) use the new names; each `Name:` block is provisional.

Recommendation: A.

- A. Ratify the eleven as built.
- B. Ratify the suffix and drop `descriptor` from the functions (`zoned_bytes`, `serviced_bytes`,
  `verbs_bytes`), keeping `_note_` where the function measures the whole note.

Door: two-way inside nife; one-way at the first publish, when each name is in users' code.

Prior art: std names a constant by what it counts (`u32::BITS`). POSIX names limits by thing and
unit (`NAME_MAX`, `PATH_MAX`, bytes by convention). The Linux kernel's ELF code says `n_namesz` and
`n_descsz` after the specification, keeping "desc" to tell the descriptor from the note. Where nife
differs: the unit is spelled out, not left to convention. Example: a reader of
`const D: [u8; capability_manifest_note::zoned_descriptor_bytes(ZONES)]` learns at the call that the
number is bytes of descriptor, not a count of zones.

What it blocks: the first publish.

### Ratify `asks_service`

Background: `pub fn asks_service(d: &[u8]) -> Result<bool, Error>` answers whether a program's
manifest asks to run as a service. It was `service`. The rule for a function that answers yes or no
asks for `is_`, `has_`, `can_` or a third-person verb phrase, because a bare noun reads as a getter.
The maintainer decided `asks_service` on 2026-11-14 (UTC), provisionally, using the crate's own
word for the request; `zones` keeps its name, since it returns a view. Its callers are
`program_asks` and `jig`.

Recommendation: A.

- A. Ratify `asks_service` as built.
- B. `is_service`, the plain predicate.

Door: two-way inside nife; one-way at the first publish.

Prior art: std's `Path::is_absolute` and `Metadata::is_dir`; the crate's own `Zones::has_root` and
`Verbs::is_empty` follow the same rule. `is_service` reads as a property of the program, while the
manifest only asks and the reader may refuse. Example:
`if capability_manifest_note::asks_service(d)? { ... }` reads as the condition it is, where
`service(d)?` read as a getter.

What it blocks: the first publish.

### Ratify `BARE_VERSION`

Background: the note's descriptor has three layouts. Version 1 has no zones and no service byte,
version 2 (`ZONED_VERSION`) adds zones, and version 3 (`SERVICED_VERSION`) adds the service byte;
`decode` reads all three. Version 1's constant was `VERSION`, which a stranger reads as the crate's
or the format's current version. The maintainer decided `BARE_VERSION` on 2026-11-14 (UTC),
provisionally. The value is a wire byte, 1, and does not change; only the identifier did.

Recommendation: A.

- A. Ratify `BARE_VERSION` as built.
- B. `PLAIN_VERSION`, or another word for the layout with nothing added.

Door: two-way inside nife; one-way at the first publish.

Prior art: FreeBSD's `NT_FREEBSD_ABI_TAG` names what the version is of, and GNU's
`GNU_PROPERTY_*` constants name the property, never a bare `VERSION`. Where nife differs: the three
layouts are named for what each adds. Example:
`match version { BARE_VERSION => ..., ZONED_VERSION => ..., SERVICED_VERSION => ... }` reads as
three layouts, where `VERSION` in that list read as a mistake.

What it blocks: the first publish.

### Clear the review, which creates nifeos/capability_manifest_note

Background: a crate that leaves nife gets a review package under `notes/crate-reviews/`, which an
architect reads; clearing it is the go to create the crate's repository and move the crate there.
No package exists for this crate yet, so a package lane writes one first. The crate leaves in step
6 of milestone 1017, after `machine_statistics_contract`, `domain_name_resolution_contract` and
`grant_plan`, and the move pull request carries the stranger run (P0-2) and the fuzz workspace
(P1-8).

Recommendation: A.

- A. A package lane writes the review package once the two statics are renamed (BUGS), and the
  architect clears it when it arrives; the move waits for its place in step 6.
- B. Write the package now, with the statics as a condition of the clearance.

Door: one-way. A repository and a moved crate are acted on by the template's workflows and by
nife's pins from the first day; moving back is a second move.

Prior art (recalled, not read): the Rust project moved `libc` out of the compiler's tree into a
repository of its own, and the compiler consumes it back by version. Where nife differs: a review
package checks that the crate stands alone before it leaves, not after. Example:
`jh7110_entropy`, whose review was cleared on 2026-11-01 (UTC), lives in nifeos/jh7110_entropy,
and nife depends on it at a pinned revision.

What it blocks: the move, and so the first tag and the first publish.

## Goal

A stranger who finds the crate in its own repository can tell in one screen what an ELF manifest note is, and can build and prove the crate from the README alone. The stranger can trust the one claim the crate makes: that one manifest has exactly one encoding, in both directions. What the compiler accepts into a
program's bytes, the reader accepts; what the reader refuses, the compiler refuses. The repository's
gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1082-capability-manifest-note-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1082-capability-manifest-note-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a wire byte, a
dependency, a license); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The manifest cannot stand alone.
   Finding: `license.workspace = true` and `[lints] workspace = true` inherit a workspace the new
   repository does not have, so `cargo generate-lockfile` fails in the staged tree. There is no
   `rust-version`, no `description`, no `repository`, no `readme`, no `keywords`, no `categories`.
   The `publish = false` comment sends the reader to `crates/publish-allowlist.txt`, a file outside
   the repository. `domain_name_resolution_contract` is a path dependency with no `version`, so the
   move cannot rewrite it to a `git` or crates.io line. The crate carries no `LICENSE-MIT` or
   `LICENSE-APACHE`.
   Evidence: `crates/capability_manifest_note/Cargo.toml` lines 5, 6, 8, 9, 16; the dry run's
   `build` and `standalone` failures (10 lines).
   Change: write `license = "MIT OR Apache-2.0"` and the lint table as literals, add
   `rust-version`, `description` (one sentence: the ELF note a program carries to ask for authority,
   and its decoder), `repository`, `readme = "README.md"`, `categories = ["no-std",
   "no-std::no-alloc", "parsing"]`, `keywords`. Give every dependency a `version`. Copy the two license files. Replace the `publish` comment with one sentence a stranger can act on. The
   dependencies are the organization's own, approved in advance.
   Door: the license is one-way and already ruled for the organization's crates; the rest is
   two-way. Effort: S.

2. There is no README.
   Finding: the dry run stages the scaffold's shape and fails on it. The module documentation is
   the only description, and it opens with a project-internal citation (item 5).
   Evidence: `crates/capability_manifest_note/` holds no `README.md`; dry run `readme: FAIL`.
   Change: a README in the scaffold's sections. It says what a manifest note is in one paragraph. It lists the three note types and the three descriptor versions as the module tables already give them. It shows how to carry one (`carry!`) and how to read one (find the `PT_NOTE` with owner `OWNER` and type `MANIFEST` with any ELF reader, then `decode`, `zones`, `service`). It covers building and proving (`cargo test`, `cargo kani`, the three commands around a falsification patch) and the no_std and no-allocation facts. It states the refusal policy: the reader refuses a later version and does not skip it. Then
   record a stranger run in `docs/stranger-test.md`.
   Door: two-way. Effort: M.

3. The encoders accept two manifests the decoder refuses.
   Finding: the crate makes three claims. The compiler builds a program's note from a constant. `encode*` panics (a compile error) on what `decode` refuses. A reader therefore never meets a note its own program could not have carried. Two holes, each probed:
   (a) `encode_serviced` never calls `domain_name_resolution_contract::is_canonical_zone`. Its
   documentation says it panics on "a zone the resolver would not spell"; it does not.
   `encode_serviced(&m, &["Example.NET"], true)` and `encode_serviced(&m, &["example.net."],
   false)` both evaluate at compile time, and `decode` of each returns `Err(BadField(57))`.
   `encode_zoned` has the check (line 649) and panics on the same input. A zone longer than 255
   bytes also writes `zone.len() as u8` silently truncated (line 932). The canonical check's 253-byte bound guards `encode_zoned`; `encode_serviced` has no such guard.
   (b) `encode` writes `MemoryGrantDeclaration::Required { minimum, maximum }` without checking
   `minimum <= maximum`; `decode_mem` refuses the result. `encode(&Manifest { memory_grant:
   Required { minimum: 8, maximum: 4 }, ..NO_NOTE_MANIFEST })` evaluates, `Note::of` of it builds
   a 76-byte note, and `decode` returns `Err(BadField(8))`. `grant_plan` places no invariant on
   the pair (its `MemoryGrantDeclaration` is a plain enum with two public `u64` fields), so a
   program source can state it.
   Why it matters: a program built with `carry!` from either manifest compiles, links and ships
   a note that the progenitor and the shell refuse at run time, with no diagnostic at build time.
   Nothing gains authority it should not have (the readers fail closed), so this is not a security finding. It is the crate's central guarantee failing in one direction, and the Kani proof
   covers only the other (`decode` then `encode`).
   Evidence: `src/lib.rs` lines 900 to 947 against 630 to 669 (`is_canonical_zone` at 649 to 652
   has no counterpart in `encode_serviced`); lines 298 to 302 against 496 to 499; the probe
   results above.
   Change: one `const fn` that writes a zone block and checks each zone, shared by `encode_zoned`
   and `encode_serviced`, so the two cannot disagree again. Add `assert!(minimum <= maximum, ...)` in `encode` and a test for each. Add a Kani harness in the encoding direction, `any_manifest` built field by field with `kani::any` under the type's invariants (`ShortOptions::try_new`, a declared subtree letter, the slot). It asserts `decode(&encode(&m)) == Ok(m)`, and a falsification patch removes the new `minimum <= maximum` assertion. The harness is cheap:
   `encode` has no data-dependent loops beyond 16 letters.
   Door: two-way (a stricter encoder refuses only what the reader already refused). Effort: S for
   the fixes, M with the harness.

### P2: later

The P2 items are in [the appendix](1082-capability-manifest-note-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1082-capability-manifest-note-outside-review/before-the-tag.md).

## Decisions for the architect

The architect ruled the wire literal, below. The three decisions on public names (the `_BYTES`
family, `asks_service` and `BARE_VERSION`) are built under provisional names and wait for the
architect's ratification under [Open decisions](#open-decisions), with the review clearance that
creates the repository.

### The wire literal `nife` in a crate that may not mention nife

Background: every manifest note begins with the owner string `nife` and its NUL; every program
basalt has built carries those bytes, and every reader compares them. The macro places the note in
a section named `.note.nife.manifest`, and the link script keeps `.note.nife.*` in the `PT_NOTE`.
The rule for a leaving crate is that it never mentions nife; the externalization lint holds this
crate at a ceiling of 37 hits, of which 10 lines in `src/lib.rs` are these literals and the static's
name `NIFE_MANIFEST_NOTE`. The prose mentions can all go (item 4). The literals cannot change
without a new owner string, which is a wire break. A progenitor reading the new string would find no note in any program built so far. A program carrying the new string would be a native program with no authority to an old progenitor.

Ruled 2026-11-14 UTC (decision O): the owner string and the section name keep their bytes. Bytes
that already sit on a wire or in built programs may spell `nife` after the crate leaves. This is the
one exception to the rule that a leaving crate never mentions nife, and it covers wire literals
only: owner strings, section names, magic values and the tests that assert those bytes. A line that
holds one ends with the comment `// wire literal` (provisional spelling). The externalization lint
and `script/crate-move` accept a marked line and refuse an unmarked mention. The lint prints every
marked line. The lane removes every prose mention, and prose says "the owner string" without
repeating the word. The README says, once, that the owner string is the name of the
operating system that defines the note, as `FreeBSD`, `GNU` and `FDO` are. The marker is already on
the lines of `boot_ladder`, `system_log_contract` and `user_mode_runtime/link.ld`; the lint lists
them once those crates join the externalization list (it scans only listed crates, and only their
`Cargo.toml`, README and `src/`).

Prior art: the owner string namespaces ELF notes, and the owner is the project that defines
the descriptor: `FreeBSD` for `NT_FREEBSD_*`, `GNU` for `NT_GNU_PROPERTY_TYPE_0`, `FDO` for the
package note, `Go` for the build identifier. None of those projects renamed its owner string when
the code that writes it moved repositories.

Example: a program built today carries `05 00 00 00 | 38 00 00 00 | 01 00 00 00 | "nife\0" 00 00
00 | <56 bytes>`. If `OWNER` became anything else, `elf.note(OWNER, MANIFEST)` on that program
returns no note, and the shell runs it as a native program that hears no words.

## Done when

The checks are in [the appendix](1082-capability-manifest-note-outside-review/done-when.md).

## BUGS

- Identifiers that spell the word are not wire literals, so the marker does not cover them: the
  `NIFE_MANIFEST_NOTE` and `NIFE_SUBTREE_GRANTS_NOTE` statics that the `carry!` macros emit, and the
  `NIFE_DATA` and `NIFE_BOOT` constants of `globally_unique_identifier_partition_table`. Each needs a
  rename lane before its crate leaves; the lint counts them until then.
- The review inferred item 8's failure of `tests/fuzz_seed.rs` in the staged repository. The dry run's build step stopped at the manifest before any test ran. The fix is right whether or not the
  inference is.
- With item 9's opt-in off, the version 2 and 3 zone walk is fuzzed only on the project's own
  host. Kani covers a version 3 descriptor with one zone; past that, if the host stops running the
  target, the walk's coverage is the hand-written tests and the seeds alone.
- The prior-art rows marked unverified (GNU property alignment and ordering, the OpenBSD, NetBSD
  and Go type numbers) are from memory. The README cites only the owners `FreeBSD`, `GNU` and
  `FDO`, and none of those rows.
- This block does not review `grant_plan::Manifest`, whose fields the layout spells. If someone adds a field there without a byte here, `every_compiled_in_manifest_round_trips` refuses it at compile time. That holds only while that test lives in a tree that holds both crates (it lives in `program_table`'s tests).

## Index row

An outside review of `capability_manifest_note` before it leaves. Three items block the move (a manifest that cannot stand alone, no README, and two encoders that build notes their own decoder refuses). Six land before the first tag. They are project-internal citations in shipped text, stale documentation, private names against the spelled-out rule, tests bound to nife's program catalog, the fuzz target and seeds left behind, and the template's `unsafe` scan refusing the opt-in. Six more come after. Four decisions on names: the architect ruled that the wire literal `nife` keeps its bytes, and three
public names wait for ratification.
