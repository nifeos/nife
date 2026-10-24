# Booting x86_64 from real firmware

Milestone 87 (the x86_64 bare-metal machine): the port boots under QEMU by PVH, and `notes/x86-port.md`'s `BUGS`
already said what that costs:

> PVH is a hypervisor protocol and no real firmware speaks it. Milestone 87's OptiPlex will need a
> UEFI stub or GRUB's Multiboot.

This note is the answer: what was chosen, what the alternative actually cost when it was priced
rather than argued about, and how the thing works. It also holds the exact procedure for the bench,
which is the part of this milestone that a person has to carry out by hand.

The evidence and history behind each section are in [`x86-uefi-boot/`](x86-uefi-boot/README.md),
and each section links the appendix it summarizes.

## The fork, and it was decided by two commands

UEFI, not GRUB. OVMF ships with the QEMU this project already pins, QEMU's `vvfat` synthesizes the
FAT filesystem from a directory, and the OptiPlex 7050 is UEFI-native, so the machine end is one file
on a FAT32 stick at `/EFI/BOOT/BOOTX64.EFI`. GRUB cannot be installed on this macOS machine at all,
so it could be written but not proved here. The loader takes no dependency: six firmware functions
and two GUIDs, hand-written. GRUB Multiboot 2 stays cheap to add if a BIOS-only machine turns up,
and no Multiboot header of any version is in the image.
[`x86-uefi-boot/the-fork.md`](x86-uefi-boot/the-fork.md) has both pricings.

## The design, and the one decision the rest follows from

The kernel is not modified. It is entered through its existing `_start`, in 32-bit protected
mode, with PVH's register contract: `eax` = `0x336EC578`, `ebx` = the physical address of an
`hvm_start_info`. The kernel cannot tell which loader started it.

That was chosen against the obvious alternative, which was a second 64-bit entry point in the
kernel that rebuilt the boot page tables in long mode. Two entries would have meant two contracts,
two page-table builders, and a real chance of breaking the PVH path that every
`script/test --arch x86_64` run rides. Thirty-two instructions in the loader buy one entry point in
the kernel, and that is the whole trade.

Everything else falls out of it:

| The loader does | Because |
|---|---|
| synthesizes an `hvm_start_info` | `machine_discovery::x86_64` already decodes it, host-tested, and `arch::x86_64::machine` already consumes it |
| places the kernel at `p_paddr` | that is what a boot loader does, and this kernel's `p_vaddr` and `p_paddr` are unrelated (see below) |
| leaves long mode as its last act | that is the *only* state difference between what UEFI hands over and what QEMU's PVH loader hands the kernel |
| embeds the kernel and the archive | one file on the stick, and no `SimpleFileSystem` protocol to speak |

The pieces:

- `uefi_loader/src/lib.rs` and its three modules are the pure half: the firmware table layouts
  (`efi`), the `hvm_start_info` writer (`handoff`), and the physical-address ELF reading (`image`).
  All of it compiles for the host and is tested there, for the reason `crates/device_tree_blob` and
  `machine_discovery` exist rather than living inside `arch/`: a structure layout proved only by
  booting is proved by nothing that runs in milliseconds.
- `uefi_loader/src/main.rs` is the half that cannot be: it calls firmware and it changes CPU
  mode.
- `uefi_loader/src/leave_long_mode.s` is the mode switch.

### The handoff is the kernel's own, which is the hazard this closes

Milestone 87's brief named the risk directly: *make both entries produce the same internal
structure for `kernel_main`*, because a divergence would first show up on hardware nobody can
attach a debugger to. Synthesizing an `hvm_start_info` is how that is made true rather than
promised. There is one structure, one decoder, and one set of tests. `uefi_loader::handoff`'s
tests decode their own output with the crate the kernel decodes with, so the writer and the
reader cannot drift apart without a host test failing in milliseconds.

### Leaving long mode, and reading the ELF

