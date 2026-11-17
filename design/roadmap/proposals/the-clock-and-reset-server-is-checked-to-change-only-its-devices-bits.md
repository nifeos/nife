---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: 1062
decision_dependencies: 320
machine_requirements: none
specific_machine: none
needs_person: no
---
# The clock and reset server is checked to change only its device's bits, for every request

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-radon-device-authority`, as finding 2 of the
security audit [radon's device authority](../../audit-reports/2026-11-17-radon-device-authority.md).
Name: provisional.

Reuse: the `Fake` windows in `crates/jh7110_clock_and_reset_devices/src/tests.rs`, which already
log every store.

## The problem

The clock and reset server holds the first page of the SYS, AON and STG domains, and that page
holds every clock and reset word of its domain: the CPU cores, the DRAM controller, the watchdog,
the real-time clock and the power management unit among them. Nothing but the server's code keeps
a request for one device from changing another's bits.

`one_device_never_touches_another_devices_bits` checks that for `Device::bring_up`, for each of the
five devices, from one starting state of the windows. It does not reach `answer`, which is what a
request runs; `SET_RATE`, which writes a clock word with a new divider; a refused or malformed
request; or a starting state other than the fake's.

## The work

- A host test, or a Kani harness where the state space allows, over `answer`: every badge in
  `DEVICES` plus one outside it, every operation in the wire plus one past it, request words with
  reserved bits set, and windows whose words start from arbitrary values. For each request, every
  bit any store changes belongs to the badge's own device: a clock word its plan or rated clock
  names, or a reset bit its plan names. A refused request stores nothing.
- The falsification record for the check: a planted store to another device's word fails it.

## Done when

The check runs in `script/test`'s host tests and names `answer` as what it covers, and the BUGS
entry in notes/jh7110-clock-and-reset.md about the server holding three domains names the check.

## Index row

The clock and reset server is checked to change only its device's bits, for every request.
