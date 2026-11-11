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
and reopens none of the decisions it settles: the crate name, the 173 ratified names, the license
expression, the provenance notices, the six harnesses and their patches, and the two Motorcomm alias
sets that already wait for the architect. Where a later rule or a later template gate contradicts a
ratified name, the item says so and leaves the rename to the release review that the rules provide
for.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `helpers/crate_move.py`,
cargo-mutants, Kani, the crate's own `simulator`. No new tool.

## Goal

A stranger who finds `designware_ethernet` in its own repository can tell in one screen what it is
for and which board it was written against, can build, test, prove and falsify it from the README
alone, and finds the repository's gates green on the first pull request for the crate's own reasons.
The public surface at the first tag is the surface the crate means to keep.

## What was checked, and how

Verified firsthand, on a fresh clone at the head of `main`, on the tree's pinned nightly toolchain:

- `cargo test -p designware_ethernet`: 87 unit tests and 1 documentation test pass. `cargo clippy
  --all-targets -- -D warnings` is clean. `cargo doc --no-deps` with
  `-D rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags -D warnings` is clean.
- The crate builds for `riscv64imac-unknown-none-elf`, so `no_std` holds. The template's
  `unsafe_scan.py` finds no `unsafe` outside tests; there is none anywhere in the crate.
- The README's example compiles and runs when copied into an example target by hand. `cargo test`
  does not compile it (item 7).
- All six falsification patches apply today, with `patch -p3` from the crate root as the README
  says and with `patch -p1` from the tree root. Kani itself was not run here.
- Every helper the template's `check / standalone` job runs, at the template's head and again at
  nife's vendored pin (the two agree): `spelled_out` 2 findings, `register_names` 91,
  `prose_acronyms` 26, `passive_voice` 3 in the README against an allowance of 1, `prose_caps` one
  53-word sentence against a cap of 40, `typos_gate` refuses the crate for lacking `_typos.toml`;
  `unit_symbols`, `question_shape`, `british_spelling`, `service_model`, `contract_names`,
  `prose_figures`, `history_phrases`, `orphan_modules`, `check_links`, `readme_sections`,
  `readme_acronyms` and the three length ratchets find nothing; `abbreviations` reports 27
  components (report only); `standalone.py` reports `publish = false`, 178 provenance lines, one
  section citation, one milestone citation and one architect's name, all in text the move deletes.
  nife holds the same three counts as report baselines in `helpers/template_report_baseline.txt`.
- `cargo package --list` ships `src/`, `falsifications/`, the manifest, the README and both license
  files, and warns that the manifest has no `repository`, `documentation` or `homepage`.
- Every item the kernel, `components` and `machine_discovery` reach in the crate, by grep, to judge
  what the public surface must keep.
- Download figures for the prior-art crates, from the crates.io API.

Read, not run: the recorded mutation census and its triage (`notes/project-metrics/mutation-census.csv`,
`mutation-triage.csv`): the latest census row tested 638 mutants, caught 597, missed 12, timed out
14, 15 unviable; the triage holds 105 rows for the crate, 87 killed, 6 equivalent with an argument
each, and 12 `gap` rows, every one a loop that never ends under the mutant (item 3). The mutation
census was not rerun; it belongs in its window.

Not run: Kani, cargo-mutants, Miri, a build at `rust-version` 1.88 (the tree pins a nightly), the
bare-metal builds for aarch64 and x86_64, a release dry run, and the hardware itself. Every claim
about what the controller or the PHY does on silicon rests on the sources the crate cites and on
the bench record in `notes/designware-ethernet.md`, not on a measurement made for this review.

## Prior art

How the crate sits against what a driver author already knows. Recent downloads are the last 90
days at the time of the review; figures marked "not fetched" come from reading those sources
earlier and were not checked for this review.

