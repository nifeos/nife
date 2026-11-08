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
workflows did not run either, because no repository exists for any of these crates. The stranger
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
  proofs with their harness names, usage, limitations and license. Every other README that names harnesses tells a newcomer how to install Kani. No crate
  has run a stranger test, and none can until it has a repository.
- References to nife in shipped text. Crate repositories stand alone and do not mention nife.
  Each package lists the mentions that remain, each a small edit for the move lane.
- Harnesses and patches. In every crate, each Kani harness has exactly one falsification patch
  (the packet pairs them), and `script/crate-standalone` ran each patch and saw its harness fail.
  `counter_frequency_contract` has no harness.
- Units in names. The rule of 2026-11-03 spells every unit out (`_HERTZ`, not `_HZ`). A release
  review re-opens a name ratified before a later rule.

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
seven prove and all seven patches fail.

Dependencies. None.

Naming rules. The names section is [crate-reviews/extensible-host-controller-interface.md](crate-reviews/extensible-host-controller-interface.md).

| rule | result |
|---|---|
| the template's checks (446 names) | 0 findings |
| ratified names a later rule reaches | the specification's register mnemonics (`CAPLENGTH`, `USBCMD`, `CRCR`, `ERDP` and 15 more), each with a recommended name |
| parameters of public functions | spelled out in 12 functions, provisional |

Companion crate. `usb` is released with it under the published name `universal_serial_bus`, because the
name `usb` is taken on crates.io. The directory and the in-tree dependency name stay `usb` until the
move, and the crate carries `#[doc(alias = "usb")]` so a search for `usb` still finds it.

Provenance. Nothing outstanding in milestone 1019 for this crate.

Result of `script/crate-standalone`. Clean. The crate names no nife file.

Outstanding before the move. Nothing. The README states the hub limit (a device behind a hub has no
address), as milestone 820 (proven xHCI logic, released on its own) asks.

Recommendation. Ready for review.

## `generational_table`, step 4

The whole package is [crate-reviews/generational-table.md](crate-reviews/generational-table.md).

