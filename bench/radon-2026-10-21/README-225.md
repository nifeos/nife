# radon, 2026-10-21: milestone 225's twenty-five reboot draws

The files in this directory that milestone 225 (run the soak on radon, argon and xenon) added. The
rows they produced are E26 to E50 in `notes/multicore-defect-curve.md`, and the run table is
`notes/soak/radon-2026-10-21.md`.

| file | what it is |
|---|---|
| `soak-reboot-draws.log` | `script/board-console --until none`'s capture, cleaned with `briefs/bench-session.md` step 6's `tr`: draws 1 to 25, cut at draw 25's beat 20 |
| `soak-stop.log` | `script/board-console --stop`'s capture, cleaned the same way: the rest of draw 25, draw 26's boot, the escape and the board's acknowledgement |
| `soak.netboot.log` | `script/board-netboot`'s transcript: every TFTP fetch the board made |

## The image

Built from the lane's claim commit, an empty commit on `cda4ceb55`, with `script/board-image
--soak --reboot --tftp`. It printed `SEALED`. The same tree's QEMU rehearsal is in `README.md`.

## How the boots ran

calef powered radon on once, at about 01:31 UTC. The card's boot script fetched from the TFTP host
directly, so no key was sent at U-Boot. The console was watched by `script/board-console` for the
whole run. Handing the port from the watcher to `--stop` at 02:26:13 UTC lost under a second, in
draw 25, between beat 20 and the next line. `--stop` sent its one byte, `0x0d`, during draw 26, and
the board acknowledged it and disarmed its reboot loop. Draw 26 was left soaking on radon.

24 of the 25 resets went through milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)'s direct AXP15060 write. Draw 2's did not; the run
table's "The unexplained reset" has it.
