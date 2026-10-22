# radon, 2026-10-22 (UTC): milestone 249's series of 52 draws

Milestone 249 (the boot lottery is sampled by a person walking to the board), from 18:35 to 20:37
UTC. The image was `script/board-image --soak --reboot --tftp` from `80fd9428c`, milestone 593 (a
wedged kernel resets itself)'s branch with `main` merged, so every draw also armed the JH7110
watchdog. It printed `SEALED` and was served by `script/board-netboot`.

| file | what it is |
|---|---|
| `225-soak-e138.log` | the same board's console from 20:36 UTC, draw 52 soaking disarmed, until the cable was unplugged at 21:13 UTC (t=2281 s); it starts mid-boot, so it holds no banner |
| `249-lottery-draws.log` | `script/board-console --until none --quiet-after 0 --stop-after 52`'s capture, cleaned with `briefs/bench-session.md` step 6's `tr`: 52 draws |

## How the draws ran

Nobody touched the board. The series' first boot came from `reboot` typed at the ordinary image's
prompt (`README-53.md`); every boot after it came from the draw before it. The capture starts
during that first boot, so its 52 draws are the 52 that followed. Each reset went through
milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)'s direct AXP15060 write, and each
came back. No draw printed `FAILED`, and the watchdog reset none of them.

## How it ended

`--stop-after 52` sent its escape on draw 52's banner and saw no `DISARMED`. The watchdog's arming,
which comes after that banner, emptied the receive buffer and threw the byte away; milestone 593's
branch fixes that in `ffc076bcd`, after this image was built. A byte written to the console at
t=65 s, once both had armed, gave both `DISARMED` lines, and draw 52 was left soaking: 454 beats,
79,122 crossings, `refused=0 mismatch=0 stalled=0` throughout.

## The tally

`script/board-console --tally 249-lottery-draws.log`: 6 draws at zero clean cores (30,856 to
30,858/s), 46 at one (181,432 to 198,129/s), none at two or more. notes/soak/boot-lottery.md has
the table across every capture.
