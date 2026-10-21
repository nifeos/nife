# The modern aarch64 board survey: its appendices

[`notes/aarch64-modern-board-survey.md`](../aarch64-modern-board-survey.md) carries the scorecard
and the recommendation. These files carry the evidence behind it, with a URL and a read date on
every claim.

- [`cores-and-sel4.md`](cores-and-sel4.md): per-core architecture features from Arm's TRMs, and
  seL4's support tier for each board family, read from seL4's own trees.
- [`pi5-and-rk3588.md`](pi5-and-rk3588.md): Raspberry Pi 5 and the RK3588 boards.
- [`cix-genio-nxp-qualcomm.md`](cix-genio-nxp-qualcomm.md): the CIX P1 boards, MediaTek Genio,
  NXP i.MX 95, TI and Qualcomm.
- [`jetson-and-ampere.md`](jetson-and-ampere.md): Jetson Orin and Thor, DGX Spark, and Ampere.
- [`server-and-cloud.md`](server-and-cloud.md): the overview of ARM servers and cloud bare metal,
  with [`aws-graviton-metal.md`](aws-graviton-metal.md),
  [`oracle-google-azure-grace-rentals.md`](oracle-google-azure-grace-rentals.md) and
  [`buyable-arm-servers.md`](buyable-arm-servers.md).
- [`rented-machines-and-waiting-milestones.md`](rented-machines-and-waiting-milestones.md): whether
  a rented machine could carry the milestones waiting on argon.

*Name: provisional: the directory `notes/aarch64-modern-board-survey/` follows `notes/soak/`'s
shape, a page's appendices beside it. Each stem is named for the boards it covers. calef names
directories and files.*
