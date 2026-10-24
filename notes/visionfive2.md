# The VisionFive 2: first silicon

Milestone 16a's board facts and bench runbook. Everything here was established from documentation
before the board was ever powered on, and every fact names its source. What documentation could not
establish is in "To measure at the bench" at the end, deliberately, rather than guessed. The board
arrived 2026-08-14.

The summary: the JH7110 is startlingly close to QEMU's `virt` machine. UART base, PLIC base,
CLINT base, OpenSBI, SBI HSM and Sv39 all match. The differences that remain are exactly four:
where DRAM starts, how the UART's registers are strided and clocked, how the PLIC numbers its
contexts, and the monitor core that must not be started.

The evidence for each section lives in [`visionfive2/`](visionfive2/README.md), one appendix per
topic, and each section below links the one it summarizes.

Sources cited throughout:

- [QSG] StarFive, VisionFive 2 Single Board Computer Quick Start Guide,
  https://doc-en.rvspace.org/VisionFive2/PDF/VisionFive2_QSG.pdf
- [dtsi] Linux, `arch/riscv/boot/dts/starfive/jh7110.dtsi` (SoC) and `jh7110-common.dtsi`
  (board), mainline as of 2026-08-14
- [uboot-doc] U-Boot, `doc/board/starfive/jh7110_common.rst` and `visionfive2.rst`
- [uboot-img] U-Boot, `arch/riscv/lib/image.c` (mainline; verified identical logic in
  StarFive's `JH7110_VisionFive2_devel` vendor branch)
- [uboot-bootm] U-Boot, `arch/riscv/lib/bootm.c`
- [uboot-pxe] U-Boot, `boot/pxe_utils.c`
- [uboot-cfg] U-Boot, `include/configs/starfive-visionfive2.h`
- [linux-hdr] Linux, `Documentation/arch/riscv/boot-image-header.rst`,
  `arch/riscv/include/asm/image.h`, `arch/riscv/kernel/head.S`

## The boot chain

Four stages live in the board's SPI flash and run before any byte of ours [uboot-doc]:

1. BootROM (on-die, 32 KB at 0x2A00_0000) reads the boot-mode pins and picks the media.
2. U-Boot SPL (flash offset 0x0) runs from SRAM at 0x0800_0000, initializes DRAM and PLLs.
3. OpenSBI (`fw_dynamic`, inside `u-boot.itb` at flash offset 0x100000) takes M-mode and stays
   resident as the SBI.
4. U-Boot proper runs in S-mode at 0x4020_0000 and loads the payload from microSD or TFTP.

So the contract our kernel meets on the board is the one it already speaks on QEMU `virt`: entered
in S-mode with OpenSBI behind the SBI calls, `a0` = boot hart id, `a1` = device-tree pointer.
U-Boot's jump is literally `kernel(gd->arch.boot_hart, images->ft_addr)` [uboot-bootm], so the
OpenSBI register contract survives U-Boot unchanged.

One difference in who the boot hart is: on QEMU `virt` every hart is identical and OpenSBI's
lottery picks any of 0..3. On the JH7110 hart 0 is the S7 monitor core (see "Harts" below), so the
boot hart will be one of the U74s, harts 1..4.

## How the board differs from QEMU virt

Each row is built and QEMU- or host-proven; what QEMU cannot emulate is a bench fact.
[`visionfive2/jh7110-against-virt.md`](visionfive2/jh7110-against-virt.md) holds the evidence and
the code each row names.

| | QEMU `virt` | VisionFive 2 | What the kernel does |
|---|---|---|---|
| DRAM base | 0x8000_0000 | 0x4000_0000 [dtsi] | reads `/memory` from the DTB; boot table maps gigapage 1 too |
| Kernel runs at | 0x8020_0000 | 0x8020_0000 | Image header `text_offset = 0x40200000`, so `booti` relocates there |
| UART reg-shift, io-width | 0, 1 | 2, 4 [dtsi] | `Shape` read from the DTB before the first `println!` |
| UART clock | 3.6864 MHz | 24 MHz [uboot-cfg] | divisor only from a stated clock (13 at 24 MHz) |
| UART PLIC irq | 10 | 32 [dtsi] | read from the console node's `interrupts` |
| PLIC S context of hart h | `2h + 1` | `2h` | decoded from `interrupts-extended` |
| Hart 0 | a normal hart | the S7: no MMU, no S-mode | refused by `status` and by its `riscv,isa` |
| Timebase | 10 MHz | 4 MHz [dtsi] | read from the DTB |
| PCIe | generic ECAM | PLDA XpressRICH, no driver | windows mapped only when the DTB has an ECAM node |

The `text_offset` is an exception and a foot gun, on the record. Linux means "2 MiB into RAM,
wherever RAM is"; ours means "0x8020_0000 absolute, on any board whose RAM starts at 0x4000_0000".
A future board with a different DRAM base gets the wrong address from this header.

The DTB caveat matters at the prompt. `$fdtcontroladdr`, the control DTB near the top of RAM, is
outside the boot page table, so every boot path moves it to 0x8600_0000 (inside gigapage 2).

## "The test suite where semihosting allows", concretely

There is no semihosting on this board. The riscv test exit is QEMU's `sifive_test` finisher, an
MMIO word at 0x10_0000 the JH7110 does not have. The board build reports instead with a UART marker
line (`NIFE-TEST-EXIT: PASS` or `FAIL <code>`) and SBI SRST shutdown, which landed on 2026-08-21
([`visionfive2/on-board-test-suite.md`](visionfive2/on-board-test-suite.md)).

## Harts, and what the bench stops found

Five harts, one of which must not be started: 1x SiFive S7 (hart 0) and 4x U74 (harts 1..4). The
first nights on silicon, 2026-08-14 and 2026-08-15, made five bench stops. Other files cite them by
number, and each conclusion is here with the appendix that holds it.

- The second bench stop: the vendor tree lies about the S7. It marks all five cpu nodes `okay` and
  gives cpu@0 an Sv39 MMU, and vendor OpenSBI dies if asked to start it. Startability therefore
  requires supervisor mode, read from the hart's own `riscv,isa`. A bare `u` without `s` in the
  single-letter run is a denial; a modern string with no privilege letters is silence.
  [`visionfive2/hart-roster-stops.md`](visionfive2/hart-roster-stops.md) has it, with the first
  stop's correction.
- The third bench stop: the online set was {1,2,3}, the first not contiguous from zero, and every
  count-as-index loop was wrong. Fixed in `1329874` and swept; `crates/cpu_set` holds the shape.
  [`visionfive2/hart-roster-stops.md`](visionfive2/hart-roster-stops.md)
- The fourth bench stop: boots 7 and 8 looked like a hang and were read as an undelivered wake.
  The wake gate and the pop-own-current guard were built against it and stay as hardening.
  [`visionfive2/boots-7-and-8.md`](visionfive2/boots-7-and-8.md)
- The fifth bench stop: boots 9 and 10 overturned that reading. The "hung" dumps were the terminal
  state of a finished tour, identified five independent ways, so no corruption has been observed
  on this board. [`visionfive2/boots-9-and-10.md`](visionfive2/boots-9-and-10.md)
- Boots 12 to 14 closed the story: measured boot refused a mismatched pair, then ran the right
  pair, and boot 14 put all four U74s online.
  [`visionfive2/bench-boots-2026-08.md`](visionfive2/bench-boots-2026-08.md)

## SBI extensions

OpenSBI is the vendor firmware's M-mode resident, so TIME, IPI, RFENCE and HSM are the standard
set, and SRST is how the board reboots or powers off from S-mode. The OpenSBI version and its PMU
extension are bench questions, read off the boot log rather than guessed.

## Boot-mode switches

Two DIP switches (RGPIO_1, RGPIO_0) select the boot media, read once at power-on [QSG]:

| RGPIO_1 | RGPIO_0 | Mode |
|---|---|---|
| 0 (L) | 0 (L) | 1-bit QSPI NOR flash (the vendor firmware; use this) |
| 0 (L) | 1 (H) | SDIO 3.0 (SD card holds the firmware too) |
| 1 (H) | 0 (L) | eMMC |
| 1 (H) | 1 (H) | UART recovery (XMODEM loader) |

QSPI is both the factory arrangement and StarFive's recommendation (the QSG notes SD/eMMC boot
fails on some cards) [QSG]. It is also what we want: the flash's SPL + OpenSBI + U-Boot chain
stays untouched, and our payload rides a microSD card that U-Boot merely reads files from. UART
recovery (1:1) is the unbrickable fallback if flash is ever corrupted [uboot-doc].

