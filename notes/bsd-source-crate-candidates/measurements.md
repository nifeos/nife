# Source measurements for the BSD crate candidates

The table behind [the ranked survey](../bsd-source-crate-candidates.md): every unit the survey read,
at `openbsd/src@bc5d5205881e579d8da936d6039dee52c2284971` and
`NetBSD/src@36eddebcc36cb0775aa2962005b9c05419a6ff01`. NetBSD's `subr_thmap.c` came from the same
NetBSD commit.

The columns, as the survey's script counted them:

- **lines**: every line in the listed files.
- **code**: non-blank lines once comments are gone.
- **in hard-glue blocks**: code lines inside a top-level block that calls a kernel service (DMA
  maps, mbufs, interrupt establishment, `spl`, timeouts, sleeping, locks, pools, `ifnet`,
  autoconfiguration, PCI or FDT attachment). Register access through `bus_space` and the softc
  does not count.
- **portable (tool)**: code outside those blocks, as a share of code. It is coarse: one sleep in a
  long function marks all of it. The main note corrects it by reading for the top rows.
- **license**: the families the header carries. "BSD-4, advertising clause" fails `deny.toml`.

| unit | tree | files | lines | code | in hard-glue blocks | portable (tool) | license |
|---|---|---|---|---|---|---|---|
| AHCI SATA host (QEMU ich9-ahci) | OpenBSD | `sys/dev/ic/ahci.c, sys/dev/ic/ahcireg.h, sys/dev/ic/ahcivar.h` | 3875 | 2914 | 1143 | 61% | ISC |
| UFS host controller (QEMU ufs) | OpenBSD | `sys/dev/ic/ufshci.c, sys/dev/ic/ufshcireg.h, sys/dev/ic/ufshcivar.h` | 2639 | 1839 | 1094 | 41% | ISC |
| SD host controller standard (QEMU sdhci) | OpenBSD | `sys/dev/sdmmc/sdhc.c, sys/dev/sdmmc/sdhcreg.h, sys/dev/sdmmc/sdhcvar.h` | 1770 | 1298 | 605 | 53% | ISC |
| SDIO CIS tuple parser | OpenBSD | `sys/dev/sdmmc/sdmmc_cis.c` | 191 | 147 | 0 | 100% | ISC |
| SD/MMC card identification and CSD/CID decode | OpenBSD | `sys/dev/sdmmc/sdmmc_mem.c, sys/dev/sdmmc/sdmmcreg.h` | 1689 | 1288 | 164 | 87% | ISC |
| PL011 UART (QEMU virt) | OpenBSD | `sys/dev/ic/pluart.c, sys/dev/ic/pluartvar.h` | 969 | 712 | 329 | 54% | ISC |
| 16550 UART | OpenBSD | `sys/dev/ic/com.c, sys/dev/ic/comreg.h` | 1883 | 1387 | 673 | 51% | BSD-3 |
| RTL8139 Ethernet (QEMU rtl8139) | OpenBSD | `sys/dev/ic/rtl81x9.c, sys/dev/ic/rtl81x9reg.h` | 2467 | 1582 | 662 | 58% | BSD-4, advertising clause |
| RTL8169/8111 Ethernet | OpenBSD | `sys/dev/ic/re.c, sys/dev/ic/rtl81x9reg.h` | 3762 | 2634 | 1499 | 43% | BSD-4, advertising clause |
| NE2000/DP8390 (QEMU ne2k) | OpenBSD | `sys/dev/ic/ne2000.c, sys/dev/ic/dp8390.c, sys/dev/ic/dp8390reg.h` | 2442 | 1230 | 461 | 63% | BSD-2; one file with no header |
| i8042 PS/2 controller | OpenBSD | `sys/dev/ic/pckbc.c, sys/dev/ic/i8042reg.h` | 1172 | 855 | 381 | 55% | BSD-2; one file with no header |
| Broadcom GENET Ethernet (Raspberry Pi 4) | OpenBSD | `sys/dev/ic/bcmgenet.c, sys/dev/ic/bcmgenetreg.h` | 1188 | 882 | 447 | 49% | BSD-2 |
| DesignWare I2C master (JH7110, many SoCs) | OpenBSD | `sys/dev/ic/dwiic.c, sys/dev/ic/dwiicvar.h` | 681 | 481 | 276 | 43% | ISC |
| Realtek PCIe card reader | OpenBSD | `sys/dev/ic/rtsx.c, sys/dev/ic/rtsxreg.h` | 1998 | 1443 | 458 | 68% | ISC |
| Intel 8255x Ethernet (QEMU i82559) | OpenBSD | `sys/dev/ic/fxp.c, sys/dev/ic/fxpreg.h` | 2254 | 1485 | 832 | 44% | BSD-2 |
| Sun GEM Ethernet (contrast) | OpenBSD | `sys/dev/ic/gem.c, sys/dev/ic/gemreg.h` | 2366 | 1610 | 731 | 55% | BSD-2 |
| DesignWare EQoS (nife has designware_ethernet) | OpenBSD | `sys/dev/ic/dwqe.c, sys/dev/ic/dwqereg.h` | 1762 | 1394 | 766 | 45% | ISC |
| NVMe (nife has non_volatile_memory_express) | OpenBSD | `sys/dev/ic/nvme.c, sys/dev/ic/nvmereg.h` | 2685 | 2105 | 622 | 70% | ISC |
| generic MII PHY status decode | OpenBSD | `sys/dev/mii/ukphy_subr.c, sys/dev/mii/mii_physubr.c` | 715 | 446 | 296 | 34% | BSD-2 |
| Realtek Gigabit PHY | OpenBSD | `sys/dev/mii/rgephy.c, sys/dev/mii/rgephyreg.h` | 663 | 451 | 224 | 50% | BSD-4, advertising clause |
| HID report descriptor parser | OpenBSD | `sys/dev/hid/hid.c, sys/dev/hid/hid.h` | 1220 | 997 | 8 | 99% | BSD-2 |
| virtio ring (contrast; virtio-drivers exists) | OpenBSD | `sys/dev/pv/virtio.c, sys/dev/pv/virtioreg.h` | 1271 | 783 | 176 | 78% | BSD-2; BSD-3 |
| virtio-gpu | OpenBSD | `sys/dev/pv/viogpu.c, sys/dev/pv/viogpu.h` | 1177 | 876 | 271 | 69% | ISC; BSD-3 |
| KVM pvclock | OpenBSD | `sys/dev/pv/pvclock.c, sys/dev/pv/pvreg.h` | 517 | 364 | 108 | 70% | ISC |
| classic BPF interpreter | OpenBSD | `sys/net/bpf_filter.c` | 637 | 475 | 0 | 100% | BSD-3 |
| ART longest-prefix-match routing table | OpenBSD | `sys/net/art.c, sys/net/art.h` | 1270 | 758 | 136 | 82% | ISC |
| Toeplitz RSS hash | OpenBSD | `sys/net/toeplitz.c, sys/net/toeplitz.h` | 353 | 197 | 0 | 100% | ISC; BSD-3 |
| FQ-CoDel queue discipline | OpenBSD | `sys/net/fq_codel.c` | 923 | 645 | 258 | 60% | ISC |
| HFSC scheduler | OpenBSD | `sys/net/hfsc.c` | 1724 | 1182 | 299 | 75% | Carnegie Mellon permissive notice |
| Van Jacobson TCP/IP header compression | OpenBSD | `sys/net/slcompress.c` | 602 | 373 | 0 | 100% | BSD-3 |
| 802.1D/802.1w spanning tree | OpenBSD | `sys/net/bridgestp.c` | 2293 | 1770 | 438 | 75% | BSD-2 |
| BFD liveness protocol | OpenBSD | `sys/net/bfd.c` | 1045 | 761 | 487 | 36% | ISC |
| passive OS fingerprinting | OpenBSD | `sys/net/pf_osfp.c` | 626 | 509 | 110 | 78% | ISC |
| PPP LCP/IPCP state machines | OpenBSD | `sys/net/if_spppsubr.c` | 5014 | 3749 | 2043 | 46% | BSD-2 |
| PPPoE discovery | OpenBSD | `sys/net/if_pppoe.c` | 1633 | 1293 | 1057 | 18% | BSD-2 |
| WireGuard Noise IK and cookies | OpenBSD | `sys/net/wg_noise.c, sys/net/wg_cookie.c` | 2036 | 1537 | 779 | 49% | ISC |
| PF_KEY v2 message parser | OpenBSD | `sys/net/pfkeyv2_parsemessage.c` | 941 | 722 | 0 | 100% | BSD-4, advertising clause |
| ISO 9660 boot reader | OpenBSD | `sys/lib/libsa/cd9660.c` | 382 | 279 | 0 | 100% | BSD-4, advertising clause |
| FFS2 boot reader | OpenBSD | `sys/lib/libsa/ufs2.c` | 766 | 482 | 0 | 100% | BSD-3 |
| TFTP client | OpenBSD | `sys/lib/libsa/tftp.c` | 425 | 301 | 0 | 100% | BSD-2 |
| BOOTP client | OpenBSD | `sys/lib/libsa/bootp.c` | 323 | 226 | 0 | 100% | BSD-4, advertising clause |
| BSD disklabel | OpenBSD | `sys/lib/libsa/disklabel.c, sys/lib/libsa/dkcksum.c` | 125 | 42 | 0 | 100% | BSD-3 |
| AES key wrap RFC 3394 | OpenBSD | `sys/crypto/key_wrap.c` | 112 | 65 | 0 | 100% | ISC |
| TKIP Michael MIC | OpenBSD | `sys/crypto/michael.c` | 98 | 62 | 0 | 100% | ISC |
| non-repeating ID generator | OpenBSD | `sys/crypto/idgen.c` | 138 | 96 | 0 | 100% | ISC; public domain |
| Blowfish | OpenBSD | `sys/crypto/blf.c` | 606 | 521 | 0 | 100% | BSD-3 |
| bcrypt_pbkdf | OpenBSD | `lib/libutil/bcrypt_pbkdf.c` | 175 | 101 | 0 | 100% | ISC |
| SipHash | OpenBSD | `sys/crypto/siphash.c` | 181 | 108 | 0 | 100% | BSD-3 |
| blist radix-tree bitmap allocator | OpenBSD | `sys/kern/subr_blist.c` | 1328 | 811 | 33 | 96% | BSD-3 |
| extent resource-range allocator | OpenBSD | `sys/kern/subr_extent.c` | 1266 | 692 | 451 | 35% | BSD-2 |
| imsg framed IPC | OpenBSD | `lib/libutil/imsg.c, lib/libutil/imsg-buffer.c, lib/libutil/imsg.h` | 1710 | 1363 | 20 | 99% | ISC |
| BER encoder and decoder | OpenBSD | `lib/libutil/ber.c, lib/libutil/ber.h` | 1613 | 1288 | 187 | 85% | ISC |
| open-addressing hash | OpenBSD | `lib/libutil/ohash.c` | 327 | 268 | 0 | 100% | ISC |
| human-scaled numbers | OpenBSD | `lib/libutil/fmt_scaled.c` | 327 | 218 | 0 | 100% | BSD-3 |
| vis(3)/unvis(3) encoding | OpenBSD | `lib/libc/gen/vis.c, lib/libc/gen/unvis.c` | 560 | 418 | 0 | 100% | BSD-3 |
| b64_ntop/b64_pton | OpenBSD | `lib/libc/net/base64.c` | 307 | 148 | 0 | 100% | ISC |
| CIDR text parse/print | OpenBSD | `lib/libc/net/inet_net_pton.c, lib/libc/net/inet_net_ntop.c` | 406 | 271 | 0 | 100% | ISC |
| DNS name compression | OpenBSD | `lib/libc/net/res_comp.c` | 466 | 303 | 0 | 100% | ISC; BSD-3 |
| strtonum | OpenBSD | `lib/libc/stdlib/strtonum.c` | 66 | 43 | 0 | 100% | ISC |
| NTP packet | OpenBSD | `usr.sbin/ntpd/ntp_msg.c, usr.sbin/ntpd/ntp.h` | 234 | 96 | 0 | 100% | ISC |
| HTTPS Date time constraint | OpenBSD | `usr.sbin/ntpd/constraint.c` | 1192 | 919 | 36 | 96% | ISC |
| DHCP client state machine | OpenBSD | `sbin/dhcpleased/engine.c` | 1959 | 1655 | 233 | 86% | ISC |
| SLAAC and RFC 7217 address engine | OpenBSD | `sbin/slaacd/engine.c` | 2712 | 2272 | 110 | 95% | ISC; BSD-3 |
| LLDP PDU decode | OpenBSD | `usr.sbin/lldpd/pdu.c, usr.sbin/lldpd/pdu.h` | 125 | 66 | 0 | 100% | ISC |
| Internet checksum (contrast) | OpenBSD | `sbin/dhcpleased/checksum.c` | 80 | 25 | 0 | 100% | BSD-3 |
| NetBSD constant database reader | NetBSD | `common/lib/libc/cdb/cdbr.c` | 313 | 232 | 55 | 76% | BSD-2 |
| NetBSD radix tree | NetBSD | `common/lib/libc/gen/radixtree.c` | 1751 | 1187 | 264 | 78% | BSD-2 |
| NetBSD Patricia tree | NetBSD | `common/lib/libc/gen/ptree.c` | 1229 | 776 | 0 | 100% | BSD-2 |
| NetBSD red-black tree | NetBSD | `common/lib/libc/gen/rbtree.c` | 1356 | 835 | 0 | 100% | BSD-2 |
| radix priority search tree | NetBSD | `common/lib/libc/gen/rpst.c` | 716 | 505 | 101 | 80% | BSD-2 |
| NPF longest-prefix match | NetBSD | `sys/net/npf/lpm.c, sys/net/npf/lpm.h` | 499 | 348 | 0 | 100% | BSD-2 |
| NPF TCP state tracking | NetBSD | `sys/net/npf/npf_state_tcp.c` | 592 | 356 | 0 | 100% | BSD-2 |
| NetBSD concurrent trie hash map | NetBSD | `sys/kern/subr_thmap.c` | 1077 | 627 | 0 | 100% | BSD-2 |
| PL031 RTC | NetBSD | `sys/dev/ic/pl031.c` | 85 | 43 | 0 | 100% | BSD-2 |
| PL061 GPIO | NetBSD | `sys/dev/ic/pl061.c, sys/dev/ic/pl061reg.h` | 161 | 86 | 27 | 69% | BSD-2 |
| DesignWare watchdog | NetBSD | `sys/dev/ic/dwc_wdt.c` | 173 | 116 | 0 | 100% | BSD-2 |
| MurmurHash2 | NetBSD | `common/lib/libc/hash/murmurhash/murmurhash.c` | 117 | 70 | 0 | 100% | public domain dedication |

For contrast, the bus attachments OpenBSD keeps beside the chip drivers, which a crate leaves
behind whole: `sys/dev/pci/ahci_pci.c` 448 lines, `sdhc_pci.c` 432, `dwiic_pci.c` 344,
`if_re_pci.c` 251, `nvme_pci.c` 150; `sys/dev/fdt/sdhc_fdt.c` 483, `dwiic_fdt.c` 178,
`ahci_fdt.c` 127. `ahci.c` also sits on `sys/dev/ata/atascsi.c` (1,885 lines, ISC) and
`atascsi.h` (398), which translate SCSI to ATA; a Rust AHCI crate would take the ATA identify and
command parts and leave the SCSI translation.

FreeBSD headers read at `freebsd/freebsd-src@7806832f23fb31d28386d42692b221776c9f4ee4`:
`sys/dev/re/if_re.c` (4,295 lines, `BSD-4-Clause`), `sys/dev/sdhci/sdhci.c` (2,915, `BSD-2-Clause`),
`sys/dev/mmc/mmc.c` (2,587, `BSD-2-Clause`) and `sys/dev/ichiic/ig4_iic.c` (1,272, BSD-3, the
DragonFly Project), the Intel LPSS driver for the same DesignWare I2C block.
