# The microSD payload, in detail

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md), which keeps the commands. This file holds
why the card is one matched set, what `script/card-check` reads, why extlinux is a dead end on this
board, and what the boot script says line by line.

## What `script/board-image` builds

`script/board-image` (name provisional) builds the payload. It holds the flat `Image`-format kernel
(`llvm-objcopy -O binary`, header at offset 0), the userspace archive the kernel measures, and a
U-Boot boot script. Given `--card <dir>` it copies all three onto a card you have already formatted
and mounted. Without it, it prints the steps. It still formats nothing. `dd` and
`diskutil eraseDisk` name a whole device, and that is the person at the bench's decision.

## Three files, and they are one set

The kernel compiles in a hash of the archive it was built against. So a card carrying a new kernel
over an old archive halts at `MEASURED BOOT REFUSED`. That is the gate working, and it fired for
real on 2026-09-01. It is why `--card` exists at all. Milestone 217 (the card carries a kernel and an archive from different builds) asked whether a script may copy
files. Its answer: copying a set into a named, mounted filesystem is not the destructive act the
formatting steps are, and only the script can make the set indivisible.

## Ask the board's question before you walk to the board

This is milestone 223 (read a card and say whether its kernel and archive match). `--card` makes a
card written by this script consistent by construction, and that is all it can do. A card written
by hand, written by an older script, half-copied, or carried in from another machine is still
expressible. So:

```
script/card-check /Volumes/NIFE     # 0 it boots, 1 it halts at MEASURED BOOT REFUSED, 2 nothing there
```

It reads the card and writes nothing. It hashes each archive entry the kernel may enter. Then it
looks for that digest in the kernel image's compiled-in trust root, which is the fact the board's
own refusal turns on. When the answer is no, it says which of the two files is the stale one, by
comparing both against `target/board`. `script/board-image` now runs it on every payload it builds
and on every card it writes. So the ordering this section relies on is read off the bytes rather
than asserted in a comment. The kernel's check is still the authority, and this one is the early
warning. If they ever disagree, the board is right.

## The card layout

U-Boot's distro boot wants one FAT32 partition [uboot-doc]; MBR or GPT both work. The special GPT
partition GUIDs in [uboot-doc] matter only when the card holds the firmware itself, and ours stays
in QSPI flash. U-Boot scans each partition first for `/extlinux/extlinux.conf`. Then it scans for
the boot scripts named in `$boot_scripts`, which is `boot.scr.uimg boot.scr` [uboot-doc].

## The extlinux path is a dead end on this board

The reason is upstream of us. Milestone 218 (every boot of the VisionFive 2 needs a human typing four commands into U-Boot) captured it on 2026-09-01 in
`crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-extlinux-refused.log`. With no
`fdt`/`fdtdir` line in the label, U-Boot's pxe path hands `bootm` no device tree at all. RISC-V's
`boot_prep_linux` refuses rather than guessing:

```
Moving Image from 0x40200000 to 0x80200000, end=802ff000
Device tree not found or missing FDT support
### ERROR ### Please RESET the board ###
```

Read what that transcript does *not* say. The image was loaded and relocated, and then the firmware
stopped. No instruction of ours ran. So this is not the boot-map caveat below arriving as a fault,
and widening the kernel's page table cannot touch it. The error is `boot_prep_linux`'s `hang()`
[uboot-bootm], which only the reset button clears. The vendor build's own
`bad CRC, using default environment` means the `fdt_addr_r` that pxe path wanted was not there to
fall back to.

## The boot script

So the card carries `boot.scr.uimg` and no `extlinux.conf`, and U-Boot's script scan runs it. The
script is the manual sequence, unchanged, which is the point. Every line of it is a line the same
day's successful boot already proves (`vf2-2026-09-01-manual-boot.log`). `cargo xtask board-script`
writes it, and `target/board/boot.cmd` is the same text in readable form. This is what it says:

```
echo nife: boot.scr is driving this boot, milestone 218
load ${devtype} ${devnum}:${distro_bootpart} ${kernel_addr_r} /nife-vf2.img
load ${devtype} ${devnum}:${distro_bootpart} 0x90000000 /nife-initial-ramdisk.img
setenv nife_archive_size ${filesize}
fdt addr ${fdtcontroladdr}
fdt move ${fdtcontroladdr} 0x86000000
booti ${kernel_addr_r} 0x90000000:${nife_archive_size} 0x86000000
```

0x8600_0000 is inside boot gigapage 2. It is clear of the image, which ends well below 0x8100_0000,
and of `kernel_comp_addr_r` = 0x8800_0000 [uboot-cfg]. 0x9000_0000 is clear of both. The device
comes from the variables distro boot sets before sourcing a script, not from a literal `mmc 1:1`.
The archive's length is stashed under a name of ours the moment `load` reports it. So nothing later
that touches `filesize` can change what `booti` is handed.

The DTB fallback caveat still stands for anyone hand-typing `booti`. `$fdtcontroladdr` is the
control DTB near the top of RAM. It sits above gigapage 2 on every variant and above 4 GiB on this
8 GB board. That is why the sequence moves it to 0x8600_0000 rather than passing it where it lies.

## What the 2026-09-01 captures settled

Both halves of this were captured on 2026-09-01. The transcripts are committed under
`crates/serial_console/tests/fixtures/captured/`: a successful manual boot, and the extlinux path
failing. Read those rather than re-deriving what the board prints. They settled two facts the
parent note did not have.

The extlinux path does not work. It loads and relocates the image, then prints
`Device tree not found or missing FDT support` and `### ERROR ### Please RESET the board ###`. That
is not the fallback-DTB caveat arriving as a firmware error, which is how the runbook first read it.
U-Boot passed `bootm` no device tree at all, so nothing of ours ran and the kernel's boot map was
never consulted. Milestone 218 replaced that path with a boot script on the card, which types the
manual sequence for you. The commands themselves are unchanged.

And the card's U-Boot environment is degraded: `bad CRC, using default environment`, repeated
`Invalid partition 3`, and `"boot2" not defined`. The board boots through all of it, and none of it
is our payload's doing.
