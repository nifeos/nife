---
status: PARTIAL
raised: 2026-08-30
milestone_dependencies: 801, 802
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 198. A package manager

Ruled 2026-10-06 (UTC) by calef, option B: this milestone is PARTIAL, the umbrella over milestone
801 (packages over the internet), rung 3c, and milestone 802 (the trivial install), rungs 1d and 4,
and closes when both do. It keeps the top slot of principle 1's customer path. Its scope is a
package manager, and the trivial install that makes a second customer possible. Rung 3a, the
package manager, is complete. The rung table below maps every rung to its milestone.

**Outstanding** (checked against the tree 2026-10-24 UTC): nothing inside this block's own rung.
Two milestones keep it PARTIAL. Milestone 801 (packages over the internet) has one open criterion,
the run on xenon's installed system, which waits on milestone 494's bench boot and a fresh install.
Milestone 802 (the trivial install) waits on milestones 242, 400, 515, 801 and 809 and on calef's
act of publication. Through 802 it also waits on milestone 809 (`jig`). Its items 7 and 8 wait
on milestone 849 (a package declares what it needs at run time), NOT-STARTED, and on an unproposed
milestone for §306 (the web's authority list is a package and a capability). Each of those is calef's hands or
a lane of its own; none is work this block can do.

Under §159 (only a new kernel needs a reboot), a package update needs no reboot; a kernel update
does.

Rung 3a's producer half is the one archive file §197 ruled a package is (`crates/package_archive`,
written and read by one definition). `cargo xtask package` turns a reviewed recipe into a package,
its digest and a catalog line. And the fuzz target and Kani harnesses §197 accepted are the
container's price. Rung 3a's consumer half is built: the image carries a package catalog, a host
serves the package over plain HTTP, and `net_stack`'s client accepts it only by the image's digest,
on aarch64 and riscv64. The table §208 (installing is granting) versions is in
`crates/installed_programs`. Run by digest and the installer are built under §219 (how the shell names
an installed program to the spawner): `jig install`, `remove` and `rollback` at the prompt (milestone
809), across a reboot, on all three architectures. `jig install <name>` fetches over the booted
system's network on all three (x86_64 over the `e1000e`), and `greeting`, which no image carries,
runs on all three. So does §219's gate D2: a session holding the run-unvouched capability runs bytes
nobody vouched for. §221 (the boot prompt is the owner's console) adds `vouch`, and the owner's list
of who may run new code. notes/packages.md has the account.

[§219](../decisions/0219-naming-an-installed-program-to-the-spawner.md) is decided (2026-09-26 UTC):
option D with gate D2, all built. Every earlier fork is ruled:
[§195](../decisions/0195-a-recipe-vouches-and-the-owner-may-overrule.md),
[§197](../decisions/0197-a-package-is-one-archive-file.md),
[§208](../decisions/0208-installing-is-granting.md) and §196 (nife carries TLS). The rulings of §157
(a trivial install is a web page, a USB drive, and packages over the internet) gate later rungs,
each on a milestone of its own. The install layout waits on milestone 515 (rung 2a). Secure Boot
waits on milestone 500 (a stick that boots with Secure Boot on) for rungs 1d and 4. And publication
is an architect's act at rung 4.

In brief, from calef on 2026-08-30: *"I don't think we expose nife to third parties (aka other
customers) until we have a package manager and a trivial install process."* And, in the same breath,
that he wants it early, to make our own lives easier. Why that puts this block ahead of the customer
path, and the prior art it builds on, is in [why-early.md](0198-package-manager/why-early.md).

## What "trivial install" has to mean

Milestone 802 (the trivial install) has it, with §157's definition.

## The rungs to a trivial install, under §157

§157's definition maps onto rungs, each shipping something on its own.

Every rung is proved on xenon first and on a second PC after. xenon is the only PC on the bench;
milestone 243's fleet (the family's other x86 machines) is what turns a claim about one Dell into a
claim about PCs, and each rung's last exit criterion is that second machine.

### The rungs

