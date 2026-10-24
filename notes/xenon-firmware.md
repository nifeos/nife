# xenon's firmware, page by page

*Name provisional. calef names the interfaces (§75), and this note's own filename is one of them;
`xenon-firmware.md` is what a lane picked, and it is expected to change.*

Dell OptiPlex 7050 Micro, BIOS revision 1.27.0. Every setting below was read off a photograph of the machine's own setup UI, taken by
calef on 2026-09-04 during the first light of milestone 87 (the x86_64 bare-metal machine) (`notes/x86-uefi-boot.md`).

The originals are 70 `.HEIC` files, named `..._o_IMG_40NN.HEIC`. They are **not in this
repository**: 120 MB of a format no browser renders. They and this unit's identifiers are kept
off-tree. This note is the record and the photographs are the negatives, so each entry carries its `IMG_` number.

Anybody who changes a setting at the bench owes this file an edit; the next lane reads it instead of
walking to the machine.

## What nife depends on

The short list a bench session looks up.

| Setting | Value | Where | Why nife cares |
|---|---|---|---|
| Boot List Option | UEFI | IMG_4027 | The stick is `\EFI\BOOT\BOOTX64.EFI`; Legacy would look for an MBR boot sector that is not there |
| Secure Boot Enable | Disabled | IMG_4058 | The image is unsigned and nothing in this tree signs it |
| Enable Legacy Option ROMs | unticked | IMG_4028 | Off is what UEFI boot mode wants, and Secure Boot forbids it being on |
| Serial Port | COM1 (`3F8h`, IRQ 4) | IMG_4032 | Exactly where `arch::x86_64::port` and `drivers/ns16550.rs` look |
| Enable USB Boot Support | ticked | IMG_4036 | Without it the stick is not a boot device |
| Enable VT for Direct I/O | ticked | IMG_4084 | VT-d is on. See below |
| Enable Intel Virtualization Technology | ticked | IMG_4083 | |
| SATA Operation | AHCI | IMG_4033 | Not RAID, so the NVMe is a plain PCIe function rather than hidden behind Intel RST |
| M.2 PCIe SSD-0 | enabled, `Micron 2450 NVMe 256GB`, 256 GB | IMG_4034 | The NVMe DECISIONS §86 (whether an NVMe driver can leave the kernel) wants an IOMMU in front of |
| SATA-0 / SATA-4 | enabled, both `(none)` | IMG_4034, IMG_4026 | Nothing is attached; the only storage is the M.2 |
| Multi Core Support | All (4 cores) | IMG_4062 | The boot tour's "4 cores enumerated, 4 enabled" is a setting, not a coincidence |
| Memory Installed | 16384 MB (2x8192 MB DDR4-2400, dual channel) | IMG_4025 | See the discrepancy note below |
| ASPM | Disabled | IMG_4094 | PCIe links stay out of low-power states, which is one fewer variable for a first driver |
| TPM 2.0 Security | TPM On, Enabled, SHA-256 | IMG_4050 | `measured_boot` has a TPM here if it ever wants one |
| Wake on LAN/WLAN | Disabled | IMG_4071 | Nothing can wake this machine over the network |
| AC Recovery | Last Power State | IMG_4067 | A smart plug cycling the mains returns the machine to whatever it was |
| Warnings and Errors | Prompt on Warnings and Errors | IMG_4080 | This machine stops at POST and waits for a keypress. See the hazard below |
| Enable Keyboard Error Detection | ticked | IMG_4076 | A missing keyboard is one of the things it stops for |
| Fastboot | Thorough | IMG_4077 | Full hardware and configuration init every boot, which is the setting a bring-up wants |
| Enable UEFI Network Stack | ticked (2026-10-04) | photos owed | PXE needs a network stack |
| Integrated NIC | Enabled w/PXE (2026-10-04) | photos owed | The LOM offers PXE only in this mode |

### 1. Is VT-d enabled? Yes.

IMG_4084, panel "VT for Direct I/O": the checkbox `Enable VT for Direct I/O` is ticked. The
help text is Dell's own:

> This option specifies whether a Virtual Machine Monitor (VMM) can utilize the additional hardware
> capabilities provided by Intel® Virtualization Technology for Direct I/O.
>
> [NOTE: Trusted Execution requires VT for Direct I/O to be enabled.]

Its prerequisite is satisfied too: IMG_4083, `Enable Intel Virtualization Technology`, is ticked.

