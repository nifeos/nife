---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: the-kernel-trusts-two-digests-from-the-loader
milestone_dependencies: 976
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# 695. The kernel trusts two digests handed to it by the loader

*Title and slug are drafts.*

Item 6 of milestone 607 (nife is built and updated from packages), with milestone 697 (the trust
root leaves the kernel binary). §235 (the OS is built and updated from packages) ruled the shape on
2026-09-27 (UTC), T4 with T2. This block is the boot-path half; 697 is the build half and holds the
measurements.

## What changes

Today `kernel/build.rs` compiles `TRUST_ROOT` into the kernel. It holds the progenitor's digest,
six test child builders' digests, and the digest of the archive's measurement table,
`measured_boot::PROGRAM_MEASUREMENTS`. The kernel checks the progenitor and the table against it.
The progenitor checks every program it loads against the table (milestone 104 (the measurement
continues past init)).

Under T4 with T2, the loader hands the kernel two digests on all three architectures: the
progenitor's, and the digest of the archive's `program_measurements` table. The kernel checks only the progenitor before entering it. It
passes the base digest on, and the progenitor checks the table against it before reading it. Trust
moves to the loader. Only Secure Boot (milestone 500 (a stick that boots with Secure Boot on)) or
the slot header's CRC-32 protects the loader, and §235 accepted that cost. Signatures stay where
§220 (signed builds) put them, at install and update time.

Comparable system: Fuchsia checks the `system_image` Merkle root, carried in its boot arguments, in
userspace rather than in the kernel.

## What is calef's

Two questions, asked one at a time in
[`how-the-loader-hands-the-kernel-two-digests.md`](proposals/how-the-loader-hands-the-kernel-two-digests.md).
First, the exact bytes of the handoff on the device-tree path (aarch64, riscv64) and the PVH path
(x86_64). Second, where the loader's own copy of the digests comes from. Milestone 525 (a bad
upgrade cannot brick the machine) records that the device-tree path has one initrd slot.

A third question follows and stays separate. Once the kernel can update apart from the base, the
syscall surface becomes a versioned ABI that installed programs rely on.

## The consumer, and the condition the code waits on

The first caller is milestone 976 (an OS update is a set of packages that lands through a slot),
part 4, the updater. No code for this block merges until that part exists and shows what a kernel
download costs per update (calef, 2026-10-31 UTC). A milestone names its consumer. It is not BUILT
until a record cites its first use. 697's block has the measured figures.

## The plan in parts

| part | what | needs a ruling? | size |
|---|---|---|---|
| 1 | A contract crate for how the kernel hands the base digest to the progenitor, and the progenitor's check of the table against it, host-tested. The two release together, so the lane picks the form. | no | about 200 lines and host tests in `crates/` and `crates/system_initializer` |
| 2 | The parser for the handoff's two facts, in a crate the kernel and `uefi_loader` both link, with the writer's output decoded by it in tests. | question 1 | about 150 lines and host tests |
| 3 | The kernel reads the two facts on each path (`device_tree_blob`'s `/chosen` reader on aarch64 and riscv64; the PVH command line on x86_64), checks the progenitor and hands the base digest on. Lands with 697's part 2. | question 1 | about 250 lines, one QEMU suite per architecture |
| 4 | The four writers: `uefi_loader` (device-tree patch and PVH command line), `xtask`'s QEMU direct boot, the board's U-Boot script, and 976's slot writer. | question 2 | about 300 lines; the board half needs a bench boot |

Part 1 needs no ruling and is safe behind the QEMU suites; it still waits on the condition above.

## Index row

The loader hands the kernel the progenitor's digest and the `program_measurements` table's digest, on all three
architectures, and the progenitor checks the base set (§235, T4 with T2). The boot-path half of
607's item 6; its first consumer is 976 part 4, and two handoff questions are calef's.
