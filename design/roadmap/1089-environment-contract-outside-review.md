---
status: PARTIAL
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

## Finding status

One line per finding, in the review's order: items 1 to 8 in [the before-the-move
appendix](1089-environment-contract-outside-review/before-the-move.md), items 9 to 12 in [the
later-items appendix](1089-environment-contract-outside-review/later-items.md), then the decisions
and the release steps.

- Item 1 (P0), the reader panics on an oversized length byte: Done,
  [#3529](https://github.com/nifeos/nife/pull/3529).
  `a_length_past_its_field_reads_that_field_as_absent` holds it.
- Item 2 (P1), the closed domain holds on the writer only: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566). Each accessor answers only a member of its
  key's domain.
- Item 3 (P1), the family and rule names: Done, [#3726](https://github.com/nifeos/nife/pull/3726).
  `PAGE_CONTENT_BYTES`, `ConfigurationPage` and `virtual_address`, provisional until the review.
- Item 4 (P1), the abbreviated accessors: Done, [#3726](https://github.com/nifeos/nife/pull/3726).
  `time_zone`, `locale`, `terminal_type`, `is_known_*` and `Unknown*`, each accessor with its POSIX
  alias, provisional until the review.
- Item 5 (P1), no Kani proof: Open. Kani proves `field_bytes_never_panics_for_any_length_byte`, with
  a falsification patch; the round-trip and domain-member harnesses each ran past 25 minutes under
  Kani and return only with the text scan bounded or the shape split, each with its patch.
- Item 6 (P1), the crate does not stand alone: Open. The manifest still inherits `license` and
  `lints` and has no `rust-version`, there is no README or license file, and `src/lib.rs` cites
  §111, §46 (thin primitives, whole subsystems) and `notes/env-config.md`. All of it is the move's
  work, with a README that says fuzzing stays off because the crate has `unsafe`.
- Item 7 (P1), the identity is provisional and unpinned: Done,
  [#3585](https://github.com/nifeos/nife/pull/3585). `the_frozen_identity_and_widths_are_pinned`
  asserts the literals and that `MAGIC[7]` is `b'0' + VERSION`, as a test assertion rather than the
  `const` assertion the item asked for.
- Item 8 (P1), truncation writes a plausible wrong value: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566). `const` assertions hold every domain member to
  its field, `write_field` asserts, and the private names spell their words out.
- Item 9 (P2), `Refused` lacks `Display` and `Error`: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 10 (P2), five documentation sentences: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 11 (P2), a safe reader over borrowed bytes: Open. Only `pub const unsafe fn new` exists; the
  safe reader is a new public name and an additive two-way door, which the maintainer rules
  provisionally.
- Item 12 (P2), the curated lists have no growth path: Open. The crate's `BUGS` states the policy
  ([#3566](https://github.com/nifeos/nife/pull/3566)); the README's row comes with item 6.
- Decision, the identity: Done, [#3585](https://github.com/nifeos/nife/pull/3585). Ruled
  2026-11-14 UTC.
- Decision, the accessor names: Done, [#3726](https://github.com/nifeos/nife/pull/3726). Maintainer
  ruling, provisional, 2026-11-14 UTC.
- Review package: Done, [the package](../../notes/crate-reviews/environment-contract.md). It lists
  every public name with a proposal, the checks run, the move's work, the dependents and what a
  rename would sweep.
- Review clearance and name ratification: Open. The decision has its ruling (below); the architect
  ratifies the names from the package and clears the review, which is the go to create the
  repository.

## Open decisions

None.

## Goal

A stranger who finds `environment_contract` in its own repository can tell in one screen what the
page is, and can build and test it from the README. The same stranger can trust two claims the crate
makes today. One is that a page the builder did not write reads as absence rather than a panic. The
other is that a value outside a key's closed domain cannot reach a reader. The repository's gates
are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1089-environment-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1089-environment-contract-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L more than one lane. A door is one-way when
somebody outside the repository acts on it after a release (a public name, a wire layout, an
identity value, a license). Everything else is two-way and the maintainer decides it. The crate has no release and nothing outside nife links it, so every public name is two-way until the move and
one-way after. The items say which.

### P0 and P1: before the move

The P0 and P1 items are in [the
appendix](1089-environment-contract-outside-review/before-the-move.md).

### P2: after the move, before a release

The P2 items are in [the appendix](1089-environment-contract-outside-review/later-items.md).

## Decisions for the architect

Each states the background, the recommendation first, the prior art and one concrete example. The
maintainer rules the accessor names, provisionally, and the architect ratifies them from the review
package.

### Freeze the identity: `INTERFACE_ID` 13, `VERSION` 1, `MAGIC` `ENVCONF1`, and the 107-byte layout

Ruled 2026-11-14 (UTC): option A, which widens the terminal type field from 24 to 32 bytes and
then freezes the identity.

In force. `INTERFACE_ID` is 13, `VERSION` is 1 and `MAGIC` is `ENVCONF1`. The layout starts with 8
bytes of magic. Then `TZ`, `LANG` and `TERM` each take one length byte and a field of 40, 24 and 32
bytes. The sum is 8 + 1 + 40 + 1 + 24 + 1 + 32 = 107 bytes, inside one frame. The version stays 1
because nothing outside nife has read the page. A fourth key is a new version by the contract's own
rule, so nothing is reserved. A host test pins the identity and the widths as literals.

Background. A program's `INTERFACES` note declares the pair of this id and version, and the
progenitor refuses a mismatch at spawn. A shared page is single-version: a reader built for one
magic reads another's page as no configuration at all. Once a program outside nife builds against
these values, changing any of them is a coordinated release across repositories, which is why
milestone 1017 asks for ratified values before a contract crate leaves. The review measured the
caps against the real databases. A zone takes 40 bytes against 32 for the longest IANA name. A
locale takes 24 against 20 for the longest glibc name with a modifier (`ca_ES.UTF-8@valencia`,
recalled, unverified). A terminal type took 24 against 15 in the list and 21 for the longest
common terminfo names (`screen.xterm-256color`, recalled, unverified), which left little
headroom, hence the widening to 32.

Prior art. `block_roster_contract`, `entropy_contract` and `counter_frequency_contract` each froze
their identity before leaving. Linux never renumbers an `AT_*` auxiliary-vector key and never
changes a value's meaning; a new fact is a new key.

Example. A basalt image whose kernel writes `ENVCONF2` while a published `printenv` compares
`ENVCONF1`: `printenv` prints "no configuration was granted" for every key, never a wrong value,
and the interface check at spawn refuses the pair before that. The freeze is what makes that
refusal mean something to a reader outside this tree.

### Clear the review of `environment_contract` and create its repository

Ruled 2026-11-19 (UTC): option A. A lane writes the review package, which carries the ratification
of the provisional names. The architect ratifies the eleven names from it and clears the review,
asked once the package exists; that clearance is the go to create `nifeos/environment_contract` as
a `publish = false` repository.

In force. The review package is `notes/crate-reviews/environment-contract.md`. It lists every
public name with a proposed ruling and marks them proposals. The names and the repository wait on
the architect's reading of it. The move follows, with item 6's files.

Background. `environment_contract` is the agreement between the program that writes a process's
inert configuration page (time zone, locale, terminal type) and every reader of that page, the
standard library's start-up code first among them. It leaves for a repository of its own at step 8
of milestone 1017. A crate leaves once its review package lists every public name and decision and
the architect clears that review. The identity (13, 1, `ENVCONF1`, 107 bytes) has its ruling. The
public names the lanes minted (`PAGE_CONTENT_BYTES`, `ConfigurationPage`, `virtual_address`,
`time_zone`, `locale`, `terminal_type`, the three `is_known_*` functions and the `Unknown*`
variants) are provisional. They are free to change while only this tree spells them and bind every
reader once a release does.

Door: one-way. The repository is an outward step, and a ratified name binds the standard library
overlay and every program that reads the page.

Prior art: `block_roster_contract` and `entropy_contract` each froze an identity and had a review
package cleared before the organization made its repository, with no publish. POSIX fixed `TZ`,
`LANG` and `TERM` once and has never renamed them, which is why the aliases carry those spellings.
Concrete example: ratifying `ConfigurationPage::time_zone` fixes the call in the overlay's
`sys/env/nife.rs`, and a later rename is a coordinated change across two repositories.

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

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The recommendation holds: `time_zone`, `locale` and
`terminal_type` on both types, `is_known_time_zone` and its siblings, and
`Refused::UnknownTimeZone` and its siblings. Each accessor carries `#[doc(alias = "TZ")]`, `"LANG"`
or `"TERM"`, since POSIX's variable names are the source's terms, and no alias of the old
spellings. The standard library overlay's call (`overlay/std/src/sys/env/nife.rs` line 146) moves
in the same change. Item 4 is the work.

## Done when

The checks are in [the appendix](1089-environment-contract-outside-review/done-when.md).

## BUGS

- The review did not test a `std` program against a corrupt page under QEMU, so the
  claim that `seed()` aborts before `main` on such a page comes from reading `sys/env/nife.rs`, not from a run.
- The review has no Kani verdict: its one trial harness was too wide to finish inside a
  seven-minute cap on one machine. Item 5's two wider harness shapes are a proposal, not a measured
  run; the lane that lands them measures with `/usr/bin/time -v` for the README.
- The glibc and terminfo lengths in decision 1 come from recall, not from the databases, and carry a mark saying so. The review read the IANA lengths from this host's tzdata.
- The review did not run `script/crate-move --dry-run`. The template scaffold's findings (the pull
  request template, `CONTRIBUTING.md`, the commit hook) are the runbook's and are not repeated here.
- The mutation figures are the saved baseline's; the review did not rerun the census.

## Follow-on

- **Outstanding.** Every finding that Finding status above marks Open or Ruled, each with what
  remains, and each open decision under Open decisions. A lane checked each finding line against
  the crate and its pull requests on main.

## Index row

An outside review of `environment_contract` before it leaves. Of 16 findings and steps, eleven are
done: the reader panic, reader-side validation, the family and accessor names, the identity freeze
and the compile-time cap check among them. Five are open: two wider Kani harnesses, the standalone
files, a safe reader, the README's growth-path row and the architect's reading of the review
package, which ratifies the names and clears the review, which creates the repository. The
review package exists, and no decision is open.
