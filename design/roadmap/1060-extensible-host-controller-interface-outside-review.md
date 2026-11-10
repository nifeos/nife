---
status: NOT-STARTED
raised: 2026-11-09
milestone_dependencies: 820
decision_dependencies: none
machine_requirements: an xHCI controller on a PC-class machine for the hardware checks; none for the rest
specific_machine: none
needs_person: yes
---
# 1060. `extensible_host_controller_interface`: the outside review's findings land before its first release

Raised by the lane `milestone/1060-extensible-host-controller-interface-outside-review`. A reviewer
with no prior exposure to this project read `nifeos/extensible_host_controller_interface` at commit
`bea0257` (`main`, after its pull request #1), against
[the review package](../../notes/crate-reviews/extensible-host-controller-interface.md). This block
cites files and symbols at that commit, which is still `main`. It proposes work and changes nothing
in that repository. Every item below is work for a lane there.

The review package and milestone 820 (`extensible_host_controller_interface`: proven xHCI logic,
released on its own) settle the crate name and its 146 ratified names, the four register names the
architect renamed, the spelled-out parameter names, the dual license, the version 0.1.0, that the
crate publishes, and the hub limit the README states. They stay closed here. Milestone 820 owns the
release path; this block owns the findings that land inside that path.

Reuse: the shared crate workflows at the pinned revision and their scaffold, `cargo-mutants`, Kani,
`cargo-fuzz`, and the simulated controller in the crate's own `src/tests.rs`. No new tool.

## Goal

A stranger who finds the crate can tell in one screen what it computes and what it leaves to the
driver. The same stranger can build, test, prove and falsify it from the README alone, can name
every code the crate hands back, and can write a controller driver on a weakly ordered machine
without guessing the read and write order the ring discipline needs. The weekly gates are green for
the crate's own reasons, and red only for them.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and no ruling covers a tag: a tag is a number others compare
against, so placing one is a separate question for the architect.

## What the review covered

The reviewer ran these firsthand at `bea0257`, in a fresh clone on an x86_64 Linux host:

| check | result |
|---|---|
| `cargo build`, `cargo test`, `cargo clippy --all-targets`, `cargo doc` with warnings denied | clean on stable 1.99.1 and on 1.88, the `rust-version`; 30 unit tests and 2 doctests |
| `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` | all three build; `src/` holds no `unsafe` |
| `cargo kani` (0.68.0) | 7 of 7 harnesses verify with no warning |
| the falsification patches | each fails its own harness; four name the promised assertion in the summary, three fail at an `assert_eq` or `assert_ne` that Kani reports as a placeholder |
| `cargo mutants` (27.1.0) over `src/` without `src/verification.rs` | 367 mutants: 277 caught, 49 missed, 41 unviable, no timeout |
| the three `cargo-fuzz` targets | 30 seconds each on nightly, no finding |
| `cargo deny check`, `cargo package --list` | pass, with two warnings for unmatched license allowances; the package ships `src/`, the patches and the repository's `.github/` |
| every public name against [design/naming.md](../naming.md) | no finding against a ratified name |
| each decoded register, context and TRB field against the field positions the reviewer carries from the xHCI 1.2 specification | no field misplaced |

Of the 49 missed mutants, 44 are `|` to `^` or `<<` to `>>` on bits that do not overlap, and 5 are
comparisons in clamps that no input reaches (P1-1, P2-1).

Rechecked for this block, by reading `main` and through the GitHub API: every file and symbol each
item cites, the manifest, the template pin and its distance from the template's `main` (33
commits), and that the weekly run on `bea0257` fails in `mutation` and in `stranger cadence` and
passes its other jobs. Not rerun for this block: the builds, tests, proofs, patches, mutation
counts, fuzz runs and the 99-survivor count of the weekly run, all of which come from the review.

Inferred, not verified: the two specification bounds of P1-5 and P1-7, which the reviewer recalls
from the specification and from Linux's ring code. BUGS says so.

