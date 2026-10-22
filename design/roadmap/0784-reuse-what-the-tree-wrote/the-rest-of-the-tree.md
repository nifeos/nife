# The rest of the tree: the crates the first table did not weigh

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md).

784's main table weighs candidates for about a dozen components. This page weighs every other crate
and component in the tree by the same method and columns, and adds one: whether milestone 857
(crates released on their own) would publish a second copy of something that already exists.

## Method, and what the numbers mean

- Inventory, measured 2026-10-22 (UTC) at base `6b9b21bbc`: the 109 crates under `crates/`, the 72
  entries in `components/src/`, and `xtask`. Lines are code lines (blank and comment lines removed,
  test modules included), by script. Kani counts are `#[kani::proof]`s outside `falsifications/`.
- "Proven today" means the crate is a row in `script/verify`'s crate table. That is §46 (thin
  primitives or whole subsystems)'s test since its 2026-10-04 amendment: harnesses listed there
  because of a claim the system rests on. Harnesses alone do not exempt a crate.
- "Kernel" means `kernel/Cargo.toml` names the crate. The kernel stays exempt from the take default.
- Candidates. Each `.crate` was downloaded and its `Cargo.toml` and source read for `no_std`,
  threads and async. Downloads, licenses and releases are from the crates.io API, advisories from
  `rustsec/advisory-db` at `4b587f0` (2026-10-21), and last pushes from the GitHub API, all read
  2026-10-22. Anything else is marked *from memory*.

## Already ruled, so not weighed again

784's Follow-on settles these, and this page does not reopen them:

- (a) `http_response` takes `httparse`; (e) `uefi_loader` takes `r-efi`; (f2) `mdr` takes
  `pulldown-cmark`; (g) `xtask` takes `object`. Each is BUILT.
- (d) `talc` replaces `user_mode_heap`: *"A sounds right."*
- (f) `stick_maker`'s `plist.rs` stays: *"Keep ours."*
- (h) the four `coreutils` programs stay ours: *"A"*.
- (b) waits on 705, and (c0) waits for a `virtio-drivers` release after 0.13.0.

Ruled or owned elsewhere, and only cited below:

- the FAT crate: 813, on calef's ruling of #1803 fork 2;
- DNS: 384's `Reuse:` line and `notes/name-resolution.md`'s survey of six crates;
- the package index: 801's `Reuse:` line, which takes a TUF client in 858;
- xHCI's candidates: 785;
- §100 (the terminal font) and §166 (the rasteriser dependency, and whether the glyph atlas ships
  one face or four).

## Candidates, measured

Every row below is proven today, on the kernel's path, or both. So none falls under the take
default, and each is listed so the exemption is checked rather than assumed. The last column
answers 857: does an existing crate already do the job, so that publishing ours adds a copy?

