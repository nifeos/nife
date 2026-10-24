# The loader's design, in detail: leaving long mode, and the ELF reading

An appendix to [notes/x86-uefi-boot.md](../x86-uefi-boot.md), which keeps the design and the trade
it rests on. This file holds the mode switch step by step, and why the loader reads the ELF itself.

## Leaving long mode, in the order the CPU requires

UEFI hands over in 64-bit long mode, with the firmware's identity page tables live. The kernel wants
32-bit protected mode with paging off. `leave_long_mode.s`:

1. `lgdt` a GDT carrying a 32-bit code descriptor. That is the thing the firmware's GDT does not
   have, and the only reason a GDT is loaded at all.
2. Far-*return* into compatibility mode. A far return is the shape that works from 64-bit mode:
   `jmp far ptr16:32` is invalid there. The instruction is hand-encoded (`48 CB`), for exactly the
   reason `boot.s` hand-encodes its far jumps. LLVM's Intel-syntax parser spells it several mutually
   incompatible ways, and getting it wrong is a triple fault with no output.
3. Reload the data selectors, because they still name descriptors in the GDT we just replaced.
4. Clear `CR0.PG`. This is what actually leaves long mode: `IA32_EFER.LMA` is not a bit software
   writes, it is `LME && CR0.PG`. It is safe only because the trampoline is executing from an
   identity-mapped page, so the next instruction is fetched at the same address either way.
5. Clear `LME`, set `eax`/`ebx`, jump to the kernel.

It is copied to a page below 4 GiB before it runs, and both halves of that matter. It is below 4 GiB
because the second half executes with paging off, where a linear address is a physical one, and the
firmware may well have loaded this image above 4 GiB. And the page is allocated as `EfiLoaderCode`
rather than `EfiLoaderData`. Firmware with a memory-protection policy (OVMF has one, and so does
every recent vendor firmware) sets the execute-disable bit on data allocations. The first instruction
of the copy would then fault, with boot services still live and nothing watching.

## Why the ELF reading is physical

`crates/elf` is also the kernel's *user-program* loader, and a program is placed at its `p_vaddr`. A
boot loader places an image at its `p_paddr`. On this kernel the two are not related by any offset a
caller could apply:

```text
.text            p_vaddr 0xffffffff80109000   p_paddr 0x109000
.ap_trampoline   p_vaddr 0x0000000000008000   p_paddr 0x165000
```

So `elf::Segment` carries `p_paddr` beside `p_vaddr` (milestone 196 (a physical address on
`elf::Segment`)), and `uefi_loader/src/image.rs` places each segment by it. The loader reads the
kernel with `Elf::parse` and its whole validation battery, which the x86_64 image passes because its
trampoline's code and data ship as separate segments (milestone 208 (the x86_64 kernel image ships
an RWX segment)).

The other trap the physical span has to survive is `NOLOAD`. `.boot_scratch` (the boot page tables)
and the two per-CPU stack areas arrive as `PT_LOAD` segments with `p_filesz == 0` and a real
`p_memsz`. They are address space to reserve, rather than bytes to copy. A loader that skipped them
would leave the firmware free to hand that memory to something else, while the kernel's trampoline
zeroes its own page tables on top of it.

## The loader's own file

`place_boot_file` in `uefi_loader/src/placement.rs` is rung 2a of milestone 198 (a package manager,
and the trivial install that makes a second customer possible). It solves a problem nobody had
written down until milestone 515 (a stick that puts itself on the machine's disk) looked: the running
system does not have its own file. The loader places the kernel's segments and hands the archive
over as a module, and then the PE file that contained both is gone. An installer has to write that
file to the disk's EFI system partition, and the file cannot sit inside the archive it contains.

So the loader reads its own file while the firmware can still read a FAT volume in one call, and
hands it over as a second module. That costs two protocols and one allocation. It keeps the kernel
and the archive together as the single file that already seals them, so an installer cannot copy one
without the other and produce a disk that halts at `MEASURED BOOT REFUSED`. A failure here costs the
ability to install, which is not what a stick is for on most boots, so it is reported and the boot
carries on.

Its limits, in full:

- The path is a constant, not the image's own `loaded_image::Protocol::file_path`. Turning a device
  path into text needs `EFI_DEVICE_PATH_TO_TEXT_PROTOCOL` and a parser, and every boot this serves is
  the removable-media fallback path, which is the constant. A loader started from another path on
  the volume reads the wrong file or none, and says so.
- It is x86_64 only. The device-tree architectures hand the kernel a tree rather than a module list,
  and `/chosen` has one initial ramdisk and no second slot. The trivial install of §157 (a trivial
  install is a web page, a USB drive, and packages) is a PC, so this is where it was needed first.
  Milestone 568 (the boot file has nowhere to go on a device-tree machine) prices the gap.
- The file is read whole into RAM, about 9 MiB for the tour build. Streaming it to the disk would
  need a firmware file handle that outlives `ExitBootServices`, and there is no such thing.

## The boot command line

`handoff::cmdline` writes up to four words, and each has its own contract:

- The screen the firmware was drawing on. It is all that milestone 243 (a machine with no serial
  port has no way to say anything, and no gate can read it) carries across the handoff. It is absent
  on a machine with no graphics output, such as the `OptiPlex` with its serial module.
- `boot_slot::cmdline::KEY` and a digit tell the running system which slot to confirm. They are
  written only when a chooser started the image. A stick, a `-kernel` boot and a fallback to the
  image in the chooser's own file all leave them off, and a kernel that finds none confirms nothing.
  The screen is an `Option` for this reason. Skipping the whole command line when there was no
  screen would drop the slot on every serial-only machine. The symptom would be an upgrade that
  reverts days later on one kind of machine.
- `boot_slot::medium::NVME` marks a boot from the NVMe disk, the only case in which the kernel
  mounts it (the live stick proposal's G2, ruled by calef on PR #1652, 2026-10-04 UTC). A stick
  leaves it off, and its system never touches the internal disk.
- `machine_discovery::framebuffer::SCREEN_HOLD`, under the `screen_hold` feature, is for one caller:
  `cargo xtask uefi-boot`, the gate that photographs the screen. It asks the kernel to stop between
  painting its boot tour and clearing it, announce that on the serial line, and wait for a byte back,
  so the gate reads a state rather than racing a window. `uefi_loader`'s `Cargo.toml` says what it
  costs a machine handed it by mistake.

## aarch64 and riscv64

On aarch64 the firmware's own device tree is taken whenever the firmware offers one, and ACPI is
read only when it does not. The ACPI path is what makes an SBBR server, which every aarch64 cloud
instance is, bootable at all. QEMU `virt` offers only the tree with `acpi=off` and only the tables
with `acpi=on`, so a machine offering both is rare. The order is written down anyway, because the
machine this eventually runs on may be neither. The data cache is cleaned and invalidated before the
MMU goes off so that the kernel, running with its caches off, reads what was written rather than
stale RAM.

On riscv64 UEFI runs the loader in S-mode, possibly with paging on (an identity map). It is the only
architecture where the kernel needs a number the firmware must be asked for: which hart it runs on.
There is no `riscv64` UEFI target in rustc: `rustc --print target-list` on 2026-09-19 listed
`aarch64-`, `i686-` and `x86_64-unknown-uefi` only.