| source | kind | rings and polling | coherence | what a reader coming from it will miss here |
|---|---|---|---|---|
| Linux `stmmac` (`dwmac4`) | kernel driver, the reference for this silicon | interrupts and NAPI with a per-poll budget; ring sizes in the hundreds (not fetched) | assumed from the device tree (`dma-noncoherent` or its absence) and the architecture's DMA API | interrupts, more than one queue, offloads, jumbo frames, multicast, phylink, the MDIO clock range read from the clock framework |
| OpenBSD `dwqe` and `ytphy` | the crate's source | interrupt driven, ring tail written per fill; 30,000 by 10 µs reset poll, 2,000 by 10 µs MDIO poll, which the crate keeps | none: the bus_dma layer syncs | the receive buffer size inherited from reset (the crate writes it), jumbo frames (the crate refuses them) |
| FreeBSD `eqos`, NetBSD `eqos`, FreeBSD `mcommphy` | the cross-checks the crate names | interrupt driven | bus_dma | same as OpenBSD |
| U-Boot `dwc_eth_qos` | boot-time driver, the source of the receive buffer size field | polled, a handful of descriptors | flushes and invalidates around every descriptor | nothing: closest in shape, but written for one short session |
| `smoltcp` 0.14.0, 2.82M recent / 10.1M all-time | the stack this crate feeds | `phy::Device` with `RxToken` and `TxToken` | the device's business | a token that hands the frame to a closure instead of a copy into the caller's buffer (item 13) |
| `stm32-eth` 0.8.2, 17.3k / 41.9k | Rust driver for the older DesignWare MAC (3.x descriptors) on STM32 | polled or interrupt, static rings, `smoltcp` integration | none needed on those cores | `smoltcp` `Device` implemented in the crate; the descriptor format differs |
| `embassy-stm32` 0.6.0, 191k / 822k | async HAL with an Ethernet driver for the same STM32 MACs | async with interrupts | none needed | async, interrupt waker |
| `dwmac-my` 0.2.0, 9 / 35 | Rust DWMAC driver for the StarFive VisionFive 2, the same board | polled | its ArceOS HAL flushes descriptors through the L2 cache (from `notes/designware-ethernet.md`, not fetched) | a whole driver, not logic behind traits |

Where this crate differs: it dereferences no pointer and touches no hardware; register access, the
DMA region and the two tail pointers come through three traits, so every sequence runs on a host
against a simulated controller. It measures DMA coherence once per bring-up with a MAC loopback and
names the direction of any failure, where every other driver here assumes or always flushes. It
bounds every wait. It writes the receive buffer size that OpenBSD inherits. It refuses jumbo frames
in the MAC rather than dropping them from the ring. Six machine-checked proofs ship with the patch
that makes each fail. What it costs: one queue each way, polled, no interrupts, no offloads, no
multicast, a copy per frame, and the smaller surface in the last column.

A concrete example: a receive descriptor whose word 3 reads first, last and a packet length of
`0x7fff` is judged `Drop` by `receive_verdict`, and the harness proves that for every word the
device can write; a driver that trusted the field would copy 32,763 bytes out of a 2,048-byte
buffer.

## The items

Effort: XS is under an hour for a lane, S half a day, M a day or two. A door is one-way when
somebody outside the repository acts on it after the first tag (a public name or path, a published
behavior, a layout, a dependency); everything else is two-way and the maintainer decides it. Nothing
is published yet, so a public rename is one-way only from the tag on; the items say "one-way at the
tag" for those.

### P0: blocks the move

