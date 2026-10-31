---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: 695, 697, 976
decision_dependencies: 235, 244
machine_requirements: none
specific_machine: none
needs_person: no
---
# How the loader hands the kernel two digests, and where the loader gets them

Raised 2026-10-31 (UTC) by the lane `milestone/697-the-trust-root-leaves-the-kernel-binary`. It
serves item 6 of milestone 607 (nife is built and updated from packages), which is milestones 695
and 697 (the kernel's half and the build's half).
Two questions come to an architect before any of that code can finish. This file asks them in
order, one at a time, and asks the second only after the first is answered.

Reuse: `crates/measured_boot`'s digest text (`sha256:<hex>`, the grammar of the measurement table),
`crates/device_tree_blob`'s `/chosen` reader, `uefi_loader`'s device-tree patch and command-line
writer, and `boot_slot`'s header. No new dependency.

## What is measured, and what it says

Every figure below is from 2026-10-31 (UTC), from `cargo build -p kernel --release` at
`origin/main` 25e17a56e, stripped with `llvm-objcopy --strip-all`.

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| release kernel, unstripped | 1,076,040 | 1,437,312 | 1,041,088 |
| release kernel, stripped | 860,624 | 684,536 | 877,528 |
| the same, `gzip -9` | 224,773 | 242,206 | 231,544 |

What ties the kernel to the base set is 256 bytes and nothing else. `kernel/build.rs` reads
`target/init-measure-<arch>.txt` and compiles eight digests into `TRUST_ROOT`: the progenitor's, the
six test child builders the kernel enters itself (`sealed_pair::BOOT_PROGRAMS`), and the archive's
`program_measurements` table's. Changing one byte of one digest and rebuilding changes exactly one
byte of the stripped kernel on all three architectures. Code, layout and every other byte stay put.
The unstripped ELF differs in 62 to 76 KB of symbol names as well, because the generated file
changes the crate's hash, and stripping removes all of it. So a base change makes a new kernel
binary that differs from the old one in at most 64 bytes, the progenitor's digest and the table's.

The build ties it in three places. `kernel/build.rs` reads the measurement file. `xtask` must pack
the archive before it builds the kernel (`uefi_kernel`, `base_image`'s kernel step). And
`uefi_loader/build.rs` refuses to embed a kernel and archive that `sealed_pair` finds unsealed.

What that costs an update under milestone 976's option B, ruled 2026-10-31 (UTC). The base packages
that milestone 976's lane built on 2026-10-31 (x86_64, the debug profile `base-packages` builds
today) total 10,478,152 bytes in eighteen packages, 2,864,673 under `gzip -9`. While the kernel
vouches for the base, every base update carries the kernel package too:

| an update that changes only | it downloads now | after item 6 | saved |
|---|---|---|---|
| `swish` (947,556; debug) | `swish` plus the debug kernel, stripped 1,541,256 | `swish` | 1,541,256 bytes, 62% |
| `entropy` (52,556; debug) | `entropy` plus the same kernel | `entropy` | 1,541,256 bytes, 97% |
| any package, release build | the package plus 684,536 to 877,528 | the package | the release kernel, every time |

Compressed, the kernel is 389,132 bytes (debug, stripped) or about 230 KB (release). Two caveats
hold the table to what it says. The packages are debug builds, because `base-packages` takes no
release flag; release packages will be smaller and the kernel's share of an update larger. And a
binary delta of the kernel would cost at most 64 bytes of payload, so the saving is real only while
976 downloads whole packages, which it does under option B.

## Question 1: the bytes of the handoff

### Background

When the loader starts the kernel it already hands over a small set of facts. On aarch64 and
riscv64 it is a device tree, a structured description of the machine in memory; its `/chosen` node
holds what the loader chose, such as where the archive sits (`linux,initrd-start` and
`linux,initrd-end`). On x86_64 it is `hvm_start_info`, a structure Xen defined for the PVH boot
protocol: a memory map, a module list holding the archive, and a pointer to a text command line.
nife writes the x86_64 command line already: the slot the chooser picked (`boot-slot=0`), the boot
medium and the screen, each a word the kernel parses through a shared crate. The kernel reads no
command line on the device-tree path today.

Under §235's T4 with T2, two more facts travel: the progenitor's SHA-256 digest, and the digest of
the base manifest. The kernel checks the progenitor against the first before entering it, and hands
the second to the progenitor, which checks the archive's table against it and every program
against the table. Three loaders must write the facts: `uefi_loader` on all three architectures,
QEMU's direct boot that `script/test` uses (`-kernel`, `-initrd`, `-append`), and U-Boot's boot
script on the VisionFive 2 card. Milestone 525 records that a device tree has one initrd slot, so
the facts cannot ride in a second boot module there.

