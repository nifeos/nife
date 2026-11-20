# universal_serial_bus

[![check](https://github.com/nifeos/universal_serial_bus/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/universal_serial_bus/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/universal_serial_bus/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/universal_serial_bus/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/universal_serial_bus#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.85%2B-blue.svg)](https://github.com/nifeos/universal_serial_bus/blob/main/Cargo.toml)

The host side of a Universal Serial Bus (USB) boot keyboard, as pure computation: what a device
says about itself, what a host asks it, and what a boot report means. Every byte this crate parses
came from a device a stranger plugged in, so each parser bounds its walk, checks every length before
it reads through it, and answers a refusal that names what was wrong. The crate touches no hardware,
allocates nothing and contains no `unsafe` code. It is the half of a keyboard driver that is about
USB and not about the host controller. It is the device-side counterpart of nothing: it builds no
device and drives no controller.

## What it does

- `descriptor` parses the device descriptor and the configuration header, and walks a configuration
  to the boot keyboard's interrupt endpoint, the direction `IN`, device to host (`find_boot_keyboard`). It also turns the device's
  `bMaxPacketSize0` into bytes for a given speed.
- `request` builds the four setup packets a host sends to bring a boot keyboard up: `GET_DESCRIPTOR`,
  `SET_CONFIGURATION`, `SET_PROTOCOL` and `SET_IDLE`.
- `boot_keyboard` parses the eight-byte boot report and computes the difference of two reports as
  press and release events in Human Interface Device (HID) usages. `evdev_code` maps a usage to a
  Linux evdev key code, for a keymap that already reads those.

## EXAMPLES

```rust
use universal_serial_bus::boot_keyboard::{BootReport, changes, evdev_code};
use universal_serial_bus::descriptor::find_boot_keyboard;

// A boot keyboard's configuration as QEMU's `usb-kbd` reports it, trimmed to what matters.
let configuration = [
    9, 2, 34, 0, 1, 1, 0, 0xa0, 50, // configuration 1, 34 bytes, one interface
    9, 4, 0, 0, 1, 3, 1, 1, 0, // interface 0: HID, boot, keyboard, one endpoint
    9, 0x21, 0x11, 1, 0, 1, 0x22, 63, 0, // the HID descriptor, which the walk skips
    7, 5, 0x81, 3, 8, 0, 10, // endpoint 1 `IN`, interrupt, 8 bytes, every 10 frames
];
let keyboard = find_boot_keyboard(&configuration).unwrap();
assert_eq!((keyboard.configuration, keyboard.interface, keyboard.endpoint), (1, 0, 0x81));

// Shift and `h` arrive in one report: the shift press is delivered first.
let before = BootReport::RELEASED;
let after = BootReport::parse(&[0x02, 0, 0x0b, 0, 0, 0, 0, 0]).unwrap();
let mut seen = Vec::new();
changes(&before, &after, &mut |usage, pressed| seen.push((usage, pressed)));
assert_eq!(seen, [(0xe1, true), (0x0b, true)]);
assert_eq!(evdev_code(0xe1), Some(42)); // KEY_LEFTSHIFT
```

## What is proven

Five [Kani](https://github.com/model-checking/kani) harnesses, in `src/verification.rs`. Each proof
holds for every input up to the bound it names.

- `the_header_parsers_are_total`: for every byte string of up to 20 bytes, the device descriptor
  and configuration header parsers never panic.
- `the_configuration_walk_is_total_and_accepts_only_an_interrupt_in`: for every configuration of
  up to 25 bytes, the walk never panics, and what it accepts is an interrupt `IN` endpoint numbered
  above zero.
- `two_interfaces_are_walked_as_two`: for one fixed 41-byte shape of two interfaces, cut at every
  length, the walk answers only a boot keyboard's endpoint, and a refusal names a boot interface
  the buffer holds.
- `two_reports_produce_a_bounded_number_of_events`: for every pair of eight-byte reports,
  `changes` delivers at most twenty events.
- `two_reports_produce_their_events_in_order`: for every pair of reports, modifiers come first in
  bit order, then releases, then presses, and each event names a usage held in exactly one of the
  two reports.

Two things lie outside the proofs. A configuration longer than 25 bytes in any shape but the fixed
one is what a real composite keyboard sends. The unit tests walk several, and a fuzz target in
`fuzz/` reaches the rest. The converse of the report proof, that every key which changed produces an
event, is checked by tests and not proven.

The usage table of `evdev_code` takes its usages and names from the USB Implementers Forum's
HID Usage Tables 1.5 and its numbers from the `KEY_*` names of Linux's `input-event-codes.h`, an
Application Binary Interface (ABI) header. It copies no driver text. A unit test compares the table
with Linux's `hid_keyboard[]` as a cross-check only: the table agrees on 140 of its 144 rows, and
the four differences are this crate's choices, which `evdev_code` documents.

## Building and testing

```sh
cargo build
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

Run `rustup target add` once, before the first bare-metal build. Without it, that build stops with
`can't find crate for core`. The tests run on the host in a fraction of a second.

### Running the proofs

The proofs need Kani, which `rustup` does not install. Install it once, with network access,
since the setup step downloads a model checker:

```sh
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
```

CI uses version 0.68.0, and another version may verify differently.

Then `cargo kani`, from the crate's directory, runs all five harnesses. On a desktop that takes about 7 minutes and peaks near
2.2 gigabytes of memory. To run one harness, give its full path and
`--exact`:

```sh
cargo kani --harness verification::the_header_parsers_are_total --exact
```

A passing run prints several lines that look alarming. All of them are expected:

- `warning: use of an unstable feature`, about `#![feature(register_tool)]`, which Kani sets.
- `warning: Found the following unsupported constructs`, followed by "Verification will fail if
  one or more of these constructs is reachable". A run that ends in `VERIFICATION:- SUCCESSFUL`
  never reached them.
- Hundreds of `aborting path on assume(false)` lines. They mark paths that a harness's assumptions
  rule out.
- A check described as unreachable code. It reports code that cannot run, and it is no failure.
- Source paths under `/home/runner/.rustup`, a directory that does not exist on your machine. They
  come from the toolchain Kani ships.

Trust the last line of the run: `Complete - 5 successfully verified harnesses, 0 failures, 5 total.`

### Running one falsification patch

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.the_header_parsers_are_total.patch
cargo kani --harness verification::the_header_parsers_are_total --exact   # ends VERIFICATION:- FAILED
git checkout -- src
```

The harness name is the part of the patch's file name after `verification.`. The run ends with
`Complete - 0 successfully verified harnesses, 1 failures, 1 total.`, and the failed check names
the assertion that the patch breaks. `git checkout -- src` reverts the patch.

## BUGS

- Boot protocol only: a keyboard is found by its interface triple (class 3, subclass 1,
  protocol 1), no HID report descriptor is parsed, and a device with no boot interface is refused
  as `Refusal::NoBootKeyboard`.
- The first boot keyboard interface wins; a second keyboard is not looked for.
- No key repeat: a boot keyboard reports state, not events, and typematic repeat is the host's job.
- No lock state and no light-emitting diodes (LEDs): the lock keys are reported as key presses, and the crate keeps no Caps
  Lock or Num Lock state and builds no request that sets a keyboard's LEDs.
- No enum is `#[non_exhaustive]`, so a variant added to `Refusal` or `Speed` after the first
  release breaks a caller's `match`.

The references are the USB 2.0 and 3.2 specifications for the descriptors and requests, and the HID
1.11 specification and Usage Tables 1.5 for the report.

## Minimum Rust version

1.85. To check it, install that toolchain and its bare-metal target, then test and build with it:

```sh
rustup toolchain install 1.85 --profile minimal --target x86_64-unknown-none
cargo +1.85 test
cargo +1.85 build --lib --target x86_64-unknown-none
```

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
