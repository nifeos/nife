---
status: PROPOSED
raised: 2026-11-12
milestone_dependencies: 1114
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Blocks that name the removed calendar crate

Raised by milestone 1114 (the calendar becomes `jiff`), whose lane may edit only its own block.
`crates/calendar` no longer exists, and four roadmap blocks still state it as present.

Reuse: none exists; this is an edit to four blocks, and no tool writes one.

## The edits

- Milestone 51 (wall-clock time, the `date` command, and an NTP service) says the library half is
  `crates/calendar`, host-tested and Kani-proved, and its index row lists the crate among what the
  milestone built. Both name `jiff` instead and point at notes/calendar.md.
- Milestone 1017 (the contract crates leave in this order) lists `calendar` among the four
  contribute crates and in step 8 with `environment_contract`, as a crate the kernel links. Step 8
  keeps `environment_contract` and `core-tools`, and the contribute list keeps three crates.
- Milestone 857 (crates released on their own) lists `calendar` as item 5 under fork 8. The item
  goes.
- Milestone 956 (the linked interfaces carry checkable versions) counts `calendar` among the crates
  that stay Rust APIs. The list drops it.

## Done when

`git grep -n 'calendar' design/roadmap/0051-wall-clock-time.md design/roadmap/1017-the-contract-crates-leave-in-this-order.md design/roadmap/0857-crates-released-on-their-own.md design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md` names no crate.

## Index row

Four roadmap blocks still state that `crates/calendar` exists after milestone 1114 removed it; correcting them keeps the record true.