## Serial wiring

The debug console is UART0 on the 40-pin header, 3.3 V TTL (the pins tolerate nothing higher)
[QSG]:

| Header pin | Signal | Connect to USB-serial |
|---|---|---|
| 6 | GND | GND |
| 8 | UART0 TX (GPIO 5 [dtsi]) | RX |
| 10 | UART0 RX (GPIO 6 [dtsi]) | TX |

115200 8N1, no flow control [QSG]. On macOS:
`screen /dev/cu.usbserial-* 115200`. Cross TX to RX; leave the adapter's VCC pin unconnected (the
board has its own power).

## The microSD payload

`script/board-image` (name provisional) builds three files: the flat `Image`-format kernel, the
userspace archive the kernel measures, and a U-Boot boot script. They are one set. The kernel
compiles in a hash of its archive, so a new kernel over an old archive halts at
`MEASURED BOOT REFUSED`. That is why `--card <dir>` copies all three onto a card you formatted and
mounted. It formats nothing; that is the person at the bench's decision. Before walking to the
board, ask the card the board's question:

```
script/card-check /Volumes/NIFE     # 0 it boots, 1 it halts at MEASURED BOOT REFUSED, 2 nothing there
```

The card is one FAT32 partition carrying `boot.scr.uimg` and no `extlinux.conf`. The extlinux path
is a dead end on this board, because U-Boot's pxe path hands `bootm` no device tree. The boot
script is the manual sequence, unchanged: load the kernel and the archive, move the DTB to
0x8600_0000, `booti`. [`visionfive2/microsd-payload.md`](visionfive2/microsd-payload.md) has the
script line by line, the address choices and the captured transcripts.

