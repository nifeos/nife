---
status: PARTIAL
promoted_from: reuse-what-the-tree-wrote
raised: 2026-10-04
milestone_dependencies: none
decision_dependencies: 46
machine_requirements: none
specific_machine: none
needs_person: no
---
# 784. Reuse what the tree wrote: which first-party code an existing library should replace

Written by the research lane `lane/reuse-audit` on 2026-10-04 (UTC) at base `7da1a9f29`. It answers
calef's question of the same day: under the §46 (thin primitives or whole subsystems) amendment,
*what that we already have in the tree would be a better candidate for reuse?* The ruling: outside the
kernel and the crates Kani proves, the default is to take or adapt existing code, and writing our own
needs a recorded reason. Title and slug are drafts. Every crate the table below does not weigh is
weighed in [the rest of the tree](0784-reuse-what-the-tree-wrote/the-rest-of-the-tree.md).

This is a proposal, not a decision: every replacement below adds a dependency, an architect's call
under §46. Items (a), (c0), (d), (e), (f2), (g) and (i) are built, on calef's approvals of
`httparse` (2026-10-18 UTC), `r-efi` and `pulldown-cmark` (both 2026-10-20 UTC), `talc` and
`object` (both 2026-10-21 UTC), and `uuid` and the `virtio-drivers` fork (both 2026-10-22 UTC).
Items (f) and (h) are refused.

## Method, and what the numbers mean

- Inventory. All 91 crates under `crates/`, the 58 programs in `components/src/`, `uefi_loader`,
  `redoxfs_server`, `tools/redoxfs_host` and the host tools under `xtask`. Lines are code lines (blank and
  comment lines removed, test modules included), by script; Kani counts are `#[kani::proof]`s.
- Candidates. Each `.crate` was downloaded and its `Cargo.toml` and source read; downloads,
  license and release dates from the crates.io API, advisories from `rustsec/advisory-db`, both on
  2026-10-04. Anything else is marked *from memory*.
- License. Base (needed to boot, install or fetch a package) is permissive only, and so is
  anything linked into one of our programs, per §135 (running GPL software is aggregation). A whole
  GPL program running as its own process is allowed as an `optional` package. `deny.toml` today
  allows MIT, Apache-2.0, BSD-3-Clause, 0BSD and ISC, so an MPL-2.0 crate (`fdt`, `noline`,
  `serialport`) would need a license ruling first.

## Take, adapt, fork, vendor or write

Rule 6 of the codebase rules governs. For a crate we cannot take unmodified, calef's fork test and
AI-submissions criterion (2026-10-22 UTC) are in
[take, fork, vendor or write](0784-reuse-what-the-tree-wrote/take-fork-vendor-or-write.md),
measured in [an appendix](0784-reuse-what-the-tree-wrote/upstreams-against-the-fork-test.md).

## The inventory, grouped by why it stays or goes

