---
status: PARTIAL
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
[the review package](../../notes/crate-reviews/extensible-host-controller-interface.md). The findings
cite files and symbols at that commit. The finding status list below reads the repository's current
`main`, and this block is the one record of where each finding and decision stands.

The review package and milestone 820 (`extensible_host_controller_interface`: proven xHCI logic,
released on its own) settle the crate name, its 146 ratified names, the license and the version.
They stay closed here. Milestone 820 owns the release path; this block owns the findings that land
inside it.

Reuse: the shared crate workflows and their scaffold, `cargo-mutants`, Kani, `cargo-fuzz`, and the
simulated controller in `src/tests.rs`. No new tool.

## Goal

A stranger who finds the crate can tell in one screen what it computes and what it leaves to the
driver. The same stranger can build, test, prove and falsify it from the README alone. They can
name every code the crate hands back, and write a driver on a weakly ordered machine without
guessing the order the ring discipline needs. The weekly gates are green.

Publishing stays held, and a tag needs its own ruling; both are under the decisions below.

## What the review covered

What the review read, ran and left unrun is in
[the coverage appendix](1060-extensible-host-controller-interface-outside-review/what-the-review-covered.md).

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
another repository. The block comment under `[package]` describes register mappings and a spawn
handoff that live elsewhere.

