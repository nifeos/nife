# Could a rented machine replace argon for the waiting milestones?

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md). Facts about the rented machines are in [the AWS appendix](aws-graviton-metal.md).

Rented Graviton metal is the offering assumed below (documentation for it is in the appendix).
Rewording a milestone to name a rented machine is calef's call; nothing here rewrites one.

First, a cost that applies to every row. nife's aarch64 discovery reads a device tree; ACPI is
x86_64's alone (`kernel/src/smp.rs`, `kernel/src/iommu.rs`). Graviton images are UEFI and ACPI only
(secondary sources report no device tree). So nife booting there needs an aarch64 ACPI path (MADT,
GTDT, SPCR, IORT) first. The O6's shipping firmware needs the same path.

| milestone | could a rented machine carry it | from documentation, or needs a probe |
|---|---|---|
| 74, aarch64 half (`PMCCNTR_EL0`) | Probably. AWS says all hardware PMUs are exposed on metal. | Documentation says PMUs are exposed; it never names the cycle counter or any EL3 trap. A probe (boot line prints the counter state and `PMCCFILTR_EL0`) answers it. |
| 101 | Follows from 74. Its block lists riscv64 silicon as the requirement, so it is not argon's to begin with. | Same as 74. |
| 25 and the seL4 comparison | Only with a seL4 port. seL4 has no Graviton platform and no SMMUv3 driver (its `src/plat` read 2026-10-21). 25 asks for the identical silicon under seL4's published TX1 numbers; Graviton gives no published figure to compare to, so only a nife-and-seL4 run on the same metal is possible, with both ours. That is a fair like-for-like on one machine, a different claim from 25's current one. | Documentation: no platform. A probe would be a seL4 port, not a boot. |
| 225's argon leg and 201 | Plausible. Metal is a whole host, not a shared one, and has far more cores than a TX1's four, so it is a stronger multicore soak, on a different core (Neoverse V1, V2, V3 instead of A57). Virtualized instances would not count: vCPUs are scheduled by a hypervisor. Whether Graviton5's Nitro Isolation Engine sits under metal is not found. | Metal's non-sharing: documentation. Whether a soak crosses cores and reproduces needs a run. |

One evening's bench run, on-demand and per second (secondary prices): an 8 hour soak on
`c7g.metal` is about $19, and about $31 on `c8g.metal-24xl`. Each boot adds up to about 20 minutes
of billing (about $0.8 and $1.3), and a bring-up evening of six boots adds about $5 to $8. Stopped
instances do not bill, but EBS does. Spot, quota and account-friction costs are not counted.