1. The template's naming and prose gates refuse the crate as it stands.
   Finding: `check / standalone` runs `spelled_out`, `register_names`, `prose_acronyms`,
   `passive_voice`, `prose_caps` and `typos_gate` at a ceiling of zero with no baseline file. Run
   over this crate they report 2, 91, 26, 3 (allowance 1), 1 and a missing `_typos.toml`. nife
   carries the first three as report counts (`helpers/template_report_baseline.txt` lines 73 to 75);
   a crate repository carries no baseline, so the first pull request after the move is red for
   reasons that are not that pull request's. The sibling crates already moved pass this job on
   `main`, so the gate is as described.
   The 91 register findings fall into four classes. Thirty-three are the address rule reading
   `physical_layer_device_address` (an MDIO bus address, five bits) and `station_address` (a MAC
   address) as memory addresses; the rule in `design/naming.md` is about memory, and the helper
   offers its escape hatch only for fields and constants, not for the parameters and bindings that
   make up most of these. Sixteen are a length without a unit: `PROBE_LENGTH`
   (`src/cache_coherence.rs:24`), the three `_LENGTH_MASK` descriptor fields
   (`src/descriptor.rs:101,115,143`), the two `_RING_LENGTH_REGISTER` offsets
   (`src/registers.rs:396,401`), the three burst-length fields of `BusConfiguration`
   (`src/network_interface_controller.rs:169-180`) and test names. Four are an offset that does not
   end `_REGISTER`: `DIRECT_MEMORY_ACCESS_REGISTER_PAGE` (`src/registers.rs:467`, used by the kernel
   and `components`) and `GMAC0_SYSTEM_CONTROLLER_OFFSET` (`src/jh7110.rs:107`, used by
   `machine_discovery`) among them. One is a missing alias (item 10). The 26 acronym findings are
   `RFD`, `RFA`, `IOMMU`, `MII`, `AXI`, `MTL` and their kin unexpanded at first use in doc comments,
   and `YT8531` and `PHY` in the README's first paragraph. The two `spelled_out` findings are item 11.
   Change: in the template, scope the address rule to memory addresses or accept `station_address`
   and `physical_layer_device_address` as the specification's names (both IEEE 802.3 terms), and
   accept `YT8531` as a part number; in the crate, rename the private lengths and offsets
   (`PROBE_BYTES`, a `_BYTES` on the burst lengths or a doc saying they are beats), expand the
   acronyms once per file, split the README's long sentence and recast two passives, and copy the
   scaffold's `_typos.toml` and `.github/accepted-words` with `designware`, `motorcomm`, `gmac`,
   `jh`, `yt` and `picoseconds` and their reasons. The five public names among the findings are a
   decision (below).
   Door: two-way for the private names and the prose; one-way at the tag for the five public
   names. Effort: M, split between a template lane and a crate lane.

