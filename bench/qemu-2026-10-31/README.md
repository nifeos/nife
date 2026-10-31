# Production-shell rows under QEMU, 2026-10-31 (UTC)

First rows typed at the prompt of the default kernel (milestone 961 (the kernel's special boots become commands typed at the production shell) step 1), aarch64 under QEMU TCG
with four cores and no `-icount`, taken by `script/swish-check --arch aarch64`. `production-rows-aarch64.log`
holds the transcript and the boot log's counter-frequency line that converts the ticks.

| row | production ticks | count | quiet boot (`bench/baseline-aarch64.txt`, `-icount`) |
|---|---:|---:|---:|
| `null_syscall` | 4,440,153 | 20,000 | 362,504 |
| `sink_throughput` | 50,360,309 and 52,602,468 | 80,000 bytes | 4,391,815 |
| `map_el0` | 1,813,482 and 1,873,005 | 500 | 299,956 |

The two columns are not comparable. The production rows are wall-clock ticks of a 62 MHz counter on
a TCG guest that shares the host with the console, `net_stack` and the shell; the quiet boot counts
ticks under `-icount`, which is deterministic and is what gates. Nothing gates on the production
rows (milestone 961, decision D6). They show that the commands run, print the row and agree to
about 4% between two runs of the same row. They say nothing about the hardware.

`map_el0` built its own target out of `--mem 10`. The bench boot's `map_el0`, which had the kernel
build the target, committed 8 pages by `MemoryRegion::USAGE` after 508 maps (size 540; 1 address
space, 7 frames) on the same date.
