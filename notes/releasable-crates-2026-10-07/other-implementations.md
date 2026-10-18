# The other implementations, 2026-10-07

Appendix to [the note](../releasable-crates-2026-10-07.md). Looked up, not measured, on 2026-10-07
(UTC) from the crates.io API and the GitHub API. Three read-only survey agents under
lane/releasable-crates did the lookups, and no project was contacted. Downloads are all-time and "rev" is crates.io's
reverse-dependency count. "Outside" counts pull requests merged since 2026-04-06 as outside/total.
An outside author is one GitHub does not mark as owner, member or collaborator, so the count
cannot tell a newcomer from a regular. "Proofs" is a path and workflow search, not a read of CI. "Not checked" means exactly that.

AI policy: CONTRIBUTING, README and AGENTS.md were searched in every GitHub repository below.
Two projects refuse AI-generated contributions: Redox and embedded-sdmmc, quoted under their
headings. `time` wants AI use disclosed, every message to its maintainers written by a person, and
no LLM as a co-author. `jiff` and `globset` welcome AI tools and want human-written comments.
rcore-os's `tgoskits` asks for no agent branding in commits, which collides with this tree's rule
that an agent's pull request says so. The rest are in the last section.

## calendar (11 harnesses)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| time | time-rs | 0.3.55, 2026-08-01 | MIT OR Apache-2.0 | 951M / 6,465 | none found | 6/6 |
| chrono | chronotope | 0.4.45, 2026-06-04 | MIT OR Apache-2.0 | 858M / 37,032 | fuzz | 11/14 |
| jiff | BurntSushi | 0.2.38, 2026-10-06 | Unlicense OR MIT | 215M / 1,337 | fuzz | 17/51 |
| hifitime | nyx-space | 4.3.1, 2026-08-07 | MPL-2.0 | 1.27M / 63 | Kani in CI, weekly | 18/24 |

## device_tree_blob (4)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| fdt | repnop | 0.1.5, 2026-10-01 | MPL-2.0 | 1.43M / 6 | none found | 6/6, one author |
| dtoolkit | Google | 0.4.1, 2026-09-25 | Apache-2.0 OR MIT | 4.7k / 2 | fuzz | 5/50, CONTRIBUTING |
| vm-fdt | rust-vmm | 0.3.0, 2023-11-15 | Apache-2.0 OR BSD-3-Clause | 2.86M / 13 | none found | 0/0 |
| fdt-rs | rs-embedded | 0.4.5, 2024-02-25 | MIT | 113k / 1 | none found | dormant |

## domain_name_system (3)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| hickory-proto | hickory-dns | 0.26.3, 2026-09-10 | MIT OR Apache-2.0 | 88M / 135 | fuzz | 29/290, CONTRIBUTING |
| domain | NLnet Labs | 0.12.3, 2026-09-25 | BSD-3-Clause | 12.6M / 41 | none found | 49/109, CONTRIBUTING |
| simple-dns | balliegojr | 0.12.0, 2026-07-26 | MIT | 11M / 28 | fuzz | 0/9 |
| dns-parser | tailhook | 0.8.0, 2018-08-06 | MIT or Apache-2.0 | 5.2M / 32 | none found | abandoned |

## elf (8)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| object | gimli-rs | 0.40.0, 2026-08-01 | Apache-2.0 OR MIT | 644M / 472 | none found | 100/107 |
| goblin | m4b | 0.10.7, 2026-05-28 | MIT | 76.8M / 369 | fuzz | 11/11 |
| elf | cole14 | 0.8.0, 2025-05-14 | MIT or Apache-2.0 | 12.6M / 90 | fuzz in CI | 0/0, quiet |
| xmas-elf | nrc | 0.10.0, 2025-03-26 | Apache-2.0 OR MIT | 3.08M / 121 | none found | dormant |

