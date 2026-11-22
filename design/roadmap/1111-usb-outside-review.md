---
status: BUILT
built: 2026-11-22
raised: 2026-11-11
milestone_dependencies: 820, 1060
decision_dependencies: 154
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1111. `usb` (to be published as `universal_serial_bus`): what an outside maintainer would fix before its first release

Raised by a read-only outside review of `crates/universal_serial_bus` in `nifeos/nife` at commit `cf1328a`
(`main`), read as a stranger meeting the crate for the first time would read it. It changes
nothing; it lists what the next lanes change, in nife before the move and in the new repository
after it.

The review took its baseline from the settled record and reopens none of it:

- The acronym ruling of §154 (the acronym test is whether the phrase is spoken): `usb` is a word in
  a name and does not expand. `design/naming.md` lists it among the words that stay.
- The release order in milestone 857 (crates released on their own): the crate is released with
  `extensible_host_controller_interface` and published as `universal_serial_bus`, because the name
  `usb` is taken on crates.io. Verified against the crates.io API: `usb` 0.3.0 exists ("Common
  definitions of USB constants"); `universal_serial_bus` does not.
- The provenance ruling recorded in milestone 1019 (the driver crates carry facts from their sources, not their text), item 2.
  The usage table takes its usages from the HID Usage Tables 1.5. It takes its numbers from
  the `KEY_*` names of Linux's `input-event-codes.h`. It uses Linux's `hid_keyboard[]` as a test
  cross-check only.
- The one paragraph the review package of `extensible_host_controller_interface`
  (`notes/crate-reviews/extensible-host-controller-interface.md`) gives the companion crate.

The crate has no review package of its own, no row in `notes/crate-reviews.md`, and no `Name:`
block for any public item but the crate itself (`script/names --provisional` lists one `usb` entry).
Item 5 is that gap. Milestone 820 (`extensible_host_controller_interface`: proven xHCI logic,
released on its own) owns the move and the tag; this block owns what should land inside that path.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its naming helpers
(`spelled_out.py`, `register_names.py`, `standalone.py`), Kani, cargo-mutants and cargo-fuzz. No
new tool.

## Goal

A stranger who finds `universal_serial_bus` on crates.io can tell in one screen that it is the
host side of a USB keyboard and not another device-side stack. The same stranger can build, test,
prove and falsify it from the README alone. That stranger can trust that the crate bounds every byte
a device sends before it reads the byte. A UK, German or Japanese keyboard gives the same key that a
US one gives. The template's gates are green for the crate's own reasons.

## Finding status

Every finding is Done, so the review is built.


- P0-1, manifest that cannot stand alone: Done, [#3828](https://github.com/nifeos/nife/pull/3828). The manifest carries the literal license and lint
  table, `rust-version`, description, keywords, categories and readme, with `publish = false`.
- P0-2, package name `universal_serial_bus` and its alias: Done, [#3828](https://github.com/nifeos/nife/pull/3828). The package has the name, each
  public module carries `#[doc(alias = "usb")]` (rustdoc refuses it at the crate root), and the
  driver names it with a `package =` dependency.
- P0-3, 30 names the template gates refuse: Done,
  [#3614](https://github.com/nifeos/nife/pull/3614).
- P1-4, README, license files, changelog, scaffold files and stranger test: Done, [#3828](https://github.com/nifeos/nife/pull/3828), which moved
  them into the repository `nifeos/universal_serial_bus`.
- P1-5, review package and index row: Done,
  [#3614](https://github.com/nifeos/nife/pull/3614)
  ([the package](../../notes/crate-reviews/universal-serial-bus.md)).
- P1-6, usage table stops at 0x63: Done, [#3614](https://github.com/nifeos/nife/pull/3614). The
  table runs through 0x8b.
- P1-7, two-interface paths and event order unproven: Done,
  [#3614](https://github.com/nifeos/nife/pull/3614).
- P1-8, short interface or endpoint descriptor leaves the candidate open: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-9, duplicated usage releases twice: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-10, no fuzz target: Done, [#3828](https://github.com/nifeos/nife/pull/3828). The repository holds two, one for `find_boot_keyboard` and one for
  `changes`, and its weekly fuzz is on because the crate has no `unsafe` code.
- P2-11, `control_max_packet` looser than the specification: Done,
  [#3614](https://github.com/nifeos/nife/pull/3614).
- P2-12, reserved bits of `bEndpointAddress` pass through: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-13, four documentation gaps: Done, [#3614](https://github.com/nifeos/nife/pull/3614). The
  README half is P1-4's.
- P2-14, private names the report-only gate lists: Done,
  [#3614](https://github.com/nifeos/nife/pull/3614).
- Decision 1, the public names the template's gates refuse: Done.
  [#3614](https://github.com/nifeos/nife/pull/3614) built the rename table (maintainer,
  provisional, 2026-11-14 UTC), and [#3814](https://github.com/nifeos/nife/pull/3814) recorded the
  ratification (2026-11-19 UTC, option A): every public name of
  [the package](../../notes/crate-reviews/universal-serial-bus.md) as built, with the aliases
  `bMaxPacketSize0`, `wTotalLength`, `wLength`, `ErrorRollOver` and `bcdUSB`. `MODIFIER_CODES`
  stays private.
- Decision 2, `changes` emits HID usages: Done, [#3814](https://github.com/nifeos/nife/pull/3814)
  (maintainer, provisional, 2026-11-14 UTC). `changes` emits usages, modifiers as 0xe0 to 0xe7;
  `evdev_code` answers for the eight modifiers; `MODIFIER_CODES` is private; the xHCI keyboard
  driver maps each usage through `evdev_code`.
- Decision 3, one result type and a four-way `Speed`: Done (maintainer, provisional, 2026-11-14
  UTC). [#3614](https://github.com/nifeos/nife/pull/3614) built `Speed`;
  [#3814](https://github.com/nifeos/nife/pull/3814) made `DeviceDescriptor::parse` answer
  `Result<DeviceDescriptor, Refusal>`.

## Open decisions

None.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1111-usb-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1111-usb-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two, L is longer. A door is
one-way when somebody outside the repository acts on it after the release (a published name, a
public signature, a published behavior, a layout, a dependency); everything else is two-way and the
maintainer decides it.

### P0: blocks the move or the first release

The P0 items are in [the appendix](1111-usb-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1111-usb-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1111-usb-outside-review/before-the-tag.md).

## Decisions for the architect

Three decisions, each decided provisionally below. The maintainer rules everything else above as
provisional. The ruling on the published crate name `universal_serial_bus` stands, and nothing here
reopens it.

### Decision 1. The public names the template's gates refuse

Background. The template's `check / standalone` job refuses, at a ceiling of zero, any name that
holds a word from `naming_words.toml`'s spelled-out list or a length without its unit. Nine public
names of this crate fail, and the new repository is red until they change. The caller changes with
them. The rule and the helper stand. Only the replacement spellings need a ruling, because a
public name is one-way once a release carries the crate.

Recommendation first, one row per name:

| today | recommended | why this spelling |
|---|---|---|
| `find_boot_keyboard(config: &[u8])` | `find_boot_keyboard(configuration: &[u8])` | the spelled-out list; `configuration` is also what the module calls it in prose |
| `BootKeyboard::max_packet: u16` | `maximum_packet_bytes` | `max` expands and a size carries its unit; the field is in bytes |
| `Refusal::PacketTooSmall { max_packet }` | `PacketTooSmall { maximum_packet_bytes }` | the same field, the same spelling |
| `DeviceDescriptor::max_packet0: u8` | `control_packet_size_code` | it is a size at some speeds and an exponent at SuperSpeed, so `_bytes` would lie; `code` says "interpret me", and `control_max_packet` is the interpreter; the specification's `bMaxPacketSize0` goes in the doc opening and `#[doc(alias = "bMaxPacketSize0")]` |
| `control_max_packet(b_max_packet0, superspeed)` | `control_maximum_packet_bytes(control_packet_size_code, speed)` | the name states the unit of its answer; the speed parameter is Decision 3 |
| `DEVICE_LEN`, `CONFIGURATION_LEN`, `INTERFACE_LEN`, `ENDPOINT_LEN` | `DEVICE_DESCRIPTOR_BYTES`, `CONFIGURATION_HEADER_BYTES`, `INTERFACE_DESCRIPTOR_BYTES`, `ENDPOINT_DESCRIPTOR_BYTES` | a length carries its unit; `_HEADER_` because the configuration's own length is `wTotalLength`, which the constant is not |
| `REPORT_LEN` | `REPORT_BYTES` | the same rule |
| `ConfigurationHeader::total_length: u16` | keep, as the specification's `wTotalLength` | the gate accepts a kept spelling whose doc comment opens `(total_length, USB 2.0 Table 9-10)` with `#[doc(alias = "wTotalLength")]`; it is the specification's word for the specification's field |
| `SetupPacket::length: u16` | keep, as `wLength` | the same form; the struct's documentation already says its field names are the specification's without the prefixes |
| `ERROR_ROLL_OVER` | keep, add `#[doc(alias = "ErrorRollOver")]` | the gate asks only for the alias |
| `DeviceDescriptor::usb: u16` | `usb_release_binary_coded_decimal` | not refused by any gate, and offered because `usb` alone names nothing; `bcdUSB` as the alias |
| `DeviceDescriptor::parse(b)`, `ConfigurationHeader::parse(b)`, `BootReport::parse(b)` | `parse(bytes)` | the xHCI package renamed every single-letter public parameter; parameters are no part of compatibility, so this row is the maintainer's if the architect prefers |

Prior art: the xHCI package's parameter table, which renamed `b_interval` to `descriptor_interval`
and `dci` to `device_context_index`, and its ratified `endpoint_number_and_direction` for the same
byte this crate calls `address`. Concrete example: after the change the driver reads
`keyboard.maximum_packet_bytes` where it reads `keyboard.max_packet`, and the sentence in
`context::endpoint(...)`'s call site needs no comment to say what unit it passes.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
release, which the architect may revisit. These are names in an unreleased crate, and `usb`
itself is not the name it publishes under. The table above is the rule, every row as recommended,
including `parse(bytes)`. The specification's spellings stay as aliases (`bMaxPacketSize0`,
`wTotalLength`, `wLength`, `ErrorRollOver`, `bcdUSB`). No old spelling of this tree stays as an
alias. The premises hold at `crates/universal_serial_bus/src/descriptor.rs` lines 20 to 27, 76, 111, 159 and 195,
and `boot_keyboard.rs` line 34. Item 3 is the work. The architect ratified the names on 2026-11-19 (UTC).

### Decision 2. What `changes` emits: Linux key codes or HID usages

Background. `changes(before, after, event)` calls `event(code, pressed)` with `code` an evdev
number from `input-event-codes.h`. The crate is then a USB parser whose output vocabulary is the
Linux input ABI, and `evdev_code` and `MODIFIER_CODES` are inside the diff instead of beside it. A
consumer that is not Linux-shaped (a boot loader with its own keymap, a kernel with another
numbering, a test that wants to see the usage the device sent) has to invert the table. The two
tables also split one fact in two: a modifier is usage 0xe0 to 0xe7 in the Usage Tables, and
`MODIFIER_CODES` hides that behind bit positions. This is the one place where the crate's design,
rather than its spelling, is a one-way door. The driver builds on `changes`, and its contract cannot
change after the first release.

Recommendation: `changes` emits HID usages, modifiers as 0xe0 to 0xe7, in the same order
(modifiers, releases, presses) and under the same bound of 20; `evdev_code` stays public and grows
the eight modifier rows, so the driver's closure becomes `if let Some(code) = evdev_code(usage)`.
`MODIFIER_CODES` is then unnecessary as a public item. The cost is one line in the driver. A usage
the table does not know becomes the caller's to skip, which is where the skip belongs: today a usage
with no evdev code vanishes inside `changes` and no caller can tell. The alternative keeps evdev as
the vocabulary. The driver wants it today and the tests target it. It is the less elegant shape, and
this review would not choose it if both took the same effort.

Prior art: OpenBSD's `hidkbd_decode` emits `usage | PRESS` and `usage | RELEASE` and leaves the
translation to `wskbd`; Linux translates inside `hid-input.c` because the kernel's vocabulary is
evdev by definition, which does not hold for a crate. Concrete example: left shift and `h` in one
report, today `[(42, true), (35, true)]`, would be `[(0xe1, true), (0x0b, true)]`, and the driver's
closure maps each through `evdev_code`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
release, which the architect may revisit. The door is not one-way, as the background above
reads it. Before the first release the only caller is in nife, and the codes cross no wire: the
driver feeds them into its own keymap (`components/src/usb_keyboard_driver.rs`, `keys.event(EV_KEY,
code, ...)` near line 906). `changes` (`boot_keyboard.rs` line 305) emits HID usages, modifiers as
0xe0 to 0xe7, in the same order and under the same bound. The driver maps each usage through
`evdev_code` (line 137), and `MODIFIER_CODES` (line 46) leaves the public surface. It also follows
the rule that the usage table follows the HID Usage Tables. The block chose the evdev form only
for effort.

### Decision 3. One result type for the three parsers, and a speed for the control packet size

Background. `ConfigurationHeader::parse` answers `Result<_, Refusal>`; `DeviceDescriptor::parse`
answers `Option` although `Refusal::Truncated` and `Refusal::NotThatDescriptor` describe its two
failures exactly; `BootReport::parse` answers `Option` for a short buffer and for a phantom report
alike. The driver maps the device descriptor's `None` to its own `BadDeviceDescriptor` with the
byte count, losing which of the two it was. `control_max_packet` takes `superspeed: bool`, which
cannot express the low-speed and high-speed rules (item 11). All four are signatures, one-way once
published.

Recommendation: `DeviceDescriptor::parse` returns `Result<DeviceDescriptor, Refusal>`.
`BootReport::parse` keeps `Option`, because a phantom report is not a refusal of a malformed thing
but a report that carries no state, and its documentation says that in one line. The control packet
function takes a `Speed` enum this crate defines (`Low`, `Full`, `High`, `Super`), which is USB
vocabulary and not the xHCI port speed id; the driver already has `context::is_superspeed(speed)`
and gains a four-way match. The alternative, a `u8` speed id, ties a USB crate to one host
controller's numbering.

Prior art: Linux's `usb_parse_configuration` answers `-EINVAL` with a reason in the log for every
malformed case and never a bare failure; the `usb-host` crate's trait models speed as an enum.
Concrete example: a device descriptor whose `bDescriptorType` is 2 answers
`Err(Refusal::NotThatDescriptor)` and the driver's bring-up report says so instead of "bad device
descriptor, 18 bytes".

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
release, which the architect may revisit. `DeviceDescriptor::parse` (`descriptor.rs` line 100)
returns `Result<DeviceDescriptor, Refusal>`, and `BootReport::parse` keeps `Option`. The four-way
`Speed` replaces `superspeed: bool`. Item 11 is the work of the `Speed` half.

## Follow-on

- **Milestone 820.** A tag of `nifeos/universal_serial_bus` needs its own approval. No publish.

## Done when

The checks are in [the appendix](1111-usb-outside-review/done-when.md).

## BUGS

- Nobody has verified the claim that let chains set the minimum Rust version at 1.88. The `msrv`
  job decides the value.
- No mutation census ran; the record cited is the repository's own, from before this review.

## Index row

An outside review of `usb` before it moves and publishes as `universal_serial_bus`. Three items
block the move: a manifest that cannot stand alone, a crate that contradicts its ruled name and
lacks its promised alias, and thirty names the template's gates refuse. Seven land before the tag.
They are the scaffold files, a review package the crate never had, and a usage table that stops
short of every non-US keyboard. They also cover the two-interface paths no proof or test reaches, a
lenient short-descriptor rule, a doubled release, and the missing fuzz target. Four wait for later. The maintainer has decided three decisions, provisionally: the public rename table, that
`changes` speaks HID usages, and one result type for the parsers with a speed for the control
packet size.
