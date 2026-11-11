---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017, 956
decision_dependencies: 111, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1089. `environment_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/environment_contract` as a stranger to this tree would meet
it, before the crate leaves for a repository of its own at step 8 of milestone 1017 (the contract
crates leave in this order). It changes nothing; it lists what the next lanes change.

The crate has no review package in `notes/crate-reviews/`, so this block has no settled baseline to
inherit. The names it treats as settled are the ones whose `Name:` blocks read ratified: the crate
name, `KNOWN_TIME_ZONES`, `KNOWN_LOCALES`, `KNOWN_TERMINAL_TYPES`, `MAXIMUM_TIME_ZONE_BYTES`,
`MAXIMUM_LOCALE_BYTES` and `MAXIMUM_TERMINAL_TYPE_BYTES`. None of those collides with a later rule;
each spells its words and its unit out. The decision the crate implements, §111 (inert configuration
is a read-only page, and each declared key is validated against a closed domain), is not reopened.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-move`, Kani with the falsification-patch shape `block_roster_contract` carries,
cargo-mutants. No new tool.

## Goal

A stranger who finds `environment_contract` in its own repository can tell in one screen what the
page is, and can build and test it from the README. The same stranger can trust two claims the crate
makes today. One is that a page the builder did not write reads as absence rather than a panic. The
other is that a value outside a key's closed domain cannot reach a reader. The first claim is false
today and the second holds only on the writer's side. The repository's gates are green for the
crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1089-environment-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1089-environment-contract-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L more than one lane. A door is one-way when
somebody outside the repository acts on it after a release (a public name, a wire layout, an
identity value, a license). Everything else is two-way and the maintainer decides it. The crate has no release and nothing outside nife links it, so every public name is two-way until the move and
one-way after. The items say which.

### P0: a claim the crate makes is false

1. The reader panics on a length byte past its field, and the documentation says it cannot.
   Finding: `read_field` (`src/lib.rs` line 304 to `:310`) slices `page[data_off..data_off + n]` with `n`
   read from the page. `bytes()` is exactly `PAGE_BYTES` (99) long, so a `TERM` length above 24, a
   `LANG` length above 49 or a `TZ` length above 90 indexes past the slice and panics. The doc
   comment at `:300` to `:303` says a foreign or corrupt page answers "absence rather than a panic".
   The test comment at `:523` to `:529` says the page "must not read out of bounds or panic" and
   then tests only a length of zero. The crate docs of `program_argument_contract` cite this reader
   as the "default-honest shape" that checks "every length against what is left". Reached from
   `sys/env/nife.rs`'s `seed()` before `main`, from `printenv` and from the shell's `caps` preview.
   Only the progenitor writes the page today, so no untrusted writer reaches it. The defect is a
   false claim and a startup abort on a corrupt frame, not a memory-safety hole, since the code checks the index. Change: give `read_field` the field's cap and answer `None` when `n` is zero or above
   the cap. Add one test per field with a length one past the cap and one with 255. Reword the two
   comments to state the bound. Then `program_argument_contract`'s citation becomes true. Door:
   two-way; no page `PageBuilder` writes has such a byte. Effort: S.

### P1: before the move

2. The closed domain is a writer-side guarantee only. Finding: `tz()`, `lang()` and `term()`
   (`src/lib.rs` line 380 to `:398`) return whatever UTF-8 the length byte covers. With item 1 fixed a
   length under the cap still returns a value with trailing NUL bytes, or any string a writer other
   than `PageBuilder` put there. §111's reason for the domains is that no shape tells bytes on a page apart. The domains live in this crate and cost 20, 12 and 11 string compares,
   once, at process start. Change: an accessor answers `Some` only for a member of its key's domain
   (`domain::is_valid_tz(value).then_some(value)`), and the doc comments say a reader never sees a
   value outside the set. Add a test that a recognized page carrying a non-member reads as absent.
   The alternative, leaving the reader raw and documenting that callers check the domain, is the
   shape `counter_frequency_contract` kept for `hertz()`. Here the domain is the whole point of the
   contract, and a later release can loosen the stricter reader, but once a program depends on the looser one, no release can tighten it. Door: two-way until the move, since only pages a conforming writer
   never produces change behavior. The maintainer can decide it. Effort: S.

3. The pre-ruled family and rule names. Finding: `PAGE_BYTES` (`src/lib.rs` line 162) is 99 bytes of
   content, and the page contract family rule in `design/naming/crates.md` names that
   `PAGE_CONTENT_BYTES`, reserving `PAGE_BYTES` for a real page size. `helpers/name_release.py
   --draft` prints the rename as pre-ruled. `ConfigPage` (`:337`) breaks the spelled-out rule
   (`config`) and sits in `design/naming-rule-baseline.txt`. `ConfigPage::new`'s parameter `va`
   (`:359`) is on the spelled-out list, and the family rule names the parameter `virtual_address`.
   The template's `spelled_out` gate would refuse the last two on the repository's first pull
   request. Change: `PAGE_CONTENT_BYTES`, `ConfigurationPage`, `virtual_address`, each `provisional`
   until the name review, with `crates/swish/src/preview/tests.rs`, `components/src/printenv.rs`,
   `crates/swish/src/endowment.rs`, `crates/swish/src/preview.rs`,
   `components/src/swish/capabilities.rs`, the overlay's `sys/env/nife.rs` and
   `sys/pal/nife/mod.rs`, and the two baselines moving in the same change.
   `grant_plan::Manifest::configuration` and `SHELL_CONFIGURATION_SLOT` already use the spelled-out
   word. Door: public names, one-way after the move and two-way now. The rules that decide them are
   already ruled, so nothing new needs deciding. Effort: S.

4. The abbreviated accessor family: `tz`, `lang`, `term`. Finding: the public functions `tz`,
   `lang`, `term` (on both `PageBuilder` and `ConfigPage`), `is_valid_tz`, `is_valid_lang`,
   `is_valid_term`, the variants `UnknownTz`, `UnknownLang`, `UnknownTerm`, and the private
   `OFF_TZ*` and `OFF_LANG*` constants carry none of the words the ratified constants spell out
   (`TIME_ZONE`, `LOCALE`, `TERMINAL_TYPE`). `TZ`, `LANG` and `TERM` are POSIX's own variable names,
   which the naming rules let a name keep only with the expansion in the doc comment and a
   `#[doc(alias)]`. The accessors carry neither (the aliases sit on the constants). `abbreviations`
   reports all 19 components. No `Name:` block covers any of them. Change: the decision below.
   Whichever way it goes, each accessor gains `#[doc(alias = "TZ")]`, `"LANG"` or `"TERM"`, and the
   overlay's `seed()` moves in the same change. Door: public function names, one-way after the move;
   an architect names public functions. Effort: S after the ruling.