What this settles. `design/decisions/0086-el0-nvme-driver.md` was decided on 2026-09-03 and its
research recorded that no board this project owns has an IOMMU in front of a real NVMe controller.
xenon has both, and now both are known to be *switched on*. VT-d is enabled in firmware. A
`Micron 2450 NVMe 256GB` sits on M.2 PCIe SSD-0 with SATA in AHCI rather than RAID mode, so the
controller is an ordinary PCIe function rather than something Intel RST has swallowed. The
confined-driver experiment §86 exists to enable has hardware it can actually run on.

What it does not settle. A ticked box means the firmware was asked to bring the DMAR up. It is
not the same as a DMAR table with the remapping units this kernel expects, and nothing here has
read one. The next thing worth doing at the bench is dumping the DMAR, which the boot tour's ACPI
table list already gets close enough to touch.

### 2. Is interrupt remapping on? Not shown in the menu; the hardware offers it.

**Say this plainly rather than infer it: none of the 70 pages mentions interrupt remapping, an
interrupt remap table, DMA protection, or an IOMMU by any name.** The whole of the Virtualization
Support menu is three pages, all photographed, and this is all of it:

- IMG_4083, `Virtualization`: `Enable Intel Virtualization Technology`, ticked
- IMG_4084, `VT for Direct I/O`: `Enable VT for Direct I/O`, ticked
- IMG_4085, `Trusted Execution`: `Trusted Execution`, unticked

So the photographs were the wrong instrument for the question `notes/x86-uefi-boot.md` says only
xenon can answer. A 7050 of this vintage exposes one VT-d switch and no sub-controls. Whether the
remapping hardware reports the interrupt-remapping capability, and whether firmware left it enabled,
lives in the DMAR table's flags and in the unit's capability register, not in a menu.

That is a useful narrowing rather than a failure. It converts an open question from "go to the
bench and photograph a screen" into "read the DMAR", which is code this kernel can run on its own
and a lane can write without a null modem. `notes/confinement-claims.md`'s fifth claim (a confined
component's MSI-X write is a memory write, so DMA remapping does not cover it, which is why VFIO
refuses userspace drivers without interrupt remapping) stays latent, and now has a defined next
step: `design/roadmap/0378-read-the-dmar-on-xenon.md`.

The unit's half is answered. Milestone 317 (the interrupt-remapping flags) reads `ECAP.IR` on every
boot. xenon's tour of 2026-09-17 at 22:51 UTC printed `interrupt remapping offered (unused)` on its
`iommu` line (`bench/xenon-2026-09-17/tour-display-225100.log`, line 119). The platform's half is
unread on xenon. The same line now ends `dmar INTR_REMAP set` or `clear` (milestone 378 (read the
DMAR)), and the first xenon boot of a kernel carrying it is the one that says whether xenon's
firmware and its units agree.

### A number that disagrees with the boot tour, recorded rather than resolved

**Firmware's own System Information page says `Memory Installed = 16384 MB` and
`Memory Available = 16286 MB`** (IMG_4025), from two 8192 MB DDR4 DIMMs. `notes/x86-uefi-boot.md`'s
first-light account says the boot tour reported 17,119 MiB total and calls the machine
"17 GB of RAM".

Both were read honestly off a screen and they do not agree. This note does not resolve it, because
resolving it means looking at what the kernel is actually summing over 148 memory-map regions and
that is code rather than a photograph. It is recorded here so that the next person to quote either
number knows the other exists.

It does not disturb the `AlreadyMapped` hypothesis in that note, which needs only that RAM
reaches the framebuffer aperture at `0xd0000000` (3.25 GiB). Sixteen gigabytes does that as
comfortably as seventeen.

### The hazard a headless xenon will hit

