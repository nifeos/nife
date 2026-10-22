# radon, 2026-10-22 (UTC): milestone 53's network rate and storage write

Milestone 53 (the board's own peripherals: network and storage on real silicon), four boots, 18:10
to 18:35 UTC. The network boots ran from `main` at `699fcf61a`, the storage and ordinary boots
from `03fc15894` (this branch with `main` at `2f641c7e2` merged). Each was built with
`script/board-image --tftp`, `SEALED`, served by
`script/board-netboot` and powered by calef at radon's own outlet. Captures were taken with
`script/board-console --until none` and cleaned with `briefs/bench-session.md` step 6's `tr`.

| file | boots | image | verdict |
|---|---|---|---|
| `53-network-rate.log` | 1 and 2 | `--extra-features network_bench`, peer on the TFTP host's wired adapter, port 9494 | `LEASED-AND-MEASURED`, 14.329 and 14.313 Mbit/s |
| `53-network-rate-peer.txt` | 2 | the peer's own record of its socket, from `53-network-rate-peer.py` | |
| `53-storage-write.log` | 3 | `NIFE_STORAGE_BENCH_WRITE=scratch`, `--extra-features storage_bench` | `READ-OK 10960 KiB/s, WRITE-VERIFIED sectors 2040..2048 (were zero), restored, EL0 block server served` |
| `53-ordinary-and-reboot.log` | 4, then the next boot's start | the ordinary image (features `board`) | reached the `$` prompt |

## The network rate

Boot 1's peer was the runbook's `nc` loop. Boot 2's was `53-network-rate-peer.py`, which serves
the same 100 MiB and samples its socket's `TCP_CONNECTION_INFO`, because the runbook's `tcpdump`
needs `sudo` and the run had none. Its sender-side time, 73.6 s, runs from accept to close and is
not the rate; radon's own `transfer` line is. notes/designware-ethernet/the-receive-rate.md reads
the result.

## The storage write

Step 2 of notes/designware-mobile-storage.md's bench step. The card's partition 1 starts at sector
2048, so the scratch range was sectors 2040 to 2047, inside the alignment gap no filesystem uses.
The log's `scratch write test: Verified(2040, 8, true)` is the kernel's half and `EL0 block server:
Served` the server's. The eMMC socket answered `NoCard(NoResponse(1))`; it holds no part.

## The ordinary image, and `reboot`

Step 3. Boot 4 printed `payload came from net`, `nife self-test: 11 of 13 passed, 2 absent:
blockdev deviceirq` and the prompt. `echo hello` typed over the UART echoed and ran. `reboot`
then printed `the kernel was asked to restart the machine` and the AXP15060 lines of milestone
592 (radon's cold reboot dies in OpenSBI's PMIC write)'s direct route, and the board came back through U-Boot SPL with nobody at it. That next boot
was milestone 249 (the boot lottery is sampled by a person walking to the board)'s series, `bench/radon-2026-10-22/README-249.md`.
