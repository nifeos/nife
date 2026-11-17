# Done when

An appendix to [the milestone](../1111-usb-outside-review.md).

- `standalone.py --manifest-dir .` exits clean on the new repository with no `--allow-unpublished`,
  and `cargo package --list` ships `src/`, the manifest, the lockfile, the README, the changelog and
  both license files.
- The crate root carries `#[doc(alias = "usb")]`, the package is `universal_serial_bus`, no
  `Name:` block remains, and nife's driver builds against the pinned revision.
- The seven template naming helpers exit clean over `src/` at a ceiling of zero, with the names of
  Decision 1 as ruled.
- A review package `notes/crate-reviews/universal-serial-bus.md` and its index row exist, and the
  package tables every public name with its state.
- The README follows the scaffold, names the three proof bounds and what lies outside them, and a
  stranger run is on record with no open finding.
- `evdev_code` answers for 0x64, 0x65, 0x67, 0x68 to 0x73 and 0x87 to 0x8b, each row has a source as
  the   existing rows do, and the cross-check fingerprint changes with the rerun on record.
- A unit test covers the `without_endpoint` refusal, a shaped harness reaches a second interface,
  and every harness has a falsification patch that `proofs / falsify` replays red.
- A short interface or endpoint descriptor is `Malformed { at }`; a duplicated usage releases once;
  `endpoint` carries only number and direction.
- Two fuzz targets build and run in the weekly job with the test configurations as seeds.
- The crate builds Decisions 2 and 3 as decided before the tag.
- `cargo test`, `cargo kani`, `cargo mutants` (none missed) and both fuzz targets are green at the
  commit the release tags.