Change: a description of what the crate holds ("xHCI host controller logic as pure computation:
register offsets and decode, TRBs, rings, contexts and port status, proven with Kani"), `keywords =
["xhci", "usb", "no-std", "kani", "driver"]`, one sentence on why the crate stays off crates.io in
words that hold here (the publishing hold above), and the block comment cut to what the crate
holds. Two-way. XS.

### P1: before the first release

#### P1-1. The weekly mutation verdict is red, and the exemption file holds a census

The weekly run's `mutation` job fails with 99 survivors. Fifty sit in `src/verification.rs`, which
compiles only under `cfg(kani)`, so no host test can kill them. The other 49 are the ones a local
run reports. Most are `|` to `^` on constants whose bits a `_DO_NOT_OVERLAP` assertion proves
disjoint, such as `PORT_CHANGE_BITS_DO_NOT_OVERLAP` in `src/port.rs`. The rest are `<<` to `>>` on
`1 << 0`, and the dead clamps of P2-1. The source wrote those assertions to justify exemptions and then wrote none.
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
the mask. The variant names come from the specification's spellings and stand as built (2026-11-16 UTC). The harness asserts that bits 6 to 15 of dword 1 are
zero and that the type reads back as the variant's code, and the falsification patch drops the
mask. One-way (public signature). XS.

#### P1-4. The read order a consumer needs on a weakly ordered machine is stated nowhere

The doc of `TransferRequestBlockPlacement` in `src/ring.rs` says the driver writes the cycle dword
last, which is the producer's half. Nothing states the consumer's half. The controller writes an
event's cycle dword last, so a driver that reads all four dwords and then calls
`EventRing::is_ready` can see a fresh cycle bit beside stale data on aarch64 or riscv64. It must
read the control dword first, with acquire ordering. Nothing says a release fence belongs between
the TRB writes and the doorbell write. Two of the three bare-metal targets are weakly ordered.

Change: one paragraph on `EventRing::is_ready` and one on `ProducerRing::enqueue` stating the order
and the barrier, and a "What the caller does" section in the README listing the volatile accesses
and fences in sequence. Two-way. XS.

#### P1-5. The rings accept sizes a controller refuses

`EventRing::new` accepts any `entries > 0` and `ProducerRing::new` any `entries >= 2`. The reviewer
recalls the specification's Event Ring Segment Table entry as valid for 16 to 4,096 TRBs per
segment. The host tests in `src/tests.rs` build event rings of 2 and 4 entries,
`fuzz/fuzz_targets/event_ring.rs` ranges over 1 to 256, and the proofs over every `u16`, so they
spend part of their range on configurations no controller runs.

Ruled 2026-11-09. `EventRing::new` and `ProducerRing::new` refuse sizes outside the
specification's bounds, with named constants `MINIMUM_SEGMENT_ENTRIES` and
`MAXIMUM_SEGMENT_ENTRIES`. The lane reads the bounds from the specification text before it writes
them. The harness assumptions and the fuzz range narrow to match. One-way (published behavior). XS.

#### P1-6. A producer ring has no notion of being full

`ProducerRing::enqueue` always places the next TRB and wraps, so a caller who enqueues more than
`entries - 1` TRBs between two consumptions overwrites TRBs the controller has not reached. The crate's
own driver never does; a stranger's may. The only statement of the rule is a
comment in the host test `a_simulated_controller_runs_exactly_what_was_produced_across_laps`.

Change: `ProducerRing::capacity() -> u16` (`entries - 1`), a sentence on
`enqueue` stating the caller's rule, and a README `BUGS` line. P1-13 adds the refusing variant.
Two-way. XS.

#### P1-7. The 64 kibibyte buffer rule is not stated on the data builders

`TransferRequestBlock::data` and `TransferRequestBlock::normal` take any buffer and length. A `u16`
length keeps a TRB under 64 kibibytes, but a buffer that is not aligned can still cross a 64
kibibyte boundary, which Linux splits a TD at and the reviewer recalls as a specification rule.
The crate's own layout never crosses one; a stranger's might.

Change: read the rule, state it on both builders and in the README `BUGS`, and add
`fits_one_transfer_request_block(buffer, len) -> bool`. Two-way. XS.

#### P1-8. The codes the crate hands back have no public names

`TransferRequestBlock::kind` and `Event::Other` return a type code, and
`src/transfer_request_block_type.rs` keeps `SETUP`, `LINK`, every command and every event code
`pub(crate)`; only `NORMAL`, `DATA` and `STATUS` are public. In `src/port.rs`, `speed::FULL` is
public and `LOW`, `HIGH`, `SUPER` and `SUPER_PLUS` are not, so a user cannot compare `speed_of`
against anything but full speed. Five of the seven change bits (`ENABLE_CHANGE`,
`WARM_RESET_CHANGE`, `OVER_CURRENT_CHANGE`, `LINK_CHANGE`, `CONFIGURATION_ERROR_CHANGE`) are
`pub(crate)`. `src/completion.rs` names five of the roughly 37 completion codes, so a driver that
meets Babble, Stopped or Command Aborted gets a number it cannot name.

Ruled 2026-11-09. These names land before any tag. The existing type codes, speed ids and change
bits become public. The crate adds the missing completion codes with spelled-out names and the
specification's name as `doc(alias)`. `completion::name(code) -> &'static str` sits beside
`port::speed_name`. The new names stand as built (2026-11-16 UTC). One-way (names). S.

#### P1-9. The crate holds no `unsafe` and does not say so

No `unsafe` block exists, yet the manifest enables `unsafe_op_in_unsafe_fn` and
`undocumented_unsafe_blocks`, `SECURITY.md` speaks of the crate corrupting "memory it confines",
and `.github/workflows/fuzz.yml` keeps crash inputs private. Change: `#![forbid(unsafe_code)]` in
`src/lib.rs`, and drop the two lints. Reword the security policy to what a report here can be: a
wrong field position, a wrong ring step, a proof that does not hold. Set `public-crash-inputs:
true`, which the standing rule allows for a crate with no `unsafe`. Two-way. XS.

#### P1-10. The next template release brings the mutation stack

Every caller pins `nifeos/crate-workflows` at `cd1b484`, at or past release `v7`, so the `lag` job
passes. The template's unreleased `main` adds a `mutation.yml` caller, `check / mutation-diff` as a
required check, and a release gate that wants a green full run.

Change, once the template releases: take the pin bump, and add the scaffold's `mutation.yml`
caller with this repository's own cron line and the budget file. Dispatch `mutation.yml` on
`main`, then apply the settings, so the tenth check becomes required only after it has reported. P1-1
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
`DeviceToHost`. The enum and variant names stand as built (2026-11-16 UTC). One-way (names and shapes). XS.

#### P1-13. A fake controller a stranger can run, and a ring that refuses when full

The host test `a_simulated_controller_runs_exactly_what_was_produced_across_laps` holds a consumer
that follows links and toggles its cycle, which is the test double a driver author wants, and it
is private to the tests. No `examples/` directory exists.

Ruled 2026-11-09. Before any tag, `ProducerRing` gains `try_enqueue`, which refuses when the ring
is full, and `complete(transfer_request_block_physical_address)`, which the caller reports each
completion event to. `examples/fake_controller.rs` builds the rings, enqueues a control transfer,
consumes it as the controller would and decodes the events it writes. The names stand as built
(2026-11-16 UTC). One-way (names). M.

#### P1-14. The Event Data bit is ignored

When a transfer event has its Event Data bit (bit 2 of its last dword) on, its pointer field holds
the event data value, not a TRB address. For a driver that issues Event Data TRBs, `Event::decode` in
`src/event.rs` reports that value as an address.

Ruled 2026-11-10. `Event::Transfer` grows no `event_data` field now. Its documentation states that
the pointer is a TRB address only while the driver issues no Event Data TRBs. The field arrives
with a builder for Event Data TRBs. Two-way (documentation). XS.

## Finding status

- P0-1, publish-facing manifest fields: Done, nifeos/extensible_host_controller_interface#7 and #8.
- P1-1, red weekly mutation verdict: Done, nifeos/extensible_host_controller_interface#5.
- P1-2, stranger run and README commands: Done, nifeos/extensible_host_controller_interface#2, #4,
  #20 and #23.
- P1-3, endpoint type kept to its field: Done, nifeos/extensible_host_controller_interface#11 and
  #20.
- P1-4, read and write order on a weakly ordered machine: Done,
  nifeos/extensible_host_controller_interface#9.
- P1-5, ring sizes a controller refuses: Done, nifeos/extensible_host_controller_interface#11 and
  #20.
- P1-6, a producer ring has no notion of full: Done, nifeos/extensible_host_controller_interface#9.
- P1-7, the 64 kibibyte buffer rule: Done, nifeos/extensible_host_controller_interface#9.
- P1-8, public names for every code handed back: Done,
  nifeos/extensible_host_controller_interface#12.
- P1-9, no `unsafe`, stated: Done, nifeos/extensible_host_controller_interface#8.
- P1-10, the template release with the mutation stack: Done,
  nifeos/extensible_host_controller_interface#3 and #21.
- P1-11, port 0 alias and unbounded port offsets: Done,
  nifeos/extensible_host_controller_interface#13.
- P1-12, two `Option<bool>` tri-states: Done, nifeos/extensible_host_controller_interface#13.
- P1-13, a fake controller and a refusing enqueue: Done,
  nifeos/extensible_host_controller_interface#14.
- P1-14, the Event Data bit: Done, nifeos/extensible_host_controller_interface#13.
- P2-1, dead interrupt interval clamps: Done, nifeos/extensible_host_controller_interface#15.
- P2-2, five missing proofs: Done, nifeos/extensible_host_controller_interface#10.
- P2-3, code on the crate's front page: Done, nifeos/extensible_host_controller_interface#15 and
  #20.
- P2-4, the busy bit on the dequeue pointer: Done, nifeos/extensible_host_controller_interface#15.
- P2-5, one interrupter: Done, nifeos/extensible_host_controller_interface#15.
- P2-6, the Supported Protocol capability: Done, nifeos/extensible_host_controller_interface#15.
- P2-7, named constants for the driver's waits: Done,
  nifeos/extensible_host_controller_interface#15.
- P2-8, one driver's direct memory access layout: Done,
  nifeos/extensible_host_controller_interface#15.
- P2-9, `doc(alias)` on the bit constants: Done, nifeos/extensible_host_controller_interface#15.
- P2-10, small text fixes: Done, nifeos/extensible_host_controller_interface#15, with the repository
  description and topics set.
- P2-11, `rust-version` above what the code needs: Done,
  nifeos/extensible_host_controller_interface#15.
- The hardware checks of the done-when appendix: Waiting on a board run.
  nifeos/extensible_host_controller_interface#23 names each one in the README's BUGS as unrun on
  silicon, and none has a bench record.
- Publishing: Waiting on the last outside-review milestone (ruled 2026-11-18 UTC); `publish = false`
  stays.

## Decisions this block needs

Ruled, and in force:

1. The shapes of P1-3, P1-5, P1-8, P1-11, P1-12, P1-13 and P1-14 (ruled 2026-11-09 and
   2026-11-10), each stated in its finding.
2. The names of P1-3, P1-5 to P1-9, P1-12 and P1-13 stand as built (ruled 2026-11-16; recorded by
   nife #3664). They are `EndpointType` and its variants, the two segment bounds, `ProducerRing::capacity`,
   `fits_one_transfer_request_block`, the public type codes, speed ids, change bits and completion
   codes with `completion::name`, `DebounceVerdict`, `TransferType`, `try_enqueue`, `complete` and
   `examples/fake_controller.rs`.
3. The crate takes the pre-release tag `v0.1.0-pre.3` after a green dry run on `main`, with nothing
   published (ruled 2026-11-19). Done: `nifeos/extensible_host_controller_interface` #22 merged and
   the tag exists.
4. No crate publishes until every outside review milestone is delivered (ruled 2026-11-18).
   `publish = false` stays.
5. The names the P2 batch added stand as built, placement included (ruled 2026-11-19), listed under
   Open decisions.

The maintainers' provisional calls: every two-way item above and in the appendix. Nothing is open.

## Open decisions

None. The names the P2 batch added stand as built, placement included (ruled 2026-11-19 UTC):
`Capabilities::interrupter_register`, `SUPPORTED_PROTOCOL_EXTENDED_CAPABILITY_ID`,
`SupportedProtocol` (fields `major_revision`, `minor_revision`, `name`, `first_port_number`,
`port_count` and `speed_id_count`; methods `decode` and `contains_port`), the crate-root
`HALT_TIMEOUT_MILLISECONDS`, `CONTROLLER_RESET_TIMEOUT_MILLISECONDS` and
`COMMAND_COMPLETION_TIMEOUT_MILLISECONDS`, and `port::POWER_ON_WAIT_MILLISECONDS` and
`port::RESET_TIMEOUT_MILLISECONDS`. The publishing hold stands.

## Prior art

Linux, U-Boot, Redox and the Rust USB crates, against this one, are in
[the prior art appendix](1060-extensible-host-controller-interface-outside-review/prior-art.md).

## Done when

The checks, and the hardware steps the review claims none of, are in
[the done-when appendix](1060-extensible-host-controller-interface-outside-review/done-when.md).

## BUGS

- The fuzz runs took 30 seconds per target, and the repository commits no seed corpus.
- The crate decodes only what the controller writes. The hostile input a USB device presents
  (descriptors, report lengths) belongs to the review of the crate that parses descriptors.
- The P2 order is the judgment of the reviewer on what a user asks for first. Nobody outside has
  asked for anything yet.

## Follow-on

- **Done.** Every P0, P1 and P2 item, in `nifeos/extensible_host_controller_interface` #2 to #15,
  #20 and #21; the finding status list names each one's pull request. The bounds of P1-5 and the rule
  of P1-7 come from the specification text (xHCI 1.2 section 4.9.2.2, Table 6-1, section 6.4.1),
  not from the reviewer's memory.
- **Done.** `nifeos/extensible_host_controller_interface` #23 names
  every hardware check of the done-when appendix in the README's BUGS as unrun on silicon.
- **Done.** The `v0.1.0-pre.3` tag, by #22. Read from the tags.
- **Outstanding.** The hardware checks of
  [the done-when appendix](1060-extensible-host-controller-interface-outside-review/done-when.md):
  none has a bench record. Read from the README's BUGS and nife's `bench/`.

## Index row

An outside reading of `extensible_host_controller_interface`: one manifest fix, fourteen items
before the first release and eleven after it, all landed. Every name, the P1 and the P2 batch, is ratified. The hardware checks are unrun, and publishing stays held.
