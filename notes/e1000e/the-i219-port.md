# The I219 port: `crates/e1000e`'s sources, and its PCH bring-up

An appendix to [the `e1000e` NIC](../e1000e.md). It holds what the crate's doc comments link to:
where its register facts come from, and how `crates/e1000e/src/pch/` carries FreeBSD's I219
bring-up. The stem `the-i219-port` is a provisional name.

## Where the crate's facts come from

Offsets, bits, descriptor layouts and device ids were read, not recalled, on 2026-10-04 (UTC). The
sources were Linux's `drivers/net/ethernet/intel/e1000e/regs.h`, `defines.h` and `hw.h`. They were
cross-checked the same day against FreeBSD's BSD-licensed `sys/dev/e1000/`: `e1000_regs.h`, and
`e1000_api.c`'s SPT class, which is exactly the I219 rows of `e1000e::DEVICE_IDS`.

QEMU's `hw/net/e1000e_core.c` says what the emulated 82574L does with them. It uses legacy
descriptors unless `RFCTL.EXTEN` is set, and it receives only with `RCTL.EN`, link up and bus
mastering. No Linux code was copied.

Intel's own I219 datasheet was not read. Where the I219 is believed to differ from the 82574L, the
item says so.

## Why the I219 needs a port at all

The I219 is not a PCIe NIC in the 82574L's sense. It is a MAC inside the platform controller hub
(PCH) talking to a separate PHY, which it shares with the Management Engine. It carries workarounds
that only field exposure finds.

FreeBSD's Intel shared code (BSD-3-Clause) is the only source this tree reads for them. Linux's
`e1000e` is GPL and was not consulted for any of it. This follows calef's amendment of 2026-10-04 to
§46 (thin primitives or whole subsystems): outside the kernel and the Kani-proved crates, take or
adapt first. The constants are facts about the hardware. The sequences are FreeBSD's, adapted, and
each adapted file carries Intel's license as its first condition requires.

Everything was read from `sys/dev/e1000/` on FreeBSD's `main`: `ulp`, `flush` and `reset` on
2026-10-04, the rest on 2026-10-05. The last commit touching `e1000_ich8lan.c` was then 46cf612d
(2026-08-30).

| module | what it holds | FreeBSD source |
|---|---|---|
| `ulp` | leaving ultra-low-power mode through the Management Engine | `e1000_disable_ulp_lpt_lp` (`e1000_ich8lan.c`), its ME branch |
| `flush` | the descriptor-ring flush SPT parts need before a reset | `em_flush_desc_rings`, `em_flush_tx_ring`, `em_flush_rx_ring` (`if_em.c`) |
| `reset` | the MAC-register constants around the global reset | `e1000_reset_hw_ich8lan` (`e1000_ich8lan.c`), `e1000_disable_pcie_master_generic` (`e1000_mac.c`) |
| `phy` | MDIO through `MDIC`, the software/firmware semaphore, the paged PHY registers | below |
| `nvm` | reading the `GbE` flash region through BAR0, as Sunrise Point does | below |
| `sequence` | the bring-up itself, in FreeBSD's order | below |

Every sequence is generic over `pch::Hw`, so it runs on the host against a simulated I219
(`pch/sim.rs`) as well as in the kernel. QEMU's 82574L is not a PCH part and never reaches
`sequence`. It does reach `phy::id_at`, which shares `phy::read_mdic` with everything here, so the
MDIO primitive is the one piece the QEMU gates prove.

## The three passes

FreeBSD brings an SPT-class I219 up in three passes, and `kernel/src/e1000e.rs` calls
`pch::sequence`'s functions in the same order. Only the branches for FreeBSD's `e1000_pch_spt`
class are carried, because every I219 the crate claims is in it (`pch::is_pch`). Each function's
doc comment names the FreeBSD function it follows.

1. `init_phy_workarounds`, at attach. It gates the hardware's automatic PHY configuration and
   forces the PHY out of ultra-low-power. That goes through the Management Engine if there is one,
   and through PHY registers if not. It makes sure the MAC-PHY interconnect is PCIe rather than
   `SMBus`. If the PHY does not answer, it forces `SMBus` and power-cycles the PHY with `LANPHYPC`.
   Then it resets the PHY so it starts from a known state. The sources are
   `e1000_init_phy_workarounds_pchlan`, `e1000_disable_ulp_lpt_lp`,
   `e1000_phy_is_accessible_pchlan` and `e1000_toggle_lanphypc_pch_lpt`.