## network_time_protocol (7)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| ntp-proto | Pendulum (ntpd-rs, Trifecta Tech) | 1.9.0, 2026-06-12 | Apache-2.0 OR MIT | 68k / 6 | fuzz in CI | 34/144, CONTRIBUTING |
| sntpc | vpetrigo | 0.11.0, 2026-07-06 | MIT OR Apache-2.0 | 822k / 10 | none found | 4/23, CONTRIBUTING |
| ntp-parser | rusticata | 0.6.0, 2021-09-13 | MIT or Apache-2.0 | 1.3M / 1 | none found | 0/0 |

`ntp-proto` is `std`; `sntpc` is `no_std`.

## glob (6), generational_table (4), intrusive_fifo (1), http_response (0)

| implementation | for | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|---|
| glob | glob | rust-lang | 0.3.4, 2026-07-21 | MIT OR Apache-2.0 | 641M / 4,022 | none found | 2/4 |
| globset | glob | BurntSushi | 0.4.20, 2026-08-04 | Unlicense OR MIT | 253M / 1,694 | fuzz | 13/26 |
| wildmatch | glob | becheran | 2.6.1, 2025-11-14 | MIT | 21.9M / 109 | none found | 2/2, CONTRIBUTING |
| slotmap | table | orlp | 1.1.1, 2025-12-06 | Zlib | 112M / 514 | fuzz | 2/4 |
| thunderdome | table | LPGhatguy | 0.6.1, 2023-06-25 | MIT OR Apache-2.0 | 1.10M / 32 | none found | 10/10 |
| generational-arena | table | fitzgen | 0.2.9, 2023-05-22 | MPL-2.0 | 10.2M / 57 | none | archived |
| intrusive-collections | fifo | Amanieu | 0.10.3, 2026-08-04 | MIT OR Apache-2.0 | 21.7M / 45 | none found | 9/10 |
| cordyceps | fifo | hawkw | 0.3.5, 2026-07-29 | MIT | 7.6M / 7 | none found | 1/2 |
| httparse | http | seanmonstar | 1.10.1, 2025-03-03 | MIT OR Apache-2.0 | 768M / 568 | fuzz, OSS-Fuzz | 2/2 |

## paging (37 harnesses, 41 falsification patches), page_frames (5)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| x86_64 | rust-osdev | 0.16.0-rc.0, 2026-09-24 | MIT or Apache-2.0 | 5.47M / 86 | Kani in CI (`kani-github-action`), harnesses in `addr.rs` and `paging/` | 5/16 |
| aarch64-paging | Google | 0.12.1, 2026-04-02 | MIT OR Apache-2.0 | 237k / 2 | none | 9/9, CONTRIBUTING |
| riscv | rust-embedded | 0.16.1, 2026-05-28 | MIT OR Apache-2.0 | 10.3M / 292 | none found | 24/24 |
| page_table_multiarch | arceos-org | 0.6.1, 2026-04-02 | GPL-3.0+ OR Apache-2.0 OR MulanPSL-2.0 | 115k / 3 | none | 0/0 |
| buddy_system_allocator | rcore-os | 0.13.0, 2026-03-30 | MIT | 3.35M / 20 | none | 1/1 |
| bitmap-allocator | rcore-os | 0.4.6, 2026-06-03 | Apache-2.0 | 153k / 3 | none | 4/4 |

`x86_64` is the only repository in this survey that already runs Kani on code like ours.

## pci (8), extensible_host_controller_interface (3), usb (3)

| implementation | for | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|---|
| pci_types | pci | rust-osdev | 0.10.1, 2026-02-03 | MIT or Apache-2.0 | 347k / 11 | none | 0/0 |
| virtio-drivers | pci | rcore-os | 0.13.0, 2026-03-03 | MIT | 390k / 91 | none | 22/23 |
| xhci | xhci | rust-osdev | 0.9.2, 2023-07-19 | MIT OR Apache-2.0 | 108k / 2 | none | dormant since 2024-09 |
| crab-usb | xhci | rcore-os (`tgoskits`) | 0.12.1, 2026-09-11 | Apache-2.0 | 28k / 2 | none found | monorepo |
| usb-device | usb | rust-embedded-community | 0.3.2, 2024-03-06 | MIT | 4.98M / 129 | none | 1/3 |
| usbd-hid | usb | twitchyliquid64 | 0.10.2, 2026-09-02 | MIT OR Apache-2.0 | 1.45M / 20 | none | 7/7 |
| nusb | usb | kevinmehall | 0.2.7, 2026-08-03 | Apache-2.0 OR MIT | 1.62M / 148 | fuzz | 5/22 |
| embassy-usb | usb | embassy-rs | 0.6.0, 2026-03-20 | MIT OR Apache-2.0 | 1.23M / 39 | none found | CONTRIBUTING |

