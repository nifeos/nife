---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The driver crates carry facts from their sources, not their text

A read-and-report lane raised this on 2026-11-01 (UTC) after it audited the outside-driver citations
of eight crates: `e1000e`, `designware_ethernet`, `designware_mobile_storage`,
`jh7110_clock_and_reset`, `jh7110_watchdog`, `extensible_host_controller_interface`, `usb` and
`machine_discovery`. The audit used the method of the `jh7110_entropy` licence audit
(nifeos/jh7110_entropy#3). For each citation it fetched the cited file at a pinned revision, read
its licence header, and compared it with the crate as runs of identical lower-case word tokens,
with hexadecimal normalized and identifiers split at underscores. The pull request that adds this
file also fixes the citations that were only wrong about a path, a function name or what a source
says. What remains here touches a licence, a notice or a design claim, and each item needs an
architect's ruling. This is an engineering audit, not legal advice.

Reuse: none exists. The work rewrites or annotates nife's own text, and the method is the one
`jh7110_entropy` already used.

## Background

Most of these crates read GPL drivers for facts: offsets, bit positions, register names and
bring-up order. The `jh7110_entropy` audit treated those as facts and rewrote only copied prose and
quoted device-tree nodes. Three of the crates also adapt permissive code on purpose: OpenBSD's
`dwqe`, `ytphy` and `dwmmc` under ISC, and FreeBSD's Intel shared code under BSD-3-Clause. Each one
carries the notice of the file it names. The findings below are the places where the code
follows a source more closely than the crate says, or follows a file whose notice it does not
carry.

## Items, most severe first

Each item gives the recommendation first.

1. `jh7110_clock_and_reset::standard_mode_100k` translates GPL-2.0+ code. Recommendation:
   rewrite it from the I2C bus specification's standard-mode minimums and the DesignWare count
   equations, with a period fill nife chooses and states. If no public statement of the equations
   exists, record the function as GPL-derived and move it out of the published crate. It follows the
   vendor U-Boot `designware_i2c.c` functions `dw_i2c_calc_timing` and `calc_counts` step for step:
   the same back-solve, the same "split the difference and bias to lcnt" fill, and the same
   rounding division. The SCL equations describe the hardware. The fill is U-Boot's own choice.
   Milestone 819 (JH7110 clock and reset logic, proven)'s `Reuse:` line says the lane read its sources and copied
   none.
2. `usb::boot_keyboard::TABLE` and `MODIFIER_CODES` copy Linux's `hid_keyboard[]` (GPL-2.0 or
   later). Recommendation: derive both from the HID Usage Tables names mapped to the `KEY_*` codes in
   `include/uapi/linux/input-event-codes.h` (GPL-2.0 WITH Linux-syscall-note), and keep
   `hid_keyboard[]` only as a cross-check in a test. Usages 0x32 and 0x46 then become nife's own
   choices, stated as such. The alternative is to rule that the table is a list of facts. The table
   holds 104 numbers in Linux's order, and it attributes them.
3. `e1000e::pch` adapts FreeBSD's `if_em.c` and `if_em.h` without their notice. Recommendation:
   add their BSD-2-Clause notice and its three copyright lines (Intel 2001-2024, Nicole Graziano
   2016, Kevin Bowling 2024) beside the Intel shared-code notice in `pch/mod.rs`. Then name the file
   correctly where the crate README, `pch/mod.rs`, `notes/e1000e/the-i219-port.md` and the licence
   column of milestone 494 (the I219 driver)'s Reuse table describe it as BSD-3-Clause shared code.
   The module takes the descriptor-ring flush, the transmit errata and `em_reset`'s buffer split from
   those files, and it quotes a 25-token comment and an 11-token comment.
4. `kernel/src/e1000e.rs` follows FreeBSD's `em_flush_tx_ring` and `em_flush_rx_ring`.
   Recommendation: rule the same way as item 3. Either the kernel function carries the notice, or it
   drives the sequence through a `pch` function that already carries it. It copies no text run longer
   than five tokens.
