# radon, 2026-10-20: milestone 225's thirteen reboot draws

The files in this directory that milestone 225 (run the soak on radon, argon and xenon) added. The
rows they produced are E13 to E25 in `notes/multicore-defect-curve.md`, and the run table is
`notes/soak/radon-2026-10-20.md`.

| file | what it is |
|---|---|
| `soak-reboot-draws.log` | the console capture, cleaned with `briefs/bench-session.md` step 6's `tr`: 13 boots, ending just before the 14th `U-Boot SPL` |
| `soak.netboot.log` | `script/board-netboot`'s transcript: every TFTP fetch the board made |
| `soak-driver.log` | the U-Boot driver's transcript, with the UTC time of each countdown it saw, each key it sent and each reset it observed |

## The image

Built from commit `22c19e1d5` with `script/board-image --soak --reboot`. It printed `SEALED`, with
features `board,soak_test,reboot_soak_test`. The same build passed its QEMU rehearsal first:
`script/soak-test --arch riscv64 --for 45s`, 787,279 round trips in 40 s, 3,743 crossings,
`refused=0 mismatch=0 stalled=0`.

## How the boots ran

calef powered radon on once, at about 19:58 UTC. Every later boot was the previous draw's own
reset: after 120 seconds of soak, the kernel wrote the AXP15060 directly (milestone 592 (radon's
cold reboot dies in OpenSBI's PMIC write)), and the board came back through `U-Boot SPL` and
`nife: payload came from net`. That held on 13 of 13 draws, with no plug cycle after the first.

The console was GNU `screen` logging the CH343 port, not `script/board-console`, so the
watcher's live checks did not run; `script/board-console --tally` and `--exposure` read the
finished file instead. At each U-Boot countdown the driver sent one key to stop autoboot, then
`setenv nife_boot_server 192.168.8.192`, then `boot`. The card's boot script (written 2026-09-16)
asks for an older server address and for the archive by its older name, `nife-initrd.img`, so the
TFTP root served a copy of the archive under that name too.

## How the soak ended

No `--stop` byte was sent. During draw 13 the TFTP root was switched to a different image, so draw
13's reset, at about 20:36:43 UTC, booted that image and not this one. The capture is cut just
before that 14th boot's `U-Boot SPL`.

## What it showed

Every boot's last beat, at t=120 s, reads `refused=0 mismatch=0 stalled=0`, and wakerate stayed at
401 to 404/s against radon's 400. Eleven draws crossed cores, 3,924 to 27,502 times each, 218,216
in all. Two (E16 and E22) held 19 and 23 crossings from the first beat to the last; they never
crossed and are excluded from the curve. The tally put all thirteen on one clean core, at 182,096
to 197,384/s.

The `nife self-test` verdict on every boot is `11 of 13 passed, 2 absent: blockdev deviceirq`.
radon has neither a virtio block device nor a device that interrupts on its own, so both absences
are its normal verdict.
