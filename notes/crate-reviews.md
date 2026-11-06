# Review packages for the crates on journey 5

*Name: provisional, minted 2026-11-05 (UTC) by lane/journey-5-review-packages. The file name says what the page holds: the evidence an architect's crate review starts from.*

The architect reads every crate by hand before it leaves this tree
([journey 5](../design/journeys/05-crates-released-on-their-own.md), steps 2, 3, 5, 6, 7, 8 and 10).
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

## What is the same for all seven

- Version. Every manifest says `0.1.0`, edition 2024, `rust-version = "1.88"`, license
  `MIT OR Apache-2.0` unless a package says otherwise. The proposed first version is `0.1.0`: the move commit carries the tag `v0.1.0`, and nife depends on it the way it depends on the pilot, by `git` and `rev`
  with `version = "0.1.0"` beside it. No tag exists for any of these crates in this clone.
- Publish flag. Every manifest carries `publish = false`, and `crates/publish-allowlist.txt` lists
  no crate. The caller of the template sets `allow-unpublished` until the architect approves a
  publication. Nothing here publishes.
- Dependencies in nife today. Every consumer reaches these crates by a path dependency with a
  `version` key (the packet's dependents tables list the manifests). None is a `git` and `rev`
  dependency yet, so each move converts every consumer's line to the ruled form.
- Unsafe. No crate has an `unsafe` site. Each is pure computation behind a caller's volatile
  accesses.
- Template status. [Milestone 887 (crate repositories share one CI template)](../design/roadmap/0887-crate-repositories-share-one-ci-template.md)
  stands built: the template's workflows, the allowlist lint and the `crates-io` environment exist and a
  canary repository adopts each release. What a crate gets at its move is that template. The local
  stand-in for the template's `standalone` job is `script/crate-standalone`, whose result is in each
  package. No further check can run before a repository exists.
- README readiness. Every README has the sections a stranger looks for: what the crate does, the
  proofs with their harness names, usage, limitations and license. Only
  `file_allocation_table`'s and `non_volatile_memory_express`'s READMEs tell a newcomer to install
  Kani; the other five say `cargo kani` and no more, which is the gap the pilot's stranger run found and fixed. No crate has run a
  stranger test, and none can until it has a repository.
- References to nife in shipped text. Crate repositories stand alone and do not mention nife.
  Each package lists the mentions that remain, each a small edit for the move lane.
- Harnesses and patches. In every crate, each Kani harness has exactly one falsification patch
  (the packet pairs them), and `script/crate-standalone` ran each patch and saw its harness fail.

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

Proofs. 31 harnesses, each with a falsification patch. 300 tests check against `lamfat` (rust-fatfs, an independent FAT library)
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

Proofs. Seven harnesses: the producer ring, the event ring's wrap, the capability walk, the device
context index, the slot and endpoint contexts, the port status write and the interrupt interval. All
seven prove and all seven patches fail. The proofs cover the context and port decoders.

Dependencies. None.

Companion crate. `usb` is released with it under the published name `universal_serial_bus`, because the
name `usb` is taken on crates.io. The directory and the in-tree dependency name stay `usb` until the
move, and the crate carries `#[doc(alias = "usb")]` so a search for `usb` still finds it.

Provenance. Nothing outstanding in milestone 1019 for this crate.

Result of `script/crate-standalone`. Clean. The crate names no nife file.

Outstanding before the move. Nothing. The README states the hub limit (a device behind a hub has no
address), as milestone 820 (proven xHCI logic, released on its own) asks.

Recommendation. Ready for review.

## `non_volatile_memory_express`, step 5

Name and version. `non_volatile_memory_express` `0.1.0`, ratified with NVMe spelled out. 27 `Name:`
blocks are provisional: the new modules, the eleven constants spelled out (the register offsets, the
two Controller Status bits, the two queue-creation opcodes and the two Identify selectors), the two
assertion constants, `PhysicalRegionPagePair` and its fields, `Namespace::new`. The rest are
ratified.

API. Ten modules behind a flat public surface: `registers`, `Capabilities`, `Command` with the
opcodes, `Completion`, `SubmissionQueue` and `CompletionQueue`, `Doorbell`, `doorbell`,
`PhysicalRegionPagePair` with `physical_region_page_pair`, `Namespace` and the identify parse. The
caller supplies MMIO and DMA memory. `SubmissionQueue::push` returns `None` on a full ring.
`Completion`, `IdentifyNamespace` and `Namespace` are `#[non_exhaustive]`. The README is the crate
documentation, so its usage example runs as a doctest. The process plan lives in
`non_volatile_memory_express_contract`, not here.

Proofs. Eight harnesses (submission push, completion pop and phase, doorbell collisions, PRP pair,
transfer command, identify parse, capabilities decode, completion decode). All eight prove and all
eight patches fail.

Dependencies. None.

Provenance. Nothing outstanding.

Result of `script/crate-standalone`. One failure: `names`, the 27 provisional names above. Build,
proofs and patches pass, and no citation names this tree.

Outstanding before the move.

| item | owner |
|---|---|
| Ratify the 27 provisional names with `python3 helpers/name_release.py --draft non_volatile_memory_express`. | architect |
| `design/decisions/0086-el0-nvme-driver/prior-art.md` and milestone 569 (the disk an installer names has no model, only a size) still cite `prp_pair`, `CNS_CONTROLLER` and `CNS_NAMESPACE`. | maintainer |

Recommendation. Blocked by the unratified names; ready for review once they are ratified.

## `e1000e`, step 6

Name and version. `e1000e` `0.1.0`. The architect ratified the crate name as the family name QEMU and Linux
both use, permanent on first publication. The three `flush` names are the only provisional ones.

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
items 3 and 4 hold. The `flush` module adds three provisional names
(`flush::descriptor_rings`, `flush::RingAccess`, `flush::Outcome`) that need ratifying. The manifest
license field reads `(MIT OR Apache-2.0) AND BSD-3-Clause AND BSD-2-Clause`, naming the license of
each adapted source.

Names. The maintainer ratified three names provisionally, pending the architect's review: the register
`HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER` (`non_volatile_memory.rs`, with `HSFSTS` as a
`#[doc(alias)]`), `acquire` (`physical_layer_device.rs`) and `setup_copper_link` (`sequence.rs`).
The maintainer's ruling stands unless the architect changes it at the crate's name review.

Outstanding before the move.

| item | owner |
|---|---|
| Ratify the `flush` names (`descriptor_rings`, `RingAccess`, `Outcome`) with `python3 helpers/name_release.py --draft e1000e`. | architect |
| The README describes the crate as built for the nife kernel; reword it. | move lane |

Recommendation. Blocked by the three provisional `flush` names.

## `designware_ethernet`, step 7

Name and version. `designware_ethernet` `0.1.0`, ratified (Synopsys's name, spelled out). 173
`Name:` blocks, all ratified.

API. The DesignWare Ethernet QoS controller: `registers`, `network_interface_controller` (bring-up
behind the `Hardware` trait), `management_data_input_output`, `motorcomm_physical_layer_device` (the
YT8531 decode, `link_from_registers`), `cache_coherence` (the probe's verdict), `jh7110` (glue and what
each device tree says), `DataPlane`, descriptors and `layout`. No nife type appears in a public
signature.

Proofs. Six harnesses: the delivered frame, the ring offsets, the PHY link decode, the transmit
descriptor's length fields, the MDIO command word's address fields and the coherence verdict. All
six prove and all six patches fail. A published driver needs more than three, so the review added three; MDIO sequencing, the PHY configuration writes and controller bring-up keep unit tests
against the simulator and no proof.

Dependencies. None.

Provenance. The crate adapts OpenBSD's `dwqe` and `ytphy` (ISC, with copyright lines in `lib.rs`)
and `ukphy_status` (NetBSD Foundation BSD-2-Clause). Both notices are in the tree. Milestone 1019 item 5
holds and item 9 stays open. Item 9 (a source claim in
`motorcomm_physical_layer_device.rs` that OpenBSD's `ytphy.c` and FreeBSD's `mcommphy.c` also name the
registers) is not done as far as this review could read. The manifest license field reads
`(MIT OR Apache-2.0) AND ISC AND BSD-2-Clause`, naming the license of each adapted source.

Result of `script/crate-standalone`. Clean.

Outstanding before the move.

| item | owner |
|---|---|
| Item 9 of milestone 1019: the false claim about which sources name the YT8531 registers. | maintainer, with an architect ruling on the names |

Recommendation. Item 9 needs a home before the review.

## `designware_mobile_storage`, step 8

Name and version. `designware_mobile_storage` `0.1.0`, ratified. 135 `Name:` blocks, all ratified.

API. The whole SD and eMMC driver for the Synopsys dw-mshc, behind one `Registers` trait. The modules are `registers`, `command`, `host_controller`, `internal_direct_memory_access`, `card` (identification and
the card-specific data decode), `sd` (the sequences), `partition` (the master boot record rule for where
a write test may land), `bench` and `jh7110`. No nife type appears in a public signature.

Proofs. Eight harnesses, including the MBR parse and the capacity decoders. All eight prove and all
eight patches fail.

Dependencies. None.

Provenance. `dwmmc.c` (OpenBSD, ISC, Kettenis) provides adapted code, with its notice in `lib.rs`. On `main`
`lib.rs` also carries a second notice (Stuehler) for `sdmmc_mem.c`; #3200 removes it, after finding the
crate follows only that file's order, and changes `license` to `(MIT OR Apache-2.0) AND ISC`. Until
#3200 merges, the tree and the evidence disagree. Milestone 1019 item 11 (pin the 50 MHz clock claim to
the revision it holds at) is open.

Result of `script/crate-standalone`. Clean, on the `main` form.

Outstanding before the move.

| item | owner |
|---|---|
| Merge #3200 (item 6 and the license expression). | #3200's lane |
| Item 11: the "50 MHz" claim in `jh7110.rs` and `host_controller.rs` names the vendor tag and the branch head separately; confirm each pin. | maintainer |
| Test fixtures use the manufacturer text `NIFE1`; harmless, rename with the move. | move lane |
| `lib.rs` says "spawn words are nife's". | move lane |

Recommendation. Blocked by #3200; ready for review after it merges.

## `jh7110_clock_and_reset`, step 10

Name and version. `jh7110_clock_and_reset` `0.1.0`, ratified (the CRG spelled out, no alias). 71
`Name:` blocks; 70 ratified, 1 recorded only.

API. One `lib.rs`, 69 public items: register offsets and bit positions, `Domain`, `DevicePlan`,
`BringUpReport`, the fixed bring-up plans (entropy, ethernet, the PMIC bus), `with_parent`, the
divider and enable decoders, and the compatible strings of both device trees. It reads no device tree
and has no nife type in a signature.

Proofs. Nine harnesses. All nine prove and all nine patches fail.

Dependencies. None.

Provenance. Milestone 1019 item 1 (`standard_mode_100k` rewritten from a permissive source) stands
done. Items 7 and 11 remain open on `main`. Item 7: the doc comments quote two PMIC nodes, 9 and 7 tokens. Item 11: pin the vendor links to the revision each claim holds at. The crate cites the vendor tag
`VF2_v2.10.4` and a mainline commit by hash.

Names. The maintainer ratified `CLOCK_PARENT_MASK` for the clock parent field mask provisionally,
pending the architect's review. It says what the field holds, as `with_parent` and `Step::SelectParent`
already do, and `CLKMUX_MASK`, `JH7110_MUX_MASK` and `JH71X0_CLK_MUX_MASK` are `#[doc(alias)]`es.
The ruling refused `CLOCK_MUX_MASK`, `CLOCK_MULTIPLEXER_MASK` and `CLOCK_PARENT_SELECT_MASK`.

Outstanding before the move.

| item | owner |
|---|---|
| The PMIC reset path with the rewritten `standard_mode_100k` (counts 198 high, 297 low at 49.5 MHz) has not run on the board, so milestone 819 (JH7110 clock and reset logic, proven, then released on its own) does not call it proven. | maintainers (bench) |
| Items 7 and 11 of milestone 1019. | maintainer |
| The README describes the crate as built for the nife kernel; `lib.rs` says "the fill is nife's own". | move lane |

Recommendation. Blocked by the unrun bench check.

## Summary

| crate | step | recommendation |
|---|---|---|
| `file_allocation_table` | 2 | ready for review |
| `extensible_host_controller_interface` | 3 | ready for review |
| `non_volatile_memory_express` | 5 | blocked by 27 unratified names |
| `e1000e` | 6 | blocked by three provisional `flush` names |
| `designware_ethernet` | 7 | item 9 of milestone 1019 needs a home |
| `designware_mobile_storage` | 8 | blocked by #3200 |
| `jh7110_clock_and_reset` | 10 | blocked by the unrun bench check |