## Booting over the network

Milestone 257 (boot radon over the network) serves the payload over TFTP from the Mac that builds it
and that radon's UART goes into, so the card stops being the tax. Two commands, in two terminals:

```
script/board-image --tftp --card /Volumes/NIFE   # once, ever
script/board-netboot                                # in the other terminal, while you work
```

Then every later boot is `script/board-image --tftp` and a power cycle. The card is still
underneath: the script falls back to the card's own copy when any transfer fails, and takes both
files from one place, never half a pair. The server address is read off the serving machine when
the card is written and echoed at boot. When it has moved, `setenv nife_boot_server <addr>` then
`source ${scriptaddr}` retries at the prompt. A TFTP-loaded boot measured the same as a card boot.
[`visionfive2/network-boot.md`](visionfive2/network-boot.md) has the design, the measurements and
why this is not dnsmasq.

## The bench runbook

Setup, in order:

1. microSD: format it once by hand (`diskutil eraseDisk FAT32 NIFE MBR /dev/diskN`, and be certain
   of the device), then `script/board-image --tftp --card /Volumes/NIFE` puts the matched set on it
   with a boot script that fetches over the network and falls back to what is on the card. Eject,
   insert the card. This step is once, not once per boot, which is what the section above is
   for; `script/board-image --card /Volumes/NIFE` without `--tftp` is still the card-only script
   and is what to write when there will be no serving machine.

   Then, on that Mac and in a second terminal, `script/board-netboot`, for as long as the session
   lasts. It serves `target/board` on udp/69 and prints the `setenv nife_boot_server` line for each
   address the board might reach it on. Leave it running: every later boot is
   `script/board-image --tftp` and a power cycle, with no card in anybody's hand.