Name and version. `generational_table` `0.1.0`, ratified, with all 14 item names (#3119), `EntryName<T>` and its
`to_bits`, `from_bits` and `NEVER_MINTED`. The
architect ruled that it publishes, as milestone 1033 (`generational_table`: the fixed-capacity,
proved slotmap, released on its own) records.

API. One type, `Table<T, N>`, whose names are `EntryName<T>` values over a `u64`: generation high, slot low. Removal raises
the generation, and a slot whose generation reaches `u32::MAX` retires.

Proofs. Five harnesses, among them an induction step showing that a dead name stays dead after any
number of operations. All five prove and all five patches fail.

License. `MIT OR Apache-2.0`, written in the crate's manifest, with both license files; nothing adapted.

Result of `script/crate-standalone`. Passes; a run without Kani skips the proofs.

Recommendation. Ready for review.

## `non_volatile_memory_express`, step 5

Name and version. `non_volatile_memory_express` `0.1.0`, ratified with NVMe spelled out. Every
`Name:` block reads ratified.

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

Result of `script/crate-standalone`. No failure. Build, names,
proofs and patches pass, and no citation names this tree.

Outstanding before the move.

| item | owner |
|---|---|
| `design/decisions/0086-el0-nvme-driver/prior-art.md` and milestone 569 (the disk an installer names has no model, only a size) still cite `prp_pair`, `CNS_CONTROLLER` and `CNS_NAMESPACE`. | maintainer |

Recommendation. Ready for review.

## `e1000e`, step 6

The whole package is [crate-reviews/e1000e.md](crate-reviews/e1000e.md).

Name and version. `e1000e` `0.1.0`, ratified as the family name QEMU and Linux both use, permanent on
first publication. All 166 `Name:` blocks read ratified. Four register names copied from a source
are spelled out with the source spelling as an alias; `HV_KMRN_MODE_CTRL` is pending.

API. Registers, descriptors and ring logic for the 82574L and the I219 behind four small traits,
with the I219 bring-up in `platform_controller_hub`. Every error implements `Display` and
`core::error::Error`, and the types that will grow are `#[non_exhaustive]`. No nife type appears in a
public signature.

Proofs. Three harnesses. All three prove and all three patches fail.

Dependencies. None.

Naming rules. The package has the detail. Of 1,046 names, the template's checks find one, the
public `LanInitTimeout`. It and `HV_KMRN_MODE_CTRL` wait for the architect. The package also lists
borderline bit constants that mirror a FreeBSD define with no expansion.

Provenance. Milestone 1019 items 3 and 4 hold, and the notices are in the files. The manifest license
field reads `(MIT OR Apache-2.0) AND BSD-3-Clause AND BSD-2-Clause`.

Result of `script/crate-standalone`. Clean. The crate names no nife file.

Recommendation. Ready for review, with four rulings on names and the move (the package ends with them).

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
and `ukphy_status` (NetBSD Foundation BSD-2-Clause). Both notices are in the tree. Milestone 1019
items 5 and 9 hold. The manifest license field reads `(MIT OR Apache-2.0) AND ISC AND BSD-2-Clause`,
naming the license of each adapted source.

The YT8531 register sources, checked against each file at a pinned revision. The home of these
attributions is the module documentation of `motorcomm_physical_layer_device`.

| crate item | source that names it |
|---|---|
| the register and bit constants other than the two below, and `configure`'s order | OpenBSD `sys/dev/mii/ytphy.c` at `openbsd/src@9d7a05f92003` (revision 1.6): its `YT8511_` to `YT8531_` defines and `ytphy_yt8521_init`, `ytphy_yt8521_update`, `ytphy_yt8531_ds` and `ytphy_yt8531_init` |
| `MOTORCOMM_EXTENDED_CLOCK_GATING_REGISTER` (0x0c), `MOTORCOMM_EXTENDED_SLEEP_CONTROL_1_REGISTER` (0x27) | `ytphy.c`'s `YT8511_EXT_CLK_GATE` and `YT8511_EXT_SLEEP_CTRL`; FreeBSD `sys/dev/mii/mcommphy.c` at `freebsd/freebsd-src@2ad32d3a6868`, `PHY_CLOCK_GATING_REG` and `PHY_SLEEP_CONTROL1_REG`; Linux `drivers/net/phy/motorcomm.c` at `torvalds/linux@fc697291ad0a`, `YT8521_CLOCK_GATING_REG` and `YT8521_EXTREG_SLEEP_CONTROL1_REG` |
| `CGR_RX_CLK_EN` (bit 12), `ESC1R_SLEEP_SW` (bit 15) | `motorcomm.c` only, `YT8521_CGR_RX_CLK_EN` and `YT8521_ESC1R_SLEEP_SW`, which `yt8531_config_init` clears for `motorcomm,keep-pll-enabled` and `motorcomm,auto-sleep-disabled` |
| `MOTORCOMM_YT8531_PHYSICAL_LAYER_DEVICE_ID` (`0x4f51e91b`) | `motorcomm.c`'s `PHY_ID_YT8531`, matched with `PHY_ID_MATCH_EXACT`; `ytphy.c`'s `ytphys` and `mcommphy.c`'s `mcommphys` match the OUI and model |

Names a later rule reaches. Both register identifiers stand: each says what its register is, and
FreeBSD's spelling supports "clock gating" and "sleep control 1". Their aliases do not meet
`design/naming.md` (Registers and constants, and the alias rule before a first release):

| item | alias now | recommended alias set |
|---|---|---|
| `MOTORCOMM_EXTENDED_CLOCK_GATING_REGISTER` | `clock gating register`, `EXT_CLK_GATE`, `YT8521_CLOCK_GATING_REG` | `clock gating register`, `YT8511_EXT_CLK_GATE`, `PHY_CLOCK_GATING_REG` |
| `MOTORCOMM_EXTENDED_SLEEP_CONTROL_1_REGISTER` | `extended register, sleep control 1`, `EXT_SLEEP_CTRL`, `YT8521_EXTREG_SLEEP_CONTROL1_REG` | `extended register, sleep control 1`, `YT8511_EXT_SLEEP_CTRL`, `PHY_SLEEP_CONTROL1_REG` |

`EXT_CLK_GATE` and `EXT_SLEEP_CTRL` are this tree's earlier spellings, which a crate carries none
of before its first release, and the OpenBSD spelling is the whole one. Linux's spelling is cited
only where no BSD source names the thing, and both BSD sources name these two. The doc comments'
openings follow the same choice.

Result of `script/crate-standalone`. Clean.

Names. The naming rules table, the renames and the ratified names a later rule reaches are in
[crate-reviews/designware-ethernet.md](crate-reviews/designware-ethernet.md). The template's three
checks find nothing. Six groups of ratified names wait for the architect, and `CRC_BYTES` is
borderline.

Outstanding before the move.

| item | owner |
|---|---|
| The two alias sets above. | architect |

Recommendation. Ready for review once the architect rules on the two alias sets.

## `designware_mobile_storage`, step 8

The whole package is
[crate-reviews/designware-mobile-storage.md](crate-reviews/designware-mobile-storage.md).

Name and version. `designware_mobile_storage` `0.1.0`, ratified. 111 `Name:` blocks, all ratified.

API. The whole SD and eMMC driver for the Synopsys dw-mshc behind one trait,
`RegisterWindowAndClock`, in eight public modules and about 100 items. The error enums and `CardKind` are
`#[non_exhaustive]` (ruled), and the errors implement `Display` and
`core::error::Error`.

Proofs. Eight harnesses. All eight prove and all eight patches fail.

License. `(MIT OR Apache-2.0) AND ISC`, for the code adapted from OpenBSD's `dwmmc.c`, whose notice
`src/lib.rs` carries.

Result of `script/crate-standalone`. Clean. The template's unit, spelled-out and question-shape
checks find nothing. No shipped text names this tree.

Outstanding before the move.

| item | owner |
|---|---|
| `repository` in the manifest. | move lane |

Recommendation. Ready for review.

## `jh7110_clock_and_reset`, step 10

The whole package is [crate-reviews/jh7110-clock-and-reset.md](crate-reviews/jh7110-clock-and-reset.md).

Name and version. `jh7110_clock_and_reset` `0.1.0`, ratified. 80 names, all with ratified blocks;
`CLOCK_PARENT_MASK` by the maintainer, pending the architect.

API. One `lib.rs`, 69 public items: offsets and bits of three domains, the bring-up plans, the I2C
and SD clock arithmetic, and both device trees' strings. No other project's type appears in a
signature. The maintainer marked five types `#[non_exhaustive]` provisionally.

Proofs. Nine harnesses. All nine prove and all nine patches fail.

License. `MIT OR Apache-2.0`; nothing adapted. No quotation of a GPL source remains.

Result of `script/crate-standalone`. Clean, before and after this lane's edits.

Provenance. Milestone 1019 items 1, 7 and 11 stand done for this crate: no comment reproduces node text and
every vendor claim links the revision it holds at, checked against the fetched files.

Names. Nine groups of ratified names fail a later rule and wait for the architect (`SDIO_BRING_UP`, the
three `mux` report fields, `SYSTEM_BASE` and the `base` fields, `controller`, the `w` parameters,
`sdcard_divider_for`, and the parameters `power_management_chip_reg`, `bus_reg` and `reg_names`).
The package lists each with its recommended name.

Naming rules. The spelled-out check finds `SDIO_BRING_UP`, which is public and listed. The unit,
question-shape and British-spelling checks find nothing. The package holds the table, the renames
and the borderline names.

Outstanding before the move.

| item | owner |
|---|---|
| Four decisions, listed in the package. | architect |
| The PMIC reset path (198 high and 297 low counts at 49.5 MHz) has not run on a board; the README says so. | maintainers (bench) |
| Mark milestone 1019 items 7 and 11 done for this crate. | maintainer |
| `repository` in the manifest, the tag, the `Name:` blocks, the stranger test. | move lane |

Recommendation. Ready for review.

## `block_roster`, the first contract crate

The whole package is [crate-reviews/block-roster.md](crate-reviews/block-roster.md).

Name and version. `block_roster` `0.1.0`, ratified, with all 17 public names (#3282).

API. The read-only page that lists attached block devices: `write`, `Roster::read` and its readers,
`Device`, and the transport numbers. Milestone 1017 keeps it unpublished, pinned by `rev`.

Proofs. Two harnesses: `read` is total on any bytes, and `write` then `read` round-trips. Both
prove and both patches fail.

License. `MIT OR Apache-2.0`; nothing adapted.

Result of `script/crate-standalone`. Clean on `main` without Kani; the proofs ran before the renames
and passed.

Defect. `write` panics on a page shorter than its 16-byte header when the device list is empty.
The package proposes the fix and a harness.

Recommendation. Blocked by the identity value `INTERFACE_ID = 5` and the `write` defect.

## `counter_frequency_contract`, a contract crate

The whole package is
[crate-reviews/counter-frequency-contract.md](crate-reviews/counter-frequency-contract.md).

Name and version. `counter_frequency_contract` `0.1.0`, ratified, with all 13 names (#3059); the units are
spelled out (#3283).

API. The 16-byte page that carries a counter's rate on the architectures with no register for it,
its fixed address, and the band of believable rates. The reader is `unsafe`, since it names a raw
address.

Proofs. None; five host tests.

License. `MIT OR Apache-2.0`; nothing adapted.

Result of `script/crate-standalone`. One failure: `proofs`, because the crate has no harness.
The build passes on 1.88 for the host and the three bare-metal targets.

Recommendation. Blocked by the identity value `INTERFACE_ID = 9` and three text edits naming nife.

## Summary

| crate | step | recommendation |
|---|---|---|
| `file_allocation_table` | 2 | ready for review |
| `extensible_host_controller_interface` | 3 | ready for review |
| `generational_table` | 4 | ready for review |
| `non_volatile_memory_express` | 5 | ready for review |
| `e1000e` | 6 | ready for review |
| `designware_ethernet` | 7 | two alias sets for the architect |
| `designware_mobile_storage` | 8 | ready for review |
| `jh7110_clock_and_reset` | 10 | blocked by the unrun bench check |
| `block_roster` | contract | blocked by its identity value and one defect |
| `counter_frequency_contract` | contract | blocked by its identity value and three text edits |
