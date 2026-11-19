# Review package: `clock_contract`

*Name: provisional, minted by lane lane/review-prep-clock-contract. The `clock_contract` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
the leaves `disk-tools` links. It describes the crate as `main` plus this branch leaves it.

## In one paragraph

The wall clock as a contract between a clock service and everything that reads or sets the time.
Reading is a read-only mapping of a 56-byte clock page, setting is a read/write mapping of the same
page, and proposing a time is a two-word message the service answers with `policy::decide`. Wall
time is the monotonic counter plus an offset, and only the offset is ever written, so a step never
moves `Instant`. The page keeps two copies of the time and a publish count, so a reader never waits
on a writer, and its reader and writers sit in different address spaces. Six loom models check its
orderings against the C11 memory model. The crate is `no_std`, has
no run-time dependency, and has `unsafe` in one place: `ClockPage::new` takes a raw address. Two
Kani harnesses prove the policy and the request encoding. Nothing in it points back into nife. Every
public name stands as built (ruled 2026-11-19 UTC).

## Identity

| item | value |
|---|---|
| crate | `clock_contract`, ratified ("the service's own word, `clock`, with the contract suffix") |
| identity values | frozen: `INTERFACE_ID` 8, `VERSION` 1, `MAGIC` `0x434c_4f43_4b76_3031` (ASCII `CLOCKv01`), and the seven-word layout (the magic, the publish count, the writers' claim, then the state and the offset of copy 0 and of copy 1). A layout break ships as a new magic and version together. The crate header and README state them, and host tests pin them as literals. Frozen 2026-11-14 (UTC) on #3576 |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| dependencies | none at run time; `loom = "0.7"` only under `cfg(loom)`, which only a model-checking run sets |
| size | one `src/lib.rs` of about 1,550 lines: the library, its host tests, the loom models and the Kani harnesses |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`: `kernel` (the clock service in
`kernel/src/user/clock_service.rs`, which also grants the `x86_64` CMOS ports),
`components`, `crates/user_mode_runtime`, `crates/network_time_protocol`, `crates/timetable`,
`crates/uptime`, `fixtures`, and `system_tests` (development only). Fifteen programs in
`components` declare the pair of `INTERFACE_ID` and `VERSION` in their `INTERFACES` note: every
program of `disk-tools`, `process-tools` and `core-tools`.

Two more readers reach the crate outside its dependency lines:

- The standard library. `cargo xtask std-src` copies the crate's `src/lib.rs` verbatim to
  `sys/pal/nife/clockproto.rs` in the generated `std` source. It finds the file in the directory
  `cargo metadata` gives for the package (`xtask/src/farm/copied.rs`), so the std source reads a pinned crate
  from Cargo's checkout. The time support in
  `patches/std-nife/overlay/std/src/sys/time/nife.rs` reads `NANOSECONDS_PER_SECOND`, `wall_nanoseconds`,
  `Reading::offset_nanoseconds` and `state::is_known` from that copy.
- The tooling. `script/interleaving-check` and `.github/workflows/ci.yml` run the loom models.
  `helpers/interface_stability.py` reads the source for the series of milestone 610 (the
  interface's stability is measured weekly). `helpers/vendored_name_gates.py` exempts the overlay
  names, and `helpers/template_report_baseline.txt` holds three rows.
  `.cargo/mutants-baseline.txt` holds the census row (caught 54, missed 3, timeout 7, unviable 5).
  `packages/contracts.package.toml` and `crates/externalization-list.txt` list the crate.

What references back into nife from the crate: nothing. It has no path or development dependency,
no `include_str!` outside its own README, no test that reads a nife file, and no fuzz target in
nife's `fuzz/` workspace. The manifest writes `license` and `lints` as literals.

## What was run

The runs below predate the renames. They name the old spellings where those were
the finding. The move lane runs each check again on the crate as it now stands, and the template's naming helpers checked the renames when they landed.

| check | result |
|---|---|
| `script/crate-standalone clock_contract` | two failures. `names`: 37 names lacked a ratified block. `proofs`: no Kani harness. The build passes on 1.88 for the host and the three bare-metal targets. The crate has since gained two harnesses with falsification patches, and the ratified names close the `names` failure once the blocks carry the ruling |
| `script/crate-move --dry-run` | stages at template `v7`; builds on 1.88 for the host and three targets; 0 patches to rewrite. Three findings, all in the template's scaffold and listed in the runbook's `BUGS`: the pull request template names the architect, `CONTRIBUTING.md` cites a nife path, and the commit hook, `deny.toml`, `rustfmt.toml` and `docs/stranger-test.md` mention nife. The crate's own README and `OPERATION_SHIFT` doc named nife too; this branch rewords both |
| `cargo test -p clock_contract` | 12 host tests and 3 doctests pass; one doctest is the README's example |
| `script/interleaving-check` | the four loom models of `mod interleavings` pass |
| `cargo clippy --all-targets -D warnings`, `cargo doc` with warnings denied | no warning |
| `cargo package` | passes; the one warning is the missing `repository` key, which the move adds |
| `script/names clock_contract` | the crate name ratified; `script/names --unratified` lists six provisional items, four of them read outside the crate |
| `python3 helpers/name_release.py --draft clock_contract` | 41 names: 4 ratified, 6 provisional, 31 with no block |
| the template's naming checks (`vendor/crate-workflows/helpers/`, `--manifest Cargo.toml` over `src/lib.rs`) | `question_shape` and `contract_names` refuse none of 145. the then-current spellings drew refusals from `spelled_out`, `unit_symbols`, `register_names` and `british_spelling`; the renames below answer each. `abbreviations` (report only) finds 16 components, the baseline's 16 |
| `python3 helpers/lint_externalized_crates.py` | 0 hits after this branch; its waiver for the crate goes |
| `script/lint --quick` | all sections pass |

Not run, and so not claimed: the stranger test, mutation, `cargo-semver-checks` (a first release
has no baseline) and the template's workflows, which run in the repository.

## Public API

| kind | items |
|---|---|
| conversion | `NANOSECONDS_PER_SECOND`, `wall_nanoseconds(offset_nanoseconds, monotonic_nanoseconds)`, `offset_for(wall_nanoseconds, monotonic_nanoseconds)` |
| page identity and layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `WORDS` (7) |
| the page | `ClockPage`, `unsafe const fn new(virtual_address: u64)`, `initialize`, `read -> Reading`, `publish(new_state, offset_nanoseconds) -> u64` |
| a reading | `Reading { state, offset_nanoseconds, generation }` |
| `state` | `UNKNOWN` (0), `REAL_TIME_CLOCK` (1), `SET` (2), `SYNCHRONIZED` (3), `is_known(state)` |
| `propose` | `OPERATION_SHIFT` (56), `request(operation)`, `operation(word)`, `PROPOSE` (1), `STATE` (2) |
| `status` | `ACCEPTED`, `REFUSED_IMPLAUSIBLE`, `REFUSED_TOO_FAR_FORWARD`, `REFUSED_TOO_FAR_BACKWARD`, `BAD_REQUEST` |
| `policy` | `NOT_BEFORE_NANOSECONDS` (2026-01-01), `NOT_AFTER_NANOSECONDS` (2100-01-01), `MAXIMUM_STEP_FORWARD_NANOSECONDS` (an hour), `MAXIMUM_STEP_BACKWARD_NANOSECONDS` (a second), `is_plausible(unix_nanoseconds)`, `decide(current_state, current_nanoseconds, proposed_nanoseconds)` |
| `real_time_clock` | `NONE` (0), `PL031` (1), `GOLDFISH` (2), `CMOS` (3) |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- States, opcodes, statuses and clock kinds are bare `u64` constants in modules, not enums. They
  are words on a page and in a message, so a reader keeps a value a newer writer added.
- `ClockPage` is `Copy`, `Send` and `Sync` by `unsafe impl` around a raw pointer; `new`'s safety
  contract carries the whole burden. `publish` on a read-only mapping faults the caller, which the
  documentation calls the correct outcome.
- `read` never fails and never waits on a writer: a frame without the magic reads as `UNKNOWN`,
  and a read retries only when a publish finished during it. A writer stopped while it holds the
  claim blocks later writers, and nothing clears the claim; both `BUGS` sections say so.
- `propose` defines no reply layout type: the reply is two words, documented on `PROPOSE` and
  `STATE`.

## Proofs

Two Kani harnesses, each with a falsification patch under `falsifications/`. Six loom models
check the page. A reader never sees half a publish, nor a torn pair across two publishes, and a
reading's generation matches its pair. Two writers serialize, a stopped writer never makes a reader
wait, and a reader racing `initialize` sees an unrecognized page or a whole one. Each model asserts
that some execution reached each branch it expects. Loom judges against C11, not the Arm or RISC-V
model, and the crate's `BUGS` says that too.

## Names

Ruled 2026-11-19 (UTC), option A: every public name stands as built. The crate name, `MAGIC`,
`VERSION`, `INTERFACE_ID`, the two `MAXIMUM_STEP_` constants and the names below are final.

| item | kind | spelling |
|---|---|---|
| `NANOSECONDS_PER_SECOND` | constant | the unit spelled out, alias `"nanoseconds per second"`; the std overlay reads this name |
| `Reading::offset_nanoseconds` | field | the unit spelled out; the overlay reads it |
| `ClockPage::new`'s `virtual_address` | parameter | spelled out |
| `propose::request` | function | spelled out; no alias |
| `propose::operation`'s `word` | parameter | a word of a message |
| `state::SYNCHRONIZED` | constant | spelled out |
| `state::REAL_TIME_CLOCK`, the module `real_time_clock` | constant, module | each with `#[doc(alias = "RTC")]` |
| `real_time_clock::CMOS`, `PL031`, `GOLDFISH` | constants | keep their spellings: part and device names from the device tree's `compatible` strings |
| `WORDS` | constant | keeps its spelling: a count of the page's words |
| the other items, the modules `state`, `propose`, `status` and `policy`, and the fields `state` and `generation` | | pass every rule as written |

No alias carries an old spelling. The `Name:` blocks in the crate source still read provisional for
some of these items; a later lane rewrites them to ratified. Private names are for the move lane:
the word indices are `MAGIC_WORD_INDEX`, `PUBLISH_WORD_INDEX`, `CLAIM_WORD_INDEX`,
`STATE_WORD_INDEX` and `OFFSET_WORD_INDEX`, and the template's `british_spelling` check wants
American spellings in the loom test names and statics. `notes/interleaving.md`,
`notes/interleaving/what-was-modeled.md` and the mutation regression note cite those test names, so
they move together.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
`smp_wmb()` and `write_seqcount_begin` name the Linux ordering the code mirrors; the crate copies no
Linux text.
`PL031`, `goldfish-rtc` and the CMOS ports are register facts.

## What still blocks the move

| item | owner |
|---|---|
| Rewrite the provisional `Name:` blocks of the ratified names to ratified. | a later lane |
| `cargo xtask std-src` copies the crate's source by path; after the move it must read the pinned checkout, as must `helpers/interface_stability.py`, `helpers/interface_declarations_check.py`, `script/interleaving-check` and the farm stamp. | move lane |
| Convert the eight path dependencies to `git` and `rev` with `version = "0.1.0"`. | move lane |
| `helpers/template_report_baseline.txt`, `helpers/vendored_name_gates.py` and `.cargo/mutants-baseline.txt` hold rows for the crate. | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

1. Whether `propose` should carry a reply type. Recommendation: no; the reply is two words of a
   message, and the request has no type either. The maintainer can decide it.

## Recommendation

Ready for review. The ruling ratifies the names, and the crate is a leaf with nothing pointing back into
nife.