5. No proof, and the one that matters would have caught item 1. Finding: the crate has no
   `#[cfg(kani)]` module and no `falsifications/`; `crate-standalone` fails `proofs`, and the
   template's `proofs.yml` would run nothing. `block_roster_contract`'s
   `read_is_total_and_every_counted_entry_is_gettable` is the shape this crate lacks: for any bytes,
   `read` neither panics nor indexes out of bounds. The review's trial of that harness over a fully
   symbolic 99-byte page did not finish in seven minutes ("What was checked"). The expensive part is
   `from_utf8` over a symbolic slice of up to 255 bytes, and it is also the part that does not
   matter: the bound to prove is on the slice, not on its decoding. Change: a `verification` module
   with three harnesses, each with a falsification patch the template's `falsify` job replays red.
   One: for any three length bytes and a correct magic, the slice `read_field` takes lies inside the
   page (prove the bound on the index arithmetic with the bytes of the fields concrete, so the UTF-8
   loop is not symbolic). Two: for any choice of one member per domain, build then read returns the
   same three members. Three: after item 2, any value an accessor returns is a member of its domain.
   Measure the run with `/usr/bin/time -v` for the README, as the scaffold asks. Door: two-way.
   Effort: M.

6. The crate does not stand alone. Finding: `crate-standalone` lists the rows. The manifest
   inherits `license` and `lints`, has no `rust-version`, `description`, `repository`, `readme`,
   `keywords` or `categories`, and its `publish = false` comment points at
   `crates/publish-allowlist.txt`. There is no README, `LICENSE-MIT` or `LICENSE-APACHE`.
   `src/lib.rs` cites `notes/env-config.md` (`:7`), three decision sections, two milestones,
   `script/lint`, `clock_contract::MAGIC`, `clock_contract::status`, `filesystem_contract`,
   `PageFrame`, `init` and "the `std` PAL", and its `Name:` blocks name the architect, which the
   template's `standalone` check refuses as a finding of its own. The template refuses a fuzz opt-in
   for a crate with `unsafe`, and this crate has three sites (`new`, `bytes`, the `Send`/`Sync`
   impls), so the README must say fuzzing stays off and why. Change: the manifest rows as
   `clock_contract` writes them (`rust-version = "1.88"`, literal `license`, the `[lints]` tables
   inlined, `description`, `readme`, `keywords`, `categories`). A README in the scaffold's shape
   (one line, the contract as a bullet list with the layout table, `## EXAMPLES`, `## What is
   proven`, `## Building and testing`, `## BUGS` with the curated-list policy and how to add a zone, `## Minimum Rust version`, `## License`). Both license texts. Every citation rewritten in
   the crate's own terms ("the assembler", "the reader", "a frame mapped read-only"), and the
   `Name:` blocks stripped by `script/crate-move` as the runbook already does. Door: two-way.
   Effort: M.

