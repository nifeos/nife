# The x86_64 kernel port: the appendices to milestone 161

[`design/roadmap/0161-x86-64-kernel-port.md`](../0161-x86-64-kernel-port.md) carries the status,
why x86_64 is the third architecture, a one-line summary of each build step, where each open item
ended, the follow-on and the closing lane's results. This directory carries the working its prose
budget has no room for.

- [`early-boot.md`](early-boot.md): steps 1 to 10, from the PVH boot path to a PIT line routed
  through the IO APIC.
- [`ring-3.md`](ring-3.md): step 11 and item 3, the `syscall` pair, the `iretq` choice, the syscall
  ABI decision and the two `CR4` bits.
- [`processes-and-userspace.md`](processes-and-userspace.md): steps 12 and 13 and item 4, the
  scheduler, real processes, userspace, the latent bugs and the 67 skips.
- [`open-list.md`](open-list.md): items 0 to 2 as recorded, the port-I/O question and the renames.
- [`smp-and-vt-d.md`](smp-and-vt-d.md): items 5 and 6, the three SMP failures and the VT-d driver.
- [`closing.md`](closing.md): the 2026-09-19 lane's working and measurements.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default. calef names
directories.*

- `early-boot` Name: provisional. For steps 1 to 10 of the parent's "What was built". calef names
  files.
- `ring-3` Name: provisional. For step 11 and item 3. calef names files.
- `processes-and-userspace` Name: provisional. For steps 12 and 13 and item 4. calef names files.
- `open-list` Name: provisional. For items 0 to 2 of the parent's "What is still open" and the
  port-I/O question. calef names files.
- `smp-and-vt-d` Name: provisional. For items 5 and 6. calef names files.
- `closing` Name: provisional. For the parent's "Closing the follow-on (2026-09-19)". calef names
  files.
