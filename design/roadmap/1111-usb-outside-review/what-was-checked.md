# What was checked, and how

An appendix to [the milestone](../1111-usb-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `cf1328a`, on the pinned nightly
toolchain:

- `cargo test -p usb`: 26 unit tests and 2 documentation tests pass. `cargo clippy -p usb
  --all-targets -D warnings` and `cargo doc -p usb --no-deps` with broken links denied are clean.
- The crate builds for `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf`.
- Kani 0.68.0 verifies all three harnesses, and the walk harness's single cover holds. Times:
  177 s for the configuration walk at 25 bytes, 6 s for the report bound, under 1 s for the header
  parsers. Each of the three falsification patches applies cleanly to the current source (`git
  apply --check`); the patches were not replayed under Kani.
- The template's naming helpers at `crate-workflows` head, run as its `check / standalone` job runs
  them: `spelled_out.py` 20 findings, `register_names.py` 10 findings, `unit_symbols.py`,
  `british_spelling.py`, `service_model.py`, `contract_names.py` and `question_shape.py` 0 findings
  over 204 names; `abbreviations.py` (report only) 27 components; `standalone.py` 10 findings.
- Seven scratch tests of edge cases, which I wrote in the clone and deleted afterwards. They cover
  a usage duplicated in the earlier report, the refusal path through a boot interface with no
  endpoint, a header `bLength` other than 9, and a truncated interface descriptor after the
  keyboard. They also cover usages 0x64 to 0x8b, a report longer than eight bytes, and reserved
  bits in `bEndpointAddress`. Each item cites its result.
- The only caller, `components/src/usb_keyboard_driver.rs`, and how it consumes each public item.
- The mutation record: `notes/mutation-testing/survivor-classification.md` rows 7 missed, 7 noise,
  0 real holes for `usb`, all seven being `|` to `^` on disjoint bit fields in `request.rs`.
- U-Boot `common/usb_kbd.c`, OpenBSD `sys/dev/hid/hidkbd.c` and `sys/dev/usb/ukbd.c`, and Linux
  `drivers/usb/core/config.c`, fetched at their default branches, for the prior-art rows that cite
  them.
- `extensible_host_controller_interface::context::endpoint_index` at the pinned revision masks the
  endpoint number to four bits and reads the direction from bit 7, so item 12 is a hygiene item and
  not a defect downstream.

Inferred from reading, not run: that let chains (`if cond && let Some(x) = ...`, used in
`boot_keyboard::changes`) set the minimum Rust version at 1.88 (unverified; the template's `msrv`
job will say).

Not run: a mutation census (outside its window) and a replay of the falsification patches under
Kani. Also not run: `cargo semver-checks` (no earlier release), a fuzz campaign (the crate has no
fuzz target), and a release dry run. I did not read the HID Usage Tables PDF itself, so the usage
names in item 6 come from memory and say so.
