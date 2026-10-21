# radon, 2026-10-21 (UTC): milestone 53's storage read, run down

Two boots that milestone 53 (the board's own peripherals: network and storage on real silicon)
added, each netbooted from `script/board-image --tftp --extra-features storage_read_matrix`
(SEALED) and watched by `script/board-console --until none`, cleaned with
`briefs/bench-session.md` step 6's `tr`. Both are read-only: the banner says `Read-only build:
nothing on any card is written.` Each ends with the build's own cold reset into the next image.

| file | commit | what it shows |
|---|---|---|
| `53-read-matrix.log` | `f7ae6ee29` | the storage bench as before, then the read matrix: every `CMD18` at the nominal 25 MHz fails with a data CRC error once the FIFO fills; every one at 12.5 and 6.25 passes, faster than those clocks allow |
| `53-card-clock.log` | `ffac21104` | the card clock set to the tree's rate (49.5 MHz): `READ-OK 10959 KiB/s, EL0 block server served`, and every matrix row passes with the FIFO never full |

The first boot came from `reboot` at the prompt of milestone 890 (riscv64 and x86_64 join the shared
boot path)'s fix image. Between the two, radon
netbooted while the TFTP server was being restarted, fell back to its card's old image, and needed
a power cycle (calef, on radon's own outlet). That boot is not kept.
