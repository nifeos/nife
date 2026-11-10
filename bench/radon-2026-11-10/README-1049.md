# radon, 2026-11-10 (UTC): milestone 1049's second board check

The second run of the bench session in [../radon-2026-11-09/README-1049.md](../radon-2026-11-09/README-1049.md),
on the same board, with the same crate commit (`nifeos/jh7110_entropy`
`ee0bde8feb9f78a67da011c928f9a2a697f3bbc1`) and the same payload (SHA-256
`e421000bc0b66d35ac60315cbe618ba1c8e15fb46d9bf8aac44f3a27b82054e8`). The person says the board sat
unpowered for about an hour before power-on; nothing reports the plug state, so row R4 keeps that as
their statement.

| file | what it is |
|---|---|
| `1049-board-check-results.md` | the filled table; `results.py check` printed `PASS, 0 problem(s)`; R4 and 9g filled by hand |
| `1049-board.log` | the console, unedited; SHA-256 `fde9661f55f814865cb6f843b76f93ff5d3717a1c6290ebbd072db48500c8a92` |

calef ruled that the log goes in as is, as in the first run, so it holds the board's serial number, its
MAC addresses and the private LAN addresses. The capture (`capture.bin`, 1,310,720 bytes, SHA-256
`208c42b41ae4118ae000ee2839cc62ecd182e3d4f760b20694395f8b2529637f`) stays with the maintainers.

## One boot

The card was rewritten with `script/board-image --tftp` before this run, so its boot script asks for
`nife-initial-ramdisk.img` and both files came over TFTP. The log says `nife: payload came from net` on
its only boot. The first run needed a second boot because the old card asked for `nife-initrd.img`.
Card files: `nife-vf2.img` `239e047a...6b57`, `nife-initial-ramdisk.img` `7feae619...15cb2`,
`boot.scr.uimg` `8638da95...fc7`.

## What the table says against the first run

Every judged row passed again. The first `STAT` read was `0x08040308` both times, with the same `MODE`,
`FEATURES` and `BCONF`, so the reading repeats across two power-ons. It is still the state the bootloader
left. The rows that differ:

| row | first run | second run |
|---|---|---|
| 4a, `RAND_GENERATING` seen with `RAND_RDY` acknowledged first | `together_loose=1` | `together_loose=0` |
| 4b, second `GENERATE` over a latched `RAND_RDY` | `together=8775` | `together=8777` |
| 6c and 6d, reseed median | 174 ticks, 43.5 microseconds | 172 ticks, 43.0 microseconds |
| 9b, FIPS 140-2 blocks failed | 2 of 524 | 1 of 524 |
| 9c to 9e | 7.999854, p 0.3333, mean 127.5159 | 7.999873, p 0.8560, mean 127.4203 |
| 9g | not run | `ent` agrees with 9c to 9e |

The generation bytes in 1d and 1e and the capture are new. Row 9g's `ent` run recomputes the same
statistics as 9c to 9e, so it cross-checks the arithmetic and is not an independent suite; `dieharder`
is not installed. Two captures are two measurements and do not show that the output is unpredictable.