2. DIP switches to QSPI: RGPIO_1 = 0 (L), RGPIO_0 = 0 (L) [QSG].
3. Serial: pins 6/8/10 as wired above, 115200 8N1, terminal attached before power so the SPL
   banner is not missed. `script/board-console` (milestone 216) is that terminal, and it recognises
   the sequence rather than leaving it to your eyes: it logs every byte to a file, stops on a
   deadline, and returns different exit status for a hang than a refusal. See
   notes/board-console.md. A `screen /dev/cu.usbmodem* 115200` still works and is what to reach for
   when you need to *type* at U-Boot, which the tool deliberately cannot do.
4. Power: USB-C. The board boots on power, there is no power button.

Transcripts of a successful manual boot and of the extlinux failure are committed under
`crates/serial_console/tests/fixtures/captured/`; read those rather than re-deriving what the board
prints. The card's U-Boot environment is degraded (`bad CRC, using default environment`), and the
board boots through it.

What appears, in order, on a good day: the SPL banner, OpenSBI's banner (version line included:
record it), U-Boot's banner and countdown, then either the extlinux menu or the `StarFive #`
prompt for the manual commands, then `## Flattened Device Tree`/`Starting kernel ...`, then ours:

With a `--tftp` card the boot script says which path it took before any of that. The line is
worth reading rather than skipping. A session that thinks it is testing a fresh build off the
network, and is silently booting the card's older copy, is a session whose numbers mean nothing:

```
nife: tftp server is 192.168.8.216, setenv nife_boot_server to point somewhere else
nife: payload came from net
```

`payload came from card` is the fallback having fired, and `nife: nothing came over the network,
falling back to the card` on the line above says so explicitly.

a blank line and

```
nife on RISC-V (rv64, S-mode, Sv39)
```

(`kernel/src/main.rs`; the console comes up before the DTB is touched, so this line precedes any
memory-map work).

### The failure-triage ladder