This is a wire format between the loader and the kernel. Installed disks will carry loaders that
write it, so it is not reversible once one ships. A missing or malformed fact must halt the boot
with `MEASURED BOOT REFUSED`, as today's empty trust root does.

### The question

**Recommendation: A.** Two words on the kernel's command line, on all three architectures:
`nife.progenitor=sha256:<64 hex digits>` and `nife.base=sha256:<64 hex digits>`, in
`measured_boot`'s existing digest text. On a device tree they sit in `/chosen/bootargs`; on x86_64
in the PVH command line, beside `boot-slot=`. One parser, in a crate, serves all three paths.

| option | the bytes | what it costs |
|---|---|---|
| A. Command-line words (recommended) | 2 × 80 ASCII bytes in the command line every loader already writes | QEMU writes it with `-append`, U-Boot with `setenv bootargs`, `uefi_loader` with its command-line writer. A device-tree kernel learns to read `bootargs`. Text is 2.5 times the binary size, 160 bytes. |
| B. Binary `/chosen` properties, a word on x86_64 | `nife,progenitor-sha256` and `nife,base-sha256`, 32 bytes each, on a device tree; command-line words on PVH | Linux's own `/chosen` vocabulary looks like this (`kaslr-seed` is a binary value). Two encodings and two parsers for one fact. QEMU cannot write a custom property, so `script/test` needs a device-tree overlay step. |
| C. A record in a second module | one binary record, its own module on PVH and a `/chosen` pointer on a device tree | Room for more facts later. Most moving parts, and the device tree has no second module slot (525), so it needs a reserved memory range. |
| D. A field appended to `hvm_start_info` | 64 bytes past Xen's version 1 | Refused by `uefi_loader/src/handoff.rs`'s own record: an appended field forks Xen's format and looks real to the next reader. |

Why A. It is the one channel all three loaders, and QEMU's direct boot, already write without new
code in the loader, and nife already uses it on x86_64 for the chooser's facts. Would I still choose
A if B cost the same? Yes: one encoding beats two for a fact two programs must agree on, and the
size difference is under 100 bytes.

If the answer is B, the device-tree half needs a host step that patches QEMU's tree, and the parser
splits in two. If C, item 6 also waits on a reserved-memory convention for device trees, the gap
milestone 568 (the boot file has nowhere to go on a device-tree machine) already names.

### How others solved it

- systemd's `roothash=` and `usrhash=` (read 2026-10-31,
  `systemd-veritysetup-generator(7)`): a hex root hash on the kernel command line, which the
  generator turns into a verity device. In a unified kernel image the stub measures the embedded
  command line into TPM PCR 11 (read, `systemd-stub(7)`), so the digest the next stage trusts rides
  in a measured command line. Nife has no TPM in the boot path and measures nothing into one.
- Linux `dm-mod.create=` (read, `dm-init.rst`): a whole dm-verity table, root digest included, on
  the command line. ChromeOS's design document (read) requires the root hash to be "signed or stored
  in a signed/verified location"; its kernel partition signs the command line with the kernel, a
  detail recalled, not read.
- Linux `/chosen` (read, `chosen.txt`): `kaslr-seed` is a binary value a loader writes; the EFI stub
  overwrites it. Option B follows this shape.
- Fuchsia (read, the kernel command-line reference): boot arguments carry Merkle roots as ASCII hex
  (`zircon.system.pkgfs.file.<path>=<blobid>`). I recall, and did not read, that the `system_image`
  package's root travels the same way, in `zircon.system.pkgfs.cmd`.
- U-Boot verified boot (read): a FIT image carries hashes of each image inside it, and U-Boot checks
  them against a key in its own control device tree. Nothing passes a measurement on to the kernel;
  the check ends in U-Boot.

Nife differs from all of them in one way that shapes the answer: nothing here signs the command
line or measures it. The digests carry exactly the trust of the slot they come from, which §235
accepted as T2's cost.

### A concrete example

A machine boots slot 1 through `uefi_loader` on aarch64. The loader patches the firmware's device
tree so `/chosen` holds the archive's range and
`bootargs = "nife.progenitor=sha256:cce306d2…7f5a nife.base=sha256:b289e58e…9e1f"`. The kernel finds
the archive, reads the progenitor's bytes, hashes them, and compares against the first word; a
mismatch prints `MEASURED BOOT REFUSED` and halts. It hands the second digest to the progenitor,
which hashes `program_measurements` and compares before it reads a line of
it. Under QEMU, `script/test` passes the same two words through `-append`.

