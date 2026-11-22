---
status: BUILT
built: 2026-11-18
raised: 2026-11-16
---
# 1119. The file_system_contract falsification patches each turn their harness red

The rule in force: each of the eleven Kani harnesses in `crates/file_system_contract` carries a
replayable falsification patch. Every patch applies to the current tree. Each turns its own harness
red on the assertion it names. The branch is `milestone/1119-filesystem-contract-falsifications-repaired`;
the number is provisional.

What drives it: milestone 1090 (`file_system_contract`: what an outside maintainer would fix before
it leaves) added six harnesses and six patches. Its pull request left the crate's patches for a
separate lane to check. One patch looked like a survivor. A change to `Rights::substitute_parent_rights` alone passes
a harness that checks only the child, since the `&` in `attenuate` masks the answer.

Reuse: `script/falsifications --sweep` already applies each patch, requires red and reverts. This
milestone adds no tool.

## What is in force

- `a_resolved_request_never_widens` carries two assertions. The first, `child.bits() & !parent.bits()
  == 0`, cannot see `substitute_parent_rights` widen. The second, `parent.substitute_parent_rights(dir::PARENT_RIGHTS) ==
  parent.bits()`, can, and the patch's `Expected to fail` line names it. The patch stays as written.
  A patch on `attenuate` would repeat `attenuate_never_widens` and leave `resolve` without evidence.
- `script/falsifications --sweep file_system_contract` reports 11 swept, 0 survivors, 0 stale or
  broken. A hand replay with `cargo kani --harness <name>` shows that each patch fails the check its
  `Expected to fail` line names.
- Four patches fail inside an `assert_eq!` with a message: the two `attenuate` patches and the two
  reply patches. Kani reports those as its placeholder message from `core::panicking::assert_failed_inner`,
  so the harness line is the only name for the failed check.

## Follow-on

- **None.** Every patch of the crate is red on the assertion it names, so no repair remains.

## Index row

Every falsification patch of `file_system_contract` applies and turns its own harness red on the assertion it names, so none of its eleven harnesses survives.