The loader's last act is the mode switch in `leave_long_mode.s`: load a GDT with a 32-bit code
descriptor, far-return into compatibility mode, reload the data selectors, clear `CR0.PG`, clear
`LME`, and jump. It runs from a copy below 4 GiB in an `EfiLoaderCode` page, because paging is off
for its second half and firmware marks data allocations non-executable. The loader reads the kernel's
ELF itself rather than through `crates/elf`, because a boot loader places segments at `p_paddr` and
must reserve the `NOLOAD` ones. [`x86-uefi-boot/loader-design.md`](x86-uefi-boot/loader-design.md)
has each step and why.

## What it proves, measured

The same kernel booted under PVH and under OVMF at `-m 256M` differs only by firmware. It sees 118
memory regions instead of 9, a revision-2 RSDP with an XSDT instead of a scanned RSDT, and an ECAM window
from the MCFG at `0xe0000000` instead of the constant `0xb0000000`. Four code paths ran for the
first time, and `(xsdt)` is the string the gate checks, because only the firmware path can print it.
Milestone 195 (finish the UEFI boot path) reclaimed boot-services memory, so usable RAM on that
machine went from 206684 KiB to 233148 KiB, against PVH's 261627.
[`x86-uefi-boot/measured-under-ovmf.md`](x86-uefi-boot/measured-under-ovmf.md) has the table and
the reclaim.

## Running it

```console
$ cargo xtask uefi-image                       # kernel + archive + loader, staged at target/esp
$ helpers/qemu-uefi-x86_64.sh target/esp       # boot it under OVMF
$ cargo xtask uefi-boot                        # the same pair, plus the assertions
$ cargo xtask uefi-image --features soak_test  # a non-default kernel for the stick (notes/soak.md)
```

`target/esp` is what the bench copies from. `uefi-boot` stages its own copy at `target/esp-screen`,
whose loader holds the screen for the gate, and is not for a stick.

```console
$ cargo xtask uefi-test                        # the kernel's TEST binary under the same firmware
$ cargo xtask uefi-test --test <name> --repeat 200 --stage-only
                                               # one test, 200 times, staged at target/esp-test for a stick
```

Both run inside `script/test --arch x86_64`, after the PVH suite. `uefi-boot` boots the tour at two
cores and asserts `(xsdt)`, no `rsdp 0x0`, the tour's completion line, `smp: 2 core(s) online`, and
the tour's tail read back off the screen. It then boots the same image at 1920x1080 and 2560x1440
and asserts on the serial line that the screen was handed to `framebuffer_driver` and the shell came
up, because OVMF's default 1280x800 is no bench panel's mode. `uefi-test` boots the test binary and asserts both the
harness's verdict and QEMU's exit status. On 2026-09-02 the suite's numbers under OVMF matched PVH's
exactly. Running it under firmware moved `PHYS_START` from 1 MiB to 32 MiB, and the loader now names
the descriptors in the way when an allocation is refused. The suite stays at one core because of
`ap_boot`'s open two-core defect. [`x86-uefi-boot/running-under-firmware.md`](x86-uefi-boot/running-under-firmware.md)
has the assertions and what the suite found.

## First light on xenon, 2026-09-05

The machine talked without a serial cable: the framebuffer console carried the boot tour on its
first contact with real firmware, read off a photograph. It found real legacy PICs, an ECAM constant
that disagrees with the MCFG, and 38 memory regions. The boot menu showed a Micron NVMe behind VT-d,
which is the hardware the confined-driver experiment of §86 (whether an NVMe driver can leave the
kernel) needs. Then it panicked with `AlreadyMapped`.
The cacheable fill of firmware reservations reached the local APIC, because xenon's RAM ends above
the MMIO hole. The fill now stops where the firmware's map stops describing memory, and the panic
names both ranges. Only xenon can confirm the fix: the line to reach is `mmu : fine W^X 4-level map
installed`. [`x86-uefi-boot/first-light-2026-09-05.md`](x86-uefi-boot/first-light-2026-09-05.md)
has the photographs' transcripts, the diagnosis and the confirming session.

## The bench: booting nife on the OptiPlex 7050