2. The manifest points outside the repository and names no repository.
   Finding: `publish = false` carries a comment that sends the reader to
   `crates/publish-allowlist.txt`, a file of this tree (`Cargo.toml:11`); the comment block below it
   names `net_stack`'s transport file by path (`Cargo.toml:14-22`); and the manifest has no
   `repository` or `documentation` field, which `cargo package` warns about and which the scaffold's
   badge row and docs.rs link need.
   Change: the comment says what a stranger can act on ("set to `true` only in the release that
   publishes"); the `net_stack` sentence names the two halves by role, not by path; add
   `repository`. Publishing itself is ruled elsewhere and not reopened here.
   Door: two-way. Effort: XS.

### P1: before the first tag

3. The receive loop and the reclaim loop have no bound, and the recorded mutation runs show it.
   Finding: `DataPlane::receive` (`src/data_plane.rs:137-172`) loops until a descriptor reads
   `NotYet` or a frame is delivered; a run of `Drop` descriptors is consumed one per iteration with
   no cap. `reclaim` (`src/data_plane.rs:216-226`) loops until the cursors meet. Twelve `gap` rows
   in the triage, and the 14 timeouts of the latest census, are mutants of these two loops and of
   `receive_descriptor_words` that never end, so cargo-mutants reads a hang where a test would have
   failed; the triage defers the fix as a change to production logic. On hardware, a storm of error
   frames keeps `receive` from returning to its caller for as long as the device refills faster than
   the loop reposts; Linux's NAPI budget and every polled driver in the table bound one poll.
   Change: bound `receive` at `RECEIVE_ENTRIES` iterations per call and return `None` when the
   budget is spent (the caller polls again); `reclaim` is bounded by the ring once its arithmetic is
   correct, so give it a `TRANSMIT_ENTRIES` cap too. Add one test with sixteen error frames in a
   row. The twelve `gap` rows then become kills, and the census's timeouts drop.
   Door: two-way (a caller that saw a frame after sixteen drops now sees `None` and polls again).
   Effort: S.

4. `Link` and `Speed` live in the PHY's module but belong to the MAC.
   Finding: `motorcomm_physical_layer_device::Link` and `Speed`
   (`src/motorcomm_physical_layer_device.rs:152-187`) are what `network_interface_controller::
   mac_configuration` and `start` take (`src/network_interface_controller.rs:12`). They describe the
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
   twelve unit-typed constants beside the API. The three in `descriptor.rs` are in a private module
   and are not reachable, which shows the shape wanted.
   Change: make each `pub(crate)` with `#[allow(dead_code)]`, as `descriptor.rs` does; the
   exclusion still names them. Or name them with a leading underscore and drop the attribute.
   Door: one-way at the tag if they stay public (removing a public item later is a breaking
   change). Effort: XS. The architect confirms (below).

6. The visibility of register and clause 22 constants follows no rule.
   Finding: `MAC_CONFIGURATION_REGISTER` is `pub(crate)` (`src/registers.rs:22`) while seven of its
   bits are `pub` (lines 36 to 79); `BASIC_MODE_CONTROL_RESET` and
   `BASIC_MODE_STATUS_AUTONEGOTIATION_COMPLETE` are `pub` (`src/management_data_input_output.rs:88,180`)
   while the registers they live in and `BASIC_MODE_STATUS_LINK_STATUS` are `pub(crate)`; the
   `ManagementDataInputOutput` trait takes `register: u8` yet a caller or an implementer must
   restate `0x00` and `0x01`. No caller outside the crate uses any of the stray public bits; the
   callers reach `DIRECT_MEMORY_ACCESS_REGISTER_PAGE`, `REGISTER_WINDOW_BYTES`, the two tail
   pointer registers, `MAC_ADDRESS_0_HIGH_REGISTER`, `MAC_ADDRESS_0_LOW_REGISTER` and
   `CHANNEL_STATUS_REGISTER`.
   Change: one rule, written at the top of `registers` and `management_data_input_output`. The
   compatible choice is the narrow one: `pub(crate)` for every constant no caller uses, since
   widening later breaks nobody and narrowing later breaks everybody. The whole-map alternative
   (every BSD `*reg.h` and Linux's `dwmac4.h` publish the complete map, and a kernel half outside
   the crate is this crate's design) is also defensible; it is the architect's if wanted, and this
   block recommends against it until a caller asks.
   Door: two-way now; one-way at the tag. Effort: S.

7. The README does not yet carry a stranger through a build, a proof and a falsification.
   Finding: the scaffold's README shape has `## Building and testing`, `### Running the proofs`
   and `### Running one falsification patch`; the README has the install line and `patch -p3` in
   prose (`README.md:73-78`) and no build, test, bare-metal, single-harness or undo command. The
   example (`README.md:15-48`) is not a documentation test, so `cargo test` would not notice it
   breaking; it names its bindings `off`, `v`, `b` and `buf`, the clipped forms the crate's own
   names forbid. After the move the patches apply with `-p1`, not `-p3`. The first paragraph says
   "PHY (physical layer device)" and "YT8531" before either is introduced (item 1 counts them).
   Change: add the three sections with real commands from this crate; include the README as the
   crate's documentation (`#![doc = include_str!("../README.md")]`) or move the example to a
   documentation test so it is compiled; spell the bindings out; write `-p1` once the move has
   regenerated the patches.
   Door: two-way. Effort: S.

8. The crate's documentation names a bench machine sixteen times and the board never.
   Finding: `radon` appears in the first line of the crate documentation (`src/lib.rs:2`), in
   `jh7110.rs` five times, in `network_interface_controller.rs` six times and in
   `motorcomm_physical_layer_device.rs` four times, as the owner of the bus configuration, the
   fallback PHY settings, the device tree and the ports. A reader of the standalone crate has no
   radon. The board the settings describe, the StarFive VisionFive 2, is named nowhere in `lib.rs`
   or the README; "both trees" is explained only inside `jh7110.rs`.
   Change: name the board and its two device trees once in `lib.rs` and the README, and say "the
   board" or "the VisionFive 2" where the text says `radon`. The bench record stays in nife's notes.
   Door: two-way. Effort: S.

9. The falsification patches are written for this tree, and the move must regenerate them.
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
   (`src/management_data_input_output.rs:21`), a spelling no source has; its doc comment names the
   real one, `GMAC_MAC_MDIO_ADDR_CR_SHIFT`, and `register_names` flags the mismatch. The two
   `100BASE-TX` constants open their doc comments with their own names in the place the source
   spelling goes (`src/management_data_input_output.rs:129,134`) where every neighbor opens with
   `ANLPAR_…`; the aliases `ANLPAR_TX_FD` and `ANLPAR_TX` are right.
   Change: the alias reads `GMAC_MAC_MDIO_ADDR_CR_SHIFT`; the two openings read `(ANLPAR_TX_FD,
   OpenBSD mii.h)` and `(ANLPAR_TX, OpenBSD mii.h)`.
   Door: two-way. Effort: XS.

11. `100BASE-TX` meets the spelled-out gate.
   Finding: `spelled_out.py` refuses `AUTONEGOTIATION_LINK_PARTNER_100BASE_TX` and
   `…_100BASE_TX_FULL_DUPLEX` (`src/management_data_input_output.rs:133,139`): `tx` is on the
   spelled-out list and the helper never keeps a source spelling for an integer constant. The
   review package accepted the names as the standard's term, and `design/naming.md` says a
   specification's term of art keeps its spelling inside a name this tree authors. `100BASE-TX` is
   IEEE 802.3's name for the physical layer; its `X` is the 4B5B encoding, so "transmit" would be
   a wrong word, and the names are `pub(crate)`.
   Change: in the template, a term-of-art allowance for `100BASE_TX` (and `100BASE_T4`'s kin)
   that `spelled_out.py` reads, with the reason; no rename. If the template keeps the rule as it
   is, the fallback is `…_100BASE_T_X…`, which nobody writes, or a renaming that drops the
   standard's name, which this block recommends against.
   Door: two-way (private names). Effort: XS, template lane.

### P2: later

12. Literal field values in the controller sequences.
   Finding: `receive_queue_mode` writes `1 << 7`, `0x3f << 14`, `0x3f << 8`, `0x3 << 14` and
   `0x1 << 8` for the flow-control enable and its two thresholds (`src/network_interface_controller.rs:203-208`),
   with the source's names only in a comment; `stop` polls the flush bit `10_000` times at 1 µs
   (`src/network_interface_controller.rs:610`) where the reset and the MDIO waits have named limits;
   a test checks the jumbo bit as `1 << 16` (`src/network_interface_controller.rs:843`). The
   module's own rule is that every bit it writes is a named constant with the source's spelling in
   an alias.
   Change: five constants in `registers` (OpenBSD's `GMAC_MTL_CHAN_RX_OP_MODE_EHFC`, `_RFD_*` and
   `_RFA_*` name them, not fetched for this review), a `FLUSH_LIMIT_MICROSECONDS`, and a named jumbo
   bit for the test.
   Door: two-way. Effort: S.

13. A caller's short buffer loses the frame.
   Finding: `receive` judges each descriptor against `out.len()` (`src/data_plane.rs:140`), so a
   frame longer than the buffer offered is `Drop`: consumed, counted and gone. The documentation of
   `receive` says a `Drop` is "a bad frame". `components` always offers `MAXIMUM_FRAME_BYTES`, so no
   caller today meets it, and the proof that a delivered frame fits the room is what makes the
   contract safe.
   Change: say so at `receive`, in one sentence; or return the length and leave the descriptor for a
   larger buffer, as `smoltcp`'s `RxToken` hands the frame to a closure sized by the device. The
   sentence is enough before the tag.
   Door: two-way. Effort: XS for the sentence.

14. The PHY's extended-address port is restored only on success.
   Finding: `configure` and `apply_speed` save the address port first and write it back last
   (`src/motorcomm_physical_layer_device.rs:508,569,609,621`); any `?` between leaves the port on
   the last extended register. The next access rewrites it, so the cost is one firmware access that
   lands on the wrong register, which the module says it is restoring the port to avoid.
   Change: restore in both arms, or say the restore is best effort.
   Door: two-way. Effort: XS.

15. An unknown drive current gets the part's default without a word.
   Finding: `drive_code` returns 3 for a `(voltage, microamps)` pair the table does not hold
   (`src/motorcomm_physical_layer_device.rs:392-418`), as OpenBSD does by the crate's account.
   Linux's `motorcomm.c` refuses the probe for such a pair (from memory, not verified for this
   review). A tree with a typo in `motorcomm,rx-clk-drv-microamp` then runs at the default drive
   and nobody learns of it.
   Change: `Applied` carries which codes were defaults, or `SettingsSource` gains a variant; the
   kernel's report line can then print it.
   Door: two-way. Effort: S.

16. The fallback settings claim both trees and the test checks half of it.
   Finding: `Settings::FALLBACK_GMAC0` says both trees give these values
   (`src/motorcomm_physical_layer_device.rs:339-351`), with the transmit clock inverted at all three
   speeds; the test that compares the trees (`src/motorcomm_physical_layer_device.rs:683-692`)
   builds mainline's settings with `[false, true, true]` and compares delays and drives only. Which
   tree is right about 10 Mbit/s is not verified here.
   Change: compare the inversions too, or say in the fallback's documentation which tree it follows
   for the inversion and why.
   Door: two-way. Effort: XS.

17. One verdict name covers two disagreements, and one precondition is unsaid.
   Finding: `classify`'s last arm (`src/cache_coherence.rs:198`) returns `TransmittedPayloadStale`
   for every delivered descriptor that is not a `Frame(128)` with a matching payload, including a
   descriptor reporting an error summary or another length over a payload that matches: the
   descriptor disagrees, not the payload. The module header does not say that MAC loopback needs the
   PHY's receive clock (the databook loops the transmit path back but not the clock, which is why
   the driver keeps that clock running; from the sources the crate cites, not verified here).
   Change: a `DescriptorDisagrees` verdict, or one sentence at the arm; one sentence in the header
   on the receive clock.
   Door: two-way (a new variant on a public enum before the tag). Effort: S.

18. The transport re-derives the data plane's peek.
   Finding: `components/src/designware_ethernet_transport.rs:143-148` reads the next descriptor
   from `receive_cursor()` and judges it with `receive_verdict` to know whether a frame waits,
   repeating the first two lines of `receive`.
   Change: `DataPlane::is_frame_waiting(&self, region: &impl DirectMemoryAccessRegion) -> bool`,
   and the transport calls it.
   Door: two-way (an addition). Effort: XS.

19. `start` twice desynchronizes the cursors.
   Finding: `DataPlane::start` reposts the ring and clears the transmit ring but leaves
   `receive_next`, `transmit_next` and `transmit_clean` where they were (`src/data_plane.rs:106-124`);
   the documentation says only to call it first.
   Change: reset the three cursors in `start`, or say it runs once after `new`.
   Door: two-way. Effort: XS.

20. Small text fixes.
   Finding: "current current" (`src/motorcomm_physical_layer_device.rs:390`); `MTL` is used at
   `src/registers.rs:7` and expanded only at line 174; `Identity::direct_memory_access_address_bits`
   is a width, and the address rule reads it as an address
   (`src/network_interface_controller.rs:75`): `…_address_width_bits` says what it is.
   Door: two-way, the last one one-way at the tag. Effort: XS.

21. The crate root re-exports 27 descriptor items.
   Finding: `lib.rs` re-exports every descriptor bit and the two descriptor constructors at the
   crate root beside `DataPlane` and the three traits (`src/lib.rs:106-121`), while the register map
   has its own module. A docs.rs reader meets thirteen `RECEIVE_DESCRIPTOR_WORD_3_*` constants
   before `DataPlane`.
   Change: a public `descriptor` module and a shorter root, or leave it and say why. The
   maintainer's call before the tag; after it the paths are fixed.
   Door: two-way now, one-way at the tag. Effort: S.

## Decisions for the architect

Only one-way doors. Each is a public name or path that the first tag fixes.

### 1. Where the link type lives

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

### 2. Whether the compile-time gates are API

Background: item 5. Twelve `pub const …_DO_NOT_OVERLAP: ()` items in public modules exist so the
mutation exclusions can name them. Rust has no way to publish a name for tooling without publishing
it to users; a `pub(crate)` constant is still a name the exclusion file can cite, and the three in
`descriptor.rs` already work that way. No other crate in the organization publishes such items, as
far as this review read (not verified across every repository).

Recommendation: `pub(crate)` with `#[allow(dead_code)]`, before the tag. Keeping them public costs
nothing today and a breaking change on the day anyone wants them gone.

Example: `pub(crate) const COMMAND_WORD_FIELDS_DO_NOT_OVERLAP: () = assert!(…);` and the exclusion
line in `.github/mutants-exempts` cites it unchanged.

### 3. The five public names the register gate refuses

Background: item 1. The template's `register_names` gate refuses, among its 91 findings, five names
that are public and ratified: `TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_MASK`,
`TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_MASK` and `RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_MASK`
(a length carries its unit), `DIRECT_MEMORY_ACCESS_REGISTER_PAGE` (an offset ends `_REGISTER` or
another suffix the rule lists), and `GMAC0_SYSTEM_CONTROLLER_OFFSET` (the same). The rule that a
name ratified before a later rule is rechecked at its release review applies; this is that review.
The three masks mirror OpenBSD's `TDES2_BUF1_LEN`, `TDES3_FRAME_LEN` and `RDES3_LENGTH`, which
measure bytes. The page constant is the one page of the window a process is handed; the syscon
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

- `check / standalone`, `proofs / kani` and `proofs / falsify` are green on the first pull request
  in the crate's repository, with no baseline file and no allowance added for a finding this block
  names as the crate's to fix.
- `DataPlane::receive` returns after at most `RECEIVE_ENTRIES` consumed descriptors, a test shows
  it, and the twelve `gap` rows of the triage read `killed` in the next census.
- `Link` and `Speed` have the home the architect named, the twelve gates are private, and the five
  names of decision 3 carry the spelling the architect ruled, each with its alias.
- The README has a build, test, proof and falsification section with the crate's own commands, its
  example is compiled by `cargo test`, and `radon` appears nowhere in the crate.
- The manifest carries `repository`, and no comment in it points at a file outside the repository.
- A new stranger run is on record with no open finding against the README.

## BUGS

- Items 1 (the address rule) and 11 are the template's to fix. Until they land, the crate cannot
  reach zero findings without renames this block recommends against, and the move waits on them or
  on an allowance file the template does not yet read.
- Item 16's question, which tree inverts the 10 Mbit/s transmit clock, was not verified against
  either device tree for this review.
- Items 15 and 17 cite Linux's `motorcomm.c` and the controller databook from memory, marked so in
  the text. The crate's own account of OpenBSD was taken as given.
- The review did not run Kani, cargo-mutants or Miri, and did not build at `rust-version` 1.88.
  The template's weekly `msrv` job will; a lane that touches the proofs runs `cargo kani` before
  it pushes.
- `notes/project-metrics/mutation-rowless-survivors.csv` still names the crate's functions by their
  earlier spellings (`fifo_bytes`, `mac_conf`, `rx_verdict`); a nife housekeeping item, not the
  crate's.

## Index row

An outside review of `designware_ethernet` before its move: two items that would turn the new
repository's first pull request red, nine to land before the first tag, ten for later. The nine
cover the unbounded receive loop behind twelve recorded mutation gaps, the link type's home, twelve
public compile-time gates, an unruled visibility mix, the README's missing stranger path, a bench
machine named sixteen times, the patches the move regenerates, two rename artifacts, and
`100BASE-TX` against the spelled-out gate.
