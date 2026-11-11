---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1074. `designware_ethernet`: what an outside maintainer would fix before the move and the first tag

Raised by a read-only outside review of `crates/designware_ethernet`, as a stranger meeting it for
the first time would. It changes nothing; it lists what the next lanes change.

The review took its baseline from [the review package](../../notes/crate-reviews/designware-ethernet.md)
and reopens none of the decisions it settles. Those are the crate name, the 173 ratified names, the
license expression, the provenance notices, the six harnesses and their patches, and the two
Motorcomm alias sets that already wait for the architect. Where a later rule or a later template gate contradicts a
ratified name, the item says so and leaves the rename to the release review that the rules provide
for.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `helpers/crate_move.py`,
cargo-mutants, Kani, the crate's own `simulator`. No new tool.

## Goal

A stranger who finds `designware_ethernet` in its own repository can tell in one screen what it is
for and which board the authors aimed it at. The same stranger can build, test, prove and falsify
it from the README alone, and finds the repository's gates green on the first pull request for the
crate's own reasons.
The public surface at the first tag is the surface the crate means to keep.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1074-designware-ethernet-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the prior art appendix](1074-designware-ethernet-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S half a day, M a day or two. A door is one-way when
somebody outside the repository acts on it after the first tag (a public name or path, a published
behavior, a layout, a dependency); everything else is two-way and the maintainer decides it. The
crate has no release yet, so a public rename is one-way only from the tag on; the items say
"one-way at the tag" for those.

### P0: blocks the move

1. The template's naming and prose gates refuse the crate as it stands.
   Finding: `check / standalone` runs `spelled_out`, `register_names`, `prose_acronyms`,
   `passive_voice`, `prose_caps` and `typos_gate` at a ceiling of zero with no baseline file. Run
   over this crate they report 2, 91, 26, 3 (allowance 1), 1 and a missing `_typos.toml`. nife
   carries the first three as report counts (`helpers/template_report_baseline.txt` lines 73 to 75);
   a crate repository carries no baseline, so the first pull request after the move is red for
   reasons that are not that pull request's. The sibling crates already moved pass this job on
   `main`, so the gate is as described.
   The 91 register findings fall into four classes. Thirty-three come from the address rule reading
   `physical_layer_device_address` (an MDIO bus address, five bits) and `station_address` (a MAC
   address) as memory addresses. The rule in `design/naming.md` is about memory. The helper offers
   its escape hatch only for fields and constants, not for the parameters and bindings that make up
   most of these. Sixteen are a length without a unit: `PROBE_LENGTH`
   (`src/cache_coherence.rs:24`), the three `_LENGTH_MASK` descriptor fields <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   (`src/descriptor.rs:101,115,143`), the two `_RING_LENGTH_REGISTER` offsets <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   (`src/registers.rs:396,401`), the three burst-length fields of `BusConfiguration` <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   (`src/network_interface_controller.rs:169-180`) and test names. Four are an offset that does not <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   end `_REGISTER`: `DIRECT_MEMORY_ACCESS_REGISTER_PAGE` (`src/registers.rs:467`, used by the kernel <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   and `components`) and `GMAC0_SYSTEM_CONTROLLER_OFFSET` (`src/jh7110.rs:107`, used by <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `machine_discovery`) among them. One is a missing alias (item 10). The 26 acronym findings are
   `RFD`, `RFA`, `IOMMU`, `MII`, `AXI`, `MTL` and their kin unexpanded at first use in doc comments,
   and `YT8531` and `PHY` in the README's first paragraph. The two `spelled_out` findings are item 11.
   Change in the template: scope the address rule to memory addresses, or accept `station_address`
   and `physical_layer_device_address` as the specification's names (both IEEE 802.3 terms), and
   accept `YT8531` as a part number. Change in the crate: rename the private lengths and offsets
   (`PROBE_BYTES`, a `_BYTES` on the burst lengths or a doc saying they are beats). Expand the
   acronyms once per file, split the README's long sentence and recast two passives. Copy the
   scaffold's `_typos.toml` and `.github/accepted-words` with `designware`, `motorcomm`, `gmac`,
   `jh`, `yt` and `picoseconds` and their reasons. The five public names among the findings are a
   decision (below).
   Door: two-way for the private names and the prose; one-way at the tag for the five public
   names. Effort: M, split between a template lane and a crate lane.

2. The manifest points outside the repository and names no repository.
   Finding: `publish = false` carries a comment that sends the reader to
   `crates/publish-allowlist.txt`, a file of this tree (`Cargo.toml:11`). The comment block below it <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   names `net_stack`'s transport file by path (`Cargo.toml:14-22`). The manifest has no `repository` <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   or `documentation` field, which `cargo package` warns about and which the scaffold's badge row
   and docs.rs link need.
   Change: the comment says what a stranger can act on ("set to `true` only in the release that
   publishes"). The `net_stack` sentence names the two halves by role, not by path. Add
   `repository`. An earlier ruling settles publishing itself, and this block does not reopen it.
   Door: two-way. Effort: XS.

### P1: before the first tag

3. The receive loop and the reclaim loop have no bound, and the recorded mutation runs show it.
   Finding: `DataPlane::receive` (`src/data_plane.rs:137-172`) loops until a descriptor reads <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `NotYet` or a frame arrives. It consumes a run of `Drop` descriptors one per iteration with no
   cap. `reclaim` (`src/data_plane.rs:216-226`) loops until the cursors meet. Twelve `gap` rows in <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   the triage, and the 14 timeouts of the latest census, come from mutants of these two loops and of
   `receive_descriptor_words` that never end. Cargo-mutants reads a hang where a test would have
   failed, and the triage defers the fix as a change to production logic. On hardware, a storm of error
   frames keeps `receive` from returning to its caller for as long as the device refills faster than
   the loop reposts; Linux's NAPI budget and every polled driver in the table bound one poll.
   Change: bound `receive` at `RECEIVE_ENTRIES` iterations per call and return `None` when the
   budget runs out (the caller polls again). The ring bounds `reclaim` once its arithmetic is
   correct, so give it a `TRANSMIT_ENTRIES` cap too. Add one test with sixteen error frames in a
   row. The twelve `gap` rows then become kills, and the census's timeouts drop.
   Door: two-way (a caller that saw a frame after sixteen drops now sees `None` and polls again).
   Effort: S.

4. `Link` and `Speed` live in the PHY's module but belong to the MAC.
   Finding: `network_interface_controller::mac_configuration` and `start` take
   `motorcomm_physical_layer_device::Link` and `Speed` (`src/motorcomm_physical_layer_device.rs:152-187`; <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `src/network_interface_controller.rs:12`). They describe the <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   link the MAC runs at; nothing in them is Motorcomm's. A second PHY module would import its link
   type from Motorcomm's, and a user who reads the controller module finds the type under a vendor's
   name.
   Change: a neutral home (`link`, or the controller module) with the PHY module re-exporting both,
   before the tag. The architect names the module (below).
   Door: one-way at the tag (a public path). Effort: S.

5. Twelve compile-time gates are public API.
   Finding: `pub const …_DO_NOT_OVERLAP: ()` items sit in every public module:
   `network_interface_controller` (lines 220, 251, 468, 541), `management_data_input_output`
   (234, 351, 394), `motorcomm_physical_layer_device` (140, 281, 586), `jh7110` (77, 136) and
   `cache_coherence` (62). They exist so a mutation exclusion can cite them, and docs.rs will list
   twelve unit-typed constants beside the API. The three in `descriptor.rs` sit in a private module
   and no caller reaches them, which shows the shape wanted.
   Change: make each `pub(crate)` with `#[allow(dead_code)]`, as `descriptor.rs` does; the
   exclusion still names them. Or name them with a leading underscore and drop the attribute.
   Door: one-way at the tag if they stay public (removing a public item later is a breaking
   change). Effort: XS. The architect confirms (below).

6. The visibility of register and clause 22 constants follows no rule.
   Finding: `MAC_CONFIGURATION_REGISTER` is `pub(crate)` (`src/registers.rs:22`) while seven of its <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   bits are `pub` (lines 36 to 79). `BASIC_MODE_CONTROL_RESET` and
   `BASIC_MODE_STATUS_AUTONEGOTIATION_COMPLETE` are `pub` (`src/management_data_input_output.rs:88,180`) <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   while the registers they live in and `BASIC_MODE_STATUS_LINK_STATUS` are `pub(crate)`. The
   `ManagementDataInputOutput` trait takes `register: u8`, yet a caller or an implementer must
   restate `0x00` and `0x01`. No caller outside the crate uses any of the stray public bits; the
   callers reach `DIRECT_MEMORY_ACCESS_REGISTER_PAGE`, `REGISTER_WINDOW_BYTES`, the two tail
   pointer registers, `MAC_ADDRESS_0_HIGH_REGISTER`, `MAC_ADDRESS_0_LOW_REGISTER` and
   `CHANNEL_STATUS_REGISTER`.
   Change: one rule, written at the top of `registers` and `management_data_input_output`. The
   compatible choice is the narrow one: `pub(crate)` for every constant no caller uses, since
   widening later breaks nobody and narrowing later breaks everybody. The whole-map alternative is
   also defensible: every BSD `*reg.h` and Linux's `dwmac4.h` publish the complete map, and a kernel
   half outside the crate is this crate's design. It is the architect's if wanted, and this block
   recommends against it until a caller asks.
   Door: two-way now; one-way at the tag. Effort: S.

7. The README does not yet carry a stranger through a build, a proof and a falsification.
   Finding: the scaffold's README shape has `## Building and testing`, `### Running the proofs`
   and `### Running one falsification patch`; the README has the install line and `patch -p3` in
   prose (`README.md` `## What is proven`) and no build, test, bare-metal, single-harness or undo command. The
   example (`README.md` `## EXAMPLES`) is not a documentation test, so `cargo test` would not notice it
   breaking. It names its bindings `off`, `v`, `b` and `buf`, the clipped forms the crate's own
   names forbid. After the move the patches apply with `-p1`, not `-p3`. The first paragraph says
   "PHY (physical layer device)" and "YT8531" before the text introduces either (item 1 counts them).
   Change: add the three sections with real commands from this crate. Include the README as the
   crate's documentation (`#![doc = include_str!("../README.md")]`), or move the example to a
   documentation test so `cargo test` compiles it. Spell the bindings out. Write `-p1` once the move
   has regenerated the patches.
   Door: two-way. Effort: S.

8. The crate's documentation names a bench machine sixteen times and the board never.
   Finding: `radon` appears in the first line of the crate documentation (`src/lib.rs:2`), in <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `jh7110.rs` five times, in `network_interface_controller.rs` six times and in
   `motorcomm_physical_layer_device.rs` four times. It appears as the owner of the bus configuration,
   the fallback PHY settings, the device tree and the ports. A reader of the standalone crate has no
   radon. Neither `lib.rs` nor the README names the board the settings describe, the StarFive
   VisionFive 2, and only `jh7110.rs` explains "both trees".
   Change: name the board and its two device trees once in `lib.rs` and the README, and say "the
   board" or "the VisionFive 2" where the text says `radon`. The bench record stays in nife's notes.
   Door: two-way. Effort: S.

9. The falsification patches target this tree, and the move must regenerate them.
   Finding: every patch names `a/crates/designware_ethernet/src/…` and three carry a `Name:` line
   in their context (`falsifications/proofs.a_coherent_verdict_…`, `…transmit_descriptor_…`,
   `…every_ring_offset_…`). The template's `proofs / falsify` applies with `git apply -p1` from the
   crate root and the move deletes every `Name:` block, so the patches as they stand would fail
   there. `helpers/crate_move.py` already regenerates them (`regenerate_patch`) and refuses a patch
   that applies only at an offset (`patch_application`, `-F0`).
   Change: nothing new; the move's pull request shows all six regenerated patches applying with
   `-F0` and each turning its own harness red, and the README's command follows (item 7).
   Door: two-way. Effort: XS.

10. Two rename artifacts in documentation.
   Finding: `CLOCK_RANGE_SHIFT` carries
   `#[doc(alias = "GMAC_MAC_MANAGEMENT_DATA_INPUT_OUTPUT_ADDRESS_REGISTER_CR_SHIFT")]`
   (`src/management_data_input_output.rs:21`), a spelling no source has; its doc comment names the <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   real one, `GMAC_MAC_MDIO_ADDR_CR_SHIFT`, and `register_names` flags the mismatch. The two
   `100BASE-TX` constants open their doc comments with their own names in the place the source
   spelling goes (`src/management_data_input_output.rs:129,134`) where every neighbor opens with <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `ANLPAR_…`; the aliases `ANLPAR_TX_FD` and `ANLPAR_TX` are right.
   Change: the alias reads `GMAC_MAC_MDIO_ADDR_CR_SHIFT`; the two openings read `(ANLPAR_TX_FD,
   OpenBSD mii.h)` and `(ANLPAR_TX, OpenBSD mii.h)`.
   Door: two-way. Effort: XS.

11. `100BASE-TX` meets the spelled-out gate.
   Finding: `spelled_out.py` refuses `AUTONEGOTIATION_LINK_PARTNER_100BASE_TX` and
   `…_100BASE_TX_FULL_DUPLEX` (`src/management_data_input_output.rs:133,139`): `tx` is on the <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   spelled-out list and the helper never keeps a source spelling for an integer constant. The
   review package accepted the names as the standard's term, and `design/naming.md` says a
   specification's term of art keeps its spelling inside a name this tree authors. `100BASE-TX` is
   IEEE 802.3's name for the physical layer. Its `X` stands for the 4B5B encoding, so "transmit"
   would be a wrong word, and the names are `pub(crate)`.
   Change: in the template, a term-of-art allowance for `100BASE_TX` (and `100BASE_T4`'s kin)
   that `spelled_out.py` reads, with the reason, and no rename. If the template keeps the rule as it
   is, the fallback is `…_100BASE_T_X…`, which nobody writes, or a renaming that drops the
   standard's name, which this block recommends against.
   Door: two-way (private names). Effort: XS, template lane.

### P2: later

The P2 items are in [the appendix](1074-designware-ethernet-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. Each is a public name or path that the first tag fixes.

### Decision 1: Where the link type lives

Background: a resolved link is a speed and a duplex. The controller's `mac_configuration` and
`start` take it, and the PHY module produces it from clause 22 registers. Today the type is
`motorcomm_physical_layer_device::Link`, so the controller module imports its own input from a
vendor's module. Linux keeps the equivalent in `phylink` and `linux/phy.h`, shared by every MAC and
every PHY; OpenBSD keeps it in `mii` (`ifmedia` words), shared the same way.

Recommendation: a crate-level `link` module holding `Link` and `Speed`, with
`motorcomm_physical_layer_device` re-exporting both, so `read_link` still returns a type found next
to it. The alternative, `network_interface_controller::Link`, also works and saves a module; it
puts the PHY's output under the MAC's name, which reads backwards when a second PHY arrives.

Example: `use designware_ethernet::link::{Link, Speed}; controller::start(&mut hardware, Link {
speed: Speed::Thousand, full_duplex: true }, false);`

### Decision 2: Whether the compile-time gates are API

Background: item 5. Twelve `pub const …_DO_NOT_OVERLAP: ()` items in public modules exist so the
mutation exclusions can name them. Rust has no way to publish a name for tooling without publishing
it to users. A `pub(crate)` constant is still a name the exclusion file can cite, and the three in
`descriptor.rs` already work that way. No other crate in the organization publishes such items, as
far as this review read (not verified across every repository).

Recommendation: `pub(crate)` with `#[allow(dead_code)]`, before the tag. Keeping them public costs
nothing today and a breaking change on the day anyone wants them gone.

Example: `pub(crate) const COMMAND_WORD_FIELDS_DO_NOT_OVERLAP: () = assert!(…);` and the exclusion
line in `.github/mutants-exempts` cites it unchanged.

### Decision 3: The five public names the register gate refuses

Background: item 1. The template's `register_names` gate refuses, among its 91 findings, five names
that are public and ratified: `TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_MASK`,
`TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_MASK` and `RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_MASK`
(a length carries its unit), `DIRECT_MEMORY_ACCESS_REGISTER_PAGE` (an offset ends `_REGISTER` or
another suffix the rule lists), and `GMAC0_SYSTEM_CONTROLLER_OFFSET` (the same). The rule that a
release review rechecks a name ratified before a later rule applies; this is that review. The
three masks mirror OpenBSD's `TDES2_BUF1_LEN`, `TDES3_FRAME_LEN` and `RDES3_LENGTH`, which measure
bytes. The page constant names the one page of the window the kernel hands a process; the syscon
word is the register at byte `0xc` of the AON syscon.

Recommendation: `…_BUFFER_1_LENGTH_BYTES_MASK`, `…_FRAME_LENGTH_BYTES_MASK`,
`…_PACKET_LENGTH_BYTES_MASK`, `DIRECT_MEMORY_ACCESS_REGISTER_PAGE_OFFSET_BYTES` and
`GMAC0_INTERFACE_SELECT_REGISTER` (it is the syscon register holding `gmac0`'s interface select;
`SYSTEM_CONTROLLER` then moves into the doc comment). Each keeps its alias. The alternative is a
template allowance for these five, which keeps names the rule would refuse in any new crate.

Example: `let length = (word_3 & RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK) as usize;`

### Not brought

The two Motorcomm alias sets wait for the architect already and are not restated here. Items 6 and
21 are two-way until the tag and carry a recommendation; they come to the architect only if the
maintainer wants the wider option. Item 11 is a template rule, not a name.

## Done when

The checks are in [the done-when appendix](1074-designware-ethernet-outside-review/done-when.md).

## BUGS

- Items 1 (the address rule) and 11 are the template's to fix. Until they land, the crate cannot
  reach zero findings without renames this block recommends against, and the move waits on them or
  on an allowance file the template does not yet read.
- The review did not verify item 16's question, which tree inverts the 10 Mbit/s transmit clock,
  against either device tree.
- Items 15 and 17 cite Linux's `motorcomm.c` and the controller databook from memory, marked so in
  the text. The review took the crate's own account of OpenBSD as given.
- The review did not run Kani, cargo-mutants or Miri, and did not build at `rust-version` 1.88.
  The template's weekly `msrv` job will; a lane that touches the proofs runs `cargo kani` before
  it pushes.
- `notes/project-metrics/mutation-rowless-survivors.csv` still names the crate's functions by their
  earlier spellings (`fifo_bytes`, `mac_conf`, `rx_verdict`); a nife housekeeping item, not the
  crate's.

## Index row

An outside review of `designware_ethernet` before its move. Two items would turn the new
repository's first pull request red, nine land before the first tag, and ten follow later. The nine
cover the unbounded receive loop behind twelve recorded mutation gaps, the link type's home, twelve
public compile-time gates, an unruled visibility mix and the README's missing stranger path. They
also cover a bench machine named sixteen times, the patches the move regenerates, two rename
artifacts, and `100BASE-TX` against the spelled-out gate.