| Rung | Ships | Exit criterion a stranger could check | Milestones and proposals on it |
|---|---|---|---|
| **1a. A prompt from a stick, on xenon, over serial** | Nothing new: the pieces are built | xenon boots `\EFI\BOOT\BOOTX64.EFI` from a FAT32 stick, `$` appears on the serial console, and `echo hello` answers; transcript filed under `bench/` | **BUILT 2026-10-21** (`bench/xenon-2026-10-21/boot-to-prompt.log`: from the stick, `echo hello` answers on serial); 87, 299, 182 and 243, all BUILT |
| **1b. The prompt on the screen** | A pre-set-framebuffer driver behind the framebuffer contract | Under OVMF, `serial_console::screen` reads `$` back off the framebuffer; on xenon, the prompt is on the monitor and a serial keystroke echoes there | [400](0400-the-shell-on-the-firmware-screen.md) (PARTIAL: built and gated under OVMF, xenon outstanding; found by this lane); shares its driver with 157 |
| **1c. A USB keyboard** | xHCI, enumeration, HID boot protocol | xenon with a monitor and a USB keyboard, no serial cable: `echo hello` | 242 (PARTIAL), which closes 192 (PARTIAL) |
| **1d. A PC that is not xenon** | Nothing new if 1a to 1c hold | The same stick on one fleet machine reaches `$` at its own keyboard and monitor | [milestone 802](0802-the-trivial-install.md), with 243's fleet |
| **2a. Installed onto a disk, under QEMU** | An installer; the boot mounting the nife partition off NVMe | OVMF boots the stick image with an empty NVMe attached; the installer names the disk, asks, partitions, formats and copies; the machine reboots **with the stick detached**, reaches `$`, and reads back a file written before the reboot. One `cargo xtask` gate | **BUILT 2026-09-21** (#1056, `notes/installing.md`; re-run green 2026-10-03); [milestone 515](0515-the-installer-a-stick-runs-to-put-itself-on-the-disk.md) is PARTIAL on the bench half |
| **2b. Installed onto xenon's disk** | The bench half | The 2a sequence on xenon's Micron 2450, photographed, stick removed before the second boot | **BUILT 2026-10-04** (`bench/xenon-2026-10-04/`; needs #1636; [milestone 515](0515-the-installer-a-stick-runs-to-put-itself-on-the-disk.md) criterion 2 has the caveats) |
| **3a. A package over the LAN, under QEMU** | The package client this milestone is; a host-side recipe that produces a package; a small HTTP client | A package absent from the image is fetched from a host on the same network over plain HTTP, verified by digest, installed onto the running system, run, still present after a reboot, and removed | this block; all three rulings it needed are in (§195, §197, §208). **Producer half BUILT 2026-09-23** (`crates/package_archive`, `cargo xtask package`, `packages/uptime.recipe.toml`, notes/packages.md). Fetch and verify built 2026-09-24 (`helpers/package-http-peer`, `crates/http_response`, a QEMU test on aarch64 and riscv64); **run by digest and the installer built 2026-09-26** on all three architectures; x86_64 fetches since 2026-10-05. No TLS, since §195's digest decides whether bytes may run |
| **3b. The network card xenon has** | An Intel I219 (`e1000e` family) driver in 261's shape | Under QEMU `-device e1000e` behind `intel-iommu`, milestone 30 (the network stack as a confined component)'s DHCP and TCP gates pass through the new driver; on xenon, a lease from the house router and a measured transfer | [milestone 494](0494-a-driver-for-the-network-card-a-pc-actually-has.md) (PARTIAL: built under QEMU, xenon's bench boot outstanding) |
| **3c. Over the internet** | Name resolution; HTTPS; §250's index | From xenon's installed system, a package fetched through `basalt.nifeos.org`'s index by host name, verified and installed | [milestone 801](0801-packages-over-the-internet.md) |
| **4. The web page** | A published release and a page | A stranger with a PC, a USB stick and no prior knowledge follows the page to rung 3c's result; the stranger harness (`notes/stranger-test.md`) runs against the **download**, not the build | [milestone 802](0802-the-trivial-install.md); calef's act |

### Order, and which rungs are too big

Recommended order (reversible): 1a, then 2a and 3a in parallel lanes (they touch disjoint
subsystems: the installer and boot mount, and the package client), then 1b, 2b, 3b, 3c, with 1c
running beside all of them, then 1d, then 4.

- Rung 1 is too large for "a milestone or two", and one piece is why: 1c is milestone 242, which
  milestone 192 prices at *"months rather than weeks"* and 242 itself declines to price. It is split
  on purpose so that 2 and 3 do not wait on it: on xenon, the installer and the package client
  can be driven over the serial console. 1c is on rung 4's critical path and on nothing else's.
- 1a is one attended boot, and it should capture two more facts while it is there. The PCI
  survey now prints every function (`kernel/src/pci.rs`, since 2026-09-18), which names xenon's
  network card for 3b. And the `vt-d` line's DMAR scope, which milestone 261 (the NVMe driver
  leaves the kernel, on the machine that can finally confine it) names as its
  load-bearing unknown.
- Rung 2 is one milestone plus 261's bench step. The partitioner and `mkfs` exist; what is new
  is a layout sized to the disk, the loader handing its own file over, the ESP, and the boot mount.
- Rung 3 is three pieces and 3a is the only one that is this milestone. 3b is the NIC; 3c is the
  resolver, TLS and §250's index. 3a needed no new driver and no TLS.
- Trust T1 cannot serve rung 3: under T1 installing is rebuilding the image on a host, which a
  stranger cannot do. Rung 3 needs T2 or T3.

### Which rulings each rung needs

One line each, in the form calef would answer, with the rung that waits on it.

| Ruling | The question | Rung it blocks | Where the options are |
|---|---|---|---|
| ~~**Format**~~ | **Decided 2026-09-20. DECISIONS §197 (a package is one archive file) rules one archive file per package**, identified by name and version, with the reviewed recipe of §195 (a reviewed recipe vouches for a package) carrying its digest. The manifest travels in an ELF note (ruled 2026-09-26); the digest is a plain SHA-256 written `sha256:<hex>` (ruled 2026-10-07). | 3a | [DECISIONS §197](../decisions/0197-a-package-is-one-archive-file.md) |
| ~~**Activation**~~ | **Decided 2026-09-23 (DECISIONS §208): A3 with rollback.** Installing records that a package exists (digest and manifest spawnable, data a read-only directory a session binds by name), nothing is written into shared space, and the table of entries is versioned so a set rolls back whole. | 3a's **consumer** half | [DECISIONS §208](../decisions/0208-installing-is-granting.md), from [milestone 507](0507-installing-a-package-mutates-or-composes.md) |
| ~~**Trust**~~ | **Decided 2026-09-19 (DECISIONS §195): a reviewed recipe vouches, trust is scoped per source the owner opted into, and the owner may overrule.** No long-lived signing key is held for now; a per-source signature can be added later without changing that. | 3a | [DECISIONS §195](../decisions/0195-a-recipe-vouches-and-the-owner-may-overrule.md) |
| ~~**Install layout** (new)~~ | **Decided 2026-10-03 (DECISIONS §244 (the installed disk has four partitions, and a boot slot is 64 MiB)): four partitions**, release images only, under a 16 MiB CI budget. | 4 | [DECISIONS §244](../decisions/0244-the-installed-disk-has-four-partitions.md), from [515's appendix](0515-the-installer-a-stick-runs-to-put-itself-on-the-disk/the-layout-ruling.md) |
| ~~**Transport** (new)~~ | **Decided 2026-09-19 (DECISIONS §196): HTTPS, `rustls` for the protocol, and the crypto provider is milestone 442's work.** Under §195 a recipe's digest decides what may run, so rung 3a does not wait for TLS. | 3c (not 3a) | [DECISIONS §196](../decisions/0196-nife-carries-tls-and-builds-the-provider.md) |
| ~~**Hosting** (new)~~ | **Decided 2026-10-06 (DECISIONS §250 (an image names its distribution's package index, and the bytes may live anywhere)):** basalt's index at `basalt.nifeos.org`, bytes anywhere. | 3c and 4 | [DECISIONS §250](../decisions/0250-images-name-one-package-index-and-the-bytes-may-live-anywhere.md) |

Not a ruling but calef's hands: the wipe of xenon's NVMe, done; and DNS and a host for
`basalt.nifeos.org`.

### Rung 3c and rung 4

Rung 3c's work list is milestone 801 (packages over the internet). What must be true before the
page goes up, and the Secure Boot and publication rulings, are milestone 802 (the trivial install).

## Follow-on

These cover rung 3a only: this block's other rungs carry their own exit criteria in the table above,
and the rungs that are calef's acts are named there rather than here.

- **Outstanding.** This block's closure, which is milestones 801 and 802 and, through 802,
  milestone 809's items 7 and 8 (milestone 849 and an authority-list milestone not yet proposed).
  Checked 2026-10-24 (UTC) by reading each block's status and Follow-on and the swish-check script
  (`xtask/src/swish_check/script/after_reboot.rs`, every `jig` line): rung 3a has no open item.
- **Done.** §221 closed the `activation/` write hole (notes/packages/vouching.md).
- **Done.** §219 was ruled 2026-09-26 and its request, gate D2 and the unvouched-child
  confinement claim built (notes/packages/running-unvouched.md).
- **Done.** x86_64 fetches (2026-10-05): the kernel builds `net_stack` on the `e1000e` when there is
  no virtio-net NIC and grants the progenitor its endpoint (notes/packages/fetching.md). It leaves
  a PCH part alone, so xenon's I219 has no network at boot until milestone 494 (a driver for the
  network card a PC actually has)'s bench boot; lifting that is one predicate in
  `e1000e_service::start_for_boot`.
- **Milestone 801.** Milestone 801 (packages over the internet). Rung 3c, split out 2026-10-06:
  the resolver, a TLS client and §250's index. Signed installs are milestone 666 (a signed build
  installs up to its key's ceiling).
- **Milestone 802.** Milestone 802 (the trivial install). Rungs 1d and 4, split out 2026-10-06;
  rungs 1b, 1c, 2 and 3b were already milestones 400, 242, 515 and 494.
- **Milestone 647.** Milestone 647 (a virtio slot should come back when its driver dies). The virtio device table never reuses a slot
  (`design/roadmap/0647-a-virtio-slot-comes-back-when-its-driver-dies.md`).
- **Done.** Every archive build runs `cargo xtask package` end to end since 2026-09-24.
- **Done.** The digest: calef ruled C on 2026-10-07 (UTC), a plain SHA-256 written
  `sha256:<hex>`, built the same day. §197 (a package is one archive file) records it.
- **Recorded.** No compression. A `u32` ceiling on a member and on a package. A catalog that is
  one file in `target/` rather than a repository index. A package source compiled into the
  progenitor. And a recipe that cannot say where its source came from
  (`crates/package_archive`'s and `xtask/src/package.rs`'s BUGS sections, and
  `notes/packages.md`).
- **Recorded.** `packages/uptime.recipe.toml` records no digest on purpose, because the program it names
  is rebuilt by this checkout whenever anything it links changes (the recipe's own comment).
- **Decision.** Owed to `design/decisions/0135-running-gpl-software.md`'s amendment (calef,
  2026-09-27T15:11Z): the GPL repo's org, and how source is published per binary.

## BUGS

- Renewing `nifeos.org` (2027-10-03) gates every image that names it, and nothing enforces it; this line is the record.
- This block prices nothing. A package manager is a large piece of work and the estimate is not
  attempted; the sequencing claim is that it gates a customer, not that it is cheap.
- The package encoding is provisional, and so is `package_archive`'s name. §197 ruled the
  container, not the byte offsets. Its manifest question was ruled 2026-09-26 (an ELF note, not a
  member), and its digest question 2026-10-07 (a plain SHA-256, labeled `sha256:`). Reversible only until somebody outside this repository
  fetches a package, which §197 says fixes the format.
- The two Kani harnesses prove less than their names suggest. They cover a 272-byte file, which
  is the largest symbolic buffer that stays cheap, so the table they exercise holds at most two
  members and `MAX_MEMBERS` is never approached. Both discharge in 4 seconds together, and
  `script/verify`'s table carries the row.
- It does not decide the format, the activation shape, or the repository split. The scoping
  lane found the split's timing is not needed at all (see the gate proposal). calef decided the format,
  activation and trust forks
  (§197, §208, §195); the repository split is still open under §151 (the goal of the
  repository split is independent release).
- Rung 1's cost (milestone 242 (USB host and HID)) and the one-Dell sequencing are milestone 802 (the trivial install)'s
  `BUGS` now.
- A third party cannot author a package without cloning this repository, because the `nife-dev`
  toolchain, the target specifications and the linker script exist only as build steps inside it
  (`helpers/build-ripgrep.sh` is the one out-of-tree build and it needs them). §151 (the goal of the repository split is independent release)'s
  "third-party programs" needs a downloadable toolchain. §235 (the OS is built and updated from
  packages) ruled an SDK archive for it, proposal P4 of milestone 607 (nife is built and updated
  from packages).
- The cold build time a stranger pays was not measured, because two other lanes were gating on
  the machine when this block was scoped. Under §157 a stranger does not build at all; it stays worth measuring for
  contributors, and rung 4's stranger-harness run measures the time from the page to a prompt
  instead.
- Packages do not by themselves run `nano`: milestone 170 (nano editor) is NOT-STARTED. Corrected
  2026-10-06: milestone 205 (how a foreign program is told what to do), named here too, is BUILT.
- nife cannot build software, so a package is a thing produced by a host toolchain and consumed
  by the target. Every packaging idea borrowed from a self-hosting system needs that translation
  checked rather than assumed.

## Index row

calef, 2026-08-30: no third party sees nife until there is a package manager and a trivial
install, and he wants both early, to make our own lives easier. This block is the package manager. It is the one archive file §197 ruled a package is, `cargo xtask package` turning a reviewed recipe
into one, and a fetch over plain HTTP verified by digest. It is also `jig install`, `remove` and `rollback` at
the prompt across a reboot (§208, §219), on all three architectures since 2026-10-05. Split on
2026-10-06 so other milestones can depend on a single rung: packages over the internet are
milestone 801, and the trivial install a stranger follows is milestone 802.
