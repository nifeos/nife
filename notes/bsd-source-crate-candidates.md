# Crates nife could publish from BSD source, ranked

*Name: provisional, minted 2026-11-05 (UTC) by lane/bsd-crate-candidates. calef names things; the
unit names below are descriptions, not crate names, and every one waits for an architect.*

This note answers one question: which pieces of the BSD source trees could nife port and publish as
crates that fill a real gap in the Rust ecosystem? It ranks 77 candidate units from OpenBSD and
NetBSD, with FreeBSD read only where it offered a different license for the same hardware. The
appendices hold the measurements: [source measurements](bsd-source-crate-candidates/measurements.md)
and [the Rust ecosystem per unit](bsd-source-crate-candidates/ecosystem.md).

The rule the ranking serves is risk 8's: pick by measured crates.io demand, publish nife's own crate
first, and depend on no upstream merge
([risk 8](../design/fatal-risks/8-nobody-else-builds-on-nife.md), ruled 2026-10-27 (UTC)). Each
attempt there runs a six-month clock, and an attempt nobody takes up counts toward red. So demand
carries double weight below, and a row with no measured demand does not get built on its score alone.

## Recommendation

Build the first row and hold the rest. The SD and eMMC card protocol is the one BSD-derived unit
whose profile matches the FAT crate's: an incumbent with six-figure downloads and no release in
years, a clean ISC source, hardware-free tests, and a consumer already inside nife.

- The incumbent, `sdio-host`, drew 483,688 downloads in 90 days. It has had no release since
  2022-07-27, its repository has had no commit on its default branch since then, and it has 8 open
  issues. It holds SD and eMMC register types and commands, the same scope as OpenBSD's
  `sdmmc_mem.c` and `sdmmcreg.h`.
- nife already carries a slice of it. `designware_mobile_storage` follows `sdmmc_mem.c`'s
  identification order, carries Uwe Stuehler's ISC notice, and proves its capacity decoders. A card
  protocol crate is a cut from that crate plus the rest of `sdmmc_mem.c`'s decoders, not new work
  from nothing.
- The caveat that matters: the downloads are transitive. `sdio-host` has three dependents, and
  `embassy-stm32` drives most of them. A nife crate earns that traffic only if those projects switch,
  which is an upstream step. The figure measures the ceiling, not a forecast. It is still the largest
  measured demand of any BSD-derived row, and the only one with a dormant leader.

The DesignWare I2C master (row 3) and the AHCI host (row 2) come next, because each has a consumer
in nife and a gap the ecosystem shows by reimplementing the code inside kernels. Neither has
measured crates.io demand, so each belongs on nife's own schedule rather than risk 8's. Rows 5 to
10 rank on smallness and clean licenses, not on demand. None should start until a user asks.

## The top ten

Scores use the six criteria in the next section. D is demand, weighted double. The closest crate's
figures are 90-day downloads and the date of its newest release, measured 2026-11-05 (UTC).

| rank | unit (descriptive, provisional) | BSD source and license | D | L | S | T | Z | F | score | closest crate |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | SD and eMMC card protocol: commands, CID, CSD, SCR, OCR and EXT_CSD decode, identification order | OpenBSD `sys/dev/sdmmc/sdmmc_mem.c`, `sdmmcreg.h`, `sdmmc_cis.c`; ISC | 3 | 3 | 3 | 3 | 2 | 3 | 20 | `sdio-host`, 483,688, 2022-07-27 |
| 2 | AHCI host, with the ATA identify and command layer | OpenBSD `sys/dev/ic/ahci.c` and `sys/dev/ata/atascsi.c`; ISC | 2 | 3 | 2 | 3 | 1 | 3 | 16 | `simple-ahci`, 752, 2026-02-06 |
| 3 | DesignWare I2C master, polled | OpenBSD `sys/dev/ic/dwiic.c`; ISC | 2 | 3 | 1 | 2 | 3 | 3 | 16 | none on crates.io |
| 4 | SD Host Controller (SDHCI) | OpenBSD `sys/dev/sdmmc/sdhc.c`, ISC; FreeBSD `sys/dev/sdhci/sdhci.c`, BSD-2 | 2 | 3 | 1 | 3 | 2 | 2 | 15 | `sdhci-host`, 856, 2026-09-09 |
| 5 | Toeplitz receive-side-scaling hash | OpenBSD `sys/net/toeplitz.c`; ISC and BSD-3 (DragonFly) | 1 | 3 | 3 | 3 | 3 | 1 | 15 | none on crates.io |
| 6 | TFTP client without `std` | OpenBSD `sys/lib/libsa/tftp.c`; BSD-2 | 1 | 3 | 3 | 3 | 3 | 1 | 15 | `async-tftp`, 14,843, 2025-12-02, needs `std` |
| 7 | KVM paravirtual clock reader | OpenBSD `sys/dev/pv/pvclock.c`; ISC | 2 | 3 | 2 | 1 | 3 | 1 | 14 | none on crates.io |
| 8 | blist radix-tree bitmap allocator | OpenBSD `sys/kern/subr_blist.c`; BSD-3 (DragonFly) | 1 | 3 | 3 | 3 | 2 | 1 | 14 | `bitmap-allocator`, 61,153, 2026-06-03 |
| 9 | classic BPF interpreter | OpenBSD `sys/net/bpf_filter.c`; BSD-3 | 1 | 3 | 3 | 3 | 3 | 0 | 14 | `classic_bpf`, 76, 2021-08-05 |
| 10 | `vis` and `unvis` encoding | OpenBSD `lib/libc/gen/vis.c`, `unvis.c`; BSD-3 | 1 | 3 | 3 | 3 | 3 | 0 | 14 | `vis`, 12, 2025-08-21 |