| Symptom | Most likely cause, in order |
|---|---|
| Nothing on serial at all | TX/RX not crossed; wrong device (`cu.*` vs `tty.*`); DIP switches not on QSPI; a bad SPI flash (fall back to UART recovery mode [uboot-doc]) |
| Firmware banners but garbage | Baud mismatch in the terminal (must be 115200); a 5 V adapter on 3.3 V pins has by then possibly cost a board |
| U-Boot fine, `Bad Linux RISCV Image magic!` | The file is the ELF, not the objcopy output; `script/board-image` verifies the magic at offset 0x38 at build time, so a stale card is the other suspect |
| `payload came from card` when you expected `net` | The serving machine is not running `script/board-netboot`; or its address moved and the card's baked one is stale (the `tftp server is` line one above says which address was tried); or the board is on the other ethernet port. `setenv nife_boot_server <addr>` then `source ${scriptaddr}` retries without a card reader |
| `nife: still at the prompt, so nothing loaded` | Neither path had a payload: no serving machine AND no card, or a card whose files are named something else. The board is at the prompt and everything is still typeable |
| `Starting kernel ...` then silence | Expected until the UART driver handles reg-shift/io-width: the kernel may be running and polling LSR at the wrong offset. Also: DTB left at `$fdtcontroladdr`/`fdt_addr_r` (outside the boot map, faults with the trap path not yet printing); or the relocation did not happen (check U-Boot printed `Moving Image from ... to 0x80200000`) |
| `Starting kernel ...` then garbage | Kernel is alive and the divisor is wrong: driver reprogrammed the divisor against the wrong clock (needs 13 at 24 MHz, not 1) |
| Banner, then hang or trap dump | DTB parsing or the memory map: RAM at 0x4000_0000 exercises paths QEMU never did (bitmap placement, gigapage 1 unmapped, the S7's cpu node in `smp::init`, the PLIC context formula) |
| Banner, then an **OpenSBI** trap dump (`mepc` in firmware, hart 0) | The kernel started the S7: the vendor tree's `status`/`mmu-type` lies got past the roster. The supervisor rule ([`visionfive2/hart-roster-stops.md`](visionfive2/hart-roster-stops.md)) exists to refuse hart 0; if this dump is back, that gate has regressed or the tree found a third lie |

## Read off the board, 2026-09-03

The first session that drove radon from a script settled three guesses. `boot.scr.uimg` works, with
14.7 seconds from power to the end of the tour and nothing typed. `scriptaddr` is 0x4390_0000 and
`fdt_addr_r` is 0x4600_0000, outside the boot map, as the DTB caveat predicted. And the TRNG node
is present under the vendor spelling `trng@1600C000`, `starfive,trng`, `status = "disabled"`, which
is why the driver skipped. [`visionfive2/read-off-the-board-2026-09-03.md`](visionfive2/read-off-the-board-2026-09-03.md)
has the transcripts and the correction.

## The eight-hour soak, 2026-09-25

Clean over 8 h 09 m and 4.1 million cross-core handoffs, on one fast draw. The account and every
anomaly are in [`visionfive2/soak-2026-09-25.md`](visionfive2/soak-2026-09-25.md).

## To measure at the bench

Facts documentation could not settle, each an explicit measurement, none guessed above.
[`visionfive2/bench-questions.md`](visionfive2/bench-questions.md) has each in full.

1. The OpenSBI version, its SBI extensions, and the PMU's hpmcounters: now two lines of a boot log
   (`firmware    :`, `cycles      :`), per notes/riscv-cycle-counters.md.
2. What `sbi_hart_start` returns for hart 0. Measured 2026-08-14: vendor OpenSBI starts it and
   dies, so the refusal is ours.
3. The vendor U-Boot's environment and distro-boot scan of our card.
4. Whether vendor `booti` relocates as mainline does (the `Moving Image` line).
5. The boot hart id in `a0`, and `smp.rs`'s hwid-vs-index assumptions with hart ids 1..4.
6. This board's DRAM size, and `/memory` and bitmap placement with RAM at 0x4000_0000.
7. That byte-wide UART access at unshifted offsets truly fails, and the DW busy quirk.
8. Boot-to-banner wall time, as the first real-hardware number.
9. The TRNG node's spelling at the prompt, and its clocks and reset (milestones 239 and 159; the
   `hw entropy` line answers both). It is `design/fatal-risks/README.md`'s risk 6.
10. Whether this U-Boot can boot from a USB stick, and through UEFI (§157 (a trivial install is a web page, a USB drive, and packages)). The five
   commands and what each result means are in the appendix.

## BUGS

The bench history, boot by boot, is in
[`visionfive2/bench-boots-2026-08.md`](visionfive2/bench-boots-2026-08.md) and
[`visionfive2/on-board-test-suite.md`](visionfive2/on-board-test-suite.md). What is still open:

- An interrupt test fails on the board. `an_interrupt_that_arrives_before_the_wait_is_not_lost`
  never reaches the trap handler after the THRE fix. The PLIC/hart affinity path is the next place
  to look, not the UART; start by printing `crate::cpu::id()` and the assigned context inside
  `arch::irq::target_context` on the board.
- The shell path's userspace input driver (`components/src/input.rs`) still reads the UART at
  QEMU's byte stride, so on the board the shell's input reads garbage. It bites at the shell
  milestone.
- The `#[test_case]` suite assumes QEMU's synthetic devices. The `skip!()` of milestone 145 (a test that needs hardware the boot doesn't have can say so) covers the
  tests found so far; more may surface.
- That a keystroke at the board's prompt reaches the UART driver on source 32 is not yet observed.
- The `text_offset` in the Image header encodes one board's DRAM base; `boot.s` and the
  appendix carry the caveat.
- Everything cited from "mainline" describes current upstream; the flash runs StarFive's vendor fork
  of unknown vintage. The relocation logic was verified in the vendor branch, the environment
  defaults were not.