Not run: anything on hardware (the done-when appendix lists what waits for a bench), `cargo
semver-checks` (no release to compare against), Miri (the weekly job runs it green), a long fuzz
campaign, a release dry run, and the template's name-check helpers. The reviewer changed nothing in
the repository.

Effort sizes: XS is under an hour for a lane, S half a day, M a day or two.

## Findings, prioritized

P0 blocks any release. P1 lands before the first release. P2 follows it, in
[the appendix](1060-extensible-host-controller-interface-outside-review/after-the-first-release.md).
A door is one-way where something outside the repository acts on it after the release: a public
name, a published behavior, a layout or the license. The maintainers decide every two-way item
provisionally.

### P0: blocks any publish

#### P0-1. The manifest's publish-facing fields are wrong or point outside the repository

In `Cargo.toml`, `description` says the crate covers "HID boot reports", and no report code exists
in `src/`: the only report names are layout constants in `src/direct_memory_access.rs`.
`keywords` holds `no-std`, `kani` and `driver`, and not `xhci` or `usb`, the two words a user
searches. The comment on `publish = false` names `crates/publish-allowlist.txt`, a file from
another repository. The block comment under `[package]` describes which register pages a confined
driver is mapped and a spawn handoff, which live elsewhere.

Change: a description of what the crate holds ("xHCI host controller logic as pure computation:
register offsets and decode, TRBs, rings, contexts and port status, proven with Kani"), `keywords =
["xhci", "usb", "no-std", "kani", "driver"]`, one sentence on why the crate stays off crates.io in
words that hold here (the publishing hold above), and the block comment cut to what the crate
holds. Two-way. XS.

### P1: before the first release

#### P1-1. The weekly mutation verdict is red, and the exemption file holds a census

The weekly run's `mutation` job fails with 99 survivors. Fifty sit in `src/verification.rs`, which
compiles only under `cfg(kani)`, so no host test can kill them. The other 49 are the ones a local
run reports: `|` to `^` on constants whose bits the `_DO_NOT_OVERLAP` assertions prove disjoint
(for example `PORT_CHANGE_BITS_DO_NOT_OVERLAP` in `src/port.rs` and
`ENDPOINT_CONTEXT_FIELDS_DO_NOT_OVERLAP` in `src/context.rs`), `<<` to `>>` on `1 << 0`, and the
dead clamps of P2-1. The source wrote those assertions to justify exemptions and then wrote none.
`.github/mutants-exempts` carries the census comment from the tree the crate left, which the first
weekly run has now answered.

Change: a `.cargo/mutants.toml` excluding `src/verification.rs`, and one exemption per equivalent
mutant naming the constant that proves it equivalent (a `1 << 0` line says a shift by zero is the
identity, kept because the naming rule writes a bit as a shift). Delete the census comment. Then
dispatch the weekly run and read a green verdict. The template's scaffold ships no such
configuration, so every crate with a Kani module meets this on its first weekly run; that is work
for the template, through its own queue. Two-way. S.

#### P1-2. The stranger run has never been recorded, and the README does not carry what a stranger needs

`docs/stranger-test.md` reads `Last run: none`, and the weekly `stranger cadence` job fails closed
on it. The README gives no build or test command. Its Kani install command breaks across two lines
inside one code span. It describes the falsification patches but gives no command to apply one,
run its harness or undo it, and nothing says what a green run prints.

Change: a "Building and proving" section with `cargo build`, `cargo test`, the install line on one
line, `cargo kani`, the three commands around a patch (`patch -p1 < falsifications/<name>.patch`,
`cargo kani --harness <name>`, `git checkout -- src`), and what a green run prints. Then a reader
with no project context records a stranger run. Two-way. S.

#### P1-3. `context::endpoint` does not keep the endpoint type to its field

Dword 1 of the context that `context::endpoint` builds takes `(kind as u32) << 3` with no mask. The
Endpoint Type field is bits 5:3, so a `kind` above 7 spills into bit 6 (reserved), bit 7 (Host
Initiate Disable) and the Max Burst Size field (15:8). The harness
`the_slot_and_endpoint_contexts_keep_each_field_in_its_own_bits` in `src/verification.rs` asserts
`(endpoint[1] >> 3) & 0xff == kind`, so it proves the spill as if it were the specification, and
the doc comment on `ENDPOINT_CONTEXT_FIELDS_DO_NOT_OVERLAP` says "kind at 3:10". `context::slot`
masks its speed in the same file, so the asymmetry is an oversight.

Ruled 2026-11-09. `context::endpoint` takes an `EndpointType` enum in place of the `u8`, and keeps
the mask. The lane mints the variant names provisionally from the specification's spellings, and
they are ratified with the batch of P1-12. The harness asserts that bits 6 to 15 of dword 1 are
zero and that the type reads back as the variant's code, and the falsification patch drops the
mask. One-way (public signature). XS.

#### P1-4. The read order a consumer needs on a weakly ordered machine is stated nowhere

The doc of `TransferRequestBlockPlacement` in `src/ring.rs` says the driver writes the cycle dword
last, which is the producer's half. Nothing states the consumer's half: the controller writes an
event's cycle dword last, so a driver that reads all four dwords and then calls
`EventRing::is_ready` can see a fresh cycle bit beside stale data on aarch64 or riscv64, unless it
reads the control dword first with acquire ordering. Nothing says a release fence belongs between
the TRB writes and the doorbell write. Two of the three bare-metal targets are weakly ordered.

Change: one paragraph on `EventRing::is_ready` and one on `ProducerRing::enqueue` stating the order
and the barrier, and a "What the caller does" section in the README listing the volatile accesses
and fences in sequence. Two-way. XS.

#### P1-5. The rings accept sizes a controller refuses

`EventRing::new` accepts any `entries > 0` and `ProducerRing::new` any `entries >= 2`. The reviewer
recalls the specification's Event Ring Segment Table entry as valid for 16 to 4,096 TRBs per
segment. The host tests in `src/tests.rs` build event rings of 2 and 4 entries,
`fuzz/fuzz_targets/event_ring.rs` ranges over 1 to 256, and the proofs over every `u16`, so part
of their range is spent on configurations no controller runs.

Ruled 2026-11-09. `EventRing::new` and `ProducerRing::new` refuse sizes outside the
specification's bounds, with named constants `MINIMUM_SEGMENT_ENTRIES` and
`MAXIMUM_SEGMENT_ENTRIES`. The lane reads the bounds from the specification text before it writes
them. The harness assumptions and the fuzz range narrow to match. One-way (published behavior). XS.

#### P1-6. A producer ring has no notion of being full

`ProducerRing::enqueue` always places the next TRB and wraps, so a caller who enqueues more than
`entries - 1` TRBs between two consumptions overwrites TRBs the controller has not reached. The one
driver of the crate never does; a stranger's driver may. The only statement of the rule is a
comment in the host test `a_simulated_controller_runs_exactly_what_was_produced_across_laps`.

Change: `ProducerRing::capacity() -> u16` (`entries - 1`, name provisional), a sentence on
`enqueue` stating the caller's rule, and a README `BUGS` line. P1-13 adds the refusing variant.
Two-way. XS.

#### P1-7. The 64 kibibyte buffer rule is not stated on the data builders

`TransferRequestBlock::data` and `TransferRequestBlock::normal` take any buffer and length. A `u16`
length keeps a TRB under 64 kibibytes, but a buffer that is not aligned can still cross a 64
kibibyte boundary, which Linux splits a TD at and the reviewer recalls as a specification rule.
The crate's own layout never crosses one, so its driver is safe by construction, and a stranger's
is not told.

Change: read the rule, state it on both builders and in the README `BUGS`, and add
`fits_one_transfer_request_block(buffer, len) -> bool` (name provisional). Two-way. XS.

#### P1-8. The codes the crate hands back have no public names

`TransferRequestBlock::kind` and `Event::Other` return a type code, and
`src/transfer_request_block_type.rs` keeps `SETUP`, `LINK`, every command and every event code
`pub(crate)`; only `NORMAL`, `DATA` and `STATUS` are public. In `src/port.rs`, `speed::FULL` is
public and `LOW`, `HIGH`, `SUPER` and `SUPER_PLUS` are not, so a user cannot compare `speed_of`
against anything but full speed. Five of the seven change bits (`ENABLE_CHANGE`,
`WARM_RESET_CHANGE`, `OVER_CURRENT_CHANGE`, `LINK_CHANGE`, `CONFIGURATION_ERROR_CHANGE`) are
`pub(crate)`. `src/completion.rs` names five of the roughly 37 completion codes, so a driver that
meets Babble, Stopped or Command Aborted gets a number it cannot name.

Ruled 2026-11-09. These names are taken before any tag. The existing type codes, speed ids and
change bits become public. The missing completion codes are added with spelled-out names and the
specification's name as `doc(alias)`. `completion::name(code) -> &'static str` sits beside
`port::speed_name`. Every new name is provisional until the release review ratifies them together.
One-way (names). S.

#### P1-9. The crate holds no `unsafe` and does not say so

No `unsafe` block exists, yet the manifest enables `unsafe_op_in_unsafe_fn` and
`undocumented_unsafe_blocks`, `SECURITY.md` speaks of the crate corrupting "memory it confines",
and `.github/workflows/fuzz.yml` keeps crash inputs private. Change: `#![forbid(unsafe_code)]` in
`src/lib.rs`, drop the two lints, reword the security policy to what a report here can be (a wrong
field position, a wrong ring step, a proof that does not hold), and set `public-crash-inputs:
true`, which the standing rule allows for a crate with no `unsafe`. Two-way. XS.

#### P1-10. The next template release brings the mutation stack

Every caller pins `nifeos/crate-workflows` at `cd1b484`, at or past release `v7`, so the `lag` job
passes. The template's `main` is 33 commits further on, all unreleased, and adds a `mutation.yml`
caller, `check / mutation-diff` as a required check, a release gate that refuses a release without
a green full run, and a `mutation-diff` budget line. The ruleset requires nine checks today.

Change, once the template releases: take the pin bump, add the scaffold's `mutation.yml` caller
with this repository's own cron line, copy the budget file, apply the settings so the tenth check
becomes required only after it has reported once, and dispatch `mutation.yml` on `main`. P1-1
lands first, or that run is red. Two-way. S.

#### P1-11. Port 0 aliases port 1, and no port offset is bounded

`Capabilities::port_status_and_control` in `src/registers.rs` saturates `port_number - 1`, so port
0 returns port 1's offset, and nothing checks the port against `maximum_ports`.
`fuzz/fuzz_targets/capability_registers.rs` asserts the alias. An offset for a port that does not
exist is a register write into whatever sits at that address.

Ruled 2026-11-09. `port_status_and_control` returns `Option<u64>`: `None` for port 0 and for a
port past `maximum_ports`. The fuzz target asserts `None` where it asserts the alias today.
One-way (public signature). XS.

#### P1-12. Two tri-states are spelled as `Option<bool>`

`PortDebounce::sample` returns `Option<bool>` for pending, settled and timed out, and
`TransferRequestBlock::setup` takes `Option<bool>` for no data stage, host to device and device to
host. A reader needs the doc comment to decode each.

Ruled 2026-11-09. Two enums replace them before any tag: a debounce verdict with `Pending`,
`Settled` and `TimedOut`, and a transfer type with `NoDataStage`, `HostToDevice` and
`DeviceToHost`. The enum and variant names are provisional until the release review ratifies them
with P1-3 and P1-8. One-way (names and shapes). XS.

#### P1-13. A fake controller a stranger can run, and a ring that refuses when full

The host test `a_simulated_controller_runs_exactly_what_was_produced_across_laps` holds a consumer
that follows links and toggles its cycle, which is the test double a driver author wants, and it
is private to the tests. No `examples/` directory exists.

Ruled 2026-11-09. Before any tag, `ProducerRing` gains `try_enqueue`, which refuses when the ring
is full, and `complete(transfer_request_block_physical_address)`, which the caller reports each
completion event to. `examples/fake_controller.rs` builds the rings, enqueues a control transfer,
consumes it as the controller would and decodes the events it writes. The names are provisional
until the release review. One-way (names). M.

## Decisions this block needs

Ruled 2026-11-09, and closed here:

1. `context::endpoint` takes an `EndpointType` enum and keeps the mask; a lane mints the variant
   names provisionally from the specification's spellings, and the release review ratifies them
   with the names of decision 3 (P1-3).
2. `EventRing::new` and `ProducerRing::new` refuse sizes outside the specification's bounds, read
   from its text first, with `MINIMUM_SEGMENT_ENTRIES` and `MAXIMUM_SEGMENT_ENTRIES`; the harness
   assumptions and the fuzz range narrow to match (P1-5).
3. New public names are taken before any tag: the type codes, speed ids and change bits made
   public, the missing completion codes with the specification's names as aliases,
   `completion::name`, the two enums, `try_enqueue`, `complete` and the fake controller example
   (P1-8, P1-12, P1-13). Every name is provisional until the release review ratifies them
   together.
4. `Capabilities::port_status_and_control` returns `Option<u64>`, `None` for port 0 or past
   `maximum_ports`, and the fuzz target asserts `None` (P1-11).

For the architect, one-way, not yet ruled:

- Whether `Event::Transfer` grows an `event_data: bool` field (P2-12). Recommendation: one sentence
  of documentation now, saying the pointer is a TRB address only while the driver issues no Event
  Data TRBs, and the field only when a builder for Event Data TRBs arrives.
- Any tag, asked separately when the weekly gates pass.

The maintainers' provisional calls: every two-way item above and in the appendix, and the names a
lane mints for them (`capacity`, `fits_one_transfer_request_block` and the rest), which go through
the naming review before any tag.

The publishing hold stands.

## Prior art

How Linux, U-Boot, Redox and the Rust USB crates handle what the items touch, and where this crate
differs, are in
[the prior art appendix](1060-extensible-host-controller-interface-outside-review/prior-art.md).

## Done when

The checks, and the steps that need a controller, are in
[the done-when appendix](1060-extensible-host-controller-interface-outside-review/done-when.md).
The review claims none of the hardware steps.

## BUGS

- P1-5 and P1-7 rest on the reviewer's memory of the specification and of Linux's ring code, not
  on its text. The recalled bound of P1-5 is the Event Ring Segment Table entry's; the lane reads
  which bound, if any, the text sets for a producer ring before it refuses one. This block's lane
  fetched the specification PDF but had no tool here to extract its text, so the bounds remain
  unread.
- The fuzz runs took 30 seconds per target on one machine, and the weekly job gives five minutes.
  Neither is a long campaign, and the repository commits no seed corpus, so each run starts cold.
- The review did not run the template's name-check helpers again. The review package's run covers
  the ratified names; the new names of P1-3, P1-8, P1-12 and P1-13 meet them in their own pull
  requests.
- The hostile-input surface a USB device presents (descriptors, report lengths) is not this
  crate's: it decodes only what the controller writes. The crate that parses descriptors is where
  that review belongs.
- P1-10 waits on a template release, and nothing in this repository can move it.
- The P2 order is the judgment of the reviewer on what a user asks for first. Nobody outside has
  asked for anything yet.

## Index row

An outside reading of `extensible_host_controller_interface`: one manifest fix blocks the first
release. Thirteen items go before it. The red mutation verdict, the unrecorded stranger run, the
read order for weakly ordered machines, ring capacity, the 64 kibibyte rule, `forbid(unsafe_code)`
and the template's mutation stack are the maintainers' calls. An `EndpointType` enum with the mask,
ring sizes refused outside the specification's bounds, public names for every code the crate
returns, two enums for `Option<bool>`, `try_enqueue` with a fake controller example, and an
`Option` port offset are ruled. Twelve follow it; the Event Data field awaits the architect. Any
tag is open; publishing stays held.
