---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The device-tree patcher bounds a node name's padding

Raised 2026-11-03 (UTC) by the security audit
[2026-11-03-from-the-package-to-the-grant.md](../../audit-reports/2026-11-03-from-the-package-to-the-grant.md),
finding 4. Severity: low. Every name below is provisional.

Reuse: the bound `prop` already applies in `crates/device_tree_patch/src/lib.rs`, and the fuzz
target `device_tree_patch_initial_ramdisk`.

## The failure

`begin_node` finds a node name's terminating zero inside the structure block and rounds the end of
the name up to four bytes. It does not check the rounded end against the structure block's end or
the buffer before it copies through it. A device tree laid out so that the rounding passes the end
of the buffer panics the loader with an out-of-range slice instead of returning `Error::Truncated`.
`prop` has the check; `begin_node` does not.

The bytes come from firmware, which already runs below the loader and holds its trust, so the outcome is
a boot that stops and not authority gained. It is still a parser that panics on input its
authors meant it to refuse.

## The work

`begin_node` refuses a rounded end past the structure block as `Error::Truncated`, a host test holds
the case, and the case joins the fuzz target's seeds.
