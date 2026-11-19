# P0 and P1: before the move

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

## P0: a claim the crate makes is false

1. The reader panics on a length byte past its field, and the documentation says it cannot.
   Finding: `read_field` (`src/lib.rs` line 304 to `:310`) slices `page[data_off..data_off + n]` with `n`
   read from the page. `bytes()` is exactly `PAGE_BYTES` (107) long, so a `TERM` length above 32, a
   `LANG` length above 57 or a `TZ` length above 98 indexes past the slice and panics. The doc
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

## P1: before the move

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

3. The pre-ruled family and rule names. Finding: `PAGE_BYTES` (`src/lib.rs` line 162) is 107 bytes of
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
   reports all 19 components. No `Name:` block covers any of them. Change: the names the milestone's
   accessor decision rules, provisionally. Each accessor gains `#[doc(alias = "TZ")]`, `"LANG"` or
   `"TERM"`, and the overlay's `seed()` moves in the same change. Door: two-way before the first
   tag; the architect may revisit the names. Effort: S after the ruling.

5. No proof, and the one that matters would have caught item 1. Finding: the crate has no
   `#[cfg(kani)]` module and no `falsifications/`; `crate-standalone` fails `proofs`, and the
   template's `proofs.yml` would run nothing. `block_roster_contract`'s
   `read_is_total_and_every_counted_entry_is_gettable` is the shape this crate lacks: for any bytes,
   `read` neither panics nor indexes out of bounds. The review's trial of that harness over a fully
   symbolic page did not finish in seven minutes ("What was checked"). The expensive part is
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
   Change: the identity decision in the milestone; then a host test that asserts the three literals, and a
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