5. `designware_ethernet::motorcomm::link_from_registers` adapts OpenBSD's `ukphy_status`
   (`sys/dev/mii/ukphy_subr.c`, NetBSD Foundation BSD-2-Clause). Recommendation: add that notice
   beside the ISC notice in `src/lib.rs`, and name the file in the README, in
   `notes/designware-ethernet.md` and in milestone 53 (the board's own peripherals)'s `Reuse:`
   line. It reads the same registers in the same priority order.
6. `designware_mobile_storage` follows OpenBSD's `sdmmc_mem.c` (ISC, Uwe Stuehler 2006), and the
   crate root carries only Mark Kettenis's `dwmmc.c` notice. Recommendation: add Stuehler's notice if
   the crate adapts the identification sequence; otherwise say in `src/lib.rs` and `src/sd.rs` that
   the crate follows only the order. Milestone 817 (a proven DesignWare SD and eMMC host)'s `Reuse:` line says that the milestone
   takes nothing and that Linux is the reference. That contradicts the crate's own OpenBSD
   attribution, and its BUGS entry names `tests/fixtures` files that do not exist. Separately: should
   the crate's `license` field read `(MIT OR Apache-2.0) AND ISC`?
7. Fixtures and doc comments that quote upstream device-tree source. Recommendation: apply the
   rule of commit e1edb20b5 (cite the upstream TRNG nodes, do not quote them). A comment becomes a
   citation (path, revision, fetch date) and one sentence, and each fixture body keeps only the
   properties its test reads. The cases are:
   - `machine_discovery/tests/fixtures/th1520.dts`, whose header says it copies every node below and
     shares a 96-token run with `th1520.dtsi` (GPL-2.0 OR MIT), with no MIT notice.
   - `jh7110-stgcrg-mainline.dts`, whose comment quotes the stgcrg node at an unpinned revision.
   - `jh7110_watchdog/tests/fixtures/jh7110-watchdog-radon.dts`, whose body carries the vendor
     `rstgen` node whole (40 tokens) and properties no test reads.
   - `jh7110_clock_and_reset/src/lib.rs`, which quotes the two PMIC nodes in its doc comments (9 and
     7 tokens).

   Fixture bodies cut to the properties a test reads (up to 40-token runs in the gmac and pmic-bus
   fixtures) are facts under that precedent, and need a ruling only if it does not cover them.
8. Attributed quotations of text under a licence that asks for a notice.
   Recommendation: add "(CC-BY-4.0)" beside the 36-token SBI v3.0 quotation in
   `machine_discovery/src/riscv64.rs`, and keep the short attributed quotations of the VT-d, AMD
   IOMMU and ACPI specifications under the five-word rule the `jh7110_entropy` audit used. The Xen
   `hvm_start_info` layout in `notes/machine-discovery.md` comes from an MIT header without its
   notice; it is an ABI layout, so a citation of the file and revision should be enough.
9. The `designware_ethernet` YT8531 register names rest on a false claim. `motorcomm.rs` says
   Linux's `motorcomm.c` is the only source that names the clock-gating and sleep-control registers.
   OpenBSD's `ytphy.c` (`YT8511_EXT_CLK_GATE`, `YT8511_EXT_SLEEP_CTRL`) and FreeBSD's `mcommphy.c`
   name them too. Recommendation: correct the sentence, then decide whether those two constants keep
   Linux's names. The same line cites "Decision 52", a ruling recorded only in pull request #2762's
   body; read against the tree it points at §52 (a set of names is a namespace), which is unrelated. Recommendation:
   cite those rulings as "#2762 Decision N" across the crate's Name blocks.
10. `jh7110_watchdog` calls the block an Arm SP805, and no listed source says so. `WDOGIMS`
    appears in no source; SP805 names that offset `WDOGMIS`. Recommendation: cite the SP805 manual
    (ARM DDI 0270) if the identification stands, or say "SP805-like", as milestone 593 (the JH7110
    watchdog) does.
11. Unpinned links where the revision changes the fact. "Both trees assign 50 MHz" to the SD
    clock, and `u5_dw_i2c_clk_core` as a fixed factor, hold at the vendor branch head but not both at
    `VF2_v2.10.4`, the release radon's firmware comes from. Recommendation: pin the vendor links in
    `jh7110_clock_and_reset` and `designware_mobile_storage` to the revision each claim holds at.

## What the lane left alone that a lane could fix after a ruling

- `designware_ethernet/src/tests.rs` says the descriptor bits are "as the databook places them",
  and the crate cites no databook edition; `descriptor.rs` says they come from `dwqereg.h`.
- `extensible_host_controller_interface/README.md` describes QEMU's qemu-xhci as four ports in its
  example. QEMU's default is eight. The example's register value changes with it.
- The `Reuse:` paragraph of `notes/designware-ethernet.md` and milestone 53's block leave out
  `motorcomm.c`, `include/uapi/linux/mii.h` and `include/linux/stmmac.h`, which the crate reads for
  facts.
- `notes/jh7110-watchdog.md` says the tree names only the gate. The vendor node also states
  `clock-frequency = <2000000>`, which the crate does not read.
- `jh7110_clock_and_reset` attributes the PMIC reset register (0x32, bit 6) to a bench reading.
  OpenSBI's `platform/generic/starfive/jh7110.c` (BSD-2-Clause) holds the same numbers.
- The Intel 82574 datasheet, the Synopsys DW_mshc and DesignWare Ethernet databooks, JEDEC
  JESD84-B51 and the Arm DDI 0487 edition behind `aarch64.rs`'s D19.2 are not public, so the audit
  could not verify citations of them.
