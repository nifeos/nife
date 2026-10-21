# radon's receive rate: a one-segment window and a tick per round trip

*A child of [`notes/designware-ethernet.md`](../designware-ethernet.md) (milestone 53 (the board's
own peripherals: network and storage on real silicon)), whose Results row is the 2.3 Mbit/s this
page explains. Its names are provisional: `socket_contract::polled_wait`, `TCP_RECEIVE_BUF`,
`CurrentCpuPage::tick_hz`, `user_mode_runtime::tick_hz`, and this page's stem.*

## Why the row is 2.3 Mbit/s

Two things in `net_stack`, each measured, and they multiply.

- **The window was one segment.** Every TCP socket advertised 2 KiB with no window scaling. The
  peer sends a 1460-byte segment, and the 588 bytes left are under its silly-window threshold, so
  it waits for the acknowledgment. One segment per round trip.
- **Every round trip waited for a scheduler tick.** Neither polled NIC raises an interrupt, so
  when the ring was empty `net_stack` slept 1 ms. The kernel ends a timed wait only at a tick
  (DECISIONS §147 (a timer a userspace service cannot hold)), 100 Hz on every architecture, so
  the sleep lasted up to 10 ms. On a LAN the next segment comes back well within a tick, and found
  the stack asleep.

One 1460-byte segment per half tick on average is 2.34 Mbit/s, and the row is 104,857,600 bytes
in 71,820 segments over 362.965 s, 5.05 ms each.

QEMU hides both, because slirp answers in microseconds and the next frame is already in the ring
when the stack looks. A `filter-buffer` on the `e1000e`'s netdev (`interval=300`) reproduces
radon's shape on `cargo xtask network-bench`'s image, 8 MiB in. Its delay measured a 1.04 ms
median from the guest's acknowledgment to the next segment reaching it. A `filter-dump` either
side of it shows the window and the guest's turnaround:

| wait | window | no added delay | added delay | guest's data-to-ACK median |
|---|---|---|---|---|
| sleep when the ring is empty | 2 KiB | 72 Mbit/s | 1.16 Mbit/s | 10.15 ms |
| sleep when the ring is empty | 16 KiB | 240 Mbit/s | 9.3 Mbit/s | |
| look for a tick after a frame | 2 KiB | 81 Mbit/s | 8.6 Mbit/s | |
| look for a tick after a frame | 16 KiB | 235 Mbit/s | 67.7 Mbit/s | |

The tree is the last row. The receive window is 16 KiB (`socket_table::TCP_RECEIVE_BUF`), bounded
at compile time by the polled rings and the heap. The polled wait keeps looking, yielding between
looks, for one tick after the last frame and sleeps only after that
(`socket_contract::polled_wait`, host-tested). A tick is the bound because it is what a sleep
costs. The price is a busy core while a stream flows.

## The bench step

[The parent's bench step](../designware-ethernet.md#calefs-bench-step-on-radon), unchanged, from
a commit that has the fix: same peer, same image flags, same console capture. While the transfer
runs, in a fourth terminal on patagonia, capture the stream (it needs `sudo`; without it, skip
the capture and say so in the Results row):

```sh
sudo tcpdump -i en0 -nn -c 20000 -w bench/radon-$(date -u +%F)/network-bench-rate.pcap tcp port 9494
```

Then read two things out of it. The windows radon advertised, which should reach 16384:

```sh
tcpdump -nn -r bench/radon-$(date -u +%F)/network-bench-rate.pcap 'src port not 9494' | grep -o 'win [0-9]*' | sort | uniq -c | sort -rn | head
```

And the gaps between radon's acknowledgments, in milliseconds, which should be well under a tick:

```sh
tcpdump -nn -tt -r bench/radon-$(date -u +%F)/network-bench-rate.pcap 'src port not 9494' | awk '{if (p) print ($1-p)*1000; p=$1}' | sort -n | awk '{a[NR]=$1} END {print "p50", a[int(NR/2)], "p90", a[int(NR*0.9)]}'
```

On radon the two causes do not separate by rate alone. The window by itself gives 16 KiB per half
tick, about 26 Mbit/s; the wait by itself gives one segment per LAN round trip, tens of Mbit/s.
Together they allow over 100 Mbit/s at a 1 ms round trip, so radon's cores copying each frame
probably set the rate. The capture decides:

| what the run shows | meaning | next |
|---|---|---|
| over 50 Mbit/s | both changes hold on silicon | a Results row in the parent; its first `BUGS` entry comes out |
| windows stay at 2048 | the window change is not in the image | `git log -1` against the image; rebuild |
| windows reach 16384, acknowledgment gaps near 5 to 10 ms | the stack still sleeps to a tick per round trip | a lane, with the capture |
| windows reach 16384, gaps well under a tick, rate under 50 | a third limit, probably copying on radon's cores | a Results row and a `BUGS` entry naming it; a lane to profile |
| about 2.3 Mbit/s | neither change is in the image | `git log -1` against the image; rebuild |
