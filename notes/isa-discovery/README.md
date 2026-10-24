# ISA discovery: the appendices to notes/isa-discovery.md

[`notes/isa-discovery.md`](../isa-discovery.md) carries what the boot reads, the tiers, which call
sites vary, what the machine corrected, the board shapes the parser survives, the CPU-list and PSCI
sequel, and the `BUGS`. This directory carries the detail its prose budget has no room for.

- [`psci-conduit.md`](psci-conduit.md): why milestone 100 (read the machine's PSCI)'s two hardcoded
  facts failed in different shapes, and the QEMU option that made the `smc` conduit parseable but
  not callable.
- [`lookup-tables.md`](lookup-tables.md): why the crate's four lookup tables are arrays rather than
  `match`es, why SBI ids are derived from their tags, and the one line no test covers.
- [`the-crate-name.md`](the-crate-name.md): why the crate is `machine_discovery`, the prior art
  behind the name, and the names it beat.
- [`counter-rate-in-userspace.md`](counter-rate-in-userspace.md): how `user_mode_runtime::cntfrq`
  learns RISC-V's counter rate, and the lesson.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*
