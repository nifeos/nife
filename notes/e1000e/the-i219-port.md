# The I219 port: `crates/e1000e`'s sources, and its PCH bring-up

An appendix to [the `e1000e` NIC](../e1000e.md). The crate tells its own story of the I219
bring-up in `crates/e1000e/src/pch/`. This page holds what nife adds to it: why the port reads
FreeBSD, how the kernel calls it, and what the QEMU gates prove of it. The stem `the-i219-port` is
a provisional name.

## Where the crate's facts come from

The crate's README, under "Sources", says what was read for its register facts and its PCH port,
and when. Each item in `pch` names the FreeBSD function it follows. `pch::phy`'s module doc gives
the PHY layer's two deliberate divergences, and `pch::nvm`'s says why the NVM port is read only.

## Why the I219 needs a port at all

The I219 carries workarounds that only field exposure finds, so `pch` adapts FreeBSD's rather than
writing its own. FreeBSD's Intel shared code (BSD-3-Clause) is the only source this tree reads for
them. Linux's `e1000e` is GPL and was not consulted for any of it. This follows calef's amendment of
2026-10-04 to §46 (thin primitives or whole subsystems): outside the kernel and the Kani-proved
crates, take or adapt first. The constants are facts about the hardware. The sequences are
FreeBSD's, adapted, and each adapted file carries Intel's license as its first condition requires.

## The three passes

`kernel/src/e1000e.rs`'s `bring_up` calls `pch::sequence`'s three passes in FreeBSD's order:
`init_phy_workarounds` at attach, `global_reset` after the kernel's `flush_descriptor_rings`, and
`init_hw` before the rings are programmed, with `transmit_errata` once `TCTL` is written.
`pch::sequence`'s module doc gives that order. Each function's doc says what it does and names the
FreeBSD function it follows. Only the branches for FreeBSD's `e1000_pch_spt` class are carried,
because every I219 the crate claims is in it (`pch::is_pch`).

Every sequence is generic over `pch::Hw`, so it runs on the host against a simulated I219
(`pch/sim.rs`) as well as in the kernel. QEMU's 82574L is not a PCH part and never reaches
`sequence`. It does reach `phy::id_at`, which shares `phy::read_mdic` with everything here, so the
MDIO primitive is the one piece the QEMU gates prove.

## BUGS

- The port has run on silicon once, with no cable ([the note's BUGS](../e1000e.md#bugs)). Under
  the gates `sequence` is skipped, since `is_pch` is false for QEMU's 82574L. It is proved only
  against `sim`, which is this crate's reading of FreeBSD and not the device.
- The link-up half is not ported. FreeBSD reconfigures the PHY each time link comes up, from a
  link-change interrupt this driver does not take (`e1000_check_for_copper_link_ich8lan`). That
  covers the EMI receive configuration, the PLL clock gate, the 776.20 pointer gap at 1000 Mb/s,
  the beacon duration, LTR and OBFF. A link that comes up and then loses frames, especially at 10
  or 100 Mb/s, points here first. It is about 250 lines of FreeBSD, proposed in milestone 494 (a
  driver for the network card a PC actually has)'s Follow-on.
- Flow control is not configured, so it is not advertised. FreeBSD advertises symmetric pause and
  programs the watermarks to match (`e1000_set_fc_watermarks_generic`). This driver does neither
  (`sequence::setup_copper_link`).
- The transmit flush diverges from FreeBSD in one place, said at `flush::tail_after`.
- One read in FreeBSD's `SMBus` unforce is not mirrored. In `e1000_phy_is_accessible_pchlan`, a
  failed read of `CV_SMB_CTRL` leaves the previous register's value in the variable. That value is
  then written back with one bit cleared. This port writes only when the read succeeded.
- The license's second condition is not met by any binary this tree ships yet. A binary
  redistribution must reproduce Intel's notice "in the documentation and/or other materials". The
  source carries it in each adapted file. Nothing that ships an image (the stick, a package)
  carries a third-party notices file, and none exists in the tree. Milestone 494's block records
  it.
- Legacy descriptors on the I219 are an assumption. The 82574L supports them, and QEMU uses them
  unless `RFCTL.EXTEN` is set. The PCH parts are believed to, from the 8254x lineage, and that was
  not read anywhere.
