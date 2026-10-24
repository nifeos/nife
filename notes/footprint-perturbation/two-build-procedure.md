# The two-build procedure of 2026-09-04

*Name: provisional, minted 2026-10-11 (UTC) by lane/ten-longest-notes.*

An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md). This is the procedure
the 2026-09-04 session followed: two builds, unpadded and padded, interleaved on one card. It is kept
because it produced the session's numbers and because E1 and E4 still run this way. The next E3
evening uses the eight-image procedure in the parent instead, because this one cannot separate
footprint from layout.

## What you need at the bench

Everything `notes/visionfive2.md`'s bench runbook and `notes/board-console.md` already list, and
nothing more:

- radon, DIP switches on QSPI, powered from its own Kasa outlet (never switch another; the outlet map
  is kept off-tree).
- The USB TTL adapter on the 40-pin header, TX/RX crossed, 3.3 V, `/dev/cu.usbmodem*` (`cu.`, never
  `tty.`).
- A microSD card already formatted and mounted, and its mount path.
- patagonia, this checkout, and about ten minutes of building per card.

Two cards, or one card written six times; this page said something looser until 2026-09-04. E3 is a
comparison of two builds differing in exactly one Cargo feature. The page said "writes the card twice
and boots twice", which describes a *blocked* order, and step 5 requires an interleaved one. With two
cards those agree. With one card they do not, and calef has one.

So, with a single card, the order is:

```
unpadded -> padded -> unpadded -> padded -> unpadded -> padded
```

That is six writes, not two. A rewrite is about two minutes once the build is warm
(`script/board-image` rebuilds only what changed, and the copy is 9 MB). So the session costs roughly
75 minutes rather than 60.

Do not take the blocked order to save the flashes. Three unpadded boots followed by three padded ones
puts everything that drifts across the session, board temperature most obviously, entirely on the
second group, where it is indistinguishable from the effect being measured. The interleaving is not
tidiness; it is what makes a few-percent difference mean anything.

They produce the same three filenames, so nothing on the card says which is which.
`script/board-image` echoes its feature list for exactly this reason, and that line belongs in the log
beside the numbers. That the card cannot say what it is, is the reason six writes are risky rather
than merely slow. `design/roadmap/0367-a-boot-banner-that-names-the-build.md` is the fix.

## The procedure, in order

### 1. Take the static numbers first, on patagonia, with no board

```sh
script/fastpath-footprint --arch riscv64
script/fastpath-footprint --arch riscv64 --features fastpath_pad
```

Two minutes, no hardware. It proves the padding still doubles what it claims to on the exact commit
about to be flashed, which is the one thing that would silently invalidate the whole session. Copy
both `ipc_call_reply` figures into the log.

### 2. Build and write the un-padded card

```sh
script/board-image --bench --card /Volumes/NIFE
```

Confirm the line it prints:

```
  features: board,bench,single_hart
```

`single_hart` is not optional and not a tuning knob. Without it the kernel boots four U74s, and E1 and
E4 both print `skipped (needs a single hart ...)`. Its cost is stated where a reader meets it:
`smp_throughput`, `fs_read` and `fs_throughput` self-skip on this card. That is a fair trade for this
session and a bad one for any other, which is why it is a flag.

### 3. Boot it and capture everything

```sh
script/board-console --for 20m --until none --log target/radon-bench-unpadded-$(date +%s).log
```

Then power radon on. `--until none`, because the bench boot prints a couple of dozen rows over several
minutes and then halts. There is no single banner worth stopping at, and a deadline that fires
mid-sweep loses the run. Twenty minutes is generous on purpose. The last line is `bench: done`, and it
is what says the suite finished rather than faulted.

Do not power-cycle to "hurry it along". E4 alone runs 5 working sets x 3 load conditions x 5 repeat
batches, and E1 sweeps 7 pair counts with 4 repeats each.

### 4. Repeat the boot, unchanged, at least three times

Same card, same image, power-cycle between. This is the run-to-run distribution, and every comparison
is against it rather than against a single pair of numbers. The 2026-08-22 dev-Mac session found this
the hard way: an unrepeated E4 swung 2 to 3x between nominally identical conditions.

### 5. Build the padded card and repeat steps 3 and 4

```sh
script/board-image --bench --extra-features fastpath_pad --card /Volumes/NIFE
```

Confirm `features: board,bench,single_hart,fastpath_pad`. Log to a filename that says `padded`.

Interleave the boots if the session has time: unpadded, padded, unpadded, padded. Anything that drifts
over a session (ambient temperature, a card that is warming up) then lands on both conditions, instead
of on whichever was measured second.

### 6. Read the rows

| row | experiment | what it is |
|---|---|---|
| `bench: ipc_rtt <ticks> 1000` | E3 | kernel-side round trip, the SEND/RECEIVE shape |
| `bench: ipc_rtt_el0 <ticks> <iters>` | E3 | the same crossing EL0, which is what lmbench measures |
| `bench: call_reply <ticks> 1000` | E3 | the CALL/reply shape, the one services run |
| `bench: ipc_scale_<threads> <ticks> <iters>` | E1 | 7 rows, 2 to 96 threads |
| `bench: appdisp_<kib>k_solo/_ipc/_ipc96` | E4 | 15 rows, 5 working sets x 3 load conditions |
| `bench-probe: appdisp_<kib>k_*_lost_pct` | E4 | the derived percentages, printed so nobody has to divide |
| `bench: cntfrq 4000000` | all | the proof this ran on the board. 10,000,000 is QEMU `virt` |

That last row is the one to check before reading any other. A capture reading `cntfrq 10000000` is an
emulator, and E1 and E4 will have self-skipped in it.

`call_reply` is the row E3's verdict should rest on, not `ipc_rtt`. The padding now lands on both
shapes; the CALL one is what a service issues, and the phase 4 decision of milestone 188 (the IPC
fastpath) is about that path. `ipc_rtt` is kept because it is the row every previous E3 reading used,
and dropping it would break the comparison with 2026-08-22.

## EXAMPLES

### The whole session, as a shell transcript

```sh
# on patagonia, no board
script/fastpath-footprint --arch riscv64
script/fastpath-footprint --arch riscv64 --features fastpath_pad

# un-padded card
script/board-image --bench --card /Volumes/NIFE
diskutil unmount /Volumes/NIFE
# power radon on, then, for each of three boots:
script/board-console --for 20m --until none --log target/radon-bench-plain-$(date +%s).log

# padded card
script/board-image --bench --extra-features fastpath_pad --card /Volumes/NIFE
diskutil unmount /Volumes/NIFE
# three more boots:
script/board-console --for 20m --until none --log target/radon-bench-padded-$(date +%s).log

# read the comparison out of the captures
grep -h "^bench: \(call_reply\|ipc_rtt\|ipc_rtt_el0\|cntfrq\) " target/radon-bench-*.log
grep -h "^bench: ipc_scale_" target/radon-bench-plain-*.log
grep -h "^bench-probe: appdisp_" target/radon-bench-plain-*.log
```

### Checking the card before walking to the board

```sh
$ script/board-image --bench
...
built:
  features: board,bench,single_hart
  target/board/nife-vf2.img  (669792 bytes, Image header verified)
  target/board/nife-initial-ramdisk.img  (... bytes, the userspace archive this kernel measures)
```

A `features:` line missing `single_hart` is a wasted boot. It is cheaper to read it here than to read
`ipc_thread_scaling skipped` twenty minutes later.
