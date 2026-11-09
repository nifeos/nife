---
status: NOT-STARTED
raised: 2026-11-09
milestone_dependencies: 815
decision_dependencies: none
machine_requirements: an I219 (Sunrise Point) host with a cable for the hardware items; QEMU x86_64 for the 82574L items; none for the rest
specific_machine: none
needs_person: yes
---
# 1057. `e1000e`: the outside review's findings land before its first release

Raised by the lane `milestone/1057-e1000e-outside-review`. A reviewer with no prior exposure to
this project read `nifeos/e1000e` at commit `8ac75a6` (`main`, after its pull request #2), against
[the review package](../../notes/crate-reviews/e1000e.md). This block proposes work and changes
nothing in that repository. Every item below is work for a lane there. The 180 names the review
package ratifies, and the decisions it records, stay closed here. Milestone 815 (`e1000e`: proven
Intel e1000e ring logic, released on its own) owns the extraction and the release path; this block
owns the findings that land inside that path.

Reuse: the crate's own simulator and host tests, Kani 0.68.0 and the three falsification patches,
the shared crate workflows at the pinned revision, `cargo-mutants`, and FreeBSD's `sys/dev/e1000/`
as the reference for the I219 port. No new tool.

## Goal

A stranger who finds `nifeos/e1000e` can read what the crate is, build it, run its proofs, drive a
ring against a fake device from an example, and know exactly what the crate does not do, before the
first tag goes on it. The weekly gates pass, the public trait names are final, and the port of the
I219 bring-up states every place it departs from its source.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and a green gate here is not permission to publish.

## What the review covered

The reviewer verified these firsthand, in a fresh clone on a Linux host with Rust 1.99.1:

- `cargo build` for the host and for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`: all four build clean.
- `cargo test`: 67 unit tests and 2 doctests pass. `cargo clippy --all-targets` reports no warning,
  `cargo doc` with warnings as errors is clean, and `cargo deny check` passes advisories, bans,
  licenses and sources (one unmatched allowance, `Zlib`, is a note, not a failure).
- `cargo kani` (Kani 0.68.0, CBMC 6.11.0): 3 of 3 harnesses verify. With all three falsification
  patches applied in a copy, 3 of 3 fail, each at the assertion its patch header names.
- `cargo mutants` over `src/data_plane.rs` and `src/descriptor.rs` (100 mutants): 82 caught, 7
  missed, 7 unviable, 4 timeouts. The tests catch every mutant in `data_plane.rs` or time it out.
  The 7 survivors are all OR-to-XOR swaps in `descriptor.rs` that the compile-time assertions
  beside them argue are equivalent.
- The GitHub state of the repository: no tag, no release, two merged pull requests, no issue;
  `check` and `proofs` green on `main`; the latest `weekly` run red in two jobs (`stranger cadence`
  and `mutation`), with the five mutation shards reporting 82 missed, 711 caught, 27 unviable and 4
  timeouts in all.
- Every register offset and bit constant, against FreeBSD's `e1000_regs.h`, `e1000_defines.h`,
  `e1000_ich8lan.h` and `if_em.h` and Linux's `regs.h` and `defines.h` as the reviewer knows them,
  and the full I219 sequences against FreeBSD `main`'s `e1000_ich8lan.c`, `e1000_phy.c`,
  `e1000_mac.c` and `if_em.c`, fetched for the purpose. No offset or bit is wrong. The write of the
  PCI vendor id to the read-only `STRAP` register around the global reset looks like an invention;
  it is FreeBSD's own (`e1000_reset_hw_ich8lan`). The 2.5 second wait on the Management Engine and
  the ten-second flash cycle bound are FreeBSD's values.

Inferred, not verified: the behavior of any sequence on silicon, and the effect of the register
writes the port omits (P1-2).

Not run: anything on hardware or under QEMU, the stranger test, `cargo-semver-checks` (no baseline
tag exists), a full mutation run outside the two files above, Miri (the weekly job ran it green),
and the `release` workflow of the template. The reviewer pushed, tagged, branched and opened
nothing in the repository.

Effort sizes: S is an hour, M a day, L more, or a bench.

## Findings, prioritized

P0 blocks the baseline tag and any publish. P1 lands before the first release. P2 follows it, in
[the appendix](1057-e1000e-outside-review/after-the-first-release.md). A door is one-way where a
public name, the public API, a register or descriptor layout, or the license presentation changes
in a way others would act on.

### P0: names, gates and the tag

#### P0-1. One method of the `Hardware` trait carries its width in an abbreviated name

`Hardware` (`src/platform_controller_hub/hardware_access.rs`) is the whole surface a kernel
implements for the I219 sequences and for the PHY identifier read of the 82574L. It has `read`,
`write`, `read16`, `delay_microseconds`, `pci_vendor_id` and `note`, and the review package listed
the three access names as the last provisional ones. `read16` has one caller,
`non_volatile_memory`'s read of the flash status word, and it is the only 16-bit access in BAR0.

Ruled 2026-11-09. `read` and `write` stay. They match the register-window traits of the sibling
crates `designware_ethernet` and `designware_mobile_storage`, both ratified, and the ratified
`read_register` and `write_register` elsewhere in the organization. A register's width is a fact
of the register, so the access name carries none. `read16` becomes `read_flash_status`: it names
the one word it reads and keeps the width out of the name. `DirectMemoryAccessRegion::read_u64` and
`write_u64` (`src/data_plane.rs`) stay. That trait addresses byte memory, where the width is a
choice of the caller, so it is no model for a register window. The six method names of `Hardware`
are now ratified. The rename leaves the old spelling nowhere, since nothing keeps an alias for an
earlier spelling of this tree before a first release. One-way (public API). S.

#### P0-2. The weekly mutation gate fails on survivors nobody has triaged

The exemption file `.github/mutants-exempts` carries a census row as a comment and no entry, and
says the first weekly run would name each survivor. That run named 82, and the gate fails on them.
Three kinds appear:

- OR-to-XOR and shift swaps on bit constants that the compile-time assertions argue are
  equivalent. They are the majority.
- Mutants inside `#[cfg(kani)]` harness bodies, in `src/proofs.rs` and in the harness at the end of
  `src/platform_controller_hub/physical_layer_device.rs`. No host test can reach them, and
  `cargo-mutants` should not visit them.
- Real logic survivors with no test behind them. In `sequence.rs`, the identifier check
  `*id == seen || last.is_ok()` (the FreeBSD quirk its doc comment describes) mutates to `&&` and
  to `!=` with no test failing. In `non_volatile_memory.rs`, the write-one-to-clear composition in
  `initialize_cycle` survives three mutants, and the byte-count mask survives one.

Change: exclude the `cfg(kani)` files through a `.cargo/mutants.toml`, and add host tests that kill
the logic survivors: a device whose identifier differs between probes, and a flash status with
stale error bits. List each remaining equivalent swap with its reason, and replace the census
comment with the counts of the run. Two-way. M.

#### P0-3. The stranger test has never run, and its weekly job fails for that

`docs/stranger-test.md` ends `Last run: none`, and the `stranger cadence` job fails when the last
run is older than 28 days. The repository's own text calls this the whole claim of the release.
Change: a person or an agent with no project context clones fresh and follows the README alone
(build, test, proofs, one falsification). It records every lie or omission as a README finding and
writes the date on that line. Two-way. S, plus the hour of the reader.

#### P0-4. No tag, so the semver gate has no baseline

`git tag` is empty, so the `semver` job of the template compares against nothing. The review
package placed the tag on the move commit. Since then pull request #2 of the repository renamed
public register constants, so a baseline there would report those renames as breaks against the
first release.

Ruled 2026-11-09: a baseline git tag `v0.1.0`, placed on the first commit on `main` at which the
weekly gates pass, after P0-2 and P0-3 land. It is a git tag only: no publish and no release run.
Placing it still needs the go of the architect at that time. One-way (a tag is a number others
compare against). S.

### P1: before the first release

#### P1-1. The public traits do not state the volatile and ordering contract

`DirectMemoryAccessRegion` takes `&self` for reads of memory the device writes, and its doc says
only "Read the little-endian `u64` at `offset`". `Tails::barrier` says what it orders, but nothing
says the tail writes are MMIO that a compiler must not elide or merge. `Hardware::read` and
`FlushSupport::write_transmit_descriptor` (`src/platform_controller_hub/flush.rs`) say no more. An
implementation over a plain slice is legal by the signatures and wrong on hardware. Change: each
trait's doc states that every read of device-written memory is a volatile read and every register
write a volatile write. It also states what `barrier` must be on each architecture the crate builds
for (a `dmb` or `fence` of the right kind). The README's "For the rings" paragraph links it.
Two-way. S.

#### P1-2. The I219 port omits three steps of its source without saying so

FreeBSD's `e1000_init_hw_ich8lan` ends with `e1000_set_pcie_no_snoop_generic(hw, ~PCIE_NO_SNOOP_ALL)`,
a read-modify-write of `GCR` (offset `0x5B00`) that `initialize_hardware` does not perform.
FreeBSD's `em_reset` writes `WUFC = 0` after the global reset and `VET` to the VLAN ethertype after
`init_hw`. It also writes `PBA` before the descriptor-ring flush, where `global_reset` writes it
after. The module doc lists two departures (flow control, master and slave mode) and gives the rest
of `init_hw` to the kernel half, yet the `GCR`, `WUFC` and `VET` writes belong to nobody. The
reviewer checked this against FreeBSD `main`; the effect on silicon is an inference. Evidence:
`initialize_hardware` and `global_reset` in `src/platform_controller_hub/sequence.rs`, and the BUGS
section of `src/platform_controller_hub/mod.rs`. Change: either port the three writes (two new
register offsets, each with its source spelling as an alias) or record each as a departure in the
doc of `sequence` and in the module's BUGS. Move the `PBA` write before the flush, or say why not.
Two-way. S to document, M to port with a bench check.

#### P1-3. The BSD notices live only in doc comments

The `license` field of `Cargo.toml` names `BSD-3-Clause` and `BSD-2-Clause`, and the root carries
`LICENSE-APACHE` and `LICENSE-MIT` only. The Intel and FreeBSD notices sit in
`src/platform_controller_hub/mod.rs` and three sibling files. A reader of the package or the GitHub
page sees Apache-2.0, which is all the license detection of GitHub reports. `CONTRIBUTING.md` says
"dual licensed `MIT OR Apache-2.0`", which contradicts the manifest. Change: add
`LICENSE-BSD-3-Clause-Intel` and `LICENSE-BSD-2-Clause-FreeBSD` at the root with the notices as
found, and point the README's License section and CONTRIBUTING at them. The `license` expression
stays as it is. Two-way (additive). S.

#### P1-4. The manifest cites a file from another repository

The comment on the `publish = false` line of `Cargo.toml` says the exception lives in
`crates/publish-allowlist.txt`, a path that does not exist in this repository and that a stranger
cannot follow. Change: say in the crate's own words why it is unpublished: the publishing hold
above. Two-way. S.

#### P1-5. No example drives the ring, and no document gives the register order the crate assumes

The README's one example judges a status word. The sentence after it tells the reader to implement
two traits and "drive a `DataPlane`" with no code. The doc of `DataPlane::start` assumes the kernel
half left both heads and tails at zero and programmed the ring bases, and the doc of `sequence`
gives the three I219 passes. Nothing lists the 82574L bring-up a caller must write: reset, link up,
receive address, multicast table, ring base and length registers, `RCTL`, `TCTL`, `TIPG`, and
`RFCTL.EXTEN` clear. No `examples/` directory exists. Change: an `examples/host_ring.rs` that
implements the two traits over a `Vec<u8>` and two integers and sends and receives a frame, with
the test module's `SimulatedDevice` as the model and CI running it. Add a `docs/bring-up.md` with
the two register orders and where each crate function sits in them, and link both from the README.
Two-way. M.

#### P1-6. The proofs cover three pure functions and not the cursors of the ring

The harnesses prove `receive_verdict`, the layout offsets and `plan_paged_access`. Host tests
against a cooperative simulator are all that check the invariants a driver author most wants:

- `transmit` never writes a descriptor the device still owns;
- `transmit_clean` never passes `transmit_next`;
- `receive` reads at most `out.len()` bytes, and only from the slot it consumed;
- one slot always separates head and tail.

The review package names the NVM reads as the first candidate for a fourth harness; the ring is the
better one. Change: a harness over a bounded array-backed region whose descriptor status words are
`kani::any()`, asserting the four invariants across a symbolic sequence of `receive` and `transmit`
calls. It arrives with its falsification patch, as the rule for a new harness requires. A second
harness covers the range checks of `Geometry::from_strap` and `read_word`. Two-way. M.

#### P1-7. The I219 bring-up has run once on silicon, with no cable

The README and the module BUGS say so. The link-up half (`e1000_check_for_copper_link_ich8lan`,
with the latency tolerance reporting of `e1000_platform_pm_pch_lpt` and the K1 configuration) is
not ported, so a link that comes up may lose frames. That is honest, and it means no reader knows
whether the port works. Change: measure first. Boot a consumer of this crate on a Sunrise Point
I219 with a cable to a switch, and record the bring-up notes, the link state and whether frames
pass in both directions. Write the result into the README's BUGS in place of the one-run sentence.
The port of the link-up half is P2-9 and waits on that result. Two-way. L, and it needs hardware.

#### P1-8. The repository cannot show its own data plane working under QEMU

The only end-to-end evidence for the 82574L path lives in a consumer. This repository has host
tests and nothing that touches `-device e1000e`; the tree holds no QEMU reference. Change: at least
a documented manual procedure (the QEMU command line, the consumer, what a pass prints), and at
most a small bare-metal `x86_64-unknown-none` smoke test under `tests/` that the proofs workflow
does not run but a person can. Two-way. M for the document, L for the test.

## Decisions this block needs

No one-way door is open for the architect except the go to place the tag.

1. The method names of `Hardware` (P0-1). Ruled 2026-11-09: `read` and `write` stay, `read16`
   becomes `read_flash_status`, and the six names are ratified. `read_u64` and `write_u64` on
   `DirectMemoryAccessRegion` stay.
2. Which commit carries `v0.1.0` (P0-4). Ruled 2026-11-09: the first commit on `main` at which the
   weekly gates pass, after P0-2 and P0-3; a git tag only. Placing it waits for the go of the
   architect at that time.
3. The ring size stays a constant for 0.1 (P2-6). The maintainers decide this provisionally, since
   deferring changes nothing: the README's BUGS records that each ring holds 16 entries and that a
   later version may change that. Generalizing the size later changes the layout constants, the
   callers of both traits and the proofs, and that change is the one-way door when it comes.

The publishing hold stands.

## Prior art

How other drivers and stacks solve what the items touch, and why register-window names carry no
width, are in [the prior art appendix](1057-e1000e-outside-review/prior-art.md).

## Done when

The checks, and the steps that need hardware or QEMU, are in
[the done-when appendix](1057-e1000e-outside-review/done-when.md). The review claims none of the
hardware steps.

## BUGS

- The P2 order is the judgment of the reviewer on what a user asks for first. Nobody outside has
  asked for anything yet, so the order may change once anyone does.
- Hardware figures stay absent until P1-7 runs; the cost of one tail write per frame (P2-4) and the
  effect of the omitted writes (P1-2) are unmeasured.

## Index row

An outside reading of `e1000e`: four items block the baseline tag (the `Hardware` method names,
ruled; a red mutation gate; no stranger run; the tag itself, ruled, whose placing waits for a go).
Eight go before the first release: the volatile contract, three omitted FreeBSD steps, the BSD
notice files, a manifest comment, an example and a bring-up order, a ring harness, a hardware run
with a cable, and a QEMU procedure. Sixteen follow it. The code, offsets and sequences check out
against their sources, and the proofs and tests run clean. Publishing stays held.
