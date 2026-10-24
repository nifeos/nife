# radon's receive rate: a one-segment window and a tick per round trip

*Name: provisional.*

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

## What radon measured, 2026-10-22

Two boots of the bench step from `699fcf61a`, which has the fix: 14.329 and 14.313 Mbit/s, six
times the 2.3 and short of the 50 the table above asks for
(`bench/radon-2026-10-22/53-network-rate.log`). patagonia's `tcpdump` needs `sudo`, which the run
did not have, so the second boot's peer was a Python server that reads its own socket's
`TCP_CONNECTION_INFO` every 5 ms instead, which macOS gives any process
(`bench/radon-2026-10-22/53-network-rate-peer.py`). It served the same 100 MiB, and the rate
matched the first boot's `nc`, so the peer is not the limit. What its socket saw
(`53-network-rate-peer.txt`):

| field | value |
|---|---|
| radon's window scale | 0 (none offered) |
| maximum segment | 1460 bytes |
| radon's advertised window, its six commonest values | 4,596, 4,704, 5,288, 4,412, 4,996 and 4,120 bytes, 1,710 to 2,428 samples each; the next two, 9,084 and 7,624, in 8 and 6 |
| smoothed round trip, in whole milliseconds | 2 ms median and 90th percentile, 5 ms the most |
| retransmitted bytes | 0 |

So the window does reach past 2 KiB, which says the fix is in the image, and the wait does not
sleep to a tick, since a round trip is 2 ms. But radon advertises about 4.6 KiB of the 16 KiB
buffer at every acknowledgment, so about 11.5 KiB sits unread each time. A window of 4.6 KiB per
2 ms round trip allows 18 Mbit/s, near the measured 14.3 given a round trip read only in whole
milliseconds. No row of the table above has that shape. The nearest
is its fourth: a third limit, on radon's side and after the wire. The receiving program or the
copy into it drains the buffer slower than a gigabit link fills it, and which of the two is not
yet measured.
