---
status: BUILT
raised: 2026-09-14
built: 2026-10-18
---
# 405. The nine `INIT` roles, and the archive entry the kernel picks

Branch `milestone/405-nine-init-roles-and-the-entry-the-kernel-picks`.

The last nine roles of the one fixture that dispatched on `x0` are nine programs, and the fixture
is deleted rather than renamed. calef ruled the names on 2026-10-18 (UTC), with the rule that
nife's own names never use `init` for a process.

## What there is

Six builders, each a userspace parent that reads one child out of the archive by name, builds it
from an ELF it parsed and reports. Three of the children exist only to be built.

| program | builds | proves |
|---|---|---|
| `report_child_builder` | `report_child` | a userspace build produces a running thread |
| `device_child_builder` | `device_child` | device memory can be delegated and mapped device-typed |
| `interrupt_child_builder` | `interrupt_child` | an interrupt reaches a driver through a delegated capability |
| `console_server_builder` | `console` | a userspace-built console server prints for its builder |
| `least_authority_demo_builder` | `least_authority_demo` | an argument crosses `START` intact |
| `coremark_builder` | `coremark` | a real workload runs correctly, and reads the kernel's counter frequency |

`kernel::user::spawn_elf_child_builder(image, entry, report)` enters the builder named `entry` with
the same five capabilities every builder gets, and `x0` is 0, since none of them dispatches on it.
No builder looks itself up in the archive any more, so there is no role number to agree on and
the duplicated role constants in the tests are gone with them. The words a builder's child and the
kernel's test agree on (`REPORT_CHILD_WORD`, `INTERRUPT_CHILD_WORD`, `CONSOLE_SERVER_BUILDER_LINE`,
`LEAST_AUTHORITY_DEMO_INPUT`) are in `crates/capability_witness_contract`.

**The kernel enters each builder itself, so each is in the trust root.**
`sealed_pair::BOOT_PROGRAMS` names the progenitor and the six builders, and `xtask`'s
`check_declared_programs` fails a build in which one of them has no `[[bin]]`. A test that names an
entry the archive lacks panics in `spawn_elf_child_builder`, and one the trust root lacks halts in
`trust::require`, so neither waits on a report nobody sends.

The tests are `system_tests/src/user/elf_child_builder_tests.rs`, plus the two in
`cycle_counter_tests` and `counter_frequency_tests` that enter `coremark_builder`. The interrupt and
device tests are aarch64-only, for the reasons written on each.

## Follow-on

- **Refused.** Folding `report_child_builder`, `device_child_builder` and
  `interrupt_child_builder` into one program that reads its own capability table to decide what to
  build. They differ by one capability each, which argued for it, but calef ruled six program names
  on 2026-10-18 (UTC), and six programs that each do one thing is what that ruling asks for.

## Index row

The last nine roles of `fixtures/src/hello.rs` became nine programs and `hello` was deleted. Six
are builders: each reads one child out of the archive by name, builds it from an ELF it parsed in
userspace, and reports. Three are children that exist only to be built. The kernel's
`spawn_elf_child_builder` now takes the archive entry to enter rather than a role, which put the six
builders in the trust root beside the progenitor and removed the role numbers the tests and the
fixture had to agree on.
