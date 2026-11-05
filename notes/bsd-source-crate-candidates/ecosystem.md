# The Rust ecosystem per BSD crate candidate

The crates.io side of [the ranked survey](../bsd-source-crate-candidates.md). Every figure comes from
the crates.io API on 2026-11-05 (UTC): 90-day downloads, the date of the newest published version,
and the license that version declares. "`no_std`" means the crate lists the `no-std` category or
keyword, or its source says `#![no_std]`; "?" means neither was checked. Dependents are crates.io
reverse dependencies, counted only for the incumbents of rows that scored. The quality view is one
line from reading the crate page or repository, and it is a judgment.

| unit | closest crate | 90 days | newest | license | `no_std` | dependents | quality view |
|---|---|---|---|---|---|---|---|
| SD and eMMC card protocol | `sdio-host` | 483,688 | 2022-07-27 | MIT OR Apache-2.0 | yes | 3 | the right scope (SD and eMMC registers and commands); no commit on its default branch since that release; 8 open issues; `embassy-stm32` and two STM32 HALs depend on it |
| | `embedded-sdmmc` | 53,098 | 2026-08-10 | MIT OR Apache-2.0 | yes | 38 | active; SPI mode and FAT together, not a host-controller protocol layer |
| | `sdmmc-protocol` | 1,114 | 2026-09-18 | Apache-2.0 | yes | | young; no repository listed |
| AHCI host | `simple-ahci` | 752 | 2026-02-06 | MulanPSL-2.0 OR MIT | yes | 1 | preview release from an OS project |
| | `ahci-driver` | 693 | 2026-09-09 | Apache-2.0 | yes | 1 | lives inside rcore-os's `tgoskits` |
| DesignWare I2C master | none | | | | | | no crate under `designware-i2c`, `dw-i2c` or `dw_apb_i2c`; the RP2040 peripheral-access crates generate this IP's registers |
| SD Host Controller | `sdhci-host` | 856 | 2026-09-09 | Apache-2.0 | yes | 2 | inside `tgoskits`; vendor-specific siblings (`cv181x-sdhci`, `bcm2835-sdhci`) |
| Toeplitz hash | none | | | | | | code search finds one copy, inside `pcap-analyzer` |
| TFTP client | `async-tftp` | 14,843 | 2025-12-02 | MIT | no | 3 | server-first, async, needs `std` |
| | `tftp-rs` | 4,513 | 2026-10-04 | MIT | ? | | young |
| KVM pvclock | none | | | | | | at least six Rust kernels and VMMs carry their own (Asterinas, crosvm, Motor OS and three smaller kernels) |
| blist allocator | `bitmap-allocator` | 61,153 | 2026-06-03 | Apache-2.0 | yes | 3 | a fixed bitmap tree from rcore-os, not a blist |
| classic BPF | `classic_bpf` | 76 | 2021-08-05 | MIT | ? | 1 | dormant |
| | `cbpf-rs` | 101 | 2026-10-07 | MIT OR Apache-2.0 | ? | | one month old |
| `vis` encoding | `vis` | 12 | 2025-08-21 | MIT | ? | | a 0.0.1 placeholder |
| HID report parser | `hidreport` | 38,649 | 2026-01-22 | MIT | yes | 6 | maintained (repository active in 2026-10), `std` optional |
| | `hidparser` | 4,899 | 2026-08-03 | BSD-2-Clause-Patent | yes | 4 | Microsoft's, for UEFI; its license is not on nife's allow-list |
| | `usbd-hid` | 320,497 | 2026-09-02 | MIT OR Apache-2.0 | yes | | the device side: it builds descriptors, it does not parse them |
| SLAAC and stable addresses | `smoltcp` | 2,638,821 | 2026-08-17 | 0BSD | yes | | main carries `src/iface/slaac.rs` (578 lines); a search of it finds no RFC 7217 or temporary addresses |
| DHCPv4 client | `edge-dhcp` | 24,507 | 2026-06-25 | MIT OR Apache-2.0 | yes | 8 | active, sans-IO |
| | `dhcproto` | 233,787 | 2026-05-19 | MIT | ? | 8 | wire format only |
| PPP and PPPoE | `ppproto` | 2,797 | 2024-11-14 | MIT OR Apache-2.0 | yes | 1 | PPP over serial for embassy; no PPPoE crate exists |
| ART, LPM, Patricia, radix trees | `prefix-trie` | 19,386,722 | 2026-11-04 | MIT OR Apache-2.0 | ? | | maintained |
| | `ip_network_table` | 2,636,101 | 2021-07-12 | BSD-2-Clause | ? | | dormant but heavily used |
| | `patricia_tree` | 348,395 | 2026-09-19 | MIT | yes | | maintained |
| red-black tree | `intrusive-collections` | 8,621,568 | 2026-08-04 | MIT OR Apache-2.0 | yes | | maintained |
| concurrent trie map | `papaya` | 7,184,582 | 2026-08-21 | MIT | ? | | maintained; `scc` and `dashmap` beside it |
| extent and range allocator | `range-alloc` | 9,245,159 | 2026-02-27 | MIT OR Apache-2.0 | ? | | maintained; `vm-allocator` (rust-vmm, 705,469) too |
| BER | `der` | 152,136,811 | 2026-10-25 | Apache-2.0 OR MIT | yes | | RustCrypto; `rasn`, `bcder`, `asn1-rs` beside it |
| `imsg` | `imsg` | 42 | 2026-09-21 | MIT | ? | | OpenBSD interoperation only |
| key wrap, Blowfish, `bcrypt_pbkdf` | `aes-kw`, `blowfish`, `bcrypt-pbkdf` | 4.7M, 17.7M, 5.8M | 2026 | MIT OR Apache-2.0 | yes | | RustCrypto |
| SipHash | `siphasher` | 191,973,665 | 2026-09-25 | MIT OR Apache-2.0 | yes | | maintained |
| WireGuard | `boringtun` | 1,347,157 | 2026-05-01 | BSD-3-Clause | ? | | maintained; `gotatun` (Mullvad, MPL-2.0) and `defguard_boringtun` too |
| ISO 9660 | `iso9660-rs` | 5,846 | 2026-01-06 | MIT OR Apache-2.0 | yes | 2 | active; `iso9660` (13,729) is dormant since 2023-04 |
| NetBSD `cdb` | `cdb` | 3,385 | 2018-10-26 | Unlicense | ? | | djb's format, not NetBSD's |
| LLDP | `packet-dissector-lldp` | 1,590 | 2026-10-01 | MIT OR Apache-2.0 | ? | | decode only; `lldp` is a 0.0.0 placeholder |
| BFD | `packet-dissector-bfd` | 1,520 | 2026-10-01 | MIT OR Apache-2.0 | ? | | decode only |
| OS fingerprinting | `huginn-net-tcp` | 9,046 | 2026-10-17 | MIT OR Apache-2.0 | ? | | p0f-style, maintained |
| PL011, 16550 | `arm-pl011-uart`, `uart_16550` | 107,717, 439,441 | 2026 | MIT OR Apache-2.0 | yes | 3, 34 | maintained |
| PL031, PL061 | `arm_pl031`, `arm-pl061` | 5,832, 86,073 | 2026 | GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0; MIT OR Apache-2.0 | yes | | maintained |
| i8042 controller | `ps2` | 172 | 2021-07-13 | MIT | yes | | dormant; `pc-keyboard` (67,064, 15 dependents) decodes scancodes |
| virtio | `virtio-drivers` | 95,558 | 2026-03-03 | MIT | yes | 91 | maintained |
| MII PHY | `ieee802_3_miim` | 21,927 | 2026-08-25 | MIT OR Apache-2.0 | yes | | maintained |
| human-scaled numbers | `bytesize`, `humansize` | 24.5M, 14.3M | 2026, 2022 | Apache-2.0; MIT/Apache-2.0 | | | covered |
| RTL8169/8111 | none | | | | | | four Rust kernels carry their own driver; Redox has one outside crates.io |
| RTL8139 | `rtl8139-rs` | 0 | 2021-01-08 | MIT | yes | | dormant |
| Broadcom GENET | `genet` | 3 | 2026-07-13 | MPL-2.0 | ? | | a 0.0.1 name |
| UFS host controller | none | | | | | | code search finds one Rust microkernel with its own |
| DesignWare watchdog, FQ-CoDel, HFSC, PF_KEY, spanning tree, disklabel | none | | | | | | no crate under these names |
