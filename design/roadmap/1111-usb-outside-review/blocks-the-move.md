# P0: blocks the move or the first release

An appendix to [the milestone](../1111-usb-outside-review.md).

1. The manifest cannot stand alone.
   Finding: `Cargo.toml` has no `description`, `repository`, `readme`, `keywords`, `categories` or
   `rust-version`. Its description is a comment, not a field. `license.workspace = true` and
   `lints.workspace = true` inherit a workspace that will not exist after the move. The
   `publish = false` line's comment points at `crates/publish-allowlist.txt`, a file of this
   repository. `standalone.py` lists all of these and exits "does not stand alone yet".
   Evidence: `crates/universal_serial_bus/Cargo.toml` lines 1 to 15.
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
   Evidence: `crates/universal_serial_bus/src/lib.rs` lines 32 to 34; the package's "Companion crate" paragraph;
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
   Evidence: `crates/universal_serial_bus/src/descriptor.rs` lines 20 to 27, 63, 76, 111, 124, 159, 195, 234, 238,
   260; `crates/universal_serial_bus/src/boot_keyboard.rs` lines 34 and 38; `crates/universal_serial_bus/src/request.rs` lines 38
   and 124.
   Change: the public renames are Decision 1's table, decided provisionally. Rename
   locals `config` to `configuration` and `max_packet` to `maximum_packet_bytes`. Rename the test
   `the_class_requests_address_the_interface` to `the_class_requests_are_sent_to_the_interface`
   (the gate reads a leading `address` as a verb). Rename the binding `address` in the walk to
   `endpoint_number_and_direction`, the spelling the xHCI package ratified for the same byte. The
   caller changes with the public names: `usb_keyboard_driver.rs` reads `keyboard.max_packet`,
   `descriptor::DEVICE_LEN`, `descriptor::CONFIGURATION_LEN`, `REPORT_LEN` and
   `control_max_packet`.
   Door: two-way before the first release. Effort: M, counting the
   caller.
