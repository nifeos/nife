---
status: NOT-STARTED
raised: 2026-11-09
milestone_dependencies: 817
decision_dependencies: none
machine_requirements: a VisionFive 2 for the hardware items; none for the rest
specific_machine: radon (the hardware items need a VisionFive 2 with its microSD and eMMC sockets)
needs_person: yes
---
# 1050. `designware_mobile_storage`: the outside review's findings land before its first release

Raised by the lane `lane/designware-mobile-storage-outside-review`. The lane read
`nifeos/designware_mobile_storage` at its first merge (`a800e4b`) as an outside maintainer would,
against [the review package](../../notes/crate-reviews/designware-mobile-storage.md) and the name
rulings it records. This block proposes work and changes nothing. Every item names what the
reviewer verified and what the reviewer inferred, and the last section says what the reviewer did
not run. Milestone 817 (`designware_mobile_storage`: a proven DesignWare SD and eMMC host, released
on its own) owns the extraction and the release path; this block owns the findings that land inside
that path.

Reuse: the crate's own simulator, Kani harnesses and falsification patches; the shared
crate-workflows template at the pinned revision; cargo-mutants and the template's mutation gate;
the kernel's storage bench on the board. No new tool.

## Goal

A stranger who finds the repository can build the crate and read one page that says what the
driver does and what the two traits ask of them. The maintainers tag nothing, and later publish
nothing, that a later release must take back. The crate today is sound, tested and proven where it
claims to be. Between it and a first release stand a handful of one-way API shapes, the packaging
of the license notice, a tag, and three robustness gaps a card on a real bus will find.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and a green gate here is not permission to publish.

## What the review covered

The review read the whole repository. That is the eight public modules (about 100 public items),
the 92 host tests and five doctests, the simulator, and the 23 Kani harnesses with their 23
falsification patches. It also read the README, CHANGELOG, SECURITY, CONTRIBUTING and the manifest,
the eight workflow callers against the template at `cd1b484`, and the first weekly run. It read the
settings of the repository through the GitHub API. The accepted baseline is the review package: all
111 public names read ratified there, and this block re-raises none of them.

Effort sizes: S is under a day of one lane, M a few days, L a week or more.

## Findings, prioritized

P0 blocks the baseline tag and any publish. P1 lands before the first release. P2 follows it, in
[the appendix](1050-designware-mobile-storage-outside-review/after-the-first-release.md).

### P0: one-way shapes and packaging that a tag or a publish would freeze

#### P0-1. `Identity.firmware` is a positional array of five unrelated registers

`Identity` (`src/host_controller.rs`) declares `pub firmware: [u32; 5]`, and `HostController::new`
fills it with `CONTROL_REGISTER`, `PWREN`, `CLKDIV`, `CLKENA` and `CTYPE` in that order. A reader of
a board transcript has to open the source to know which word is which, and a caller who indexes it
wrong still compiles. Ruled: five named public fields replace the array, each with its
`doc(alias)`. The architect has not yet ratified the field names. The review proposes `control`,
`power_enable`, `clock_divider`, `clock_enable` and `card_type`, provisionally. Verified in the
source. One-way door (a public struct with public fields). S.

#### P0-2. The visibility of the public surface follows no stated rule

Three command-register bits in `src/registers.rs` (`COMMAND_START`, `COMMAND_SEND_INITIALIZATION`,
`COMMAND_RESPONSE_EXPECT`) are public only because a doctest uses them, while the other command
bits are crate-private. Three command indices in `src/command.rs` (`GO_IDLE_STATE`, `SWITCH`,
`SEND_STATUS`) are public and the other fourteen are crate-private, although `Command::new` is
public and takes any index. `Command::with_data` is crate-private while the `data` field it fills
is public. A caller who wants to send `READ_SINGLE_BLOCK` spells 17.

Ruled: the stricter rule. Public is only what a caller needs to drive the controller, everything
else is crate-private, and every doctest uses the public API alone. The lane that applies the rule
rewrites the doctest on `Command` in `src/command.rs` to match, so no register bit stays public for
a doctest's sake. A later release can relax the rule without a break; tightening it after a tag is
one. That lane lists, in its pull request, each item it keeps public and the caller need that keeps
it there. Verified in the source. One-way door (removing `pub` after a publish is a break). S.

#### P0-3. The ISC notice travels only inside a doc comment

The crate root `src/lib.rs` carries the notice of OpenBSD for the adapted `dwmmc.c` as rustdoc
text. The root of the repository has `LICENSE-MIT` and `LICENSE-APACHE` and no `LICENSE-ISC` or
`NOTICE`. The license detection of GitHub reports Apache-2.0 alone (verified through the API).
`CONTRIBUTING.md` says the crate is dual licensed `MIT OR Apache-2.0` and omits the ISC term the
manifest carries in its `license` field. The License section of the README is the one place that
states it whole. Recommended: add `LICENSE-ISC` with the copyright line and notice, point
`src/lib.rs` and the README at it, and correct `CONTRIBUTING.md`. The SPDX expression stays as
ruled. Verified in the files. Two-way change to a one-way license statement. S.

