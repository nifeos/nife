---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The standalone crates spell out unit symbols in their comments, if the rule covers prose

Raised by lane/maintainer-follow-ups, at the maintainer's request: milestone 1053 (a sweep spells out the unit symbols in names) covered the crates in this tree, and nobody scanned `nifeos/e1000e` or `nifeos/designware_mobile_storage`.

Name: provisional. The lane mints no name.

Reuse: `helpers/vendored_name_gates.py unit --list` and `unit --unclear`, run with a clone of each repository as the working directory.

## What the scan found

The lane cloned both repositories read-only at their default branch and scanned them with that milestone's rules: the template's `[units]` table, Rust identifiers only, and it skips comments and strings.

- `nifeos/e1000e`: 29 Rust files, 0 identifiers that carry a unit symbol, 0 listed by `--unclear`.
- `nifeos/designware_mobile_storage`: 16 Rust files, 0 identifiers, 0 listed by `--unclear`.

So no name in either repository needs a rename. What both hold is unit symbols in comments and documentation, the way the sources they cite spell them (`ms`, `us`, `MHz`, `kHz`, `KiB`, `GiB`, `GB`):

- `e1000e`, 28 lines: `src/platform_controller_hub/sequence.rs` 13, `simulator.rs` 7, `non_volatile_memory.rs` 2, `registers.rs` 2, and one each in `mod.rs`, `physical_layer_device.rs`, `ultra_low_power.rs` and `tests/qemu-smoke/src/machine.rs`.
- `designware_mobile_storage`, 14 lines: `internal_direct_memory_access.rs` 5, `sd.rs` 3, `card.rs` 2, `host_controller.rs` 2 and `proofs.rs` 2.

One more is a source name the project quotes, not a symbol it chose: `phy->reset_delay_us`, a FreeBSD field that `sequence.rs` cites.

## The decision this needs

`design/naming.md` says a name that holds a quantity ends with its unit, spelled out. It says nothing about prose. Spelling out `ms` in a comment would be a new rule, and an architect makes it. If the rule leaves prose alone, this proposal is moot and the scan result above is the record. If the rule reaches prose, the work is:

1. Spell the units out in those 42 lines of comments, with one pull request to each repository, and keep a source's own spelling inside a quotation.
2. Have the template's check count the same symbols in comments, so the two repositories and every later crate stay at zero.

## Index row

That milestone's sweep did not scan the two standalone driver repositories. A scan found no unit symbol in any identifier, and 42 comment lines that use one, which change only if an architect rules that the unit rule covers prose.
