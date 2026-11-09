# Review package: `clock_contract`

*Name: provisional, minted by lane lane/review-prep-clock-contract. The `clock_contract` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
the leaves `disk-tools` links. It describes the crate as `main` plus this branch leaves it.

## In one paragraph

The wall clock as a contract between a clock service and everything that reads or sets the time.
Reading is a read-only mapping of a 32-byte clock page, setting is a read/write mapping of the same
page, and proposing a time is a two-word message the service answers with `policy::decide`. Wall
time is the monotonic counter plus an offset, and only the offset is ever written, so a step never
moves `Instant`. The page is a seqlock whose reader and writers sit in different address spaces,
and four loom models check its two fences against the C11 memory model. The crate is `no_std`, has
no run-time dependency, and has `unsafe` in one place: `ClockPage::new` takes a raw address. It has
no Kani harness. Nothing in it points back into nife. Four names carry ratified blocks, six
provisional ones and the rest none.

## Identity

| item | value |
|---|---|
| crate | `clock_contract`, ratified ("the service's own word, `clock`, with the contract suffix") |
| identity values | `INTERFACE_ID` 8, `VERSION` 1, `MAGIC` `0x434c_4f43_4b76_3031` (ASCII `CLOCKv01`). Each matches row 8 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table and its rule that an id starts at version 1. The `Name:` blocks of `INTERFACE_ID` and `VERSION` read provisional, and no ruling has frozen the three. A host test pins all three as literals |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| dependencies | none at run time; `loom = "0.7"` only under `cfg(loom)`, which only a model-checking run sets |
| size | one `src/lib.rs` of 1,004 lines: about 567 of library, 238 of host tests and 199 of loom models |
| package | `cargo package` packs 8 files, 73.7 KiB (23.3 KiB compressed), and builds from the packed copy |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`: `kernel` (the clock service in
`kernel/src/user/clock_service.rs`, the `x86_64` CMOS read in `kernel/src/arch/x86_64/rtc.rs`),
`components`, `crates/user_mode_runtime`, `crates/network_time_protocol`, `crates/timetable`,
`crates/uptime`, `fixtures`, and `system_tests` (development only). Fifteen programs in
`components` declare the pair of `INTERFACE_ID` and `VERSION` in their `INTERFACES` note: every
program of `disk-tools`, `process-tools` and `core-tools`.

Two more readers reach the crate by its path, not by Cargo:

- The standard library. `cargo xtask std-src` copies `crates/clock_contract/src/lib.rs` verbatim to
  `sys/pal/nife/clockproto.rs` in the generated `std` source (`xtask/src/farm/generate.rs`, stamped
  by `xtask/src/farm/stamp.rs`), and the time support in
  `patches/std-nife/overlay/std/src/sys/time/nife.rs` reads `NANOS_PER_SEC`, `wall_nanos`,
  `Reading::offset_nanos` and `state::is_known` from that copy.
- The tooling. `script/interleaving-check` and `.github/workflows/ci.yml` run the loom models.
  `helpers/interface_stability.py` reads the source for the series of milestone 610 (the
  interface's stability is measured weekly). `helpers/vendored_name_gates.py` exempts the three
  overlay names, and `helpers/template_report_baseline.txt` holds three rows.
  `.cargo/mutants-baseline.txt` holds the census row (caught 54, missed 3, timeout 7, unviable 5).
  `packages/contracts.package.toml` and `crates/externalization-list.txt` list the crate.

What references back into nife from the crate: nothing. It has no path or development dependency,
no `include_str!` outside its own README, no test that reads a nife file, and no fuzz target in
nife's `fuzz/` workspace. The manifest writes `license` and `lints` as literals.

## What was run

This lane ran each check in its own worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone clock_contract` | two failures. `names`: 37 names lack a ratified block. `proofs`: no Kani harness. The build passes on 1.88 for the host and the three bare-metal targets. No falsification patch exists, so the check applied none |
| `script/crate-move --dry-run` | stages at template `v7`; builds on 1.88 for the host and three targets; 0 patches to rewrite. Three findings, all in the template's scaffold and listed in the runbook's `BUGS`: the pull request template names the architect, `CONTRIBUTING.md` cites a nife path, and the commit hook, `deny.toml`, `rustfmt.toml` and `docs/stranger-test.md` mention nife. The crate's own README and `OPERATION_SHIFT` doc named nife too; this branch rewords both |
| `cargo test -p clock_contract` | 12 host tests and 3 doctests pass; one doctest is the README's example |
| `script/interleaving-check` | the four loom models of `mod interleavings` pass |
| `cargo clippy --all-targets -D warnings`, `cargo doc` with warnings denied | no warning |
| `cargo package` | passes; the one warning is the missing `repository` key, which the move adds |
| `script/names clock_contract` | the crate name ratified; `script/names --unratified` lists six provisional items, four of them read outside the crate |
| `python3 helpers/name_release.py --draft clock_contract` | 41 names: 4 ratified, 6 provisional, 31 with no block |
| the template's naming checks (`vendor/crate-workflows/helpers/`, `--manifest Cargo.toml` over `src/lib.rs`) | `question_shape` and `contract_names` refuse none of 145. `spelled_out` refuses `va`. `unit_symbols` refuses `NS_PER_SECOND` and `NANOS_PER_SEC`. `register_names` refuses `NANOS_PER_SEC` and the private `W_OFFSET`. `british_spelling` refuses four private test names. `abbreviations` (report only) finds 16 components, the baseline's 16 |
| `python3 helpers/lint_externalized_crates.py` | 0 hits after this branch; its waiver for the crate goes |
| `script/lint --quick` | all sections pass |