This section was written before first light and is kept as the procedure. It has now been run
once; see above. Everything above is QEMU with real firmware in the loop, which is as far as this
lane could get; the machine is calef's bench. This section is the procedure, written to be followed
rather than interpreted.

### What you need

- The OptiPlex 7050 Micro, its Dell C4PDJ serial module installed, and the dev-side RS-232 chain
  (FTDI USB adapter, StarTech NM9FF null-modem barrel) already on the desk (milestone 87's own
  block).
- A USB stick, formatted FAT32 with a GPT or MBR partition table. macOS Disk Utility: *Erase*,
  format MS-DOS (FAT), scheme GUID Partition Map.

### Build and copy

```console
$ cd /path/to/nife
$ cargo xtask uefi-image
wrote .../target/esp/EFI/BOOT/BOOTX64.EFI (9110528 bytes: the loader, the kernel and the archive)

$ mkdir -p /Volumes/NIFE/EFI/BOOT
$ cp target/esp/EFI/BOOT/BOOTX64.EFI /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI
$ diskutil eject /Volumes/NIFE
```

The path and the capitalization are the interface. `\EFI\BOOT\BOOTX64.EFI` is the removable-media
fallback the firmware looks for with no configuration; anything else needs a boot entry created on
the machine.

That is the whole of it: one file. There is no kernel to copy separately and no configuration
file, because the loader carries both inside itself (`uefi_loader/build.rs`).

### Firmware settings, and the one that will bite


Enter setup with F2 at the Dell splash; F12 is the one-time boot menu. Each value below is already set
on xenon, and [`x86-uefi-boot/usb-stick-firmware-steps-xenon.md`](x86-uefi-boot/usb-stick-firmware-steps-xenon.md)
has what the machine's own menus said about each step.

1. Secure Boot: off (*Secure Boot → Secure Boot Enable → Disabled*). This image is unsigned, and a
   Secure Boot machine refuses the stick with a security-violation message. Other vendors' menus,
   and the BitLocker recovery key to write down first, are in
   [`turning-secure-boot-off.md`](turning-secure-boot-off.md).
2. Boot List Option: UEFI, not Legacy, in the lower half of *General → Boot Sequence*.
3. Serial port: COM1, a single radio under *System Configuration → Serial Port*. COM1 is I/O port
   `0x3f8` with IRQ 4, which is where the kernel's console driver looks.
4. Leave the rest alone on the first attempt, the integrated NIC and SATA mode included.

One hazard the four steps do not name: xenon stops at POST and waits for a keypress when it finds no
keyboard. Changing that is two settings and calef's call.

### Watch it

On the Mac, before powering the machine on:

```console
$ ls /dev/cu.usbserial-*                      # the FTDI adapter
$ screen /dev/cu.usbserial-XXXX 115200        # exit with ctrl-a k
```

115200 8N1, which is what `drivers/ns16550.rs` programs.

### What you should see, in order

1. On the video output, before anything reaches serial: `nife uefi_loader: milestone 87`, then
   `uefi_loader: kernel placed, exiting boot services`. The Dell's serial port does not carry
   *firmware* output, so this is the only sign the stick was read at all, which is exactly why the
   loader prints it. Attach a monitor for the first attempt.
2. On serial, immediately after: the kernel's own tour, beginning

   ```text
   nife on x86_64 (long mode, ring 0, 4-level paging)
     cpu 0 booted: high-half kernel, .bss, and the 16550 console are up.
   ```

   and ending `nife x86_64: boot complete, halting.` **The first line is milestone 87's own
   completion criterion**: the machine has printed a byte over serial.

### If it does not

Triage in this order, because each step rules out everything above it.