| Ours | Lines | Kani; proven today | Untrusted input | Tier | Candidate | License; build | Downloads, last release | Advisories | Conflict with 857 |
|---|---|---|---|---|---|---|---|---|---|
| `elf` | 1,262 | 8; yes | every program image | kernel | `elf` 0.8.0 (cole14); `object` 0.40.0's reader; `goblin` 0.10.7 | MIT/Apache, `no_std`, no deps; `object` already in `xtask` | 13.2M, 2025-05; 669M, 2026-08; 80.7M, 2026-05 | none (`xmas-elf` has RUSTSEC-2025-0018) | yes: `elf` and `object` both read ELF `no_std`. 857 says contribute to `elf`, which fits |
| `device_tree_blob` | 1,837 | 4; yes | firmware | kernel | `dtoolkit` 0.4.1 (Google); `fdt` 0.1.5 | `dtoolkit` MIT/Apache, `no_std`; `fdt` MPL-2.0 | 4.9k, 2026-09; 1.5M, 2023-02 (0.2.0-alpha3 2026-10) | none | yes: `dtoolkit`. 857 says contribute, which fits |
| `machine_discovery`, its ACPI half | 2,782 of 6,644 | 8 of 16; yes | firmware | kernel | rust-osdev `acpi` 6.1.1 | MIT/Apache, `no_std`, one dep (`bit_field`). Parses MADT, MCFG, FADT, HPET and SRAT; no DMAR, IVRS or GTDT, which our IOMMU confinement reads | 1.3M, 2026-03; pushed 2026-10-18 | none | not on 857's list |
| `pci` | 1,245 | 8; yes | device | kernel | rust-osdev `pci_types` 0.10.1 | MIT/Apache, `no_std`, `bit_field` | 367k, 2026-02; last push 2026-02-03 | none | yes. 857 keeps ours internal, which avoids the copy |
| `paging` | 3,592 | 37; yes | none: our own tables | kernel | `x86_64` 0.15.5; `aarch64-paging` 0.12.2; `riscv` 0.16.1 | all MIT/Apache, `no_std`; `x86_64` ships 3 Kani harnesses | 5.7M, 2026-07; 239k, 2026-10; 10.7M, 2026-05 | none | yes, one per architecture. 857 says contribute, which fits |
| `calendar` | 1,098 | 11; yes | the x86_64 RTC, user text | kernel (`arch/x86_64/rtc.rs`) | `time` 0.3.55; `jiff` 0.2.38; `chrono` 0.4.45 | all MIT/Apache or Unlicense/MIT, `no_std` | 996M, 2026-08; 237M, 2026-10; 902M, 2026-06 | `time`: RUSTSEC-2026-0009, fixed in 0.3.47 | yes. 857 fork 8 allows bug reports only, so nothing is published |
| `network_time_protocol` | 689 | 7; yes | network | base | `sntpc` 0.11.0; `ntp-proto` 1.9.0 | `sntpc` MIT/Apache, `no_std`, `async fn get_time` over a caller's socket trait; `ntp-proto` needs `tokio` | 849k, 2026-07; 72k, 2026-06 | none | yes: `sntpc`. 857 says contribute to `ntp-proto` |
| `globally_unique_identifier_partition_table` | 2,157 | 8; yes | a stranger's disk | base | `gpt_disk_types` 0.16.1 (Google); `gptman` 3.1.1 | `gpt_disk_types` MIT/Apache, `no_std`; `gptman` std | 118k, 2025-04; 949k, 2026-04 | none | yes: `gpt_disk_types`. 857 says contribute, which fits |
| `universally_unique_identifier` | 213 | 2; yes | none on target; parsed only from `redoxfs_host`'s argument | base | `uuid` 1.28.0 | MIT/Apache, `no_std` with defaults off, no required deps; has `try_parse_ascii`, `Builder::from_random_bytes`, `from_fields_le` | 905M, 2026-10-10 | none | yes. 857 says contribute; see the proposal below |
| `glob` | 650 | 6; yes | patterns in grants and the shell | capability (`grant_plan`) | `globset` 0.4.20; `glob` 0.3.4; `wildmatch` 2.7.0 | all std, none `no_std`; `swish` and `grant_plan` are `no_std` | 268M, 2026-08; 673M, 2026-07; 23M, 2026-10 | none | yes: `wildmatch` is the same job. 857 says contribute |
| `domain_name_system` | 1,141 | 3; yes | network | base | per 384: `hickory-proto` 0.26.3, `domain` 0.12.3, `simple-dns` 0.12.0 | as 384 measured | 93.7M, 2026-09; 12.9M, 2026-09; 11.8M, 2026-07 | `domain`: RUSTSEC-2026-0310, fixed in 0.12.3; `hickory-proto`'s three do not affect 0.26.3 | yes. 857 says contribute to `domain` |
| `timetable` | 2,967 | 10; yes | a grant's schedule | capability | `cron` 0.17.0; `croner` 4.0.2 | std, both on `chrono` | 27.3M, 2026-06; 11.0M, 2026-10-21 | none | not on 857's list |
| `usb` | 653 | 3; yes | device | base (driver) | none: the maintained crates are the device side (`usb-device`, `usbd-hid`, `embassy-usb`) or a std host (`nusb` 0.2.8, `rusb`) | | | | no copy: nothing does the host side `no_std` |
| `file_allocation_table` | 512 | 0; no | a stranger's stick, once 813 reads | base | `fatfs` 0.3.6 | MIT, `no_std` only through `core_io` | 1.7M, 2023-01; pushed 2026-10-16, no release since | none | yes, `fatfs` does the job. Writing ours is ruled (813); the crate is not proven yet |