7. The identity is provisional and unpinned.
   Finding: `INTERFACE_ID` (13) and `VERSION` (1) carry provisional `Name:` blocks, and
   `script/names --unratified` lists both crossing into `components`. Milestone 1017 asks for
   ratified identity values before a contract crate leaves. No test pins `MAGIC`, `VERSION` and
   `INTERFACE_ID` as literals (`clock_contract` has one), and nothing ties the digit `MAGIC`
   carries to `VERSION` although the doc comment on `VERSION` says they move together.
   Change: the decision below; then a host test that asserts the three literals, and a
   `const` assertion that `MAGIC[7] == b'0' + VERSION as u8`.
   Door: one-way; the identity is what a release binary compares. Effort: S after the ruling.

8. The truncation path writes a plausible wrong value. Finding: `write_field` (`src/lib.rs` line 292
   to `:298`) truncates a value to the field's cap and writes the cap as its length. A domain member
   longer than its cap would reach the page cut short, which, after item 2, a reader refuses. Before
   it, a reader returns the cut string as configuration. The crate's `BUGS` says a host test catches
   this first, which is a gate (rung 2) where a compile-time check (rung 1) costs the same. Change:
   a `const` block that walks each domain and asserts every member's length against its cap, so a
   list grown past its cap fails to compile. Then `write_field` asserts instead of truncating, and
   the `BUGS` entry and the `vendored_name_gates.py` exemption for `cap` go. Rename the parameters
   while there: `cap` reads as capability in this tree and the template's `spelled_out` refuses it.
   `val`, `len_off` and `data_off` spell out to `value`, `length_offset` and `value_offset`.
   `OFF_TZ_LEN` and its siblings to `TIME_ZONE_LENGTH_OFFSET` and `TIME_ZONE_OFFSET`, which also
   clears the three `register_names` refusals. Door: two-way; private names. Effort: S.

### P2: after the move, before a release

9. `Refused` implements neither `Display` nor `core::error::Error`.
   Finding: `src/lib.rs` line 170 to `:178`. A caller cannot use `?` into a `Box<dyn Error>` or print
   the refusal without matching. `core::error::Error` is stable in `core`, so `no_std` is no bar.
   Change: both impls, each variant's message naming the key and the domain constant.
   Door: two-way, additive. Effort: S.

