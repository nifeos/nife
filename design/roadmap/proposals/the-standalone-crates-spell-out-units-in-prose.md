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

Raised by lane/maintainer-follow-ups, at the maintainer's request: milestone 1053's sweep covered the crates in this tree, and `nifeos/e1000e` and `nifeos/designware_mobile_storage` were not scanned.

Name: provisional. No name is minted.

Reuse: `helpers/vendored_name_gates.py unit --list` and `unit --unclear`, run with a clone of each repository as the working directory.

## What the scan found

Both repositories were cloned read-only at their default branch and scanned with the rules of milestone 1053: the template's `[units]` table, Rust identifiers only, comments and strings skipped.

- `nifeos/e1000e`: 29 Rust files, 0 identifiers that carry a unit symbol, 0 listed by `--unclear`.
- `nifeos/designware_mobile_storage`: 16 Rust files, 0 identifiers, 0 listed by `--unclear`.

So no name in either repository needs a rename. What both hold is unit symbols in comments and documentation, written the way the sources they cite write them (`ms`, `us`, `MHz`, `kHz`, `KiB`, `GiB`, `GB`):

- `e1000e`, 28 lines: `src/platform_controller_hub/sequence.rs` 13, `simulator.rs` 7, `non_volatile_memory.rs` 2, `registers.rs` 2, and one each in `mod.rs`, `physical_layer_device.rs`, `ultra_low_power.rs` and `tests/qemu-smoke/src/machine.rs`.
- `designware_mobile_storage`, 14 lines: `internal_direct_memory_access.rs` 5, `sd.rs` 3, `card.rs` 2, `host_controller.rs` 2 and `proofs.rs` 2.

One more is a quoted source name, not a symbol this project chose: `phy->reset_delay_us`, a FreeBSD field cited in `sequence.rs`.

## The decision this needs

`design/naming.md` says a name that holds a quantity ends with its unit, spelled out. It says nothing about prose. Spelling out `ms` in a comment would be a new rule, and an architect makes it. If prose is not covered, this proposal is moot and the record is the scan result above. If it is, the work is:

1. Spell the units out in those 42 lines of comments, in a pull request to each repository, and keep a source's own spelling inside a quotation.
2. Have the template's check count the same symbols in comments, so the two repositories and every later crate stay at zero.

## Index row

Milestone 1053's sweep did not scan the two standalone driver repositories. A scan found no unit symbol in any identifier, and 42 comment lines that use one, which are renamed only if the unit rule is ruled to cover prose.
