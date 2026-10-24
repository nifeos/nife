# To measure at the bench, in full

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md), which keeps the list in short form. These
are the facts documentation could not settle, each an explicit measurement and none guessed.

1. OpenSBI version in the shipped flash (banner), and which SBI extensions `sbi probe` reports.
   Specifically: whether PMU is present, and how many hpmcounters it exposes on the U74s. The
   kernel now answers most of this itself (milestone 74 (cycle counters)). It probes the PMU
   extension and prints the result on the `firmware    :` line. Then it prints which counter and
   CSR firmware gave it for CPU cycles, on a `cycles      :` line beside it.
   notes/riscv-cycle-counters.md is the procedure, with a table for each line of output. This row
   is now "read two lines of a boot log" rather than "type at U-Boot".
2. What `sbi_hart_start` returns for hart 0, the disabled S7: an error, or a start that must never
   be requested. Measured 2026-08-14: the worse answer. Vendor OpenSBI does not refuse it. It starts
   the S-incapable core and dies in its own trap handler (see
   [hart-roster-stops.md](hart-roster-stops.md)). The refusal has to be ours, and now is.
3. The vendor U-Boot's actual environment. Does its distro boot scan our single-partition card?
   Vendor firmware predates some mainline conventions. And what are the values of `kernel_addr_r`
   and `fdt_addr_r` in the flashed environment (`printenv`)? The parent documents them from
   mainline [uboot-cfg].
4. Whether `booti` in the vendor build relocates as mainline does (the `Moving Image` line). The
   source says yes [uboot-img]. The flash is whatever was built from it.
5. The boot hart id OpenSBI hands us (`a0`), and whether `smp.rs`'s hwid-vs-index assumptions hold
   with hart ids 1..4.
6. DRAM size of this specific board (the memory node U-Boot patches in). And whether the `/memory`
   walk and bitmap placement behave with RAM at 0x4000_0000.
7. A UART reality check: that byte-wide access at unshifted offsets truly fails (predicted, not yet
   observed), and the DW busy quirk's visibility.
8. Boot-to-banner wall time, once there is a banner, as the first real-hardware number.
9. Whether the tree radon hands us carries the TRNG node under the vendor spelling
   (`trng@1600C000`, `compatible = "starfive,trng"`, `status = "disabled"`). That is what the
   firmware's source says, and nobody has yet confirmed it at the prompt. Also, whether its clocks
   and reset are left running by U-Boot. The first question belongs to milestone 239 (radon's device tree
   does not describe the TRNG, so a working driver never runs), and its block carries the two
   commands. The second belongs to milestone 159 (a real hardware entropy source: the JH7110's TRNG). Both are read off the `hw entropy` line the riscv64
   boot tour now prints last. `design/roadmap/0159-jh7110-trng-driver/bench-procedure.md` carries the ordered bench
   procedure and a table of what each possible line means. This is the first real,
   non-virtio device a confined userspace process on this project has been asked to drive. That
   makes it `design/fatal-risks/README.md`'s risk 6 rather than a driver.
10. Whether this U-Boot can boot from a USB stick, and through UEFI. This was added 2026-09-19,
   from §157 (a trivial install is a web page, a USB drive, and packages): the customer's stick should be the bench's stick too. Nothing in the tree
   says, and the boot log is suggestive rather than decisive. U-Boot 2021.10's init lists `MMC` and
   `Net` and no USB line (`bench/radon-2026-09-04/probe-234257.log`), so it does not bring USB up by
   itself. The BootROM cannot boot USB at all; the boot-mode table has no USB row. So "radon boots
   from USB" means U-Boot, loaded from flash as today, reads nife from the stick. Recalled, not
   read: the USB 3 ports sit behind a VL805 PCIe controller. Whether this vendor build drives it is
   the open question. With a FAT32 stick in a USB port, at `StarFive #`:

   ```
   usb start
   usb storage
   fatls usb 0:1 /
   help bootefi
   printenv boot_targets
   ```

   | Result | Meaning |
   |---|---|
   | `usb storage` lists the stick and `fatls` shows its files | radon reads USB; a nife boot from the stick is one boot script or one `bootefi` away |
   | `usb start` finds no storage, or the command is missing | this firmware cannot; the next step is a newer StarFive U-Boot in SPI flash, recoverable over UART (boot mode 1:1) |
   | `help bootefi` prints usage | the universal stick's premise holds on radon; notes/boot-stick.md, "At the bench", has the two commands that then boot it |
   | `boot_targets` contains `usb` | U-Boot scans USB unprompted, with no script |
