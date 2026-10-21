# radon's boot lottery: the rebooting soak's draws so far

*An appendix to [`notes/soak.md`](../soak.md), for milestone 249 (the boot lottery is sampled by a
person walking to the board). It holds the distribution that milestone's proof asks for, read off
the captures already in `bench/`, beside the nine hand-drawn boots in
[`rebooting-soak.md`](rebooting-soak.md).*

## The count

**41 unattended draws. The proof asks for at least fifty, so 9 remain.**

A draw counts here when it ran the rebooting soak (`script/board-image --soak --reboot`) and its
boot began from the previous draw's own reset, with nobody at the board. Two kinds of boot are left
out, and both are in the table below so the count can be checked:

- The first boot of each capture, which did not begin from a reset in the capture. On 2026-10-20
  and 2026-10-21 the record says calef powered radon on for it. There are four. Counting them gives
  45 draws and 5 remaining, but a power-on is the hand-drawn experiment, not this one.
- Row E6, the second boot in `soak-boot2.log`. It began from E5's reset with nobody there, but it
  booted the plain soak image for a ten-hour run, not the rebooting soak. The tally reads it as a
  draw (one clean core, 178,629/s) and this count does not.

Every number here is `script/board-console --tally` run over the cleaned capture, unedited. The
tally read all five files; nothing was counted by hand.

| capture | build | draws | of them unattended | 1 clean core | 2 clean cores |
|---|---|---|---|---|---|
| `bench/radon-2026-10-10/soak-boot2.log` | `bef2b40dc` | 2 | 0 (draw 2 is E6) | 1 | 1 |
| `bench/radon-2026-10-10/592-optionb-four-cycles.log` | `23f0c8d87` | 5 | 4 (draws 2 to 5) | 2 | 3 |
| `bench/radon-2026-10-20/soak-reboot-draws.log` | `22c19e1d5` | 13 | 12 (draws 2 to 13) | 13 | 0 |
| `bench/radon-2026-10-21/soak-reboot-draws.log` | `cda4ceb55` | 25 | 24 (draws 2 to 25) | 25 | 0 |
| `bench/radon-2026-10-21/soak-stop.log` | `cda4ceb55` | 1 | 1 (draw 26) | 1 | 0 |

The distribution of the 41 unattended draws:

| clean cores | draws | rates seen |
|---|---|---|
| 0 | 0 | |
| 1 | 38 | 177,520 to 206,734/s |
| 2 | 3 | 317,194 to 318,796/s |
| 3 | 0 | |
| 4 | 0 | |

Three and four clean cores have still never been drawn, by hand or unattended.

## What each number means, and where it is not comparable

- A count of draws is not a probability. The tally prints this beside every table, and it holds
  here twice over, because the 41 come from four builds.
- The builds do not look alike. The two 2026-10-10 builds drew two clean cores on 4 of their 6
  rebooting-soak boots. The two later builds drew one clean core on 39 of 39. Pooling them into one distribution hides
  that split, so the per-capture table is the one to read. This page does not say why; why a
  placement lands where it does is DECISIONS 138's question, and no quantity of draws answers it.
- The nine hand-drawn boots are not the same experiment. They were power cycles on a 2026-09-03
  build and drew two clean cores six times. These are warm resets through the AXP15060 write
  (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)), which hold firmware state
  constant in a way a power cycle does not (249's `BUGS`).
- Rate is IPC throughput, not crossings. A draw at 1 clean core and about 190,000/s can hold 17
  crossings or 27,000; the crossing count is a column of milestone 225 (run the soak on radon,
  argon and xenon), in
  [`../multicore-defect-curve/radon-rows.md`](../multicore-defect-curve/radon-rows.md).
- Draw 26 was read at one beat. `--stop` disarmed it then and it was left soaking, so its row is
  the boot census and one rate, not 120 seconds.
- The `ended` column misreads one draw. Draw 2 of 2026-10-21 reset itself at about t=105 s
  without the kernel's reset lines, and the tally reports it as `log ends here` although the capture
  continues into draw 3. The reset is milestone 225's `BUGS`; the draw's placement and rate are read
  normally.
- Draw 25 of 2026-10-21 is split across two files at beat 20, by the port handover to `--stop`; it
  is one draw, counted once.

## What the next draws need

Nine more unattended draws of `script/board-image --soak --reboot`, captured whole and tallied the
same way. A series on one build is worth more than nine scattered ones, since the build split above
is the open question.