#### P0-4. No tag, so the semver gate has no baseline

`git tag` is empty and the repository has no release (verified). The `weekly / semver` job of the
template and the per-pull-request semver rule of the labeler compare against the newest `v*` tag.
With none, they judge nothing (the template README, the note on `semver_baseline.py`), so a
breaking change merges without a hold today. Ruled: a baseline git tag `v0.1.0`, placed on the
first commit on `main` whose gates pass after the P0 items of this block merge. It is a git tag
only: no publish and no release run. Placing it still needs the go of the architect at that time.
One-way door (a tag is permanent). S.

### P1: before the first release

#### P1-1. Nothing recovers a card after a failed data phase

`src/command.rs` defines `STOP_TRANSMISSION`, and no code path sends it; a search over `src/` finds
it only in the register-word test. A multi-block read or write can end in `Error::Data` or
`Error::DataTimeout`, from the data waits in `src/host_controller.rs` and the descriptor-chain wait
in `src/internal_direct_memory_access.rs`. That leaves the card in its data or receive state, and
the next `issue` only waits on `DAT0` and resets the FIFO. Linux (`dw_mci_command_complete`,
`send_stop_abort`) and U-Boot (`dwmci_send_cmd`) send `CMD12` on a data error. Recommended: on a
data-phase error of a multi-block command, send `CMD12` as R1b, wait for `DAT0`, reset the FIFO,
and return the original error. Add a simulator knob for a card that keeps streaming after an error,
a host test, and a harness on the modeled card that every failed transfer leaves the card in the
transfer state. Verified in the source; the behavior of Linux and U-Boot is from the reviewer's
reading of those drivers, not re-read for this review. Two-way. M. The recovery sequence needs the
board to confirm.

#### P1-2. The deadlines are constants, and the busy deadline sits at the ceiling of the specification

`COMMAND_DEADLINE_MICROSECONDS` is 100 ms and `DATA_DEADLINE_MICROSECONDS` 500 ms
(`src/host_controller.rs`). The wait for a card to finish programming after a write uses the data
deadline. The SD specification gives an SDHC write 250 ms and an SDXC write 500 ms, and eMMC
devices may hold busy longer; the block layer of Linux allows ten seconds. A caller cannot change
either value. Recommended: a `Deadlines` value held by `HostController` with these defaults and a
setter, and a busy default of at least one second. Verified in the source; the figures of the
specification come from memory, not a re-reading. Two-way (additive). S. The board measures the
real busy durations.

#### P1-3. The two traits state no contract

`RegisterWindowAndClock` says "read the register" and "microseconds since some fixed point". It says
nothing about volatile access or about merging and elision, although each read of the FIFO offset
pops a word. It is silent on ordering against the DMA region, and on the clock having to advance.
`DirectMemoryAccessRegion` documents `barrier` as ordering "as the controller sees them" and never
names cache maintenance, which a board whose DMA is not coherent needs there. The one
implementation outside the tests does the right things (volatile loads and stores, a fence) and
says so in its own comments, which a stranger never sees. Recommended: a `# Contract` section on
each trait and a README section "Implementing the traits" with the coherent and the non-coherent
case. Verified in the source. Two-way. S.

#### P1-4. Forty-nine mutation survivors wait for triage, and the first weekly run did not finish

`.github/mutants-exempts` carries a census row (caught 661, missed 49, timeout 4) and lists no
survivor. In the first `weekly` run all six mutation shards ended in a runner shutdown (exit 143 in
two, cancelled in four) after reporting 24 missed mutants between them. The matrix of the template
has `fail-fast: false` and its concurrency group does not cancel, so the cause is unknown
(unverified). Most survivors visible in the logs are the disjoint-bit `|` to `^` swaps that the
compile-time assertions in `src/registers.rs` and `src/command.rs` exist for. Others are the
uninhabited `NoDirectMemoryAccess` methods. A few are real: `extract_bits` `>` to `>=` in
`src/card.rs`, twice, and `is_condition_met_within` `>` to `>=` in `src/host_controller.rs`.
Recommended: dispatch `weekly` again, exempt each equivalent mutant with its reason, and add a test
for each real one. Verified from the run logs and the file. Two-way. M.

#### P1-5. The stranger test has never run

`docs/stranger-test.md` reads `Last run: none`, and the weekly `stranger cadence` job fails closed
on it (verified in the run log). Recommended: one run by the four steps of the file, the date
recorded, each README finding fixed. Two-way. S.

#### P1-6. The README is not the front page of the crate on docs.rs, and nothing compiles its example

`src/lib.rs` does not include the README. The README example compiles (the reviewer ran it under
`rustdoc --test`), but no job does. The README also lacks the line that adds the dependency (a git
dependency now, the registry later). Recommended: `#![doc = include_str!("../README.md")]` on the
crate root, or a program under `examples/` that the tests build, plus the dependency snippet.
Verified. Two-way. S.

#### P1-7. Two error attributions mislead

