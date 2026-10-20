# The notes behind radon's exposure rows

*An appendix to [`notes/multicore-defect-curve.md`](../multicore-defect-curve.md). The table there
holds the rows; this holds what each bench evening's rows mean and what they leave out.*

E5 through E7 are one bench evening, 2026-10-09 to 2026-10-10 (milestone 225's redraw boots,
`bef2b40dc`, tree-identical to main `237cef6bc`). E5 is the 120-second draw of the rebooting soak that milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write) ended. Between its last beat and E6's `U-Boot SPL` banner, the board reset itself through OpenSBI's PMIC write and came back. That is 592's green outcome. E6 began on the self-reboot's netboot fetch and ended at its watcher's deadline. The board then soaked unwatched for about 7 hours until E7; those crossings are not recorded and are not evidence. E7's boot followed one discarded boot from the card. Its fetch failed because radon's ethernet cable was unplugged (`phy_startup() failed: -110` on both controllers). That boot never reached a heartbeat and is not a row. Zero
defects on all three.

The §259 ledger (at least 10 million crossings over at least 3 boots): counting the three plain
boots only, E4 + E6 + E7 = 10,288,805 crossings over 3 boots, which meets it. E5's reboot draw adds 20,474 on top. The series milestone 249 (the boot lottery is sampled by a person walking to the board) wants
is also unblocked by E5's reset, so future boots need no plug cycle.

E8 through E12 are milestone 592's option-B evening, `23f0c8d87`, one capture. Four 120-second draws
were each ended by the direct AXP15060 write (the reset milestone 592 (radon's cold reboot dies in
OpenSBI's PMIC write) built, proven twice-deterministic in the same log). A fifth boot ended when
calef powered the board off. The crossings spread across five identical 120-second builds, 104 to
25,223, is the boot-lottery distribution the series wanted. Milestone 249 (the boot lottery is
sampled by a person walking to the board) had never sampled it: a 243x range on identical silicon
and build, zero defects throughout.

E13 through E25 are one capture of thirteen 120-second draws, `22c19e1d5`, one power-on and every
later boot started by the direct AXP15060 write (13 of 13 resets clean). E16 and E22 are excluded
from the curve: their crossings stayed at 19 and 23 from the first beat to the last, so those soaks
never crossed cores. The other eleven add 218,216 crossings, zero defects. Their start times are
the U-Boot countdowns in `bench/radon-2026-10-20/soak-driver.log`. The account is
`bench/radon-2026-10-20/README-225.md`.

The start times are Pacific local times from `notes/soak.md` converted to UTC; the note does not
state its zone. The conversion is inferred from the commits that recorded them (2026-09-03 20:45 UTC
and 2026-09-04 00:11 UTC), which only a Pacific reading fits. None of the three logs is
in the tree and none records its build, which is why their columns are thin. So radon's curve
starts at roughly three and a half hours and 5,500-plus crossings with zero defects, which is a
confidence about one architecture and one workload, not a verdict about anything.

E4 is the first row with a log in the tree and a build recorded, and on the crossings axis it dwarfs
the three before it. It has 4.1 million against about 8,500 recorded in total, from one boot of 8.16
hours that drew the fast arrangement. With E5 through E25, radon's curve stands at about 10.60
million crossings over 23 crossing boots and about 26.5 hours with zero defects, and the boots axis
is 23 points instead of one.
