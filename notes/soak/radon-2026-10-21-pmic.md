# radon's PMIC-report draws, 2026-10-21

*An appendix to [`notes/soak.md`](../soak.md). It holds one capture's run table: rounds, rate,
wakes and crossings for each draw. Milestone 225 (run the soak on radon, argon and xenon) asks that
of every run, and this run also asks what can reset a JH7110 without a line from the kernel.*

One capture, `bench/radon-2026-10-21/225-pmic-report-draws.log`, from 17:55 to 19:19 UTC, built at
`0e0866d93` with `script/board-image --soak --reboot --tftp` (SEALED). Each draw ran 120 seconds and
ended in the kernel's reset through the AXP15060, and each boot netbooted the same image. The
capture holds 36 draws, E51 to E86 in `../multicore-defect-curve/radon-rows.md`. It ends at E86's
t=35 s, when the console cable moved to another machine; the board kept drawing.

## What the PMIC recorded

At each arming the soak reads the AXP15060's startup source, PWROK and power-off control, wake and
shutdown-sequence controls, and interrupt enables and latched status, then clears the status. All
36 reports read the same:

    AXP15060 report: startup 0x00 pwrok/voff 0x07 wakeup 0x00 down-seq 0x24 irq-enable 0x03 0x03 irq-status 0x00 0x00

No die-temperature, DCDC under-voltage or power-key event latched between any two draws. Every
`U-Boot SPL` banner in the capture follows the kernel's `rebooting now`: no reset without its
sequence. The same image's draws earlier that afternoon, outside this capture, also showed none
(per-draw summaries only; their raw log was not kept).

## Two placement classes

`script/board-console --tally` reads 35 draws with one clean core, rates 178,870 to 196,067/s, and
one with no clean core, E86 at 30,857/s. Draws with no clean core run at 12 to 16% of the usual
rate with every check clean. Two more drew it that afternoon, at 23,240/s and 30,852/s, each with
`steals=9` against the usual 2. E13 to E50 never drew it.

## The run table

| row | rounds | rate | wakes | crossings |
|---|---|---|---|---|
| E51 | 23,378,765 | 193,990/s | 48,351 (401/s) | 10,223 |
| E52 | 23,528,281 | 195,371/s | 48,350 (401/s) | 10,116 |
| E53 | 22,357,212 | 185,562/s | 48,694 (404/s) | 65, frozen |
| E54 | 23,223,680 | 192,748/s | 48,381 (401/s) | 24,169 |
| E55 | 23,482,685 | 194,907/s | 48,366 (401/s) | 23,988 |
| E56 | 23,447,303 | 194,701/s | 48,358 (401/s) | 11,545 |
| E57 | 23,512,436 | 195,315/s | 48,436 (402/s) | 25,733 |
| E58 | 23,470,954 | 194,918/s | 48,392 (402/s) | 9,666 |
| E59 | 21,476,108 | 178,870/s | 48,253 (401/s) | 19, frozen |
| E60 | 23,624,528 | 195,922/s | 48,702 (404/s) | 3,782 |
| E61 | 23,405,996 | 194,063/s | 48,410 (401/s) | 20,344 |
| E62 | 21,663,342 | 180,439/s | 48,253 (401/s) | 22, frozen |
| E63 | 21,902,386 | 182,498/s | 48,253 (401/s) | 21, frozen |
| E64 | 23,453,911 | 194,724/s | 48,362 (401/s) | 10,400 |
| E65 | 21,759,392 | 181,245/s | 48,253 (401/s) | 19, frozen |
| E66 | 23,452,695 | 194,672/s | 48,351 (401/s) | 23,738 |
| E67 | 23,439,116 | 194,576/s | 48,360 (401/s) | 9,681 |
| E68 | 23,303,375 | 193,258/s | 48,400 (401/s) | 27,003 |
| E69 | 23,399,173 | 194,194/s | 48,343 (401/s) | 10,100 |
| E70 | 23,652,278 | 196,067/s | 48,709 (404/s) | 26,207 |
| E71 | 23,266,705 | 193,172/s | 48,444 (402/s) | 3,863 |
| E72 | 23,452,843 | 194,504/s | 48,408 (401/s) | 26,019 |
| E73 | 23,437,178 | 194,570/s | 48,360 (402/s) | 24,180 |
| E74 | 23,274,690 | 192,807/s | 48,654 (403/s) | 6,998 |
| E75 | 23,450,827 | 194,643/s | 48,367 (401/s) | 23,655 |
| E76 | 23,407,208 | 194,284/s | 48,359 (402/s) | 25,517 |
| E77 | 23,003,132 | 190,884/s | 48,413 (401/s) | 17, frozen |
| E78 | 23,533,159 | 195,790/s | 48,354 (402/s) | 24,044 |
| E79 | 23,190,999 | 192,498/s | 48,363 (401/s) | 23,833 |
| E80 | 23,465,012 | 194,639/s | 48,361 (401/s) | 24,506 |
| E81 | 23,027,539 | 191,309/s | 48,444 (402/s) | 25, frozen |
| E82 | 23,456,532 | 194,474/s | 48,398 (401/s) | 10,441 |
| E83 | 21,756,897 | 181,262/s | 48,253 (401/s) | 19, frozen |
| E84 | 23,506,213 | 195,290/s | 48,383 (402/s) | 24,167 |
| E85 | 23,562,830 | 195,543/s | 48,452 (402/s) | 27,358 |
| E86 | 1,090,774 | 30,857/s | 14,249 (402/s) | 54, frozen |