**`Warnings and Errors` is set to `Prompt on Warnings and Errors` and `Enable Keyboard Error
Detection` is ticked, so this machine stops at POST and waits for a keypress when it does not find
a keyboard.** Dell's own help text on that page (IMG_4080) names a headless configuration as the
reason to change it. The machine's own event log (IMG_4093) carries **eight `Alert! Keyboard not
found` entries** across a year, so this is a thing it actually does rather than a thing it could do.
The help text, verbatim:

> The Warnings and Errors options cause the boot process to only pause when warnings or errors are
> detected, rather than stop, prompt and wait for user input. This feature is valuable in
> situations where the system may be remotely managed and therefore has no locally connected
> keyboard or console available for a user to respond; for example, in a headless configuration.
>
> Users may select to have the Power On Self-Test (POST) process either a) stop, prompt and wait
> for user input when warnings or errors are detected, or b) continue when warnings are detected
> but pause on errors, or c) continue when either warnings or errors are detected during the POST
> process.
>
> Note that errors deemed critical to the operation of the system hardware will always halt the
> system.

Nothing about this was hit during first light, because a keyboard and a monitor were both attached.
It matters for what `notes/bench-runbook.md` and `notes/serial-less-output.md` want next: a machine
that can be power-cycled and left to boot on its own. Two settings would have to change together
(`Continue on Warnings and Errors`, and keyboard error detection off), and both are an architect's
call because both change the machine's behavior for everything else it is used for.

## The full transcription

Every page of the 70 photographs, menu by menu with its `IMG_` number, is in
[`xenon-firmware/transcription.md`](xenon-firmware/transcription.md). The table above is the part
nife depends on. Beyond it, the transcription records a few facts worth knowing before a bench
session:

- `HT Capable: No` (IMG_4025). The i5-7500T has four cores and no hyperthreading, so the four
  logical CPUs the boot tour enumerates are four physical ones.
- SpeedStep, C-states and TurboBoost are all on (IMG_4063, IMG_4064, IMG_4066). The clock is free
  to move between 0.800 GHz and Intel's 3.3 GHz turbo ceiling for the i5-7500T, and the cores are
  free to sleep, which matters before anybody quotes a benchmark number off this machine. The
  firmware's "Maximum Clock Speed" of 2.700 GHz is the nominal clock, which is also the TSC's rate.
- Neither the admin nor the system password is set (IMG_4043, IMG_4044).
- Maintenance, Data Wipe, `Wipe on Next Boot` is unticked (IMG_4091). Leave it that way: the
  operation is not recoverable, cannot be stopped once started, and covers the M.2 PCIe SSD.
- The BIOS event log (IMG_4093) holds eight `Alert! Keyboard not found` entries and a run of
  `Alert! Cover was previously removed`. The second is the `On-Silent` chassis intrusion setting
  (IMG_4052), and it keeps appearing until somebody ticks `Clear Intrusion Warning`.

## What is not shown

Seventy photographs of a menu tree are not necessarily all of it, and this section is the honest
list rather than a disclaimer.

- No interrupt-remapping control exists anywhere in these pages. The whole Virtualization Support
  submenu is photographed, so this is an absence in the firmware rather than a gap in the
  photography. See question 2 above.
- The navigation tree is legible on exactly one frame (IMG_4025). Every other photograph either
  crops the left pane at the frame edge or shows it blank. So the menu structure above is
  reconstructed from that single frame plus the order the pages were photographed in, and a
  submenu that was never opened would leave no trace.
- **`POST Behavior` and `Manageability` pages are numbered in this note but their contents are
  carried in the entries themselves**, not in the section headers; if a page of either menu was
  never opened it does not appear here at all.
- No page for the F12 one-time boot menu is in this set. That is
  `art/bench/xenon-2026-09-05-boot-menu.jpg`, recorded in `notes/x86-uefi-boot.md`, and it is where
  the stick's `UEFI:` entry and the `UEFI: Micron 2450 NVMe 256GB` entry come from.
- No DMAR, no ACPI table contents, no PCI enumeration. None of that is in a firmware menu. The
  boot tour prints some of it and `notes/x86-uefi-boot.md` records what was seen.
- Help text is transcribed verbatim only where it carries a fact (the COM port addresses, the
  SATA modes, the Secure Boot precondition, the VT-d note). Everywhere else it is summarized or
  omitted, because Dell's help text is the same on every 7050 and the *setting* is what is specific
  to this machine.

## BUGS

- This is a record of 2026-09-04 and nothing keeps it true. No gate compares it to the machine,
  and no gate could: the machine is not on a network any lane can reach. A setting changed at the
  bench and not written here makes this note confidently wrong, which is worse than absent.
- Checkbox state was read from photographs of a screen, at an angle, in a dim room. A ticked and
  an unticked box differ by a few pixels of glyph. The settings in the "What nife depends on" table
  were each re-read from the original frame by a second pass; the long transcription in the appendix was
  not, and a single-character error is likelier there than in the table.
- The two memory figures in this tree disagree and this note does not resolve which is right;
  see "A number that disagrees with the boot tour" above.
- A ticked `Enable VT for Direct I/O` is a firmware intention, not an observed IOMMU. The kernel
  reads xenon's DMAR and the unit's `ECAP.IR` (question 2), so the IOMMU is observed. The DMAR's
  own `INTR_REMAP` flag is still not printed; that is what
  `design/roadmap/0378-read-the-dmar-on-xenon.md` has left.