Configuration reports a failed clock change as `Configure(CMD8, ...)` for an eMMC and
`Configure(CMD16, ...)` for an SD card (`src/sd.rs`), naming a command that did not fail. The
data-timeout bits (`DRTO`, `HTO`) surface as `Error::Data` (`src/host_controller.rs`) although
`Error::DataTimeout` exists. Recommended: a `TransferClock(Error)` variant on `InitializationError`,
provisional, and the timeout mask mapped to `DataTimeout`. Both enums are `#[non_exhaustive]`, so a
new variant is compatible. Verified. Two-way. S.

#### P1-8. The weekly caller builds the host only

`.github/workflows/weekly.yml` passes no `targets`, so `drift` and `msrv` never build the three
bare-metal targets that `.github/workflows/ci.yml` names. Recommended: the same `targets` line in
both callers. Verified. Two-way. S.

#### P1-9. `sd::Card::block_address_argument` states no precondition

The method (`src/sd.rs`) casts the logical block address to `u32` for a block-addressed card, and
for a byte-addressed card multiplies it by 512 in `u64` before the cast. An address at or past
`Card::blocks` therefore yields a wrapped argument in a release build. For a byte-addressed card,
an address above 2^55 panics on overflow in a debug build. Its doc comment says neither.
Recommended: state the precondition (`logical_block_address` below `blocks`) in the doc comment,
with a `# Panics` section for the debug build, and add a host test at the boundary. Verified in the
source. Two-way. S.

## Decisions this block needs

One-way doors only. Everything else above is the maintainers' call.

1. The field names of `Identity` (P0-1). The shape has its ruling: five named fields, no array.
   The five names stay open, with the provisional names of P0-1 as the recommendation.
2. The go for the baseline tag (P0-4). The tag and its commit have their ruling. Placing it waits
   for the go of the architect once the P0 items have merged and `main` passes.
3. Where the JH7110 facts live. The `jh7110` module (`src/jh7110.rs`) holds the addresses,
   interrupts and device-tree spellings of one chip inside a controller crate, as the
   `dw_mmc-starfive.c` glue of Linux sits beside `dw_mmc.c`. Moving it out after a publish is a
   break. Recommendation: keep it; revisit only if a second board arrives or the combine review
   chooses a shared JH7110 crate.
4. A shared DMA region trait with the sibling crate. See
   [the pair section](1050-designware-mobile-storage-outside-review/prior-art.md). Recommendation:
   not yet; each crate keeps its own trait until a third consumer exists, since a shared trait is a
   one-way door for both.

The items record the rulings on the shape of `Identity` (P0-1), the visibility rule (P0-2) and the
baseline tag (P0-4). The publishing hold stands.

## Prior art and the DesignWare pair

How other drivers solve what the items touch, the demand measured on crates.io, and what this crate
should share with `designware_ethernet` are in
[the prior art appendix](1050-designware-mobile-storage-outside-review/prior-art.md).

## Verified firsthand, inferred, and not run

Verified by the reviewer on a host:

- `cargo build`, `cargo test` (92 tests and 5 doctests pass), `cargo clippy --all-targets`
  (clean), `cargo doc` with warnings denied (clean).
- Release builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Three Kani harnesses (`the_clock_divider_is_total`, `a_register_field_never_exceeds_its_width`,
  the MBR parse) pass, and the clock-divider falsification patch turns its harness red.
  `falsify.py --check` holds all 23 records.
- The README example compiles under `rustdoc --test`. The source holds no `unsafe`, and the prose
  files hold no contractions.
- The reviewer read every source location cited above in the clone.
- The tags, releases, rulesets, team, workflow runs and job logs of the repository, through the
  GitHub API, and the crates.io figures.

Inferred, not verified: the cause of the runner shutdown in the weekly mutation shards, and the
behavior of Linux and U-Boot on a data error, from the reviewer's memory of those drivers. The busy
ceilings of the specification and the internals of `dwmmc-host` are inferences too.

Not run: the full Kani suite and the full falsification replay (CI shows both green on `main`),
cargo-mutants, Miri, the `standalone` and prose gates of the template locally, and anything on
hardware.

## Done when

The checks, and the board steps the storage bench runs, are in
[the done-when appendix](1050-designware-mobile-storage-outside-review/done-when.md). The review
claims none of the hardware steps.

## BUGS

- The P2 list is the judgment of the reviewer on what a user asks for first. Demand data for a
  DesignWare controller crate is thin (under a thousand downloads a quarter for each existing one),
  so the order may change once anyone outside asks.
- Hardware figures are absent until the bench runs; the deadline recommendation in P1-2 rests on
  the specification, not on this board.

## Index row

An outside review of `designware_mobile_storage` found no defect in its logic and thirteen things
to settle before a first release. Four are one-way shapes and packaging facts that a tag or a
publish would freeze. They are the shape of `Identity` and the visibility rule, both ruled; the
license notice; and a baseline tag `v0.1.0`, ruled, whose placing waits for a go. Nine land before
the first release. They are a missing `CMD12` recovery, fixed deadlines, undocumented trait
contracts and untriaged mutation survivors. They also cover a stranger run never made, a README
docs.rs never sees, two misleading error attributions, a weekly caller that builds the host only,
and an unstated precondition on `block_address_argument`. Publishing stays held.