`usb-device` and `embassy-usb` are the device side of the bus; this tree's `usb` is the host side.

## Device logic: NVMe, e1000e, DesignWare, JH7110

| implementation | for | owner | release | license | downloads / rev | proofs, fuzz |
|---|---|---|---|---|---|---|
| nvme-driver | NVMe | rcore-os (`tgoskits`) | 0.8.2, 2026-09-09 | MIT | 4.9k / 1 | none found |
| nvme | NVMe | lihanrui2913 | 0.0.0, 2025-04-11 | MIT or Apache-2.0 | 4.9k / 0 | not checked |
| eth-intel | e1000e | rcore-os (`tgoskits`) | 0.2.4, 2026-09-11 | MIT | 10.8k / 1 | none found |
| e1000-driver | e1000 | elliott10 | 0.1.0, 2023-02-24 | GPL-2.0 | 1.9k / 0 | none |
| dwmac-my | DW Ethernet | elliott10 | 0.2.0, 2026-02-12 | MIT | 35 / 0 | none found |
| dwmmc-host | DW MMC | rcore-os (`tgoskits`) | 0.4.2, 2026-09-09 | Apache-2.0 | 1.7k / 2 | none found |
| starfive-jh7110-dwmmc | DW MMC | rcore-os (`tgoskits`) | 0.1.8, 2026-09-09 | Apache-2.0 | 911 / 1 | none found |
| sdmmc | SD/MMC | drivercraft | 0.1.0, 2026-01-28 | GPL-3.0+ OR Apache-2.0 OR MIT | 11k / 2 | none found |
| jh71xx-pac, jh71xx-hal | JH7110 | weathered-steel (Codeberg) | 0.11.1 and 0.7.2, 2025-03 | GPL-3.0-only | 27k and 15k | not checked |

Redox's `nvmed`, `xhcid` and `e1000d` are daemons, not libraries, and their GitHub mirror is
archived. SPDK (C) is the NVMe reference and Linux's `stmmac` and `dw_mmc` (GPL-2.0) are the
DesignWare references. No Rust crate for the JH7110's TRNG or clock and reset logic was found.
`tgoskits` merged 1,606 pull requests in six months, 35 from outside authors.

## FAT (the crate ruled on 2026-10-07; `file_allocation_table` today writes one ESP)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| fatfs (rust-fatfs) | rafalh | 0.3.6, 2023-01-17 | MIT | 1.61M / 57 | none; CI builds `no_std` | fixes merged 2024-11 and 2026-03, none released |
| lamfat | lamco-admin | 0.4.2, 2026-06-08 | MIT | 16k / 0 | none found | none in window |
| embedded-sdmmc | rust-embedded-community | 0.10.0, 2026-08-10 | MIT OR Apache-2.0 | 373k / 36 | none | 7/8; refuses AI-generated contributions |
| hadris-fat | hxyulin | 3.0.0-rc.1, 2026-10-06 | MIT | 7.9k / 7 | fuzz, Miri in CI | 0/30, owner only |
| simple-fatfs | Oakchris1955 | 0.1.0-alpha.2, 2025-09-14 | MIT | 1.7k / 0 | none; a test workflow | 2/7, CONTRIBUTING, no AI policy |

