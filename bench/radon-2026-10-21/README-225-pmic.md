# radon, 2026-10-21 (UTC): milestone 225's PMIC-report draws

The file milestone 225 (run the soak on radon, argon and xenon) added that evening. Its rows are
E51 to E86 in `notes/multicore-defect-curve/radon-rows.md`, and the run table is
`notes/soak/radon-2026-10-21-pmic.md`.

| file | what it is |
|---|---|
| `225-pmic-report-draws.log` | `script/board-console --until none --log`'s capture from 17:55 to 19:19 UTC, cleaned with `briefs/bench-session.md` step 6's `tr`: 36 draws, the last cut at t=35 s |

## The image

`script/board-image --soak --reboot --tftp` from `0e0866d93`. It printed `SEALED`.

## How the draws ran

radon reached this image without a power cycle. Milestone 53's storage bench reset itself, and the
board netbooted the soak from a TFTP directory whose files were swapped only after a fetch completed.
Every draw after that was the soak's own reset. The capture starts mid-series; the draws before it
on the same image were watched but their log was not kept. It ends when the console cable moved to
another machine, while the board went on drawing. Nothing was sent to the console.
