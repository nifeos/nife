---
status: NOT-STARTED
raised: 2026-10-05
promoted_from: the-i219-reconfigures-its-phy-when-link-comes-up
milestone_dependencies: 494
decision_dependencies: none
machine_requirements: none
specific_machine: xenon (the only machine here with an I219, and the code is I219-specific)
needs_person: yes
---
# 804. The I219 reconfigures its PHY when link comes up

*(Promoted from the proposal pile on 2026-10-06 (UTC); number provisional until the merge queue lands it.)*

Raised by milestone 494 (a driver for the network card a PC actually has)'s I219 lane
(`lane/494-i219`) on 2026-10-05 (UTC). Title and slug are drafts.

## The gap

FreeBSD reconfigures an SPT-class I219's PHY each time link comes up, in
`e1000_check_for_copper_link_ich8lan` (`sys/dev/e1000/e1000_ich8lan.c`, BSD-3-Clause). It sets:

- the EMI receive configuration;
- the PLL clock gate by speed;
- the pointer gap at register 776.20 at 1000 Mb/s;
- the inter-packet gap at 10 Mb/s half duplex;
- the `FEXTNVM4` beacon duration ("I217 Packet Loss issue");
- the platform power values (LTR and OBFF, `e1000_platform_pm_pch_lpt`).

That is about 250 lines.

Milestone 494 ported FreeBSD's attach and reset path into `crates/e1000e/src/platform_controller_hub/` and not this,
because it runs from a link-change interrupt and this driver takes none: the kernel waits for
`STATUS.LU` once at bring-up and hands the device to `net_stack`. The limitation is recorded in
`crates/e1000e/src/platform_controller_hub/mod.rs`'s `BUGS`.

## When to build it

Only after xenon's bench boot (milestone 494's runbook) leases. If the transfer is clean, this is
correctness at speeds and link partners the bench did not see, and can wait. If the link comes up
and frames are lost, this is the first suspect.

## Options

1. Run it once, at bring-up, after `STATUS.LU`. The kernel already spins for link there.
   Covers the bench and any boot whose link never drops. A cable pulled and replugged at a
   different speed keeps the first speed's settings.
2. Run it on every link change. Needs the link-change interrupt delivered somewhere that may
   reach page 0 of BAR0 and the PHY, which is the kernel, not `net_stack`. x86 does not yet route
   a device line to a waiter (milestone 299 (the x86 port-range capability)'s scope note), so this
   waits on interrupt delivery or a kernel-side poll.

Reuse: adapt FreeBSD's `e1000_check_for_copper_link_ich8lan` and `e1000_platform_pm_pch_lpt`
(BSD-3-Clause, Intel's notice kept), as milestone 494 adapted the attach path; no Rust crate
covers any I219 (milestone 494's `Reuse` survey). Linux's `e1000e` is GPL and not a source.

Recommendation: 1 first, as its own small lane, with 2 recorded as its limitation. Reversible.

## Index row

FreeBSD reconfigures an I219's PHY every time link comes up (EMI, PLL clock gate, inter-packet gaps, beacon duration, platform power). That is about 250 lines, which milestone 494 (a driver for the network card a PC actually has) did not port because it takes no link-change interrupt. If xenon's bench boot leases but loses frames, this is the first suspect; running it once after `STATUS.LU` is the small first step.
