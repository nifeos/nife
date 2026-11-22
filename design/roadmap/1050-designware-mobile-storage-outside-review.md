---
status: PARTIAL
raised: 2026-11-09
milestone_dependencies: 817
decision_dependencies: none
machine_requirements: a VisionFive 2 for the hardware items; none for the rest
specific_machine: radon (the hardware items need a VisionFive 2 with its microSD and eMMC sockets)
needs_person: yes
---
# 1050. `designware_mobile_storage`: the outside review's findings land before its first release

Raised by the lane `lane/designware-mobile-storage-outside-review`, which read
`nifeos/designware_mobile_storage` at its first merge (`a800e4b`) as an outside maintainer would,
against [the review package](../../notes/crate-reviews/designware-mobile-storage.md) and the name
rulings it records. This block proposes work and changes nothing. Every item names what the
reviewer verified and inferred, and the last section says what was not run. Milestone 817 (`designware_mobile_storage`: a proven DesignWare SD and eMMC host, released
on its own) owns the extraction and the release path; this block owns the findings that land inside
that path.

Reuse: the crate's simulator, Kani harnesses and falsification patches; the shared
crate-workflows template; cargo-mutants; the kernel's storage bench. No new tool.

## Goal

A stranger can build the crate and read one page that says what the driver does and what the
two traits ask of them. The maintainers tag nothing, and later publish
nothing, that a later release must take back. The crate today is sound, tested and proven where it
claims to be. Between it and a first release stand a handful of one-way API shapes, the packaging
of the license notice, a tag, and three robustness gaps a card on a real bus will find.

Publishing stays held. No crate from this organization publishes to crates.io until every outside
review milestone is delivered and the combine review has decided what the crates share. Nothing
in this block lifts that hold, and a green gate here is not permission to publish.

## What the review covered

The review read the whole repository. That is the eight public modules (about 100 public items),
the 92 host tests and five doctests, the simulator, and the 23 Kani harnesses with their 23
falsification patches. It also read the README, CHANGELOG, SECURITY, CONTRIBUTING and the manifest,
the eight workflow callers against the template at `cd1b484`, and the first weekly run. The accepted baseline is the review package: all
111 public names read ratified there, and this block re-raises none of them.

Effort: S is under a day of one lane, M a few days, L a week or more.

## Findings, prioritized

P0 blocks the baseline tag and any publish. P1 lands before the first release. P2 follows, in
[the appendix](1050-designware-mobile-storage-outside-review/after-the-first-release.md).

### P0: one-way shapes and packaging that a tag or a publish would freeze

#### P0-1. `Identity.firmware` is a positional array of five unrelated registers

