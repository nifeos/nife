# Review packages for the crates on journey 5

*Name: provisional, minted 2026-11-05 (UTC) by lane/journey-5-review-packages. The file name says what the page holds: the evidence an architect's crate review starts from.*

The architect reads every crate by hand before it leaves this tree
([journey 5](../design/journeys/05-crates-released-on-their-own.md), steps 2 to 8 and 10, and the
first contract crates of milestone 1017 (the contract crates leave in this order)).
This note holds one package per crate, so a review starts from evidence and not from a search. Each
package follows the review of `jh7110_entropy` in
[milestone 818 (proven JH7110 TRNG logic, released on its own)](../design/roadmap/0818-the-jh7110-entropy-crate-released-on-its-own.md).
It gives the exact name and version, what the API exposes, what the proofs cover and what the crate
depends on. It also gives the provenance still owed, what the move still needs, and a
recommendation. A package gathers facts. The architect judges the API's vocabulary.

## What was run

Every result below comes from `main` as it stands. Pull request #3200 (milestone 1019 (the driver
crates carry facts from their sources, not their text), items 3 to 6) is still open, and the
packages of three crates name its effects.

| check | command | scope |
|---|---|---|
| packet | `script/crate-review-packet <crate>` | size, public API, dependencies, harness and patch pairing, dependents |
| names | `script/names <crate>` and `python3 helpers/name_release.py --draft <crate>` | every `Name:` block in the crate |
| standalone | `script/crate-standalone <crate>` | manifest, files, citations of this tree, build and tests at the manifest's `rust-version` (1.88) for the host and the three bare-metal targets, every Kani harness, every falsification patch |

Not run, and so not claimed: the stranger test, coverage, mutation, `cargo-semver-checks` (a first
release has no baseline) and the publish rehearsal. The template's `check`, `proofs` and `weekly`
workflows did not run either, because no repository exists for any of these seven. The stranger
record lives in the crate repository, in `docs/stranger-test.md`. The `file_allocation_table`
package is the exception: it ran the README's commands in a scratch copy and `cargo-semver-checks`
against itself.

## What is the same for all of them

- Version. Every manifest says `0.1.0`, edition 2024, `rust-version = "1.88"`, license
  `MIT OR Apache-2.0` unless a package says otherwise. The proposed first version is `0.1.0`: the move commit carries the tag `v0.1.0`, and nife depends on it the way it depends on the pilot, by `git` and `rev`
  with `version = "0.1.0"` beside it. No tag exists for any of these crates in this clone.
- Publish flag. Every manifest carries `publish = false`, and `crates/publish-allowlist.txt` lists
  no crate. The caller of the template sets `allow-unpublished` until the architect approves a
  publication. Nothing here publishes.