The device-logic crates on 857's publish list (814 to 820) are each in the kernel's graph, so the
take default does not reach them. Their conflict is still worth stating.

| Ours, milestone | Lines | Kani; proven today | Existing crate | Conflict with 857 |
|---|---|---|---|---|
| `non_volatile_memory_express`, 814 | 747 | 8; yes | `nvme` 0.0.0: 4.9k downloads, 7 in 90 days, one release (2025-04) | no credible copy |
| `e1000e`, 815 | 2,727 | 4; yes | `eth-intel` 0.2.4 (MIT, rcore-os `tgoskits`, first release 2026-03, 11k); `e1000-driver` 0.1.0 (GPL) | partial: `eth-intel` is a whole driver in a monorepo, not a sans-IO layer |
| `designware_ethernet`, 816 | 3,064 | 2; yes | none found | none |
| `designware_mobile_storage`, 817 | 3,462 | 4; yes | `dwmmc-host` 0.4.2 (Apache-2.0, `tgoskits`, first release 2026-05, 1.8k) | partial: same controller, a backend for another stack; `tgoskits` is closed to code offers by 857 fork 8 |
| `jh7110_entropy`, 818 | 516 | 4; yes | `jh71xx-hal` 0.7.2 (GPL-3.0-only) | none usable |
| `jh7110_clock_and_reset`, 819 | 1,241 | 0; no | `jh71xx-hal` 0.7.2 (GPL-3.0-only) | none usable; 819 must prove it before it publishes |
| `extensible_host_controller_interface`, 820 | 1,392 | 3; yes | `xhci` 0.9.2 (definitions only; last release 2023-07, last push 2024-09) | partial; 785 measures it |

Two records disagree with what was measured here:

- 857's note says `uuid` runs fuzzing, AFL and Miri in CI. Its `ci.yml` (read 2026-10-22) runs
  tests, MSRV and WebAssembly builds and nothing else. The repository has a `fuzz/` target, last
  changed 2026-06-09, that no workflow runs.
- 857's note says the DesignWare MMC crates sit in `tgoskits`. `dwmmc-host` is one, published to
  crates.io since 2026-05-18. The conclusion stands, because fork 8 closes `tgoskits` to offers.

## No credible candidate, one line each

- The ABI crates (rule 3): `abi`, `address_space_map`, `block_roster`, `boot_ladder`. Nothing
  upstream reads nife's ABI.
- The contract crates, for the same reason: `byte_sink_contract`, `capability_witness_contract`,
  `counter_frequency_contract`, `current_cpu_contract`, `domain_name_resolution_contract`,
  `entropy_contract`, `environment_contract`, `graphics_contract`, `login_contract`,
  `machine_statistics_contract`, `program_argument_contract`, `socket_contract`,
  `std_runtime_contract`, `system_log_contract`.
- The loom-checked kernel crates: `memory_regions`, `clock_contract`,
  `memory_corruption_canary_gate`, `thread_wake_handshake`, `work_steal_slot`. Kernel, exempt.
- `cpu_set`, `soak_page`, `job_mix`, `test_times`: kernel-shared constants and benchmark shapes.
- `boot_slot`, `installed_programs`, `schedule_store`, `disk_tree`: nife's own policy and formats.
- `sealed_pair`: none by design. It must hash with `measured_boot`'s SHA-256, so it cannot
  disagree with the kernel. 784's open question about that SHA-256 covers it.