## Question 2: where the loader's own copy comes from

Asked once question 1 has its answer.

### Background

A loader that hashes the archive at boot and hands over what it measured vouches for any archive at
all: swap the archive, and the loader hashes the swap. So the reference digests must come from
whoever composed the boot, written somewhere the loader reads. Four producers exist. The composer
(`cargo xtask base-image`, and basalt through it) writes a whole boot file. Milestone 976's updater
will write a version 2 slot of named members on the device, ruled 2026-10-31 (UTC). `script/test`
starts QEMU directly. The board's card writer (`cargo xtask board`) writes U-Boot's boot script.

The slot header today is 32 bytes inside a 4,096-byte reserve, with a CRC-32 over the header and
one over the image. A CRC catches a copy that died halfway; it does not stop a deliberate swap. §235
accepted that under T2: trust moves to the loader, protected only by Secure Boot (milestone 500) or
the slot checksum.

### The question

**Recommendation: A.** The composer records the two digests where it writes the archive, and every
loader copies them without computing them. In a version 2 slot they are two fixed header fields,
written by the updater from the system manifest and covered by the header's CRC. In a boot file
that embeds its archive, the build embeds them beside it, as `uefi_loader/build.rs` embeds the
kernel and the archive now. For QEMU, `xtask` reads them from the archive it packed. For the card,
the card writer puts them in the boot script's `bootargs`.

| option | where the digests live | what it costs |
|---|---|---|
| A. Recorded by the composer, copied by the loader (recommended) | slot header v2 fields; beside the embedded archive; the QEMU command; the boot script | Four writers, each beside code that already writes the archive. The v2 header grows by 64 bytes inside its existing reserve. |
| B. Read from the system manifest member | the loader hashes the slot's system manifest and reads the progenitor's digest out of it | No new header fields, but the chooser parses the manifest's format, which is still unnamed (976's next question), and gains a SHA-256 over it. |
| C. Measured by the loader at boot | the loader hashes the archive's two entries | Catches nothing a swap would not also change. Refused: it makes the check vacuous. |
| D. Compiled into the chooser | the chooser binary | The chooser on the EFI system partition serves both slots, so it cannot hold per-slot digests. It moves the coupling one stage earlier. |

Why A. The digest sits beside the bytes it describes, written by the code that wrote those bytes,
so no stage computes a reference from the thing it checks. Would I still choose A if B cost the
same? Yes: B puts a manifest parser into the chooser, the component that must never brick a
machine.

If the answer is B, the chooser waits on the system manifest's format, and question 1's words carry
the manifest's digest computed by the chooser. If the answer is that the slot header should carry
more than two digests, the v2 header question in 976 widens to match.

### How others solved it

- Android verified boot (recalled, not read): `vbmeta` holds the hash descriptors of each partition
  and the bootloader passes the vbmeta digest to the kernel as `androidboot.vbmeta.digest` on the
  command line. The reference sits in a structure written at build time beside the images.
- systemd-stub (read): the command line, initrd and kernel all live in one PE file, written at
  build time, so the references travel with the bytes. Nife's single-file boot file under option A
  matches this.
- U-Boot FIT (read): the hashes live inside the FIT next to each image, written by `mkimage` at
  build time.

### A concrete example

Basalt publishes base 0.1.1, in which only `swish` changed. The updater on a running machine fetches
`swish`, copies the other packages from slot 0 and writes the archive into slot 1. The v2 header
gets the progenitor's digest (unchanged, `cce306d2…`) and the table's new digest. It does not
download a kernel: after item 6 the kernel member is byte-identical to slot 0's, so it copies it. On
the next boot the chooser reads slot 1's header, checks its CRC, and writes the two words of
question 1 into the command line.

## The questions behind these two

Each waits its turn:

1. Whether `nife.base` names the archive's `program_measurements` table, as this file assumes, or
   976's system manifest. The table is what the kernel vouches for today and what the progenitor
   already reads; the system manifest lists packages, and the updater reads it.
2. How the kernel hands the base digest to the progenitor: a start word, or a page. The kernel and
   the progenitor release together, so this is a contract crate, reversible, and the lane decides
   it. Listed so nobody mistakes it for a third wire format.
3. What the six test child builders become once the kernel trusts only the progenitor. They are test
   programs the system-test kernel enters directly; the plan in 697 measures them through the table,
   as `trust::require_program` already does for the benchmark boots.

## Index row

The two questions item 6 of milestone 607 waits on. First, the exact bytes the loader hands the
kernel on three architectures (recommended: two command-line words). Second, where the loader's own
copy comes from (recommended: written by whoever composes the boot, copied by the loader).