| Symptom | What it means | What to do |
|---|---|---|
| Firmware says "security violation" or silently skips the stick | Secure Boot is on | Disable it (above) |
| Firmware boots to its own shell or to the internal disk | the stick was not seen as bootable | Check the path and case: `/EFI/BOOT/BOOTX64.EFI`. Re-format FAT32, not exFAT |
| `nife uefi_loader: milestone 87` and then a message beginning `uefi_loader:` | the loader ran and refused, and the message says why | Every one of those strings is a literal in `uefi_loader/src/main.rs`; read it there |
| The loader's two lines and then nothing, ever | the handoff or the mode switch failed, or the kernel died before the console | See below |
| Video lines, but serial silent while the machine is clearly alive | the serial chain, not the software | Loop back the null-modem barrel's pins 2 and 3 and confirm `screen` echoes typing |
| The machine reboots in a loop | a triple fault | See below |

A triple fault or a dead machine after the loader's lines is the hard case, and the honest
answer is that this is where the QEMU work stops helping. What is available: boot with the archive
left out (the loader builds fine with no `NIFE_UEFI_INITIAL_RAMDISK`, and the tour says
`initrd: none`), which removes five megabytes of copying and the whole userspace half of the tour.
And `-cpu qemu64` under QEMU is the nearest thing to "a CPU that has told us no" (`notes/cpu-models.md`);
`-cpu max` on the dev machine has never refused this kernel anything.

### If it works

Two things are then worth doing, in this order, and neither is in this lane's scope:

1. Record the numbers, the way `notes/visionfive2.md` does for the VisionFive 2: the memory map
   the firmware reports, the ACPI tables it carries, the measured TSC rate against the PIT, and
   whether the DMAR is present so VT-d can come up. The TSC rate will *not* be QEMU's 1001 MHz; it
   is an i5-7500T. So `user_mode_runtime::cntfrq`'s hardcoded constant will be wrong with no way for
   a caller to tell, which `notes/x86-port/user-mode-runtime.md` already records.
2. Flip milestone 87's status and open the two follow-ups the bench will inevitably produce.


## The bench: booting xenon over the network, with no stick at all

Milestone 260 (boot xenon over the network) built it, rehearsed it on patagonia under OVMF, and has
never run it on xenon. It removes the copy per boot and the need to carry the build machine to the
bench. The procedure has three steps. Two firmware settings on xenon's `Integrated NIC` page are
calef's. The house router takes the DHCP lines in `bench/xenon-netboot/dnsmasq.conf` once,
architecture match included. Then serve:

```console
$ cargo xtask uefi-image                        # every time the kernel changes
$ script/board-netboot --root target/esp        # in the other terminal, while you work
```

and power xenon on. Network boot does not fix the POST keyboard halt, so it is a faster bench session,
not an unattended one. [`x86-uefi-boot/network-boot-xenon.md`](x86-uefi-boot/network-boot-xenon.md)
has the procedure, what each screen should show, and the failure table.

## What xenon still has to establish, and what it no longer has to

Two of the three questions of milestone 215 (a PCI function's interrupt reaches nothing on
x86_64) are answered under OVMF: a firmware-placed BAR's MSI-X table,
and interrupts reaching the boot core with two local APICs. The third, whether the firmware leaves
interrupt remapping off, is not a menu setting. It lives in the DMAR the firmware publishes, which is
`design/roadmap/0378-read-the-dmar-on-xenon.md`. And the bench must show whether the Dell's firmware
leaves 32 MiB free for the kernel. [`x86-uefi-boot/first-light-2026-09-05.md`](x86-uefi-boot/first-light-2026-09-05.md)
has the details.

## BUGS

The full list, every entry as written, is [`x86-uefi-boot/limitations.md`](x86-uefi-boot/limitations.md).

- The kernel is placed at one address chosen at link time, 32 MiB. A firmware that wants that range
  refuses the boot, and making the image relocatable is a milestone.
- Nothing verifies what the loader hands over: the trust boundary is the build, and the image is
  unsigned, which is why Secure Boot has to be off.
- A stick made by hand can carry a stale `.efi` and nothing says so.
- The bench procedure and the suite under firmware both run one core.
- The firmware steps are checked against the machine; the keys, the fallback path and the triage
  table are one run old.
- `uefi-test` can go red after its suite passes, with expected DMA-escape fault lines beside it that
  read as the cause and are not.