- `allocation_cache`: none. `talc` has no per-thread cache, and the general allocators that do
  (mimalloc, rpmalloc) are C *(from memory)*.
- `number_writer`: `core::fmt`'s right alignment would do it, but that is no dependency, so no
  ruling, and it reads no untrusted input. Not proposed.
- `c_library_errno`, `c_seam`, `c_program_fixture`: none. The values are relibc's, per §265 (a C
  library started from relibc).
- `bitmap_font`: §100 chose an original drawing. `noto-sans-mono-bitmap` 0.3.2 is another face.
- `glyph_atlas`: its rasteriser is §166's question.
- `filesystem_fixture`, `walk_pricing`, `loaded_image_check`, `confined_syscall_fuzzer_contract`:
  test fixtures and measuring tools.
- The 45 programs in `components/src/` that 784 does not name (transports, caretakers, supervisors,
  the resolver, the time client): each is nife's capability plumbing (rule 3). The ones that parse
  hostile bytes do it through `domain_name_system`, `network_time_protocol` and `usb`, weighed above.
- `xtask`'s modules: they drive QEMU, boards and nife's own image formats. `toml` and `object` are
  already taken; nothing else fits.

## Ranked: trust gained per cost

1. Proposed: `uuid` 1.28.0 replaces `universally_unique_identifier`. It is the one crate here
   whose exemption does not hold. It is in `script/verify`, but its two harnesses prove the text
   form round trips and the version-4 stamp keeps its bits, and nothing the system rests on
   depends on either. §46's amendment says harnesses added to code that could have been taken do
   not exempt it. Cost: 213 lines retire; `globally_unique_identifier_partition_table`,
   `components` (`uuid`, `system_installer`, `disk_partitioner`), `uefi_loader`, `redoxfs_host`,
   `fixtures` and `system_tests` change their import. GPT keeps its own on-disk byte swap and its harness. The
   GPT harnesses that build a `Uuid` would then reach `uuid`'s `const fn`s over 16 bytes; that
   they still finish is unmeasured. Gain: small, and stated plainly. On target nothing parses a
   UUID; the code is a printer and a stamp, and `uuid` has 905M downloads behind it and a fuzz
   target. The case is §46's default, not a hazard. Loss: the two harnesses. New crates: one.
2. Not proposed, exemption holds: `network_time_protocol` against `sntpc`. Its claim, that no
   hostile packet moves the clock, is one the system rests on (a clock decides certificate
   validity), and `sntpc` proves nothing.
3. Not proposed, exemption holds: GPT against `gpt_disk_types`, glob against `wildmatch`, and
   `timetable` against `cron`. Each claim is a capability rule or a disk the installer writes.
   The candidates are std where our consumers are `no_std`, or carry no proof.
4. Not proposed, kernel: `elf`, `device_tree_blob`, `pci`, `paging`, `calendar` and
   `machine_discovery`'s ACPI. If the kernel's exemption were ever lifted, ACPI would lead: `acpi`
   is maintained, but it lacks the DMAR and IVRS tables our IOMMU confinement needs, so it could
   replace only part of ours.

Same cost? Yes for 1: at equal effort the exposed library wins, and the recommendation is not about
effort. For 2 and 3, keeping ours is also the cheaper path, so the reason recorded is the proof,
not the effort.

## What 857 should know

Of 857's review list, every crate except the device logic and FAT already has an existing crate
that does its job, and 857's dispositions already say contribute or internal for each. None is
headed for a rival publication. Two rows need attention at their reviews:

- FAT (813) and `jh7110_clock_and_reset` (819) are not proven today. 857's gate requires proofs
  before publishing, so neither can publish yet.
- `universally_unique_identifier` would leave 857's list if the proposal above is taken.