- Dependencies in nife today. Every consumer reaches these crates by a path dependency with a
  `version` key (the packet's dependents tables list the manifests). None is a `git` and `rev`
  dependency yet, so each move converts every consumer's line to the ruled form.
- Unsafe. No crate has an `unsafe` site except `counter_frequency_contract`, whose reader names a
  raw address. Each other crate is pure computation behind a caller's volatile accesses.
- Template status. [Milestone 887 (crate repositories share one CI template)](../design/roadmap/0887-crate-repositories-share-one-ci-template.md)
  stands built: the template's workflows, the allowlist lint and the `crates-io` environment exist and a
  canary repository adopts each release. What a crate gets at its move is that template. The local
  stand-in for the template's `standalone` job is `script/crate-standalone`, whose result is in each
  package. No further check can run before a repository exists.
- README readiness. Every README has the sections a stranger looks for: what the crate does, the
  proofs with their harness names, usage, limitations and license. `generational_table` has no
  README yet. Only `file_allocation_table`'s README tells a newcomer to install Kani; the others
  say `cargo kani` and no more, which is the gap the pilot's stranger run found and fixed. No crate
  has run a stranger test, and none can until it has a repository.
- References to nife in shipped text. Crate repositories stand alone and do not mention nife.
  Each package lists the mentions that remain, each a small edit for the move lane.
- Harnesses and patches. In every crate, each Kani harness has exactly one falsification patch
  (the packet pairs them), and `script/crate-standalone` ran each patch and saw its harness fail.
  `counter_frequency_contract` has no harness.
- Units in names. The rule of 2026-11-03 spells every unit out (`_HERTZ`, not `_HZ`) and leaves a
  ratified name alone until its own review. Three crates below carry ratified `HZ` names, and one
  ruling on whether a release review re-opens them decides all three.

## `file_allocation_table`, step 2

The whole package, with the signature list, the stranger run and seven questions, is
[crate-reviews/file-allocation-table.md](crate-reviews/file-allocation-table.md). It covers the
crate as #3267 leaves it, with no reference to nife. The maintainer ruled six of the questions
provisionally, and #3267 applied them; the seventh, the version, tag and repository, is open.

Name and version. `file_allocation_table` `0.1.0`, ratified with FAT spelled out. Every `Name:`
block reads ratified.

API. FAT12, FAT16 and FAT32 with no I/O: `Volume` reads, `WritableVolume` writes with a `Clock`,
`FormatPlan` formats any size, and `VolumeLayout` lays out a FAT32 EFI system partition holding one
boot file under a label the caller supplies. `VolumeLayout` publishes five methods, and every error
enum is `#[non_exhaustive]`. The caller implements `SectorDevice` and `WritableSectorDevice`.

Proofs. 31 harnesses, each with a falsification patch. 300 tests check against the `lamfat` oracle
and `fsck.fat`.

Dependencies. None at run time. Dev only: `lamfat =0.4.3` and `ruzstd =0.9.1`, both MIT.

Result of `script/crate-standalone`. Clean: every step passes, Kani and the 31 falsifications
included.

Recommendation. Ready for review. What remains is the architect's clearance of the API the
provisional rulings leave, and the answer to the seventh question.

## `extensible_host_controller_interface`, step 3

Name and version. `extensible_host_controller_interface` `0.1.0`, ratified with xHCI spelled out.
146 `Name:` blocks, all ratified.

API. The xHCI half of a USB driver as pure computation: `registers` (offsets and the `Capabilities`
decode), `port`, `context`, `direct_memory_access` layout, `TransferRequestBlock`, `ProducerRing`,
`EventRing`, `Event`, `completion` and `transfer_request_block_type`. No nife capability, handoff or
error type appears in a public signature. The spawn handoff lives in `driver_process_contract`.

Proofs. Three harnesses: the producer ring, the event ring's wrap, and the capability walk. All
three prove and all three patches fail.

Dependencies. None.

Companion crate. `usb` is released with it under the published name `universal_serial_bus`, because the
name `usb` is taken on crates.io. The directory and the in-tree dependency name stay `usb` until the
move, and the crate carries `#[doc(alias = "usb")]` so a search for `usb` still finds it.

Provenance. Nothing outstanding in milestone 1019 for this crate.

Result of `script/crate-standalone`. One failure: `citations`, 3 lines naming nife's
`helpers/mutation_exclusion_check.py` and `.cargo/mutants.toml` in comments in
`transfer_request_block.rs` and `event.rs`. Build, proofs and patches pass.

Outstanding before the move.

| item | owner |
|---|---|
| Reword the three comments so they name no nife file. The mutation exclusion they describe belongs in the crate's own configuration. | move lane |
| The README states the hub limit (a device behind a hub has no address), as milestone 820 (proven xHCI logic, released on its own) asks. | done |
| Whether the context and port decoders want proofs before 0.1 (milestone 820's open item). | architect |

Recommendation. Blocked by the three nife citations, a small lane edit; ready for review after it.

## `generational_table`, step 4

The whole package is [crate-reviews/generational-table.md](crate-reviews/generational-table.md).

Name and version. `generational_table` `0.1.0`, ratified, with all 14 item names (#3119). The
architect ruled that it publishes, as milestone 1033 (`generational_table`: the fixed-capacity,
proved slotmap, released on its own) records.

API. One type, `Table<T, N>`, whose names are bare `u64`s: slot low, generation high. Removal raises
the generation, and a slot whose generation reaches `u32::MAX` retires.

Proofs. Five harnesses, among them an induction step showing that a dead name stays dead after any
number of operations. All five prove and all five patches fail.

License. `MIT OR Apache-2.0`, inherited from the workspace; nothing adapted.

Result of `script/crate-standalone`. Seven failures, all release shape: no README, no license
files, no `rust-version`, two inherited manifest keys and one citation of this tree. Build, proofs
and patches pass.

Recommendation. Blocked by the release shape and one architect question: a typed name or a bare
`u64`, open in milestone 1033.

## `non_volatile_memory_express`, step 5

Name and version. `non_volatile_memory_express` `0.1.0`, ratified with NVMe spelled out. 55 `Name:`
blocks, all ratified.

API. One `lib.rs` of 1,205 lines: a `registers` module, `Capabilities`, `Command`, `Completion`,
`SubmissionQueue`, `CompletionQueue`, `Doorbell`, `doorbell`, `physical_region_page_pair` and the identify parse with
`Namespace`. The caller supplies MMIO and DMA memory. The process plan lives in
`non_volatile_memory_express_contract`, not here.

Proofs. Six harnesses (submission push, completion pop and phase, doorbell collisions, PRP pair,
transfer command, identify parse). All six prove and all six patches fail.

Dependencies. None.

Provenance. Nothing outstanding.

Result of `script/crate-standalone`. Clean.

Outstanding before the move. The README does not say how to install Kani (see the shared section);
move lane. Nothing else.

Recommendation. Ready for review.

## `e1000e`, step 6

Name and version. `e1000e` `0.1.0`. The architect ratified the crate name as the family name QEMU and Linux
both use, permanent on first publication. 163 `Name:` blocks; 160 ratified, 3 recorded only.

API. Registers and descriptors for the 82574L and the I219 behind two small traits. The public items are `DataPlane`,
`DirectMemoryAccessRegion`, `Tails`, `TransmitError`, the `layout` and `registers` modules, the device
id table (`DEVICE_IDS`, `is_supported_device`, `model_name`), and `platform_controller_hub`, the I219
bring-up (flush, reset, ultra low power, physical layer device access, non-volatile memory). No nife
type appears in a public signature.

Proofs. Three harnesses (a paged physical layer device access, the delivered frame, the ring
offsets). All three prove and all three patches fail.

Dependencies. None.

Provenance. The `platform_controller_hub` module adapts FreeBSD's Intel shared code
(BSD-3-Clause) and `if_em.c` and `if_em.h` (BSD-2-Clause), with notices in the module. Milestone 1019
items 3 and 4 are open on `main`; pull request #3200 resolves them and also adds new provisional names
(`flush::descriptor_rings`, `flush::RingAccess`, `flush::Outcome`) that need ratifying. The manifest
license field reads `(MIT OR Apache-2.0) AND BSD-3-Clause AND BSD-2-Clause`, naming the license of
each adapted source.

Result of `script/crate-standalone`. One failure: `names`, three items recorded but not ratified:
`HSFSTS` (`non_volatile_memory.rs`), `acquire` (`physical_layer_device.rs`) and `setup_copper_link`
(`sequence.rs`). Build, proofs and patches pass.

Outstanding before the move.

| item | owner |
|---|---|
| Ratify the three recorded names with `python3 helpers/name_release.py --draft e1000e`. | architect |
| Merge #3200 (it conflicts with `main` today) and ratify its new `flush` names. | #3200's lane, then architect |
| The README describes the crate as built for the nife kernel; reword it. | move lane |

Recommendation. Blocked by three unratified names and #3200.

## `designware_ethernet`, step 7

Name and version. `designware_ethernet` `0.1.0`, ratified (Synopsys's name, spelled out). 173
`Name:` blocks, all ratified.

API. The DesignWare Ethernet QoS controller: `registers`, `network_interface_controller` (bring-up
behind the `Hardware` trait), `management_data_input_output`, `motorcomm_physical_layer_device` (the
YT8531 decode, `link_from_registers`), `cache_coherence` (the probe's verdict), `jh7110` (glue and what
each device tree says), `DataPlane`, descriptors and `layout`. No nife type appears in a public
signature.

Proofs. Three harnesses: the delivered frame, the ring offsets, the PHY link decode. All three
prove and all three patches fail.

Dependencies. None.

Provenance. The crate adapts OpenBSD's `dwqe` and `ytphy` (ISC, with copyright lines in `lib.rs`)
and `ukphy_status` (NetBSD Foundation BSD-2-Clause). Both notices are in the tree. Milestone 1019 items
5 and 9 are open on `main`; #3200 settles item 5's Reuse text. Item 9 (a source claim in
`motorcomm_physical_layer_device.rs` that OpenBSD's `ytphy.c` and FreeBSD's `mcommphy.c` also name the
registers) is not done as far as this review could read. The manifest license field reads
`(MIT OR Apache-2.0) AND ISC AND BSD-2-Clause`, naming the license of each adapted source.

Result of `script/crate-standalone`. Clean.

Outstanding before the move.

| item | owner |
|---|---|
| Merge #3200 (item 5). | #3200's lane |
| Item 9 of milestone 1019: the false claim about which sources name the YT8531 registers. | maintainer, with an architect ruling on the names |
| Milestone 816 (proven DesignWare Ethernet QoS logic, released on its own) asks whether three harnesses are enough for a published driver. | architect |

Recommendation. Blocked by #3200; item 9 needs a home before the review.

## `designware_mobile_storage`, step 8

The whole package is
[crate-reviews/designware-mobile-storage.md](crate-reviews/designware-mobile-storage.md).

Name and version. `designware_mobile_storage` `0.1.0`, ratified. 135 `Name:` blocks, all ratified.

API. The whole SD and eMMC driver for the Synopsys dw-mshc behind one trait,
`RegisterWindowAndClock`, in nine public modules and about 125 items. Two kinds of item serve this
tree's bench and build rather than a caller: the `bench` module and nine public `()` constants that
exist to run `const` assertions.

Proofs. Eight harnesses. All eight prove and all eight patches fail.

License. `(MIT OR Apache-2.0) AND ISC`, for the code adapted from OpenBSD's `dwmmc.c`, whose notice
`src/lib.rs` carries. #3200 settled the second notice.

Result of `script/crate-standalone`. Clean.

Recommendation. Ready for review after three questions: how much to publish and
`#[non_exhaustive]` (both the maintainer's to rule provisionally), and the unit spelling of eight
ratified names (the architect's).

## `jh7110_clock_and_reset`, step 10

The whole package is [crate-reviews/jh7110-clock-and-reset.md](crate-reviews/jh7110-clock-and-reset.md).

Name and version. `jh7110_clock_and_reset` `0.1.0`, ratified. 71 `Name:` blocks, all ratified;
`CLOCK_PARENT_MASK` by the maintainer, pending the architect.

API. One `lib.rs`, 69 public items: offsets and bits of three domains, the bring-up plans, the I2C
and SD clock arithmetic, and both device trees' strings. No nife type appears in a signature.

Proofs. Nine harnesses. All nine prove and all nine patches fail.

License. `MIT OR Apache-2.0`; nothing adapted. One doc comment quotes eleven words of a GPL
comment, which the five-word rule refuses.

Result of `script/crate-standalone`. Clean.

Recommendation. Blocked by the unrun bench check and by one architect question: whether a release
review re-opens about thirty names ratified before the rules that spell out register names and
units.

## `block_roster`, the first contract crate

The whole package is [crate-reviews/block-roster.md](crate-reviews/block-roster.md).

Name and version. `block_roster` `0.1.0`, ratified. 17 public names have no ratified block; two of
them, `VERSION` and `INTERFACE_ID`, are covered by the family ruling and need only recording.

API. The read-only page that lists attached block devices: `write`, `Roster::read` and its readers,
`Device`, and the transport numbers. Milestone 1017 keeps it unpublished, pinned by `rev`.

Proofs. Two harnesses: `read` is total on any bytes, and `write` then `read` round-trips. Both
prove and both patches fail.

License. `MIT OR Apache-2.0`; nothing adapted.

Result of `script/crate-standalone`. RESULT_BR

Defect. `write` panics on a page shorter than its 16-byte header when the device list is empty.
The package proposes the fix and a harness.

Recommendation. Blocked by the 17 names, the identity value `INTERFACE_ID = 5` and the `write`
defect.

## `counter_frequency_contract`, a contract crate

The whole package is
[crate-reviews/counter-frequency-contract.md](crate-reviews/counter-frequency-contract.md).

Name and version. `counter_frequency_contract` `0.1.0`, ratified, with all 13 names (#3059). Three
carry the unit symbol `HZ`.

API. The 16-byte page that carries a counter's rate on the architectures with no register for it,
its fixed address, and the band of believable rates. The reader is `unsafe`, since it names a raw
address.

Proofs. None; five host tests.

License. `MIT OR Apache-2.0`; nothing adapted.

Result of `script/crate-standalone`. RESULT_CF

Recommendation. Blocked by the unit spelling (the architect's), the identity value
`INTERFACE_ID = 9`, and three text edits naming nife.

## Summary

| crate | step | recommendation |
|---|---|---|
| `file_allocation_table` | 2 | ready for review |
| `extensible_host_controller_interface` | 3 | blocked by three nife citations (lane edit) |
| `generational_table` | 4 | blocked by the release shape and the name type |
| `non_volatile_memory_express` | 5 | ready for review |
| `e1000e` | 6 | blocked by three unratified names and #3200 |
| `designware_ethernet` | 7 | blocked by #3200 and item 9 of milestone 1019 |
| `designware_mobile_storage` | 8 | ready for review after three questions, one the architect's |
| `jh7110_clock_and_reset` | 10 | blocked by the unrun bench check and the re-review of early names |
| `block_roster` | contract | blocked by 17 unratified names, its identity value and one defect |
| `counter_frequency_contract` | contract | blocked by the unit spelling and its identity value |