2. `global_reset`, after the kernel's descriptor-ring flush. The MAC and PHY reset together, and
   then the PHY is configured from the NVM. The sources are `e1000_reset_hw_ich8lan`,
   `e1000_get_cfg_done_ich8lan`, `e1000_post_phy_reset_ich8lan`, `e1000_sw_lcd_config_ich8lan`,
   `e1000_write_smbus_addr` and `e1000_oem_bits_config_ich8lan`.
3. `init_hw`, before the rings are programmed, and `transmit_errata` once transmit is on. These
   are `e1000_init_hw_ich8lan`'s hardware bits, `e1000_setup_copper_link_pch_lpt`, and
   `em_initialize_transmit_unit`'s SPT errata.

## The PHY layer

| item | FreeBSD source |
|---|---|
| `read_mdic`, `write_mdic` | `e1000_read_phy_reg_mdic`, `e1000_write_phy_reg_mdic` (`e1000_phy.c`); the 82574L uses the same register |
| `Swflag` | `e1000_acquire_swflag_ich8lan`, `e1000_release_swflag_ich8lan` (`e1000_ich8lan.c`) |
| `read`, `write` | `__e1000_read_phy_reg_hv`, `__e1000_write_phy_reg_hv`, `e1000_get_phy_addr_for_hv_page`, `e1000_set_page_igp` (`e1000_phy.c`) |
| `reset_blocked` | `e1000_check_reset_block_ich8lan` |

The paged access is what every PCH PHY register above page 0 needs. Each paged access takes a
`Swflag`, and the only way to hold one is `Swflag::acquire`. FreeBSD's "assumes the semaphore is
already acquired" comments are the type system here.

Two divergences, each deliberate:

- There is no retry mechanism. FreeBSD retries an MDIC transaction only on Meteor Lake and later
  (`current_retry_counter` is 2 from `e1000_pch_mtp`, 0 below it). Every part this crate claims is
  Sunrise Point, where the count is 0, so one attempt is FreeBSD's behavior exactly.
- Two page ranges are refused rather than ported. Page 800 is the host wakeup page, with its own
  address and data opcode protocol (`e1000_access_phy_wakeup_reg_bm`). Pages 1 to 767 are the
  82577 and 82578 debug registers (`e1000_access_phy_debug_regs_hv`). Nothing in `sequence`
  addresses either. Only an NVM-supplied LCD configuration word could, and that returns
  `Error::PageNotPorted` rather than writing somewhere unexamined.

## The NVM

On Sunrise Point the `GbE` flash region is reached through registers in BAR0 rather than the
chipset's SPI controller. Two things in the bring-up need it. The post-reset LCD configuration
replays (register, value) pairs the board vendor stored in the NVM's extended configuration region.
FreeBSD also checks that an NVM is present after every PHY reset.

The port is read only. Nothing in `nvm` can write or erase the flash, and no write path was ported.
It comes from the SPT branches of `e1000_ich8lan.c`:

| item | FreeBSD source |
|---|---|
| `Geometry::read` | `e1000_init_nvm_params_ich8lan` |
| `valid_bank` | `e1000_valid_nvm_bank_detect_ich8lan` |
| `read_word`, single words only | `e1000_read_nvm_spt` |
| the flash cycle beneath them | `e1000_read_flash_data32_ich8lan`, `e1000_flash_cycle_init_ich8lan`, `e1000_flash_cycle_ich8lan` |

## BUGS

- The port has run on silicon once, with no cable ([the note's BUGS](../e1000e.md#bugs)). Under
  the gates `sequence` is skipped, since `is_pch` is false for QEMU's 82574L. It is proved only
  against `sim`, which is this crate's reading of FreeBSD and not the device.
- The link-up half is not ported. FreeBSD reconfigures the PHY each time link comes up, from a
  link-change interrupt this driver does not take (`e1000_check_for_copper_link_ich8lan`). That
  covers the EMI receive configuration, the PLL clock gate, the 776.20 pointer gap at 1000 Mb/s,
  the beacon duration, LTR and OBFF. A link that comes up and then loses frames, especially at 10
  or 100 Mb/s, points here first. It is about 250 lines of FreeBSD, proposed in milestone 494's
  Follow-on.
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