Not run, and so not claimed: the stranger test, mutation, `cargo-semver-checks` (a first release
has no baseline) and the template's workflows, which run in the repository.

## Public API

| kind | items |
|---|---|
| conversion | `NS_PER_SECOND`, `wall_nanoseconds(offset_nanoseconds, monotonic_nanoseconds)`, `offset_for(wall_nanoseconds, monotonic_nanoseconds)`; the overlay's spellings `NANOS_PER_SEC` and `wall_nanos` |
| page identity and layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `WORDS` (4) |
| the page | `ClockPage`, `unsafe const fn new(va: u64)`, `initialize`, `read -> Reading`, `publish(new_state, offset_nanoseconds) -> u64` |
| a reading | `Reading { state, offset_nanos, generation }` |
| `state` | `UNKNOWN` (0), `RTC` (1), `SET` (2), `SYNCED` (3), `is_known(state)` |
| `propose` | `OPERATION_SHIFT` (56), `req(operation)`, `operation(w0)`, `PROPOSE` (1), `STATE` (2) |
| `status` | `ACCEPTED`, `REFUSED_IMPLAUSIBLE`, `REFUSED_TOO_FAR_FORWARD`, `REFUSED_TOO_FAR_BACKWARD`, `BAD_REQUEST` |
| `policy` | `NOT_BEFORE_NANOSECONDS` (2026-01-01), `NOT_AFTER_NANOSECONDS` (2100-01-01), `MAXIMUM_STEP_FORWARD_NANOSECONDS` (an hour), `MAXIMUM_STEP_BACKWARD_NANOSECONDS` (a second), `is_plausible(unix_nanoseconds)`, `decide(current_state, current_nanoseconds, proposed_nanoseconds)` |
| `rtc` | `NONE` (0), `PL031` (1), `GOLDFISH` (2), `CMOS` (3) |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- States, opcodes, statuses and clock kinds are bare `u64` constants in modules, not enums. They
  are words on a page and in a message, so a reader keeps a value a newer writer added.
- `ClockPage` is `Copy`, `Send` and `Sync` by `unsafe impl` around a raw pointer; `new`'s safety
  contract carries the whole burden. `publish` on a read-only mapping faults the caller, which the
  documentation calls the correct outcome.
- `read` never fails: a frame without the magic reads as `UNKNOWN`. It spins while a writer holds
  the sequence, and a writer killed mid-publish leaves every reader spinning; both `BUGS` sections
  say so.
- `propose` defines no reply layout type: the reply is two words, documented on `PROPOSE` and
  `STATE`.

## Proofs

No Kani harness, and so no falsification patch; the README says so. The loom models check four
claims about the seqlock: a reader never sees half a publish, a reading's generation matches its
pair, two writers serialize, and a reader racing `initialize` sees an unrecognized page or a whole
one. Each model asserts that some execution reached each branch it expects. Loom judges against C11, not the
Arm or RISC-V model, and the crate's `BUGS` says that too.

## Names

The crate name, `NS_PER_SECOND` and the two `MAXIMUM_STEP_` constants read ratified. `INTERFACE_ID`,
`VERSION`, `initialize`, `wall_nanoseconds`, `NANOS_PER_SEC` and `wall_nanos` read provisional.
Every other public item, the five modules and the three fields of `Reading` have no block. This lane
renames none of them. Each proposal is provisional; nothing outside nife has acted on the crate.

