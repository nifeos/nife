# The bench procedure, in order

An appendix to [milestone 159 (a real hardware entropy source: the JH7110's
TRNG)](../0159-jh7110-trng-driver.md). The main page is complete without it.

Written to be followed without rederiving anything. It needs radon powered on (the Kasa plug), a
serial console attached before power, and a microSD card. See
[`notes/visionfive2.md`](../../../notes/visionfive2.md) for the board facts this leans on, and its
failure-triage ladder for everything that goes wrong before the tour.

1. On the dev machine, from a checkout with this milestone's work on it:

   ```
   script/board-image
   ```

   It builds the userspace archive first and the kernel second, in that order, because packing the
   archive regenerates the measured-boot manifest the kernel compiles in.

2. Copy all three files to the card. The archive is not optional, whatever the script's own printed
   instructions say. That wording is from milestone 217 (the card carries a kernel and an archive
   from different builds), and a stale pair already cost a boot with `MEASURED BOOT REFUSED`:

   ```
   target/board/nife-vf2.img          -> /Volumes/NIFE/nife-vf2.img
   target/board/nife-initrd.img       -> /Volumes/NIFE/nife-initrd.img
   target/board/extlinux/extlinux.conf -> /Volumes/NIFE/extlinux/extlinux.conf
   ```

   The kernel and the archive must be from the same `script/board-image` run. A mismatched pair
   halts at `MEASURED BOOT REFUSED` before any of this milestone's code runs.

3. Insert the card, set the DIP switches to QSPI, serial at 115200 8N1, then power. Interrupt
   U-Boot's countdown and type these five, which are what `script/board-image` prints. The
   extlinux path does not work; that is milestone 218 (every boot of the VisionFive 2 needs a human
   typing four commands into U-Boot).

   ```
   StarFive # load mmc 1:1 ${kernel_addr_r} /nife-vf2.img
   StarFive # load mmc 1:1 0x90000000 /nife-initrd.img
   StarFive # fdt addr ${fdtcontroladdr}
   StarFive # fdt move ${fdtcontroladdr} 0x86000000
   StarFive # booti ${kernel_addr_r} 0x90000000:${filesize} 0x86000000
   ```

4. Read the `hw entropy` line, which is the last line of the tour before the banner. It is one of
   these, and each one means a different thing:

   | Line | What it means |
   |---|---|
   | `hw entropy  : JH7110 TRNG at 0x1600c000 served 32+32 bytes ... second differs; STAT after init 0x........ (256-bit: ...)` | The milestone. A confined userspace process drove a real non-virtio device, and a client got bytes through a capability naming no device. Record the whole line; it is the first such number this project has. Read the mode note before believing the byte count: it must say 256-bit. |
   | the same line, but the mode note says `128-BIT` | The device ignored the `MODE.R256` write, so only `RAND0..RAND3` are a generation's answer. Then 16 of every 32 bytes are not device output. The draws still differ and the tour still calls it a pass, because the tour cannot tell. The driver must then serve 16 bytes per generation instead of 32; nothing is wrong with the capability story. |
   | `hw entropy  : skipped (this machine's tree describes no starfive,jh7110-trng; ...)` | The board's own device tree has no TRNG node where mainline's binding says it should be. Capture the tree (see step 6) rather than guessing; this is the one fact the first lane flagged as unconfirmed. |
   | `hw entropy  : JH7110 TRNG at 0x..., but no 'jh7110_entropy' in the initial ramdisk` | The card has a stale archive. Redo step 2 with a matched pair. |
   | `hw entropy  : FAILED: ... bring-up diagnostic 0x0000000000000000 ...` | The register window read as nothing. Most likely the block's clocks are gated or its reset is not deasserted (see the driver's `BUGS`). Next most likely, the base address is not the TRNG. |
   | `hw entropy  : FAILED: ... bring-up diagnostic 0x<nonzero> ...` | The device answered and the sequence is wrong. The high 32 bits are `STAT` and the low 32 `ISTAT`; record the raw word. `STAT`: bit 3 `R256`, bit 8 `MISSION_MODE`, bit 9 `SEEDED`, bits 16-18 `LAST_RESEED` (`0x7` means unseeded/zeroized), bit 27 `SRVC_RQST`, bits 30/31 generate/seed in flight. `ISTAT`: bit 0 `RAND_RDY`, bit 1 `SEED_DONE`, bit 2 `AGE_ALARM`, bit 3 `RQST_ALARM`, bit 4 `LFSR_LOCKUP`. Every bit above is decoded in `crates/jh7110_entropy`. A bit outside them is undocumented in all three drivers and the TRM, and is a finding rather than a lookup. |
   | any line whose numbers all look like a success | Read them against `crates/jh7110_entropy` before theorizing. The 2026-09-04 session lost an hour to a diagnostic of `0x20` read as an `ISTAT` bit that does not exist. It was the number 32, in a word whose meaning changed with the report beside it. That word is unconditionally `(STAT << 32) \| ISTAT` now, so the ambiguity is gone, but the habit is the lesson. |

5. If the success line appears, do the three things that make it a measurement rather than an
   anecdote. Boot twice, and confirm the two first-draw prefixes differ across boots (a device
   reseeded per boot, rather than a constant baked into silicon or a stale register file). Read the
   mode note in that same line: anything but `256-bit` means the byte count is overstated, per the
   table above. And read the three timing figures, which are the last clause of the same line.

   Bring no stopwatch: the tour times itself. The line carries `since the pcie line N us` (the whole
   gap, which is what a stopwatch was measuring), the bring-up alone, and the two draws with a rate
   over their eight round trips. Record all three verbatim. [Milestone 306 (time the hw-entropy
   step)](../0306-time-the-hw-entropy-step.md) has what each one counts, and the QEMU reference
   figures to read them against. It also has the reason the bring-up excludes the `hw clock` line's
   own console time.

6. Whatever happened, capture the board's device tree while you have it. At the `StarFive #`
   prompt, run `fdt addr ${fdtcontroladdr}` then `fdt print /soc/rng@1600c000` (and
   `fdt list /soc` if that finds nothing). That answers the first lane's one unconfirmed fact. A
   blob dumped off the board is a drop-in fixture for `crates/jh7110_entropy`'s existing discovery
   test rather than a new code path.