Kernel, or used by the kernel for more than shared constants, keep by default (rule 3 and the
amendment's exemption). The crates: `paging` (2,866 lines, 36 Kani), `capability`,
`inter_process_communication`, `machine_discovery` (5,714 lines, 16 Kani, ACPI included), `pci`,
`non_volatile_memory_express`, `direct_memory_access_validator`. Also `page_frames`, `generational_table`,
`address_space_identifier`, `intrusive_fifo`, the five loom-checked crates, the two JH7110 drivers,
`firmware_configuration`, `screen_console`, `compositor`, `measured_boot`, and the kernel's own
35,186 lines.

Kani-proved and outside the kernel, keep unless a proof is no longer worth having. The crates, with lines and Kani proofs: `calendar`
(1,089, 11), `timetable` (2,884, 10), `globally_unique_identifier_partition_table` (2,219, 9),
`elf` (1,173, 8, also kernel), `network_time_protocol` (684, 7). Also `glob` (650, 6), `capability_plan`
(1,248, 5), `subtree_scope` (384, 5), `device_tree_blob` (1,814, 4, also kernel),
`credential_contract` (322, 3), `filesystem_contract` (2,636, 3), `package_archive` (677, 2),
`capability_manifest_note` (607, 2), `nifefs` (354, 2).

The ABI, a capability, or one of the 19 `*_protocol` crates (rule 3). The crates: every `*_protocol` crate,
`abi`, `grant_plan` (6,405), `swish` (3,404 plus the 3,690-line program), `system_initializer`
(2,972), `supervision_protocol`, `hot_swap_contract`. Also `user_mode_runtime`, `system_log` (1,214), the
`process-tools` programs (`ps`, `pgrep`, `pmap`, `top`, `free`, `vmstat`, `slabtop`, `uptime`, about 1,700
together, all reading nife's own statistics ABI), `login`, `credentialer` (already takes `argon2`).

Not kernel, not proved, not protocol: the candidates. Surprise: the kernel lists
`video_terminal` and `line_editor` as dependencies, but reads only their wire constants
(`video_terminal::status`, `line_editor::proto`). Their engines are userspace code.

| Ours | Lines | Untrusted input | Tier | Candidate | License; build, and what blocks it | Downloads, last release | Advisories |
|---|---|---|---|---|---|---|---|
| `http_response` head parser | 294 (about 70 non-test in the parser) | network | base | `httparse` 1.10.1 | MIT/Apache, `no_std`, no deps | 762M, 2025-03 | none |
| `video_terminal` escape parser | 2,523 crate | every program's output | base | `vte` 0.15.0 | MIT/Apache, `no_std` (`arrayvec`, `memchr`) | 80M, 2025-02 | none |
| `virtio` + `gpu_driver` + `keyboard_driver` + `virtio_net_transport` | 715 + 303 + 215 + 251 | device | base | `virtio-drivers` 0.13.0 | MIT, `no_std` | 387k, 2026-03 | none |
| `user_mode_heap` | 272 | every allocation | base | `talc` 5.1.1 | MIT, `no_std` | 2.1M, 2026-09 | none (`linked_list_allocator` has RUSTSEC-2022-0063) |
| `uefi_loader/src/efi.rs` | 307 | firmware | base | `r-efi` 7.1.0 | MIT/Apache/LGPL, take MIT; `no_std`, no deps | 382M, 2026-08 | none |
| `stick_maker` `plist.rs` | 335 | host OS output | conveyed host program | `plist` 1.10.1 | MIT, std | 74M, 2026-09 | none |
| `portable_executable` | 778 | our own ELF | host (`xtask`) | `object` 0.40.0 `write::pe::Writer` | MIT/Apache | 640M, 2026-08 | none |
| `serial_console` `port.rs` | part of 3,418 | none | host | `rustix` or `libc` termios | MIT/Apache | *(from memory)* very wide | none checked |
| `coreutils` programs `wc`, `rm`, `printenv`, `date` | 87 + 199 + 110 + 145 | user | base | uutils `uu_*` 0.12.0 | MIT, std | 130k each, 2026-09 | none |
| `rmle` editor | 460 | user files | optional | `kibi` 0.3.3, or GNU `nano` as GPL-as-package, milestone 170 (`nano`: a real, full-featured screen editor) | MIT/Apache; std, no threads or async, but `libc` termios and raw stdin, which the PAL does not bind | 12k, 2026-02 | none |
| `file_allocation_table` (writer) | 512 | none (writes our own) | base | `fatfs` 0.3.6 | MIT; std builds with no threads or async, `no_std` only through `core_io`; `system_installer` is a `no_std` program today | 1.6M, 2023-01 | none |
| `documentation` markdown renderer (used by `mdr`) | 802 of 2,587 | shipped docs | base | `pulldown-cmark` 0.13.4 | MIT; std (`lib.rs` uses `std::fmt`), no threads, no async, deps `bitflags`, `memchr`, `unicase`; `mdr` is `no_std` today and could be a std program | 166M, 2026-05 | none |
| xHCI, unbuilt (milestone 242 (USB host and HID)) | 0 | device | base | rust-osdev `xhci` 0.9.2 | MIT/Apache | 107k, 2023-07, last push 2024-09 | none |

## Ranked: trust gained per cost

1. BUILT: `http_response` reads its head through `httparse` 1.10.1 (default features off: `no_std`,
no allocator, no dependencies, no advisory). It serves rung 3a of milestone 198 (a package manager, and the trivial install that makes a second
customer possible). `Response::parse` and its `Partial` status replace the hand-written scan and
line parser. The request writer, the 2 KiB head cap, the `Content-Length` policy (no close-delimited
body) and strict CRLF stay in the crate. The host tests and fuzz target pass unchanged. `httparse`'s
fuzzing now vouches for the grammar; the unfinished Kani harness is gone.

2. Adapt `video_terminal` to `vte` for parsing, keeping our grid. Every program that writes to a
terminal feeds this parser, so it is the widest untrusted-input surface in userspace that nothing
proves. Our parser has four states on purpose: "a VT's full state machine
has a dozen states". `vte` is that full machine (Paul Williams' DEC parser, *from memory* the one
Alacritty ships) behind a `Perform` trait our grid can implement. Cost: medium; the engine is 2,523
lines and the lane must measure how much of it is parser rather than grid before claiming a number.
Loss: none proved; two widely used crates (`arrayvec`, `memchr`) in the base graph.

3. Adapt the virtio drivers to `virtio-drivers`, one device per milestone. About 1,500 lines of
device-facing code, with no proofs, driven by bytes a device or a hostile hypervisor writes.
`virtio-drivers` covers block, net, GPU and input over MMIO and PCI ECAM and is active (last push
2026-09-25). Release 0.13.0 (2026-03-03) lacks `VIRTIO_F_ACCESS_PLATFORM`, which our PCI devices
need behind the IOMMU, so `vendor/virtio-drivers` carries it with upstream's #232, item (c0). Its
`Hal` trait is static methods, so the DMA
capability has to sit in a global. The kernel's DMA
validation is untouched. New base-graph crates: `zerocopy`, `safe-mmio`, `thiserror`, `enumn`,
`log`, `bitflags`.

4. BUILT: `talc` replaces `user_mode_heap`, item (d); see Follow-on.

5. BUILT: `uefi_loader` binds UEFI through `r-efi` 7.1.0, item (e); see Follow-on.

6. REFUSED: `stick_maker`'s `plist.rs` stays, item (f); see Follow-on.

7. BUILT: `xtask` reads ELF and writes PE through `object` 0.40.0, item (g); see Follow-on.

8. REFUSED for base: the `coreutils` programs stay ours, and uutils is a basalt package candidate,
item (h); see Follow-on.

9. Editor: `rmle` stays until one of two takes it. `kibi` (MIT/Apache, a Rust editor inspired by
the same `kilo` `rmle` is modeled on) needs `libc` termios and raw stdin, which the PAL does not
bind. GNU `nano` is the GPL-as-package path (milestone 170). `rmle` is `optional`, so either can
replace it without touching base.

9a. BUILT: `mdr` is a std program parsing with `pulldown-cmark`, item (f2); see Follow-on.

10. xHCI, when milestone 242 starts: adapt rust-osdev `xhci` for register and TRB definitions.
Write the driver logic; take the definitions. The crate is dormant (release 2023-07), so it fails the fork test and is vendored if
it needs a patch, under §34 (RedoxFS is the primary filesystem, on three conditions)'s "we must patch it" trigger.

Not ranked: `serial_console`'s `port.rs` could take `rustix` termios (host-only). DNS is surveyed
separately in pull request #1638.

## Needing std is not, by itself, a reason to refuse a crate

calef asked on 2026-10-04 (UTC) why std blocked reuse. It should not, for a component that runs as a
userspace program. nife has its own std port, the `nife-dev` toolchain with a nife PAL
(`notes/std.md`), with `std::fs` and `std::net` bound to granted capabilities and real `sleep` and
`yield`, and `ripgrep` runs on it unmodified. The real blockers are narrower, and a refusal must name
one of them:

- The component cannot take std: it is the kernel, the loader, or an early or base program built
  `no_std` that must stay so (the allocator, a driver at the bottom of the boot).
- The crate spawns threads. `thread::spawn` is `Unsupported`, declined for want of a customer by
  §105 (`std::thread::spawn` stays declined, until a customer needs it). Threads only in a test module do not count.
- The crate needs an async IO reactor. A tokio current-thread `block_on` links, but `mio`'s
  reactor needs a file descriptor and a poller, and the PAL has neither (`notes/crates-io-on-nife.md`).
- The crate needs OS APIs the PAL does not bind: processes, signals, `libc` termios, raw stdin,
  symlinks. `libc` has no `nife` module.

Rechecked under this rule, reading each crate's source for threads and runtimes:

| Crate | Serves | That component | Threads, async, OS APIs | Verdict |
|---|---|---|---|---|
| `pulldown-cmark` 0.13.4 | `mdr` | `no_std` program, could be std | none | unblocked: candidate 9a |
| `fatfs` 0.3.6 | `system_installer` | base `no_std` program | none | measured in [an appendix](0784-reuse-what-the-tree-wrote/fat-crates-measured.md), with [the options](0784-reuse-what-the-tree-wrote/fat-adopt-or-build.md); ruling pending |
| `kibi` 0.3.3 | `rmle` | `optional` program | no threads; needs `libc` termios and raw stdin | blocked on the PAL, not on std |
| `rustyline` 18.0.0 | `line_editor` | engine used by the terminal service | no threads; `libc` and `nix` termios | blocked on the PAL, and the engine sits under the console |
| uutils `uucore` 0.12.0 | `coreutils` | base programs, could be std | `std::thread` in test modules only; `libc` in its feature modules | unchanged: survey which `uu_*` build |
| `gptman` 3.1.1 | GPT | proved crate | not the issue | unchanged: kept for its 9 Kani harnesses |
| hickory-resolver 0.26.3 | DNS, milestone 384 (in a capability system the resolver is a grant) | its own program | tokio's reactor | blocked by the reactor, not by std; said on pull request #1638 |

## Running another OS's drivers unmodified

calef asked that this path be noted. Read for this proposal:

- NetBSD rump kernels run NetBSD drivers unmodified in userspace, including on L4 through Genode
  (Wikipedia, "Rump kernel"). The `rumpkernel/rumprun` repository was last pushed 2020-05-11 and lists
  x86 and ARM, not aarch64 or riscv64; a platform supplies the `rumpuser` hypercall interface. NetBSD
  source is BSD-licensed *(from memory)*, so rump-hosted drivers could be base.
- DDE / DDEKit (TU Dresden) maps the in-kernel Linux driver interface onto a host system so Linux
  drivers run unmodified in userspace, originally on L4. Genode's release notes show `dde_linux` kept
  current, to Linux 6.18 in 26.02. Those drivers are GPL, so under §135 they can be packages only:
  acceptable for, say, a Wi-Fi card an installed system adds, never for a boot or install driver.

Either means a C toolchain and a large shim. Survey further only when a driver nobody has in Rust is
on the customer path.

## The seven questions, for the top five

1. Considered and lost. `uefi` lost to `r-efi` (`uefi` is 23k lines, ours needs the bindings
   only). `fatfs` waits because `system_installer` is `no_std` and the crate has not released since
   2023-01. `pulldown-cmark` no longer loses: needing std was the wrong test. `noline`, `fdt` and `serialport` lost on
   license (MPL-2.0, not in `deny.toml`). `goblin` and `xmas-elf` lost to our Kani-proved `elf`
   (`xmas-elf` has RUSTSEC-2025-0018, an out-of-bounds read on a malformed ELF).
2. What the tree already does. `smoltcp` is taken whole and unpatched, and argon2 and RustCrypto
   are ordinary dependencies; `httparse`, `vte` and `r-efi` fit that shape. `virtio-drivers` will
   too, once a release carries #232.
3. Prior art. In the tables above.
4. Is the premise true? Mostly; the kernel's exemption does not cover the two engines (above).
5. Cost. Lines retired are measured above; lane time is estimated. Candidate 3 owes throughput numbers
   before its ruling; candidate 4's heap numbers are in [the appendix](0784-reuse-what-the-tree-wrote/measured-for-the-rulings.md).
6. Reversibility. Each replacement is reversible code; adding a crate to the base graph is §46's
   expensive category, and each was ruled before it was built.
7. Same cost? Yes for 1, 2, 3, 5 and 6: at equal effort the exposed library wins on trust. Nothing
   here is recommended because it is less work.

## Proposed milestones, one replacement each

- (a) BUILT: `http_response` parses its head with `httparse`. Gate: existing host tests and fuzz target.
- (b) Held for milestone 705 (full-screen programs)'s engine choice (calef, 2026-10-18 UTC): a full engine has its own
  parser. Else `video_terminal` parses with `vte`; gate: host tests, new fuzz target.
- (c0) BUILT: `vendor/virtio-drivers` carries 0.13.0 plus #232 until a release includes it. calef,
  2026-10-22 21:58 (UTC), on #2586: *"A"*, once
  [a probe](0784-reuse-what-the-tree-wrote/virtio-drivers-probe.md) showed #232 works for the input
  device behind the IOMMU. It did, on all three.
- (c1) input, (c2) GPU, (c3) block, (c4) net, one device per milestone, each with a `script/bench`
  number before and after on all three architectures.
- (d) BUILT: `talc` replaces `user_mode_heap`, in `memory_region_heap` and std.
  Evidence: [an appendix](0784-reuse-what-the-tree-wrote/measured-for-the-rulings.md).
- (e) BUILT: `uefi_loader` binds UEFI through `r-efi`.
- (f) REFUSED: `stick_maker` reads `diskutil` with its own `plist.rs`, which stays.
- (f2) BUILT: `mdr` is a std program and parses with `pulldown-cmark`; the renderer and index stay.
- (g) BUILT: `xtask` takes `object` for the ELF and the PE; the conversion stays ours.
- (h) REFUSED for base: the four `coreutils` programs stay ours.
- (i) BUILT: `uuid` 1.28.0 replaces `universally_unique_identifier`.

## Recorded reasons for what stays

Each component that stays has a `Reuse:` line ready to paste, to be conformed to the amendment's
landed wording: [recorded-reasons.md](0784-reuse-what-the-tree-wrote/recorded-reasons.md).

## The measured-boot SHA-256 stays ours

calef, 2026-10-22 (UTC), on #2548: *"Keep our SHA-256."* `measured_boot` keeps its hand-written
SHA-256, checked against the FIPS 180-4 vectors. For it, §46's rule 3 (the kernel writes) wins over
rule 4 (crypto is taken), so `sha2`, an ordinary dependency of `cryptography_provider`, stays out of
the measured-boot trusted base.

## Follow-on

- **Done.** Item (a), 2026-10-18 (UTC), nifeos/nife#2289.
- **Done.** Item (e). calef, 2026-10-20 (UTC): "Yes", taking `r-efi` 7.1.0 under MIT (§46). Two
  gaps stay local, said at their call sites: RISC-V's boot protocol, which `r-efi` lacks, and
  `block_io::Media`'s flags, declared `bool` there and read as bytes.
- **Done.** Item (f), refused. calef, 2026-10-20 (UTC): "Keep ours." `plist` would bring nine crates
  new to the tree, `serde` among them, to read trusted local `diskutil` output in a host-only
  program. `plist.rs` carries the `Reuse:` line, and so do the recorded reasons.
- **Done.** Item (f2). calef, 2026-10-20 (UTC): "Yes" to `pulldown-cmark` 0.13.4 with `html` and
  `getopts` off, and to `unicase` and `bitflags` 2 (§46). Then, 16:53 UTC on #2461, "a": a std
  program's stdin is a byte stream when the line redirects or pipes one, at
  `std_runtime_contract::STDIN_STREAM_SLOT`, built here as (f2)'s prerequisite. `mdr` is a std
  program in `mdr/`, and `crates/documentation` lays out the parser's events.
- **Done.** Item (d). calef, 2026-10-21 (UTC), on #2490: "A sounds right." `talc` 5.1.1 is the heap
  everywhere, carrying SFBdragon/talc#57 until a release does (#2498).
- **Done.** Item (g), 2026-10-21 (UTC), nifeos/nife#2497, on calef's "B" on #2490. `object`
  0.40.0 does the ELF and PE work in `xtask/src/portable_executable.rs`; our policy stays. Gate:
  `cargo xtask stick-boot`, all three firmwares. Milestone 821 (`portable_executable`: an
  ELF-to-PE converter, proven, then released on its own) is refused.
- **Done.** Item (h), refused. calef, 2026-10-21 (UTC), on #2490: "A". uutils is a basalt package
  candidate under §297 (software reaches a nife system as a package `jig` installs from basalt's
  index).
- **Done.** Item (i), nifeos/nife#2564, on calef's "A" on #2548.
- **Outstanding.** (b) waits on 705, NOT-STARTED (checked 2026-10-21 UTC).
- **Done.** Item (c0), nifeos/nife#2587, on calef's "A" on #2586.
- **Outstanding.** (c1) to (c4): no pull request names them (checked 2026-10-22 UTC).

## Index row

Under the 2026-10-04 amendment to §46, taking existing code is the default outside the kernel and the crates Kani proves. This survey names which first-party code an existing library should replace, with measured candidates, licenses and advisories, and each replacement waits on its own dependency ruling.