Ties break on D, then F, then T, then fewer lines. One line each:

1. A FAT-shaped opportunity, with the transitive-download caveat above.
2. Milestone 515 (the installer: a stick that puts itself on the machine's disk) records in its BUGS that SATA disks need an AHCI
   driver nothing owns. QEMU's `q35` machine has `ich9-ahci`. The two Rust crates have under 800
   downloads each.
3. nife's kernel already holds a 241-line polled master in `kernel/src/reboot/designware_i2c.rs`
   for radon's PMIC, with offsets read from GPL vendor U-Boot. A crate from the ISC source gives it a
   permissive reference. QEMU master models the block (`hw/i2c/designware_i2c.c`, on the
   `tt-atlantis` RISC-V machine). The RP2040's I2C block is this IP, and its peripheral-access crates generate the register set.
4. The Pi 5's SD controller is `brcm,bcm2712-sdhci`. QEMU has `sdhci-pci` and the Raspberry Pi
   machines. GitHub code search finds SDHCI drivers inside at least six Rust kernels.
5. No crate exists. Microsoft's RSS specification publishes verification vectors. Nothing in nife
   spreads receive queues yet.
6. The `libsa` file is already freestanding. QEMU's user network serves TFTP. nife boots over TFTP
   through firmware (milestone 260, boot xenon over the network), so it has no consumer for a client of its own.
7. GitHub code search finds `pvclock_vcpu_time_info` reimplemented in at least six Rust kernels and
   VMMs. The test needs a KVM host, so TCG and HVF cannot check it.
8. nife's frame allocator is its own design, so the fit is weak.
9. Two small crates exist, both near zero downloads. nife has no packet filter that would use it.
10. Small and clean, and nothing in nife or crates.io asks for it.

The human-interface report descriptor parser (NetBSD Foundation `sys/dev/hid/hid.c`, BSD-2) also
scored 14 and placed eleventh on D. Its fit is the strongest in the table: milestone 242 (USB host and HID) expects
keyboards that speak only report protocol, and milestone 179 (pointer input: a mouse in text mode) needs one. But `hidreport` is
maintained, `no_std` and MIT, with 38,649 downloads, so the gap is nife's and not the ecosystem's.
nife should take `hidreport` as a dependency rather than publish a rival.

## How the scoring works

Each criterion runs 0 to 3. The score is 2D + L + S + T + Z + F, out of 21. Change the weights and
the order moves; the per-row grades are what a reader should argue with.

- D, demand or evidence of a gap. 3: an incumbent with six-figure 90-day downloads that is
  dormant or lacks a `no_std` form. 2: only small incumbents, or repeated reimplementation inside
  kernels found by code search. 1: no crate and no sign anyone wants one. 0: a maintained,
  `no_std` incumbent.
- L, license. 3: ISC, BSD-2-Clause or BSD-3-Clause, all on `deny.toml`'s allow-list. 0: an
  advertising clause, a notice the allow-list does not name, or no header. A 0 rejects the row.
- S, separability. The fraction of code left after removing kernel glue, by reading: 3 from
  85%, 2 from 60%, 1 from 40%, 0 below.
- T, testable without hardware. 3: published vectors or a fuzzable format, plus a QEMU model.
  2: a QEMU model. 1: needs KVM or captured traces. 0: needs the board.
- Z, size. Lines in the source files: 3 under 800, 2 under 2,000, 1 under 4,000, 0 above.
- F, fit with nife. 3: code in the tree or a recorded BUGS entry needs it. 2: a board or
  milestone nife already plans. 1: plausible. 0: none.

## What nife already has from BSD source

Three of journey 5's nine crates
([journey 5](../design/journeys/05-crates-released-on-their-own.md)) adapt BSD code and carry its
notice. `e1000e` follows FreeBSD's `if_em.c` (BSD-2) and Intel's shared code (BSD-3).
`designware_ethernet` follows OpenBSD's `dwqe.c` and `ytphy.c` (ISC) and the NetBSD Foundation's
`ukphy_subr.c` (BSD-2). `designware_mobile_storage` follows OpenBSD's `dwmmc.c` and `sdmmc_mem.c`
(ISC). Two more read BSD files for facts only and cite them: `jh7110_clock_and_reset` (OpenBSD
`stfclock.c`, FreeBSD `jh7110_clk.c`) and `jh7110_entropy` (NetBSD). The FAT, xHCI with `usb`,
generational table and NVMe crates cite no BSD source.

None of risk 8's three ruled picks is BSD-derived, and no BSD-derived crate sits in
[the releasable-crates survey](releasable-crates-2026-10-07.md)'s publish order. Row 1 extends
`designware_mobile_storage`, and row 3 would replace a kernel module. Neither duplicates a crate the
tree ships. Under §316 (a program package leaves as one repository, with the crates only it uses),
a crate gets a repository of its own when it is publishable or more than one repository uses it.
Rows 1 to 4 would each qualify as publishable.

## Rejected rows, and why

The appendix lists all 77. These are the ones a reader might expect higher.

- Realtek RTL8111/8168, the most common PC Ethernet chip. OpenBSD's `re.c`, NetBSD's
  `rtl8169.c` and FreeBSD's `if_re.c` all carry Bill Paul's four-clause license with its
  advertising clause, and FreeBSD's SPDX line says `BSD-4-Clause`. `deny.toml` does not allow it.
  The gap is real, since code search finds four Rust kernels with their own RTL8168 driver,
  but BSD code cannot be the source. `rgephy.c`, the Realtek PHY, has the same clause.
- HFSC. Carnegie Mellon's notice is permissive but not on the allow-list.
- ISO 9660, PF_KEY, BOOTP. Advertising clauses, and `iso9660-rs` (`no_std`, 5,846) and
  `dhcproto` cover two of them.
- Prefix tables and trees (ART, NPF's `lpm.c`, `ptree.c`, `radixtree.c`, `rbtree.c`):
  `prefix-trie` (19.4M), `patricia_tree` (348k) and `intrusive-collections` (8.6M) are maintained.
- NetBSD's `thmap`. `papaya`, `scc` and `dashmap` fill the concurrent-map space.
- Cryptography (key wrap, Blowfish, `bcrypt_pbkdf`, SipHash, Michael): RustCrypto and
  `siphasher` cover them, all `no_std`.
- BER, `imsg`, DHCP client, WireGuard. `rasn`, `der` and `bcder`; an OpenBSD-only IPC framing;
  `edge-dhcp` and smoltcp's DHCPv4; `boringtun` and `gotatun`.
- SLAAC with stable addresses (OpenBSD `slaacd`'s engine, score 12). smoltcp's main branch
  carries `src/iface/slaac.rs`; RFC 7217 stable-privacy addresses are the gap left, and small.
- UARTs, RTC, GPIO, virtio, MII PHYs, range allocators. Maintained crates exist for each:
  `uart_16550`, `arm-pl011-uart`, `arm_pl031`, `arm-pl061`, `virtio-drivers` (91 dependents),
  `ieee802_3_miim`, `range-alloc` and `vm-allocator`.
- Obsolete hardware and protocols. NE2000, Intel 8255x, Sun GEM, RTL8139, Van Jacobson
  compression, PPPoE (score 8), spanning tree, BFD.
- UFS2 boot reader. §34 (RedoxFS is the primary filesystem, on three conditions) refused UFS2.
- LLDP. OpenBSD's `pdu.c` holds 125 lines of helpers; a crate would come from IEEE 802.1AB.

## Method

1. Sources, read-only, in a scratch directory outside the repository: a shallow sparse clone of
   `github.com/openbsd/src` at `bc5d5205881e579d8da936d6039dee52c2284971` (committed
   2026-11-04T15:43:31Z) and of `github.com/NetBSD/src` at
   `36eddebcc36cb0775aa2962005b9c05419a6ff01` (committed 2026-11-04T18:41:11Z). OpenBSD's paths were
   `sys/dev/ic`, `sys/dev/pci` for contrast, `sys/dev/{sdmmc,hid,mii,pv,ata}`, `sys/net`,
   `sys/lib/libsa`, `sys/crypto`, `sys/kern`, `lib/libc`, `lib/libutil` and five daemons in `sbin/`
   and `usr.sbin/`. NetBSD's were `sys/dev/ic`, `sys/net/npf` and `common/lib/libc`, plus
   `sys/kern/subr_thmap.c`. FreeBSD supplied four file headers at
   `7806832f23fb31d28386d42692b221776c9f4ee4`, to check whether its copies of the Realtek, SDHCI,
   MMC and Intel I2C drivers carry a different license. DragonFly reached the survey only through
   files OpenBSD imported (`toeplitz.c`, `subr_blist.c`). QEMU's device lists came from
   `qemu/qemu@f29ccc287a6cfdd51d3a4fd17fbe9af3310a0ac5`. No downloaded file ran.
2. A Python script, run with `-I` from its own directory, read each unit's files. It recorded lines,
   the license families in the header, and the share of code inside top-level blocks that call
   kernel services: DMA maps, mbufs, interrupt establishment, `spl`, timeouts, sleeping, locks,
   pools, `ifnet`, autoconfiguration. Register access through `bus_space` and the softc did not
   count as glue, because a register trait replaces it.
3. Crates.io's API answered 88 searches and 95 crate lookups, one request per 1.1 seconds, under a
   User-Agent that names the survey and the nife repository. Reverse-dependency counts came from the
   same API for 19 incumbents. GitHub repository and code search filled in the code that never
   reached crates.io.
4. The tree cross-check grepped `crates/`, `kernel/` and `design/roadmap/` for each unit and read
   journey 5, risk 8, the releasable-crates survey, the crate reviews and milestone 1019.
5. Grading by hand, from the measurements and from reading the top rows' functions.

## What is measured and what is not

Measured firsthand: line counts, license headers, every crates.io figure, reverse-dependency counts,
the QEMU device lists, smoltcp's SLAAC file, and the tree's own references. Estimated: separability.
The script's figure is coarse, because one sleep in a long function marks the whole function as
glue. Reading corrected it for the top rows: `dwiic.c` measures 43% but its transfer loop is
portable once a poll replaces the sleep, about 80% by reading; `sdmmc_mem.c` measures 87% and
reading agrees once block I/O and hibernation leave. Inferred, not measured: demand where no crate
exists. A missing crate shows a gap, not a user. Code-search counts are lower bounds from the first
page of results. Recalled and not checked here: the Tegra X1's SD controller is SDHCI-compatible,
which would raise row 4's fit for argon.

## Limits of the method

- A driver is a specification written as code. It carries the errata its authors met on their
  hardware, and not the ones they did not. In `ahci.c` eight lines mention a quirk, workaround or bug, and in `sdhc.c` four do. A port
  inherits both the fixes and the gaps.
- BSD code is a reference, not a proof. It shows one working order of operations; the datasheet or
  the standard still decides what is correct, and a Kani harness decides what nife claims.
- A port is derived work. Milestone 1019 (the driver crates carry facts from their sources, not
  their text) holds the rule, ruled 2026-11-04 (UTC) on its items 3 to 6. A crate that follows a
  permissive file closely carries that file's copyright and permission notice and names the file.
  A file read only for facts needs a citation. The notice travels with the crate
  to its own repository and to crates.io. `helpers/crate_release.py` on `main` accepts only
  `MIT OR Apache-2.0` as a manifest license; pull request #3200, open, widens it to that pair joined
  with ISC, BSD-2-Clause or BSD-3-Clause. A four-clause or Carnegie Mellon source passes neither.
- Downloads measure installs, and CI drives most of them. A 90-day figure moves within weeks.
- The grades are one reader's. The table shows them so another reader can regrade a row.
