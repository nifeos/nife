# radon, 2026-10-20 (UTC): milestone 796's sweep with the hot section pinned

Four boots of the job mix on radon (VisionFive 2), power-cycled by calef on smart plug 2 from 19:01
to 19:10 UTC. They repeat the 2026-10-05 layout sweep
([`../radon-2026-10-05/`](../radon-2026-10-05/README.md), boots 16 to 19) on a kernel whose hot
trap path is pinned. The procedure and the reading table are in the block of
[milestone 796 (pin the hot trap path's placement)](../../design/roadmap/0796-pin-the-hot-trap-paths-placement.md),
"The radon sweep, for calef", and the result is recorded there.

## Payloads

All four were built from `b43f19e3c` with the block's build loop: `NIFE_FASTPATH_PAD=0
NIFE_FASTPATH_SHIFT=<S> script/board-image --job-mix --tftp --extra-features fastpath_pad`, then
`target/board` moved to `target/board-shift<S>`. Each printed SEALED. Each image's
`script/fastpath-footprint --layout` line read `pin: .text.hot 9082 B at 0xffffffc080204000, 0 B
into a 16 KiB way; 18 of 18 hot symbols inside`, with `trap_entry` at `0xffffffc080206270`, set
137. `nife-vf2.img` hashes are in [`796-payloads.sha256`](796-payloads.sha256).

## Boots

`bootN-shiftS.log` is the console, cleaned by the block's `tr` (non-printing bytes dropped); the raw
captures are not committed. `bootN-shiftS.netboot.log` is the TFTP server's log, which names the
directory served. Every boot printed `payload came from net` and `job-mix: done`.

| Boot | Shift | `nife-vf2.img` | 1 task `null_syscall` `per_job` (ticks over 336 jobs) |
|---|---|---|---|
| 1 | 16 | `c9e4ffd9943b` | 121 (40736) |
| 2 | 0 | `3edd43817fa8` | 121 (40707) |
| 3 | 48 | `fdc135205cfe` | 121 (40686) |
| 4 | 32 | `fc7b450b6a7f` | 121 (40693) |

## Where the mechanics differed from the procedure

- **Console capture.** The console was logged with GNU `screen`'s logging on the CH343 serial port,
  not with `script/board-console`, because the lane had to type at U-Boot and
  `script/board-console` has no mode that does. `screen` logs what the board sends, so the typed
  commands appear only as U-Boot echoed them, and the key that stopped the countdown does not.
- **Server address and archive name.** The card's boot script dates from 2026-09-16. It asks for an
  older server address than patagonia's and fetches the archive under its old name,
  `nife-initrd.img`. At each boot's autoboot countdown the lane sent a key, then `setenv
  nife_boot_server 192.168.8.192` and `boot`. A copy of the archive was served as `nife-initrd.img`
  beside `nife-initial-ramdisk.img`, which is why each netboot log lists both at the same size.
  The console shows the board fetching `nife-initrd.img` from 192.168.8.192.

Neither changes what ran: the boot images are the ones hashed above. Like every radon job-mix
number so far, these ran debug userspace under a release kernel (`script/board-image`'s `BUGS`,
milestone 797 (release userspace in board images)).