Status: Done: [nifeos/designware_mobile_storage#9](https://github.com/nifeos/designware_mobile_storage/pull/9).

`Identity` (`src/host_controller.rs`) declares `pub firmware: [u32; 5]`, and `HostController::new`
fills it with `CONTROL_REGISTER`, `PWREN`, `CLKDIV`, `CLKENA` and `CTYPE` in that order. A reader of
a board transcript has to open the source to know which word is which, and a caller who indexes it
wrong still compiles. Ruled: five named public fields replace the array. Ratified, in register
order: `host_control`, `power_enable`, `clock_divider`, `clock_enable` and `card_type`. Each field
carries a `doc(alias)` that cites the BSD spelling `SDMMC_<name>` first, then the specification
name (`CTRL`, `PWREN`, `CLKDIV`, `CLKENA`, `CTYPE`). Verified in the source. One-way door (a public struct with public fields). S.

#### P0-2. The visibility of the public surface follows no stated rule

Status: Done: [nifeos/designware_mobile_storage#2](https://github.com/nifeos/designware_mobile_storage/pull/2).

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
it there. Verified. One-way door (removing `pub` after a publish is a break). S.

#### P0-3. The ISC notice travels only inside a doc comment

Status: Done: [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3) (`LICENSE-ISC`, the README and `CONTRIBUTING.md`). 

The crate root `src/lib.rs` carries the notice of OpenBSD for the adapted `dwmmc.c` as rustdoc
text. The root of the repository has `LICENSE-MIT` and `LICENSE-APACHE` and no `LICENSE-ISC` or
`NOTICE`. The license detection of GitHub reports Apache-2.0 alone (verified through the API).
`CONTRIBUTING.md` says the crate is dual licensed `MIT OR Apache-2.0` and omits the ISC term the
manifest carries in its `license` field. The License section of the README is the one place that
states it whole. Recommended: add `LICENSE-ISC` with the copyright line and notice, point
`src/lib.rs` and the README at it, and correct `CONTRIBUTING.md`. The SPDX expression stays as
ruled. Verified. Two-way change to a one-way license statement. S.

#### P0-4. No tag, so the semver gate has no baseline

Status: Waiting on [nifeos/crate-workflows#84](https://github.com/nifeos/crate-workflows/pull/84). The tags `v0.1.0-pre.1` (`473b19c`) and `v0.1.0-pre.2` (`4c8dad4`) exist, but the template takes no pre-release tag as a semver baseline, so the labeler held nothing on [nifeos/designware_mobile_storage#33](https://github.com/nifeos/designware_mobile_storage/pull/33), which removed a public module.

`git tag` is empty and the repository has no release (verified). The `weekly / semver` job of the
template and the per-pull-request semver rule of the labeler compare against the newest `v*` tag.
With none, they judge nothing (the template README, the note on `semver_baseline.py`), so a
breaking change merges without a hold. Ruled: a baseline git tag `v0.1.0-pre.1`, placed on the
first commit on `main` whose gates pass after the P0 items of this block merge. It is a git tag
only; plain `v0.1.0` waits for the crates.io release. The tag sits on `473b19c`.
One-way door (a tag is permanent). S.

### P1: before the first release

#### P1-1. Nothing recovers a card after a failed data phase

Status: Done in code: [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5) sends `CMD12` after a failed multi-block data phase, with a harness. Bench: the board has not confirmed it.

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
reading, not re-read. Two-way. M.

#### P1-2. The deadlines are constants, and the busy deadline sits at the ceiling of the specification

Status: Done in code: [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5) (`Deadlines`, `HostController::set_deadlines`). Bench: busy durations not yet measured on a board.

`COMMAND_DEADLINE_MICROSECONDS` is 100 ms and `DATA_DEADLINE_MICROSECONDS` 500 ms
(`src/host_controller.rs`). The wait for a card to finish programming after a write uses the data
deadline. The SD specification gives an SDHC write 250 ms and an SDXC write 500 ms, and eMMC
devices may hold busy longer; the block layer of Linux allows ten seconds. A caller cannot change
either value. Recommended: a `Deadlines` value held by `HostController` with these defaults and a
setter, and a busy default of at least one second. Verified in the source; the specification figures come from memory. Two-way (additive). S.

#### P1-3. The two traits state no contract

Status: Done: a `# Contract` section on each trait and the README section, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3) and [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).

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

Status: Done: [nifeos/designware_mobile_storage#16](https://github.com/nifeos/designware_mobile_storage/pull/16) and [nifeos/designware_mobile_storage#18](https://github.com/nifeos/designware_mobile_storage/pull/18); `weekly` and the full mutation run pass on `main`.

`.github/mutants-exempts` carries a census row (caught 661, missed 49, timeout 4) and lists no
survivor. In the first `weekly` run all six mutation shards ended in a runner shutdown (exit 143 in
two, cancelled in four) after reporting 24 missed mutants between them. The matrix of the template
has `fail-fast: false` and its concurrency group does not cancel, so the cause is unknown
(unverified). Most survivors visible in the logs are the disjoint-bit `|` to `^` swaps that the
compile-time assertions in `src/registers.rs` and `src/command.rs` exist for. Others are the
uninhabited `NoDirectMemoryAccess` methods. A few are real: `extract_bits` `>` to `>=` in
`src/card.rs`, twice, and `is_condition_met_within` `>` to `>=` in `src/host_controller.rs`.
Recommended: dispatch `weekly` again, exempt each equivalent mutant with its reason, and test
each real one. Verified from the run logs and the file. Two-way. M.

#### P1-5. The stranger test has never run

Status: Done: [nifeos/designware_mobile_storage#14](https://github.com/nifeos/designware_mobile_storage/pull/14); the README gaps it found closed in [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).

`docs/stranger-test.md` reads `Last run: none`, and the weekly `stranger cadence` job fails closed
on it (verified in the run log). Recommended: one run by the four steps of the file, the date
recorded, each README finding fixed. Two-way. S.

#### P1-6. The README is not the front page of the crate on docs.rs, and nothing compiles its example

Status: Done: the README is the crate root's doc and its example compiles as a doctest, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3).

`src/lib.rs` does not include the README. The README example compiles (the reviewer ran it under
`rustdoc --test`), but no job does. The README also lacks the line that adds the dependency (a git
dependency now, the registry later). Recommended: `#![doc = include_str!("../README.md")]` on the
crate root, or a program under `examples/` that the tests build, plus the dependency snippet.
Verified. Two-way. S.

#### P1-7. Two error attributions mislead

Status: Done: `InitializationError::TransferClock` and the timeout mask mapped to `DataTimeout`, [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5).

Configuration reports a failed clock change as `Configure(CMD8, ...)` for an eMMC and
`Configure(CMD16, ...)` for an SD card (`src/sd.rs`), naming a command that did not fail. The
data-timeout bits (`DRTO`, `HTO`) surface as `Error::Data` (`src/host_controller.rs`) although
`Error::DataTimeout` exists. Recommended: a `TransferClock(Error)` variant on `InitializationError`,
provisional, and the timeout mask mapped to `DataTimeout`. Both enums are `#[non_exhaustive]`, so a
new variant is compatible. Verified. Two-way. S.

#### P1-8. The weekly caller builds the host only

Status: Done: [nifeos/designware_mobile_storage#4](https://github.com/nifeos/designware_mobile_storage/pull/4).

`.github/workflows/weekly.yml` passes no `targets`, so `drift` and `msrv` never build the three
bare-metal targets that `.github/workflows/ci.yml` names. Recommended: the same `targets` line in
both callers. Verified. Two-way. S.

#### P1-9. `sd::Card::block_address_argument` states no precondition

Status: Done, by a different fix: `block_address_argument` returns `Option<u32>` from a checked multiply, [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).

The method (`src/sd.rs`) casts the logical block address to `u32` for a block-addressed card, and
for a byte-addressed card multiplies it by 512 in `u64` before the cast. An address at or past
`Card::blocks` therefore yields a wrapped argument in a release build. For a byte-addressed card,
an address above 2^55 panics on overflow in a debug build. Its doc comment says neither.
Recommended: state the precondition (`logical_block_address` below `blocks`) in the doc comment,
with a `# Panics` section for the debug build, and add a host test at the boundary. Verified in the
source. Two-way. S.

## Decisions this block needs

1. The baseline tag (P0-4). Ruled and placed: `v0.1.0-pre.1` on `473b19c`. The semver rule reads it
   once the template takes a pre-release tag as a baseline (P0-4).
2. Where the JH7110 facts live. Ruled: the crate knows no chip. The JH7110 facts live in nife's
   `crates/machine_discovery/src/jh7110/designware_mobile_storage.rs`, and the crate has no
   `jh7110` module. Landed in [nifeos/designware_mobile_storage#31](https://github.com/nifeos/designware_mobile_storage/pull/31) and nifeos/nife#3729.
3. Where the master boot record parser lives. Ruled: the `partition` module (`MasterBootRecord`,
   `PartitionEntry`, `scratch_sector_range`) leaves for a new crate `master_boot_record`, staged in
   nife first. Landed: the staged crate in [#3731](https://github.com/nifeos/nife/pull/3731), the removal in
   [nifeos/designware_mobile_storage#33](https://github.com/nifeos/designware_mobile_storage/pull/33), and the pin bump in [#3757](https://github.com/nifeos/nife/pull/3757).
4. A shared DMA region trait across the organization's drivers. Provisional, option A: the crate
   keeps its own `DirectMemoryAccessRegion` for its first release (below).
5. When the first release goes. Ruled 2026-11-19 (UTC): the first release of
   `designware_mobile_storage` waits for the board confirmation of P1-1 (the `CMD12` recovery) and
   P1-2 (busy durations against the deadline defaults). P2-3, P2-4 and the 50 megahertz scope check
   follow the release as known limits stated in the README's BUGS.

## Open decisions

None. The shared DMA region trait carries a provisional answer, briefed in full in
[the open decisions appendix](1050-designware-mobile-storage-outside-review/open-decisions.md):

1. A shared DMA region trait. Provisional, option A: this crate keeps its own
   `DirectMemoryAccessRegion` for its first release, and a shared crate waits for a second storage
   driver. It is a two-way door while none of the three drivers that define such a trait has
   published, and the release review of the first of them rechecks it.

## Prior art and the DesignWare pair

How other drivers solve what the items touch, the demand measured on crates.io, and what this crate
should share with `designware_ethernet` are in
[the prior art appendix](1050-designware-mobile-storage-outside-review/prior-art.md).

## Verified firsthand, inferred, and not run

What the reviewer ran, what it inferred and what it left unrun are in
[the verification appendix](1050-designware-mobile-storage-outside-review/verification.md).

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

## Finding status

- P0-1, `Identity.firmware` as a positional array: Done, [nifeos/designware_mobile_storage#9](https://github.com/nifeos/designware_mobile_storage/pull/9).
- P0-2, a visibility rule for the public surface: Done, [nifeos/designware_mobile_storage#2](https://github.com/nifeos/designware_mobile_storage/pull/2).
- P0-3, the ISC notice only in a doc comment: Done, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3).
- P0-4, no semver baseline: Waiting on [nifeos/crate-workflows#84](https://github.com/nifeos/crate-workflows/pull/84), then the pin bump to the
  template release that carries it.
- P1-1, no recovery after a failed data phase: Waiting on a board run. The code is in [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5);
  the first release waits for the board (ruled 2026-11-19 UTC).
- P1-2, fixed deadlines at the ceiling of the specification: Waiting on a board run. The code is in
  [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5); busy durations against the defaults block the first release (ruled 2026-11-19 UTC).
- P1-3, the two traits state no contract: Done, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3) and [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).
- P1-4, forty-nine untriaged mutation survivors: Done, [nifeos/designware_mobile_storage#16](https://github.com/nifeos/designware_mobile_storage/pull/16) and [nifeos/designware_mobile_storage#18](https://github.com/nifeos/designware_mobile_storage/pull/18).
- P1-5, the stranger test never ran: Done, [nifeos/designware_mobile_storage#14](https://github.com/nifeos/designware_mobile_storage/pull/14) and [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).
- P1-6, the README is not the docs.rs front page: Done, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3).
- P1-7, two misleading error attributions: Done, [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5).
- P1-8, the weekly caller builds the host only: Done, [nifeos/designware_mobile_storage#4](https://github.com/nifeos/designware_mobile_storage/pull/4).
- P1-9, `block_address_argument` states no precondition: Done, [nifeos/designware_mobile_storage#15](https://github.com/nifeos/designware_mobile_storage/pull/15).
- P2-1, no fuzz target: Done, [nifeos/designware_mobile_storage#10](https://github.com/nifeos/designware_mobile_storage/pull/10) and [nifeos/designware_mobile_storage#19](https://github.com/nifeos/designware_mobile_storage/pull/19).
- P2-2, the simulator is not available to users: Done, [nifeos/designware_mobile_storage#12](https://github.com/nifeos/designware_mobile_storage/pull/12).
- P2-3, card presence read once: Waiting on a board run. `is_card_present` is in [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5); the
  polarity of the detect line and hot removal follow the first release (ruled 2026-11-19 UTC).
- P2-4, an eMMC runs 1-bit at 25 MHz: Waiting on a board run. The bus-width and high-speed switches
  are in [nifeos/designware_mobile_storage#11](https://github.com/nifeos/designware_mobile_storage/pull/11); the board has no eMMC module fitted, and the check follows the first release
  (ruled 2026-11-19 UTC).
- P2-5, `reset` leaves the IDMAC registers alone: Done, [nifeos/designware_mobile_storage#5](https://github.com/nifeos/designware_mobile_storage/pull/5) and [nifeos/designware_mobile_storage#19](https://github.com/nifeos/designware_mobile_storage/pull/19).
- P2-6, stale repository facts: Done, [nifeos/designware_mobile_storage#4](https://github.com/nifeos/designware_mobile_storage/pull/4).
- P2-7, user-facing text says "queue" for the FIFO: Done, [nifeos/designware_mobile_storage#3](https://github.com/nifeos/designware_mobile_storage/pull/3).
- Decision 2, where the JH7110 facts live: Done, [nifeos/designware_mobile_storage#31](https://github.com/nifeos/designware_mobile_storage/pull/31) and [#3729](https://github.com/nifeos/nife/pull/3729).
- Decision 3, where the master boot record parser lives: Done, [#3731](https://github.com/nifeos/nife/pull/3731), [nifeos/designware_mobile_storage#33](https://github.com/nifeos/designware_mobile_storage/pull/33) and
  [#3757](https://github.com/nifeos/nife/pull/3757).
- Decision 4, a shared DMA region trait: Done, provisional option A, in this block: the crate keeps
  its own trait ([the open decisions appendix](1050-designware-mobile-storage-outside-review/open-decisions.md)).
- Done when, the 50 megahertz card-interface clock against a scope: Waiting on a board run; it
  follows the first release (ruled 2026-11-19 UTC).
- Done when, the semver rule holding a breaking pull request: Waiting on [nifeos/crate-workflows#84](https://github.com/nifeos/crate-workflows/pull/84).
- Release, the first crates.io release: Waiting on the publishing hold.

## Follow-on

- **Done.** Every P0, P1 and P2 item in code, by the pull requests each finding's status names.
- **Outstanding.** P1-1 and P1-2 on the board: the blockers for delivery and for the first release.
- **Outstanding.** P0-4: [nifeos/crate-workflows#84](https://github.com/nifeos/crate-workflows/pull/84), then a template release and the crate's pin bump to it.
- **Outstanding.** The release review of the first DMA-region driver to publish rechecks the
  provisional answer on a shared trait.

## Index row

An outside review of `designware_mobile_storage` found no defect in its logic and thirteen things
to settle before a first release. Four are one-way shapes and packaging facts that a tag or a
publish would freeze. They are the shape and field names of `Identity` and the visibility rule, all ruled; the
license notice; and a baseline tag `v0.1.0-pre.1`, ruled and placed. Nine land before
the first release. They are a missing `CMD12` recovery, fixed deadlines, undocumented trait
contracts and untriaged mutation survivors. They also cover a stranger run never made, a README
docs.rs never sees, two misleading error attributions, a weekly caller that builds the host only,
and an unstated precondition on `block_address_argument`. Publishing stays held.
