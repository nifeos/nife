---
status: NOT-STARTED
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

Each has the plain background first, then the recommendation, prior art and a concrete example.
The names of decisions 3 and 4 are two-way before the first tag, and the maintainer has decided
them, provisionally.

### The wire literal `nife` in a crate that may not mention nife

Background: every manifest note begins with the owner string `nife` and its NUL; every program
basalt has built carries those bytes, and every reader compares them. The macro places the note in
a section named `.note.nife.manifest`, and the link script keeps `.note.nife.*` in the `PT_NOTE`.
The rule for a leaving crate is that it never mentions nife; the externalization lint holds this
crate at a ceiling of 37 hits, of which 10 lines in `src/lib.rs` are these literals and the static's
name `NIFE_MANIFEST_NOTE`. The prose mentions can all go (item 4). The literals cannot change
without a new owner string, which is a wire break. A progenitor reading the new string would find no note in any program built so far. A program carrying the new string would be a native program with no authority to an old progenitor.

Recommendation: keep `OWNER = b"nife"` and the section name as they are, and have the
externalization lint exempt byte-string literals, `link_section` names and identifiers that spell
the owner, while the lane removes every prose mention. The README says, once, that the owner string is
the name of the operating system that defines the note, as `FreeBSD`, `GNU` and `FDO` are.

Prior art: the owner string namespaces ELF notes, and the owner is the project that defines
the descriptor: `FreeBSD` for `NT_FREEBSD_*`, `GNU` for `NT_GNU_PROPERTY_TYPE_0`, `FDO` for the
package note, `Go` for the build identifier. None of those projects renamed its owner string when
the code that writes it moved repositories.

Example: a program built today carries `05 00 00 00 | 38 00 00 00 | 01 00 00 00 | "nife\0" 00 00
00 | <56 bytes>`. If `OWNER` became anything else, `elf.note(OWNER, MANIFEST)` on that program
returns no note, and the shell runs it as a native program that hears no words.

### The `_LEN` family of public names

Background: the units rule says a length or a limit carries its unit, and gives `MAXIMUM_NAME_BYTES,
never MAXIMUM_NAME_LEN` as its own example. The crate's public byte lengths are `DESCRIPTOR_LEN`,
`NOTE_LEN`, `SUBTREE_GRANTS_LEN`, `SUBTREE_GRANTS_NOTE_LEN`, `VERB_RECORD`, `ZONE_MAXIMUM`,
`zoned_len`, `serviced_len`, `zoned_note_len`, `serviced_note_len` and `verbs_len`. `ZONE_MAXIMUM`
mirrors the resolver's `NAME_TEXT_MAXIMUM`, which has the same gap. Every one is a public item of a crate nearly ready for its first tag, and callers in nife (`program_asks`, `system_initializer`, `jig`,
`xtask`) spell several of them. Their `Name:` blocks are provisional.

Recommendation: rename before the first tag, in one lane, with nife's callers in the same pull
request: `DESCRIPTOR_BYTES`, `NOTE_BYTES`, `SUBTREE_GRANTS_BYTES`, `SUBTREE_GRANTS_NOTE_BYTES`,
`VERB_RECORD_BYTES`, `MAXIMUM_ZONE_BYTES`, `zoned_descriptor_bytes`, `serviced_descriptor_bytes`,
`zoned_note_bytes`, `serviced_note_bytes`, `verbs_descriptor_bytes`. The rule already decides the suffix. The open choice is whether the functions say `descriptor` in the middle, and the recommendation is yes, because `zoned_len` does not say what it measures and `zoned_note_len`
sits beside it measuring something else.

Prior art: std names a method `len` and a constant by what it counts (`u32::BITS`, `size_of::<T>()`). POSIX names limits by thing and unit (`NAME_MAX`, `PATH_MAX`, in bytes by convention and documented as such). The Linux kernel's ELF code names note sizes `n_namesz` and
`n_descsz` after the specification. The template's `unit_symbols.py` does not catch `_LEN`
because it is a word, not a symbol; the rule does.