### Proposed item list

| item | kind | finding | proposal |
|---|---|---|---|
| `NS_PER_SECOND` | constant | ratified under the unit-symbol ruling of 2026-10-27; the later rule spells every unit out and accepts no SI symbol, and `unit_symbols` refuses it | `NANOSECONDS_PER_SECOND`, as `crates/abi/src/timer.rs` spells its private copy; alias `"nanoseconds per second"` |
| `NANOS_PER_SEC`, `wall_nanos` | constant, function | this tree's earlier spellings, kept because the std overlay reads them; a crate carries none before its first release | delete both, and move the overlay to the crate's spellings in the same change |
| `Reading::offset_nanos` | field | a clipped unit, kept for the same overlay | `offset_nanoseconds`, with the overlay and the five `UNIT_KEPT` entries in `helpers/vendored_name_gates.py` |
| `ClockPage::new`'s `va` | parameter | `spelled_out` refuses it | `virtual_address` |
| `propose::req` | function | `req` is an abbreviation | `request`, with no alias, since `req` is only this tree's spelling |
| `propose::operation`'s `w0` | parameter | a register-style name in a public signature | `word` |
| `state::SYNCED` | constant | a clipped participle | `SYNCHRONIZED` |
| `state::RTC`, the module `rtc` | constant, module | the acronym test: "real-time clock" is spoken; the tree's own precedent is `rtc` (`kernel/src/arch/x86_64/rtc.rs`) | `REAL_TIME_CLOCK` and `real_time_clock`, each with `#[doc(alias = "RTC")]` |
| `rtc::CMOS` | constant | nobody expands CMOS when naming the PC clock | keep; add `#[doc(alias = "mc146818")]`, the part name OpenBSD's driver carries (recalled, not read) |
| `rtc::PL031`, `rtc::GOLDFISH` | constants | part and device names from the device tree's `compatible` | keep; each doc comment already opens with the `compatible` string it matches |
| `WORDS` | constant | a count of the page's words | keep |
| the other 30 items of the draft table, the modules `state`, `propose`, `status` and `policy`, and the fields `state` and `generation` | | pass every rule | ratify as written; `INTERFACE_ID` and `VERSION` with the identity values |

Private names, for the move lane: `W_SEQ` (`seq`), `W_OFFSET` (refused by `register_names`), the
locals `s1`, `st`, `off` and `s`, and two loom test names and two statics that the template's
`british_spelling` check refuses. Proposed: `WORD_MAGIC`, `WORD_SEQUENCE`, `WORD_STATE`,
`WORD_OFFSET`, spelled-out locals, and the American spellings. `notes/interleaving.md`,
`notes/interleaving/what-was-modeled.md` and the mutation regression note cite the test names, so
they move together.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
`smp_wmb()` and `write_seqcount_begin` name the Linux ordering the code mirrors; the crate copies no
Linux text.
`PL031`, `goldfish-rtc` and the CMOS ports are register facts.

## What still blocks the move

| item | owner |
|---|---|
| Freeze the identity values, as for `block_roster`. Milestone 1017 asks for ratified `INTERFACE_ID` values before a contract crate leaves. | architect |
| The names above. The two overlay spellings and `offset_nanos` must go before the move, or the crate leaves carrying this tree's old spellings. | architect, then a lane |
| `cargo xtask std-src` copies the crate's source by path; after the move it must read the pinned checkout, as must `helpers/interface_stability.py`, `helpers/interface_declarations_check.py`, `script/interleaving-check` and the farm stamp. | move lane |
| Convert the eight path dependencies to `git` and `rev` with `version = "0.1.0"`. | move lane |
| `helpers/template_report_baseline.txt`, `helpers/vendored_name_gates.py` and `.cargo/mutants-baseline.txt` hold rows for the crate. | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

1. Whether a contract crate this small needs a Kani harness, since `script/crate-standalone` fails
   `proofs` without one. Recommendation: add two. `policy::decide` is a pure function of three
   words, so a harness costs little: it never accepts an implausible time, and never steps a known
   clock past either bound. The second proves that `propose::operation(propose::req(x))` returns
   `x` for any opcode below 256. The maintainer can decide it.
2. Whether `propose` should carry a reply type. Recommendation: no; the reply is two words of a
   message, and the request has no type either. The maintainer can decide it.

## Recommendation

Not ready for review until the architect freezes the identity values and rules on the overlay
spellings.
The crate itself is a leaf with nothing pointing back into nife.
