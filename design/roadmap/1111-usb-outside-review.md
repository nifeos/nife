---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 820, 1060
decision_dependencies: 154
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1111. `usb` (to be published as `universal_serial_bus`): what an outside maintainer would fix before its first release

Raised by a read-only outside review of `crates/usb` in `nifeos/nife` at commit `cf1328a`
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

1. The manifest cannot stand alone.
   Finding: `Cargo.toml` has no `description`, `repository`, `readme`, `keywords`, `categories` or
   `rust-version`. Its description is a comment, not a field. `license.workspace = true` and
   `lints.workspace = true` inherit a workspace that will not exist after the move. The
   `publish = false` line's comment points at `crates/publish-allowlist.txt`, a file of this
   repository. `standalone.py` lists all of these and exits "does not stand alone yet".
   Evidence: `crates/usb/Cargo.toml` lines 1 to 15.
   Change: at the move, set `license = "MIT OR Apache-2.0"`. Set `rust-version` at the value the
   `msrv` job confirms (1.88 or higher, since `changes` uses let chains; unverified). Add a
   one-sentence `description` ("The host side of a USB boot keyboard: descriptors, setup packets
   and reports, parsed to refuse"). Add `keywords = ["usb", "hid", "keyboard", "no-std", "kani"]`,
   `categories = ["no-std", "hardware-support", "parser-implementations"]` and `readme =
   "README.md"`. Copy the lint table in. Keep the `publish` line, with one sentence a stranger can
   act on until the release that publishes.
   Door: the metadata is two-way; the publication is one-way and already held by the standing
   publishing hold. Effort: S.

2. The crate contradicts its own published name, and the promised alias is missing.
   Finding: the release ruling publishes the crate as `universal_serial_bus`, and the xHCI review
   package says the crate "carries `#[doc(alias = "usb")]` so a search for `usb` still finds it".
   The crate carries no such alias, and its `Name:` block argues for the opposite name ("`usb` and
   not `universal_serial_bus` by DECISIONS §154"). Both readings are right about different things.
   §154 governs `usb` as a word inside names    (`usb_keyboard_driver`), and crates.io forces the
   published crate name. The package's claim about the alias is false today.
   Evidence: `crates/usb/src/lib.rs` lines 32 to 34; the package's "Companion crate" paragraph;
   `standalone.py`'s three findings on `src/lib.rs` lines 32 and 33 (a milestone citation, a
   provenance block and a section citation).
   Change: at the move, rename the package and the directory to `universal_serial_bus`. Add
   `#[doc(alias = "usb")]` at the crate root. Delete the `Name:` block (the move commit strips
   provenance blocks). Rewrite the first sentence of the crate documentation so a crates.io reader
   learns in one line why the name is long. In nife, the dependency line becomes `usb = {
   package = "universal_serial_bus", git = ..., rev = ... }` or the driver's `use` lines change;
   the former keeps the driver's diff to one line.
   Door: the published name is one-way and already ruled; the alias and the dependency form are
   two-way. Effort: S.

3. The template's naming gates fail on 30 names, 9 of them public.
   Finding: `spelled_out.py` refuses `max` in `max_packet` (two fields), `max_packet0`,
   `control_max_packet` and its parameter `b_max_packet0`. It also refuses `config` as the
   parameter of `find_boot_keyboard`. `register_names.py` refuses `DEVICE_LEN`,
   `CONFIGURATION_LEN`, `INTERFACE_LEN`, `ENDPOINT_LEN` and `REPORT_LEN` (a length carries its
   unit). It refuses `total_length` and `SetupPacket::length` (the same, unless the item says it
   mirrors the specification). It also refuses `ERROR_ROLL_OVER` (its doc comment names the source
   spelling, so it carries `#[doc(alias = "ErrorRollOver")]`). The rest are locals and test names. The `check / standalone`
   job runs these at a ceiling of 0, so the new repository is red on its first push.
   Evidence: `crates/usb/src/descriptor.rs` lines 20 to 27, 63, 76, 111, 124, 159, 195, 234, 238,
   260; `crates/usb/src/boot_keyboard.rs` lines 34 and 38; `crates/usb/src/request.rs` lines 38
   and 124.
   Change: the public renames are Decision 1. The private ones belong to the maintainer. Rename
   locals `config` to `configuration` and `max_packet` to `maximum_packet_bytes`. Rename the test
   `the_class_requests_address_the_interface` to `the_class_requests_are_sent_to_the_interface`
   (the gate reads a leading `address` as a verb). Rename the binding `address` in the walk to
   `endpoint_number_and_direction`, the spelling the xHCI package ratified for the same byte. The
   caller changes with the public names: `usb_keyboard_driver.rs` reads `keyboard.max_packet`,
   `descriptor::DEVICE_LEN`, `descriptor::CONFIGURATION_LEN`, `REPORT_LEN` and
   `control_max_packet`.
   Door: public names are one-way (Decision 1); private names are two-way. Effort: M, counting the
   caller.

### P2: later

The P2 items are in [the appendix](1111-usb-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1111-usb-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. The maintainer rules everything else above as provisional.

### Decision 1. The public names the template's gates refuse

Background. The template's `check / standalone` job refuses, at a ceiling of zero, any name that
holds a word from `naming_words.toml`'s spelled-out list or a length without its unit. Nine public
names of this crate fail, and the new repository is red until they change. The caller changes with
them. The rule and the helper are settled. Only the replacement spellings need a ruling, because a
public name is one-way once the crate is published.

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

## Follow-on

- **Done.** Items 8, 9 and 12: a type-4 or type-5 descriptor shorter than its layout is
  `Malformed { at }` (`a_short_interface_or_endpoint_is_malformed_and_two_bytes_is_enough`), a
  usage duplicated in the earlier report releases once (`a_duplicated_usage_releases_once`), and
  `endpoint` carries only its number and direction (`the_endpoint_carries_only_its_number_and_direction`).
- **Done.** Item 7, the unit-test half: `a_boot_interface_without_an_endpoint_is_refused_by_number`
  covers the `without_endpoint` path. The shaped two-interface harness, the event-order assertion
  and the bounds paragraph stay outstanding, because the harness cost needs a measurement first.
- **Done.** Item 13 (two sentences, on the walk's start and the report's tail; the `get_descriptor`
  and README sentences remain) and the private locals of items 3 and 14 (`device`, `difference`,
  `shift_a_b`, `a_b`, `QEMU_USB_KEYBOARD`). The three Kani harnesses pass and the
  walk harness's falsification patch is regenerated for the new line.
- **Outstanding.** Items 1, 2, 4, 5 and 10 are the move lane's (manifest, name, scaffold files,
  review package, fuzz crate). Item 6 adds usage rows that the lane must read from the Usage
  Tables and re-run against the Linux cross-check, which a reviewer cannot do from memory.
- **Outstanding.** Item 11 waits on Decision 3 and the public renames of item 3 on Decision 1. Both
  are public names or signatures.

## Done when

The checks are in [the appendix](1111-usb-outside-review/done-when.md).

## BUGS

- The three falsification patches apply but were not replayed under Kani here; the walk harness
  alone takes three minutes, and the template's `proofs / falsify` job will replay them.
- The usage names and `KEY_*` matches in item 6 are from memory. The lane that builds it reads them
  from the HID Usage Tables 1.5 and `input-event-codes.h`, as the earlier rows came from them. This
  block's list is a worklist, not a source.
- Nobody has verified the claim that let chains set the minimum Rust version at 1.88. The `msrv`
  job decides the value.
- Linux's treatment of an out-of-range `bMaxPacketSize0` at low and high speed (item 11) was not
  checked.
- The cost of a shaped two-interface harness (item 7) is a guess; the existing walk harness takes
  177 seconds at 25 bytes, and the lane measures before it commits a bound.
- No mutation census ran; the record cited is the repository's own, from before this review.

## Index row

An outside review of `usb` before it moves and publishes as `universal_serial_bus`. Three items
block the move: a manifest that cannot stand alone, a crate that contradicts its ruled name and
lacks its promised alias, and thirty names the template's gates refuse. Seven land before the tag.
They are the scaffold files, a review package the crate never had, and a usage table that stops
short of every non-US keyboard. They also cover the two-interface paths no proof or test reaches, a
lenient short-descriptor rule, a doubled release, and the missing fuzz target. Four wait for later. Three one-way doors go to the architect: the public rename table, whether `changes` speaks evdev or HID, and one result type for
the parsers with a speed for the control packet size.