10. Documentation that disagrees with the code. Finding: `MAXIMUM_TIME_ZONE_BYTES`'s doc says
    the longest IANA identifiers "run to about 33 bytes". The longest is 32, a backward link, and
    the longest canonical names are 30 in this host's tzdata. `notes/env-config.md` calls the crate
    name provisional while the crate's `Name:` block reads ratified.
    `system_tests/src/user/printenv_tests.rs` line 89 cites a test
    `a_zeroed_page_reads_as_no_configuration` that does not exist (the test is
    `a_zeroed_frame_carries_no_configuration`).
    `notes/bugs-without-milestones/kernel-abi-and-component-plumbing-1.md` line 13 says no manifest field
    declares the page; `grant_plan::Manifest::configuration` does. The `KNOWN_LOCALES` doc does not
    say that the page spells the codeset `UTF-8` while glibc's `locale -a` prints `utf8` (whether
    `setlocale` normalizes the two stays unverified here). Change: the five sentences. The nife-side
    rows belong to the move lane's pull request in nife. Door: two-way. Effort: S.

11. The reader's constructor takes a raw address where the family's newest member takes bytes.
    Finding: `ConfigPage::new(va: u64)` is `unsafe`, stores a raw pointer and is `Copy`, `Send` and
    `Sync` by `unsafe impl`. The whole burden sits on the caller's promise about a mapping.
    `program_argument_contract::ArgPage::parse(&[u8; PAGE_BYTES])` is safe, and every host test and
    doctest in this crate already holds a `[u8; PAGE_BYTES]` it casts to an address. The `Send` and
    `Sync` argument is sound as written (one writer, finished before the first reader, immutable
    bytes thereafter) and `notes/unsafe-obligations/ceilings.md` records it. Change: none to the
    family shape, which `clock_contract` and `counter_frequency_contract` share and whose consumers
    hold an address, not a slice. Recommended: a safe `fn read(page: &[u8; PAGE_CONTENT_BYTES]) ->
    ConfigurationView<'_>` or three free functions over a borrowed array, so a host program and the
    tests never write `unsafe`, with the address constructor kept for the mapped case. The
    maintainer can decide it. Door: two-way, additive. Effort: S.

12. The curated lists have no stated growth path for a stranger.
    Finding: the crate's `BUGS` says the lists "grow on demand" and points at §46; a stranger with
    `TZ=Europe/Madrid` finds it refused and no instruction. Twenty zones, twelve locales and eleven
    terminal types is a demonstration-sized set.
    Change: the README's `BUGS` names the policy (a real IANA, glibc or terminfo name, added by a
    pull request that cites the source) and the alternative refused (compiling the full tables in,
    a dependency decision). No code change.
    Door: two-way. Effort: S, inside item 6.

## Decisions for the architect

Only the one-way doors. Each states the background, the recommendation first, the prior art and
one concrete example.

### Freeze the identity: `INTERFACE_ID` 13, `VERSION` 1, `MAGIC` `ENVCONF1`, and the 99-byte layout

Background. A program's `INTERFACES` note declares the pair of this id and version, and the
progenitor refuses a mismatch at spawn. A shared page is single-version: a reader built for one
magic reads another's page as no configuration at all. Once a program outside nife builds against these values, changing any of them is a coordinated release across repositories, which is why
milestone 1017 asks for ratified values before a contract crate leaves. The layout is: 8 bytes of
magic, then for each of `TZ`, `LANG`, `TERM` one length byte and a field of 40, 24 and 24 bytes,
99 bytes in all, inside one frame.

Recommendation. Freeze all four as they are. The review measured the caps against the real
databases: 40 bytes for a zone against 32 for the longest IANA name. 24 for a locale against 20 for
the longest glibc name with a modifier (`ca_ES.UTF-8@valencia`, recalled, unverified). 24 for a
terminal type against 15 in the list and 21 for the longest common terminfo names
(`screen.xterm-256color`, recalled, unverified). The length byte and the fields leave nothing to
reserve, because a fourth key is a new version by the contract's own rule, not a gap to fill.

