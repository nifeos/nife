# What it would cost to settle milestone 225's x86_64 and aarch64 legs

*Name: provisional. After the parent's "What it would cost to settle milestone 225".*

An appendix to [`notes/rented-metal.md`](../rented-metal.md). It holds the price of one bench
evening per leg, the paragraph it replaced, and why a rented machine substitutes for xenon and argon
in a soak and in nothing else. Every price was read on 2026-09-23.

Milestone 225 (run the soak on radon, argon and xenon)'s gate is `HARDWARE` in milestone 53 (the
board's own peripherals)'s sense: the boards are on the desk and it needs hands on them. That is a
person gate. Renting a machine with a power API and a console is what removes the person. The
milestone prescribes no duration. So this prices an eight-hour soak plus two hours of setup and
reading the first heartbeat: ten hours, which is a bench evening.

| Leg | Machine | Rate | 10 hours | Note |
|---|---|---|---|---|
| x86_64 | Scaleway EM-A116X-SSD, 4 cores | €0.077/hr | **€0.77** | qualifies today; four cores is enough for a crossing soak |
| x86_64, second provider | OVHcloud Rise RISE-1 | $64/mo, no hourly | **$64** | monthly only, so one month is the minimum unit |
| aarch64 | AWS `c7g.large`, 2 vCPU | $0.073/hr | **$0.73** | blocked on nife's ACPI gap, not on the offer or the price |
| aarch64, 16 cores | AWS `c7g.4xlarge`, 16 vCPU | $0.58/hr | **$5.80** | same blocker; priced because a soak wants more than two cores |
| aarch64, boot experiment | Oracle Always Free Ampere A1, 2 OCPU | free | **€0** | not a soak: the observation that would confirm the ACPI gap |

So milestone 225's x86_64 leg costs under one euro of machine time and its aarch64 leg costs
under six, once the right instance is priced rather than the largest one in the family. On both
legs the money is not the constraint, and on aarch64 it never was. That is the finding. This
milestone was gated as `HARDWARE`, and what stood between here and an aarch64 answer was an ACPI
discovery path in `uefi_loader`, not a machine and not a budget. (That path exists; see the top of
[`aarch64.md`](aarch64.md).)

Not $23.20 on `c7g.metal`, and not *"a UEFI-bootable image that does not exist yet"*: the image
exists (`BOOTAA64.EFI`), bare metal is not required, and the actual obstacle is the one documented in
this repository.

One caveat matters more than the prices. A rented x86_64 machine is not xenon. §203 (capacity is
rented rather than bought) records that xenon is a Core i5-7500T. The tree holds per-machine
measurements keyed to it, including the fastpath footprint against its 32 KB L1i. So a rented
machine of another microarchitecture is not like-for-like for those records. The soak does not need
like-for-like, so this substitution is sound for milestone 225 and unsound for anything comparing
against xenon's numbers. Likewise the aarch64 leg. argon is a Jetson TX1 chosen by milestone 127 (the seL4 machine) precisely so identical
silicon referees the seL4 comparison, and no rented Graviton substitutes for that. A rented
aarch64 soak answers a question about aarch64 multicore; it does not answer argon's.
