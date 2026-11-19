# Review package: `environment_contract`

*Name: provisional, minted by lane 1089-review-package. The `environment_contract` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to ratify the crate's names and to clear its review. Clearing is the go to
create `nifeos/environment_contract` as a `publish = false` repository, as part of step 8 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
The outside review's findings are in
[milestone 1089 (`environment_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1089-environment-contract-outside-review.md).
This package describes the crate as `main` plus this branch leaves it. It changes no crate source
and renames nothing: every ruling below is a proposal for the architect.

## In one paragraph

The inert configuration a process starts with (time zone, locale, terminal type) as a contract
between the program that writes the page and every reader of it, the standard library's start-up
code first among them. The page is 107 bytes: an 8-byte magic, then one length byte and one fixed
field for each of `TZ`, `LANG` and `TERM`. A value reaches the page only as a member of a closed,
curated list, and the reader answers only a member of that list, so a page some other writer
produced cannot hand a program an arbitrary string. A length byte past its field reads as absent
and never panics. The crate is `no_std`, has no dependency, and has `unsafe` in one place:
`ConfigurationPage::new` takes a raw address. One Kani harness proves the reader's bound, with a
falsification patch. Nothing in the crate's code points back into nife; its comments and manifest
still cite nife sections, which the move lane rewrites.

## Identity

| item | value |
|---|---|
| crate | `environment_contract`, ratified 2026-08-23 |
| identity values | frozen: `INTERFACE_ID` 13, `VERSION` 1, `MAGIC` `ENVCONF1`, a 107-byte page (8 + 1 + 40 + 1 + 24 + 1 + 32). Ruled 2026-11-14 (UTC), and a host test pins the literals |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; the manifest has no `rust-version` yet, and the copy below ran on 1.88 |
| license | the manifest inherits `license` from the workspace; the files carry none. The move writes `MIT OR Apache-2.0` and both texts |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished |
| size | 900 lines in `src/lib.rs` (about 533 of library, then the host tests) and 29 in `src/verification.rs` |
| tests | 17 unit tests and 2 doctests |

## Who in nife depends on it

Four manifests reach it by path with `version = "0.1.0"`: `kernel`, `components`, `crates/swish`
and `system_tests`. `crates/user_mode_runtime` and `crates/program_argument_contract` mention it in
a comment only.

- Writers: `kernel/src/user/device_starters.rs` assembles the page with `PageBuilder`, and so do
  `system_tests/src/user/std_service.rs`, `system_tests/src/user/printenv_tests.rs` and
  `crates/swish/src/preview/tests.rs`.
- Readers: `components/src/printenv.rs`, `components/src/swish/capabilities.rs` and
  `crates/swish/src/lib.rs` use `ConfigurationPage`.
- The standard library. `cargo xtask std-src` copies `src/lib.rs` into the generated `std` source
  as `sys/pal/nife/envproto.rs` (`xtask/src/farm/copied.rs`), and
  `patches/std-nife/overlay/std/src/sys/env/nife.rs` calls `ConfigurationPage::new`, `time_zone`,
  `locale` and `terminal_type` on that copy at start-up.
- Lists that name it: `crates/externalization-list.txt`, `packages/contracts.package.toml`,
  `helpers/interface_stability.py`, `helpers/trust_boundary_crates.tsv`, `script/verify`,
  `script/metrics`, `.cargo/mutants-baseline.txt` (46 caught, 0 missed, 0 timeout, 7 unviable) and
  `design/module-block-baseline.txt`.

No reference runs the other way: the manifest has no path or development dependency.

## What was run

Each check ran in this lane's worktree on this branch, on 2026-11-19 (UTC).

| check | result |
|---|---|
| `cargo test -p environment_contract` | 17 unit tests and 2 doctests pass |
| `cargo clippy -p environment_contract --all-targets -- -D warnings` | no warning |
| `cargo doc -p environment_contract --no-deps` with `RUSTDOCFLAGS="-D warnings"` | no warning |
| bare-metal builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` | pass, by `cargo build` here and again by `script/crate-standalone` |
| MSRV copy outside the tree (`src/` plus a standalone manifest with `rust-version = "1.88"`) | `cargo +1.88.0 test` passes, 17 and 2. The 1.88.0 toolchain has no bare-metal target installed, so the bare-metal builds are not claimed on 1.88 |
| `cargo +nightly miri test -p environment_contract` | 17 unit tests and 2 doctests pass under Miri, so the reader's one `unsafe` slice over a live buffer shows no undefined behavior in the tests' cases |
| Kani, through `script/crate-standalone` | `field_bytes_never_panics_for_any_length_byte` verifies: 1 harness, 0 failures |
| falsification patch | `verification.field_bytes_never_panics_for_any_length_byte.patch` makes the harness fail |
| `script/crate-standalone environment_contract` | 10 failures, all the move's work (see the next section); the build, proofs and falsification rows pass |
| `script/crate-move --dry-run --into <scratch directory outside the tree> environment_contract` | stages at template `v8` (a69ab4829b02) with 27 scaffold files; 4 tracked files copy; the `Name:` blocks are removed from 2 files; the one patch applies with no offset; 7 findings, listed below |
| `python3 helpers/name_release.py --draft environment_contract` and `script/names environment_contract` | the table the Names section reads |
| `script/lint` | in the pull request's checks |

Not run, and so not claimed:

- Mutation. The census window is 08:00 to 13:00 UTC and this run was at 20:00. The saved baseline
  (46 caught, 0 missed, 0 timeout, 7 unviable, census 2026-10-19) is quoted, not rerun.
- The stranger test, `cargo-semver-checks` (a first release has no baseline) and the template's
  workflows, which run in the repository.
- A `std` program against a corrupt page under QEMU: the claim that `seed()` aborts or reads
  absence before `main` rests on reading the overlay.

## Public API

| kind | items |
|---|---|
| page identity and layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES` (107), `MAXIMUM_TIME_ZONE_BYTES` (40), `MAXIMUM_LOCALE_BYTES` (24), `MAXIMUM_TERMINAL_TYPE_BYTES` (32) |
| `domain` | `KNOWN_TIME_ZONES`, `KNOWN_LOCALES`, `KNOWN_TERMINAL_TYPES`, `is_known_time_zone(&str)`, `is_known_locale(&str)`, `is_known_terminal_type(&str)` |
| the writer | `PageBuilder` (`Default`, `Copy`), `PageBuilder::new`, `time_zone(&str)`, `locale(&str)`, `terminal_type(&str)`, each `-> Result<Self, Refused>`, and `build() -> [u8; PAGE_CONTENT_BYTES]` |
| the refusal | `Refused::{UnknownTimeZone, UnknownLocale, UnknownTerminalType}`, with `Display` and `core::error::Error` |
| the reader | `ConfigurationPage`, `unsafe const fn new(virtual_address: u64)`, `time_zone()`, `locale()`, `terminal_type()`, each `-> Option<&str>`; `Send` and `Sync` by `unsafe impl` |

No nife type, capability or handoff appears in a public signature. What a stranger may find
surprising:

- The reader answers `None` for a zeroed or foreign frame, for a key never declared, for a length
  past its field, and for a value outside its key's domain. It never panics and never returns a
  value the builder would have refused.
- `Refused` is exhaustive. A fourth key is a new `VERSION` by the contract's own rule, so adding
  the variant breaks nothing that the version check does not already refuse.
- The accessors have two spellings of the same word on the same page, the call name and the POSIX
  variable name, which the aliases join.

## Proofs

One Kani harness, `field_bytes_never_panics_for_any_length_byte`, with a falsification patch. It
covers the bound that once panicked (a length byte of 255 on the last field). The wider harnesses
(round trip, domain membership) ran past 25 minutes and are item 5 of milestone 1089, still open.
The host tests cover the round trip, the refusals, a recognized page carrying a non-member, a
zeroed frame, and one length past each cap.

## Names

Proposals for the architect. This lane renames nothing. The tree rules that decide each row are
[design/naming.md](../../design/naming.md): spell out words and units, a spec term of art spelled
out with `#[doc(alias = "<spec term>")]`, the page family's constants in
[design/naming/crates.md](../../design/naming/crates.md), a question answers with `is_`, restrictive
first, and `id` accepted. The milestone calls the provisional set eleven names. The list below has
ten groups and twelve spellings, so the proposal covers every spelling, and the architect's
ratification can name them all.

| name | kind | block now | proposal and reason |
|---|---|---|---|
| `PAGE_CONTENT_BYTES` | constant | provisional | Ratify. It is the page family's name for the content length (`design/naming/crates.md`), it spells its unit, and the crate has no real page size to confuse it with |
| `ConfigurationPage` | type | provisional | Ratify. It spells `Config`, matches the writer `PageBuilder` and the sibling reader types `ClockPage`, `CurrentCpuPage` and `CounterFrequencyPage`, which name their subject and then `Page`; the crate name carries `environment` |
| `ConfigurationPage::new`'s `virtual_address` | parameter | provisional | Ratify. It is the page family's spelling of an address parameter, and `PAGE_VIRTUAL_ADDRESS` is absent here because the caller picks the address |
| `PageBuilder::time_zone`, `ConfigurationPage::time_zone` | methods | provisional | Ratify, with `#[doc(alias = "TZ")]`, which both carry. `TZ` is POSIX's variable name, so the alias is the spec term; the old spelling `tz` takes no alias |
| `PageBuilder::locale`, `ConfigurationPage::locale` | methods | provisional | Ratify, with `#[doc(alias = "LANG")]`. `locale` is the standard word and `LANG` the variable |
| `PageBuilder::terminal_type`, `ConfigurationPage::terminal_type` | methods | provisional | Ratify, with `#[doc(alias = "TERM")]`. `terminal_type` spells `term`; `TERM` is the variable |
| `domain::is_known_time_zone`, `is_known_locale`, `is_known_terminal_type` | functions | provisional | Ratify. They answer yes or no with `is_` (`design/naming/functions-that-answer-yes-or-no.md`), and `known` names the claim about the list, which matches `KNOWN_TIME_ZONES` and its siblings. `valid` would claim more |
| `Refused::UnknownTimeZone`, `UnknownLocale`, `UnknownTerminalType` | variants | no block | Ratify. They spell their words and say what was wrong with the value, `unknown` against the list, in step with `is_known_*`. They carry no alias, because a variable name does not name an error |
| `PageBuilder`, `PageBuilder::new`, `build`, `Refused`, the module `domain` | items | no block | Ratify as built. `design/naming/crates.md` puts `PageBuilder::build` outside the page family rule, and each passes the template's spelled-out and unit checks. `Refused` is also the name `program_argument_contract` and `base_digest_contract` give their refusal enums |
| `MAGIC`, `VERSION`, `INTERFACE_ID` | constants | ratified 2026-11-14 | Stand |
| `KNOWN_TIME_ZONES`, `KNOWN_LOCALES`, `KNOWN_TERMINAL_TYPES`, `MAXIMUM_*_BYTES` | constants | ratified 2026-10-27 | Stand; each carries its alias and spells its unit |
| the module `verification` | module | provisional | Ratify. It is the name every crate's harnesses use |

The private names (`tz`, `lang` and `term` as the builder's fields, and the offset constants) pass
the template's checks and are the move lane's to touch. The `Name:` blocks of the ratified rows are
ratified, so they need no edit. The provisional blocks (ten distinct names, six accessor blocks and
three function blocks among them) and the four items with no block are the ones this ratification
settles. The move strips the `Name:` blocks (`crate-move` reports them removed from 2 files), so a
lane need not rewrite them first; if the architect wants them rewritten in nife before the move, that
is a short lane.

A rename would sweep: the overlay (`sys/env/nife.rs` lines 144 to 146, three accessor calls and
one `new`), `components/src/printenv.rs` (lines 115 to 117), `system_tests/src/user/std_service.rs`
(lines 324 to 328), `system_tests/src/user/printenv_tests.rs`, `crates/swish/src/preview/tests.rs`,
`kernel/src/user/device_starters.rs` (lines 299 to 306, with their `expect` messages), the crate's
tests and doctests, and `notes/env-config.md`. After the move it also costs a coordinated change
across two repositories.

## What still blocks the move

`script/crate-standalone` names ten failures and `script/crate-move --dry-run` seven findings. Both
are the move lane's work.

| item | owner |
|---|---|
| Write a README (the scaffold is staged; the prose cap refuses a 423-word draft), say there that fuzzing stays off because the crate has `unsafe`, and add item 12's growth-path row | move lane |
| Write `license`, `lints` and `rust-version = "1.88"` into the manifest, and add `LICENSE-MIT` and `LICENSE-APACHE` | move lane |
| Rewrite 5 section citations, 2 milestone citations and 3 paths into the nife tree (the manifest's comment and the crate header's cites of §111, §46 and `notes/env-config.md`) | move lane |
| `cargo generate-lockfile` fails in the staged copy only until the manifest stops inheriting from the workspace | move lane |
| Convert four manifests (`kernel`, `components`, `crates/swish`, `system_tests`) to `git` and `rev` with `version = "0.1.0"` | move lane |
| `xtask/src/farm/copied.rs` copies `src/lib.rs` by path; it must read the pinned checkout. The move also reads `helpers/interface_stability.py`, `helpers/template_report_baseline.txt`, `helpers/trust_boundary_crates.tsv`, `script/verify`, `script/metrics`, `crates/externalization-list.txt`, `packages/contracts.package.toml` and the mutants baseline | move lane |
| Items 5, 11 and 12 of milestone 1089 (wider Kani harnesses, a safe reader, the growth path) stay open; none blocks the repository, and the two-way ones are the maintainer's | later lanes |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository | move lane |

## Open questions

None. The names are the architect's, and the proposals above carry the reasons.

## Recommendation

Ready for review. Ratify the names as proposed and clear the review, which is the go to create
`nifeos/environment_contract` as a `publish = false` repository. The crate is a leaf with no
reference back into nife and a frozen identity.