rust-fatfs's `master` is 163 commits past v0.3.6. Issue #81 ("crates.io 0.4.0 release", opened
2023-03-17) has thirteen comments asking for a release through 2026-04 and no maintainer reply
found. In #110 (2026-03-03) the maintainer wrote "I want to release soon". embedded-sdmmc's
CONTRIBUTING: "This project does not take contributions substantially or wholly generated by AI."
simple-fatfs, recommended by a commenter on rust-fatfs #81, was added 2026-10-07 at calef's
request. Its README says it was started because rust-fatfs needs buffered I/O unsuited to embedded
use, and it claims `no_std` and FAT12/16/32, with exFAT unchecked on its own list. It warns that
writing a file already open can corrupt data (its issue #14). Last push 2026-08-30, 56 stars.

## globally_unique_identifier_partition_table (8), universally_unique_identifier (2)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| gpt_disk_types, gpt_disk_io, uguid | Google (`gpt-disk-rs`) | 0.16.1 / 0.17.0 / 2.2.1, 2025-04 to 2026-10 | MIT OR Apache-2.0 | 115k / 99k / 2.04M | none found | 4/8, CONTRIBUTING |
| gpt | Quyzi | 4.1.0, 2025-03-16 | MIT | 2.07M / 26 | none | 1 |
| gptman | rust-disk-partition-management | 3.1.1, 2026-04-28 | MIT OR Apache-2.0 | 920k / 5 | none | 2, CONTRIBUTING |
| uuid | uuid-rs | 1.27.0, 2026-10-02 | Apache-2.0 OR MIT | 857M / 21,731 | fuzz, AFL and Miri in CI | 11/27, CONTRIBUTING |

## portable_executable (0)

No ELF-to-PE converter crate exists on crates.io. systemd's `tools/elf2efi.py` (LGPL-2.1+) takes
the same approach, translating a static PIE's relocations. `object` has a PE writer but no
converter; Rust's `*-unknown-uefi` targets emit PE directly.

## cryptography_provider (0)

| implementation | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|
| rustls-rustcrypto | RustCrypto | 0.0.2-alpha, 2024-04-24 | MIT OR Apache-2.0 | 216k / 35 | none | 15, mostly dependency bumps; 19 open, three from September unmerged |
| rustls-graviola | ctz | 0.4.0, 2026-06-17 | Apache-2.0 OR ISC OR MIT-0 | 180k / 21 | fuzz, ctgrind, reused verified assembly | 16/30 |
| rustls-mbedcrypto-provider | fortanix | 0.1.1, 2025-07-01 | MPL-2.0 | 55k / 0 | none | 7 |

rustls-rustcrypto's README: "USE THIS AT YOUR OWN RISK! DO NOT USE THIS IN PRODUCTION".

## video_terminal (0), line_editor (0)

| implementation | for | owner | release | license | downloads / rev | proofs, fuzz | outside |
|---|---|---|---|---|---|---|---|
| vte | terminal | alacritty | 0.15.0, 2025-02-02 | Apache-2.0 OR MIT | 80.9M / 161 | none found | 0; a parser, no grid |
| vt100 | terminal | doy | 0.16.2, 2025-07-12 | MIT | 13.1M / 265 | fuzz | 0 |
| termwiz | terminal | wezterm | 0.23.3, 2025-03-20 | MIT | 25.4M / 46 | none found | CONTRIBUTING |
| noline | line editor | rustne-kretser | 0.5.1, 2024-12-12 | MPL-2.0 | 37k / 1 | none | 2, `no_std` |
| embedded-cli | line editor | funbiscuit | 0.2.1, 2024-03-02 | MIT OR Apache-2.0 | 55k / 0 | none | 2 |

## RedoxFS (vendored, two patches)

`redoxfs` 0.9.1 was released 2026-07-01, and `master` was at `b87b0976` on 2026-09-07. Its `no_std`
build is still broken there: a survey agent ran `cargo check --no-default-features --target
aarch64-unknown-none-softfloat` on a scratch clone and got four errors, `Box` and `Vec` not in
scope, the same class this tree's `redoxfs-no-std-vec-import.patch` fixes. Redox's CONTRIBUTING,
section "AI Policy": "Redox OS does not accept contributions generated by LLMs".

## Other stated AI policies

uefi-rs: AI help is disclosed in the commit or pull request, and a person must understand the code.
rustls: AI coding tools are welcome; AI-written comments to maintainers may be hidden. systemd: AI
tools allowed, never as a co-author, and contributors speak for themselves.
