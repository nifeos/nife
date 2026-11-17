---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: 1062
decision_dependencies: 251
machine_requirements: none
specific_machine: none
needs_person: no
---
# The PMIC route refuses a request with reserved bits set

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-radon-device-authority`, as finding 4 of the
security audit [radon's device authority](../../audit-reports/2026-11-17-radon-device-authority.md).
Name: provisional.

Reuse: the refusal `entropy_contract` makes for its own reserved bits, ruled 2026-11-14 (UTC).

## The problem

`driver_process_contract::power_management_chip_route::operation` reads the top eight bits of
request word 0 and the route ignores the other 56 bits and all of word 1. A `RESET` with any
reserved bit set resets the board. The only sender, `components/src/reboot.rs`, sets none, so
nothing misbehaves today. A later operation that takes an argument would be misread by a route
built before it, and the clock and reset wire (§320) and the entropy wire already refuse reserved
bits before they act.

## The work

- The contract gains a check that word 0's low 56 bits and word 1 are zero, and the route answers
  a request that fails it with a refusal status before it touches the bus.
- A host test of the contract's check, and of the route's order: the refusal comes before any I2C
  transfer.

## Done when

A `RESET` with a reserved bit set is refused, and the board does not reset.

## Index row

The PMIC route refuses a request with reserved bits set.