Example: a stranger reading `const D: [u8; capability_manifest_note::zoned_len(ZONES)]` has to
open the function to learn that the number is bytes of descriptor, not zones; `zoned_descriptor_bytes(ZONES)`
says it at the call.

### `service` as the name of a yes-or-no function

Background: `pub fn service(d: &[u8]) -> Result<bool, Error>` answers whether the manifest asks to
run as a service. The rule for functions that answer yes or no asks for `is_`, `has_`, `can_` or a
third-person verb phrase, because a bare noun reads as a getter. The name is provisional and has
three callers in nife (`program_asks`, `jig`).

Recommendation: `asks_service(d)`, the crate's own word for the request ("the service ask"), with
`is_service` as the alternative if the architect prefers the plain predicate. `zones(d)` stays: it
returns a view, not a yes or no.

Prior art: std's `Path::is_absolute`, `Metadata::is_dir`; the crate's own `Zones::has_root` and
`Verbs::is_empty` already follow the rule.

Example: `if capability_manifest_note::service(d)? { ... }` reads as "if the service", and
`if capability_manifest_note::asks_service(d)? { ... }` reads as the condition it is.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag, which the architect may revisit. `service` (`src/lib.rs` line 737) becomes `asks_service`, the crate's own word for
the request, and its callers in `program_asks` and `jig` follow. `zones` keeps its name.

### `VERSION` as the name of version 1

Background: `pub const VERSION: u32 = 1` is the descriptor version with no zones and no service
byte; `ZONED_VERSION` is 2 and `SERVICED_VERSION` is 3, and `decode` reads all three. A stranger
reads `capability_manifest_note::VERSION` as the crate's or the format's current version, and the
constant's own comment still says it is the only version read (item 5). The name is the lane's own design, and no architect has ratified it.

Recommendation: `BARE_VERSION`, beside `ZONED_VERSION` and `SERVICED_VERSION`, so the three names
say what each layout adds. The value is a wire byte and does not change; only the identifier does,
with the same callers as decision 2.

Prior art: `NT_FREEBSD_ABI_TAG` names what the version is of; GNU's `GNU_PROPERTY_*` constants
name the property, never a bare `VERSION`.

Example: `match version { BARE_VERSION => ..., ZONED_VERSION => ..., SERVICED_VERSION => ... }`
reads as three layouts; `VERSION => ...` in that list reads as a mistake.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag, which the architect may revisit. `VERSION` (`src/lib.rs` line 117) becomes `BARE_VERSION`, beside `ZONED_VERSION`
and `SERVICED_VERSION`. The wire byte 1 does not change.

## Done when

The checks are in [the appendix](1082-capability-manifest-note-outside-review/done-when.md).

## BUGS

- The review inferred item 8's failure of `tests/fuzz_seed.rs` in the staged repository. The dry run's build step stopped at the manifest before any test ran. The fix is right whether or not the
  inference is.
- Item 9's recommendation leaves the version 2 and 3 zone walk fuzzed only on nife's private
  host. If that host stops running the target, the walk's coverage past 57 bytes is the
  hand-written tests and the seeds alone until item 12 lands.
- The prior-art rows marked unverified (GNU property alignment and ordering, the OpenBSD, NetBSD
  and Go type numbers) are from memory, and the lane should check them against the specifications before the README cites them.
- This block does not review `grant_plan::Manifest`, whose fields the layout spells. If someone adds a field there without a byte here, `every_compiled_in_manifest_round_trips` refuses it at compile time. That holds only while that test lives in a tree that holds both crates (item 7 moves it to `grant_plan`'s).

## Index row

An outside review of `capability_manifest_note` before it leaves. Three items block the move (a manifest that cannot stand alone, no README, and two encoders that build notes their own decoder refuses). Six land before the first tag. They are project-internal citations in shipped text, stale documentation, private names against the spelled-out rule, tests bound to nife's program catalog, the fuzz target and seeds left behind, and the template's `unsafe` scan refusing the opt-in. Six more come after. Four one-way names go to the architect, the wire literal `nife` first.