Prior art. `block_roster_contract`, `entropy_contract` and `counter_frequency_contract` each froze
their identity before leaving. Linux never renumbers an `AT_*` auxiliary-vector key and never
changes a value's meaning; a new fact is a new key.

Example. A basalt image whose kernel writes `ENVCONF2` while a published `printenv` compares
`ENVCONF1`: `printenv` prints "no configuration was granted" for every key, never a wrong value,
and the interface check at spawn refuses the pair before that. The freeze is what makes that
refusal mean something to a reader outside this tree.

### The accessor names: `tz`, `lang`, `term` or `time_zone`, `locale`, `terminal_type`

Background. The ratified constants spell the words out (`KNOWN_TIME_ZONES`, `MAXIMUM_LOCALE_BYTES`, `KNOWN_TERMINAL_TYPES`). The functions that read and write the same three
fields, and the refusal variants, use POSIX's abbreviations: `tz()`, `lang()`, `term()`,
`is_valid_tz`, `UnknownTz`. The naming rules let a foreign term stay only as a kept foreign name
with its expansion and an alias, and these items carry neither. After the move the names are public
API in a repository others read, and the standard library overlay calls `page.tz()`.

Recommendation. Spell them out, matching the ratified constants: `PageBuilder::time_zone`, `locale`,
`terminal_type`. `ConfigurationPage::time_zone`, `locale`, `terminal_type`.
`domain::is_known_time_zone`, `is_known_locale`, `is_known_terminal_type` (the lists carry the prefix `KNOWN_`, and "valid" is a claim about the value, "known" a claim about the list).
`Refused::UnknownTimeZone`, `UnknownLocale`, `UnknownTerminalType`. Each accessor carries
`#[doc(alias = "TZ")]`, `"LANG"` or `"TERM"` so a reader who searches for the variable name lands on
it. The alternative keeps `tz`, `lang`, `term` as POSIX's terms of art with the expansion opening
each doc comment and the same aliases. It is cheaper by a dozen call sites and leaves the crate with
two spellings of one thing.

Prior art. Rust's standard library spells its words (`std::env::var`, `temp_dir`, `current_dir`).
POSIX's `TZ` and `LANG` are the variable names, and the C functions that consume them are `tzset`
and `setlocale`. `chrono-tz` names its type `Tz`, the one widely used Rust abbreviation, and nothing
in this tree's own vocabulary uses it.

Example. `PageBuilder::new().time_zone("UTC")?.locale("C")?.terminal_type("dumb")?.build()` against
today's `.tz("UTC")?.lang("C")?.term("dumb")?`; a refusal reads `Refused::UnknownTimeZone` in a
log a stranger has to understand.

## Done when

The checks are in [the appendix](1089-environment-contract-outside-review/done-when.md).

## BUGS

- A test the review wrote and deleted showed item 1. The lane that fixes it writes the test
  into the crate. The review did not test a `std` program against a corrupt page under QEMU, so the
  claim that `seed()` aborts before `main` on such a page comes from reading `sys/env/nife.rs`, not from a run.
- The review has no Kani verdict: its one trial harness was too wide to finish inside a
  seven-minute cap on one machine. Item 5's harness shapes are a proposal, not a measured run; the
  lane that lands them measures with `/usr/bin/time -v` for the README.
- The glibc and terminfo lengths in decision 1 come from recall, not from the databases, and carry a mark saying so. The review read the IANA lengths from this host's tzdata.
- The review did not run `script/crate-move --dry-run`. The template scaffold's findings (the pull
  request template, `CONTRIBUTING.md`, the commit hook) are the runbook's and are not repeated here.
- The mutation figures are the saved baseline's; the review did not rerun the census.

## Index row

An outside review of `environment_contract` before it leaves. One false claim comes first (a corrupt
length byte panics the reader that every `std` program runs before `main`). Seven items come before
the move (reader-side validation, the pre-ruled family names, the abbreviated accessors, three Kani
harnesses, the standalone files and manifest rows, the identity freeze, a compile-time check in
place of a truncation). Four more follow for the first release.
