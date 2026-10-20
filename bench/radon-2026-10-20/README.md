# radon, 2026-10-20 (UTC): milestone 53's first network and storage runs

Four boots of radon, power-cycled by calef at its own outlet. Every image was built from this
branch at `290d3ba8b` and served by `script/board-netboot` from patagonia (192.168.8.192). The
logs are console transcripts with non-printing bytes dropped; the raw captures are not committed.
Read with notes/designware-ethernet.md and notes/designware-mobile-storage.md, whose Results
sections these logs fill.

| log | built with | cable in | last line before `done` |
|---|---|---|---|
| `network-bench-1.log` | `NIFE_NETWORK_BENCH_PEER=192.168.8.192:9494 script/board-image --tftp --extra-features network_bench` | `ethernet@16040000`'s jack | `verdict FAILED: no link` |
| `network-bench-2.log` | the same image | `ethernet@16040000`'s jack | `verdict FAILED: no link` |
| `network-bench-3.log` | the same image | `ethernet@16030000`'s jack | `verdict LEASED-AND-MEASURED` |
| `storage-bench-read.log` | `script/board-image --tftp --extra-features storage_bench` | `ethernet@16030000`'s jack | `verdict FAILED: read of block 0: Host(Data(17, 2060))` |

`script/board-image` reported every image `SEALED` when it built it (its own output, not in these logs). The network peer was the runbook's `nc` loop on patagonia,
serving 100 MiB per connection on port 9494.

## How the boots were run

- **The console was GNU `screen` logging on the CH343 serial port, not `script/board-console`.**
  Each boot needed typing at U-Boot's prompt, and `board-console` only reads.
- **At each autoboot countdown the lane sent a key, then `setenv nife_boot_server 192.168.8.192`,
  then `boot`.** radon's card carries a boot script written 2026-09-16, which asks for a server
  address that is not patagonia's and for the archive as `nife-initrd.img`. `script/board-image`
  writes it as `nife-initial-ramdisk.img`, so a copy under the old name was served beside it.
  Each log shows both `Filename` lines and `nife: payload came from net`.
- **Boots 1 and 2 had the cable in the wrong jack.** U-Boot timed out on `ethernet@16030000` and
  linked on `ethernet@16040000`, and nife drives only `0x16030000`, so `link : NONE after 5003
  ms` is the cabling, not the driver. Both still ran the coherence probe, which needs no link.
  The cable was moved for boot 3.
- **Storage stopped at step 1**, as its runbook says for a `FAILED` verdict. The banner said
  `Read-only build: nothing on any card is written.` Steps 2 and 3 were not run.

During boot 3's transfer, patagonia's TCP send queue for the connection stayed near 131 KB and
barely drained. That observation was made by hand on patagonia and is not in the log.
