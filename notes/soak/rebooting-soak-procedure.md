# The rebooting soak on radon: the procedure and its outcomes

*Name: provisional.*

An appendix to [notes/soak.md](../soak.md), following [rebooting-soak.md](rebooting-soak.md), which
says why the loop exists and what the bench found on 2026-09-04: radon's firmware cannot complete
the reset. This procedure stands for a board whose firmware can, and its escape check stands for
every board.

## The procedure, in order

Steps 1 and 2 need no board. The procedure assumes the cabling and the U-Boot behavior in
notes/visionfive2.md. It also assumes the boot script of milestone 218 (every boot of the
VisionFive 2 needs a human typing four commands into U-Boot). As this was first written, that
script had itself never run on the board. If the card lands at `StarFive #` instead of booting,
that is 218 and not this, and the manual commands `script/board-image` prints still work from there.

1. Build the rebooting payload and write the card.

   ```
   script/board-image --soak --reboot --card /Volumes/NIFE
   ```

   That script builds the archive before the kernel, and the order is load-bearing; the parent's
   radon procedure says why. It prints a warning block naming what the card will do and how to stop
   it. *If it refuses with "--reboot needs --soak"*, that is the flag pair, not the board.

2. Read the boot script that will run, so step 5's transcript is being compared against something:
   `cat target/board/boot.cmd`.

3. Start the watcher before powering the board. Two hours is fifty draws at two minutes each, plus
   boot time. Make it longer than you think, and stop it with a key.

   ```
   script/board-console --for 3h --until none --log target/radon-lottery-$(date +%s).log
   ```

   `--until none` is what makes it a sustained watch. Leave `--quiet-after` alone. A reboot's dark
   period is a few seconds of SPL and U-Boot output rather than silence, so the fifteen-second window
   is not at risk. Shortening it would make a slow boot look like a hang.

4. Power the board. On the FIRST boot, press a key, once, after `soak-test: started` appears. This
   is the step that verifies the escape, and it is not optional.

   Expect, within five seconds, `soak-test-reboot: DISARMED at t=Ns: a byte arrived on this console.`
   The board then keeps soaking and never reboots. If that line does not come, the escape does not
   work on this cable, and nothing further in this procedure should be run. Power the board off,
   find out why the receive path is dead, and only then start again.

   Then power-cycle to start the series for real, and type nothing at it after this.

   Milestone 324 (the bench console cannot speak to any of the three boards) makes this step a command rather than a keystroke, and the check it performs is
   the same one:

   ```
   script/board-console --stop
   ```

   It sends the byte itself, on the board's own arming announcement, and prints it into the log in
   hex. Then it waits fifteen seconds for the `DISARMED` line. Exit `0` is this step passing. Exit
   `3` with *sent the escape and the board did not acknowledge it* is this step failing, which is the
   stop that matters. It is worth preferring to a keystroke for one reason beyond convenience. A key
   pressed at a terminal leaves nothing in the capture. This leaves both halves of the exchange in
   the artifact the run is judged from. No byte of it has yet reached a board; see
   notes/board-console.md.

   A whole series can be ended the same way, in place of step 3's deadline.
   `script/board-console --stop-after 50` watches, counts draws, and sends the escape on the
   fiftieth. That is a series with exactly the sample it was asked for, rather than one cut off by a
   clock. That has not been run on a board either.

5. Watch the first two draws before you walk away. The whole cycle should read:

   ```
   soak-test-reboot: THIS BUILD REBOOTS THE BOARD. It soaks for 120s, then asks the firmware ...
   soak-test-census: core=1 threads=... (the spawn placement)
   soak-test: t=5s beat=1 rounds=... rate=.../s ... drifted=0 ...
   soak-test-census: where the workers are NOW, ...        (about 25s in, once)
   soak-test: t=120s beat=24 ...
   soak-test-reboot: window reached at t=120s. Cold-rebooting in 5s ...
   soak-test-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot). ...
   U-Boot SPL 2021.10                                  (the next draw)
   ```

   The two beats worth checking are still those of milestone 221 (the soak never crosses cores, so build the hook that makes it), for its reasons: `wakerate` about
   `100 * harts` (roughly 400 here), and `crossings` rising between beats.

6. Leave it. The watcher stops at the deadline, on a failure, or after three missed beats.

7. Tally the log. This needs no board, and can be run on a partial capture at any time:

   ```
   script/board-console --tally target/radon-lottery-....log
   ```

   It prints one row per draw (clean cores over online cores, the last rate, how it ended), then the
   distribution. A core is clean when it holds a responder or a caller and no grinder. That is read
   from the *last* census that boot printed, because the spawn placement is the lottery's ticket and
   the settled arrangement is what the machine ran.

8. Record the table in the parent note, beside the nine hand-drawn boots, with the date and the
   build. The comparison that matters is against those nine. They are the control, and they were
   drawn by a power cycle rather than by a warm reset. If the automated distribution does not overlap
   them where it should, *that* is the finding rather than the distribution.

## What each outcome means

Read this against the log, in this order. The first row that matches is the one to act on.

| What the console shows | What it means | What to do |
|---|---|---|
| `soak-test-reboot: DISARMED` on boot 1 after you press a key, or after `script/board-console --stop` reports exit 0 | The escape works. This is step 4 passing. | Power-cycle and start the series. |
| No `DISARMED` after pressing keys for a beat or two, or `--stop` exiting 3 having sent the byte | The receive path is dead, and the escape does not exist on this cable | Stop. Power off. Check the adapter's TX into the board's RX and the ground; nothing else here is safe until this works. |
| `--stop` exiting 3 having sent nothing | No armed reboot loop announced itself: the card may not carry a `--reboot` build, or the watch ended before a draw came round | Check `target/board/boot.cmd` and the build flags, and give `--for` longer. Nothing was written to the board. |
| `soak-test-reboot: DISARMED` on boot 1 with nobody typing | Something wrote to the port, or U-Boot left a byte the arming drain did not catch | Detach anything else holding the port. Harmless: it fails toward not rebooting. |
| `rebooting now`, then `U-Boot SPL` a few seconds later | The mechanism works. SRST reset type 1 is implemented and the loop is running. | Nothing. This is the series. |
| `rebooting now`, then `i2c read` retries and `cannot read pmic power register` | What radon actually does (2026-09-04). OpenSBI's PMIC write fails and it hangs before any reset (notes/board-reboot.md). A third outcome. | Power-cycle to recover. See notes/board-reboot.md for the kernel-side fix; milestone 224 is the alternative. |
| `rebooting now`, then `soak-test-reboot: FAILED ... sbiret.error=-2` | This OpenSBI implements SRST shutdown and not cold reboot | The route is closed. The soak keeps running and the board is fine. A smart-plug power cycle is the alternative mechanism; raise it. |
| `rebooting now`, then nothing, and the board is dark | The firmware treated reset type 1 as a shutdown | Power the board back on. Same conclusion as the row above; record which of the two happened, because they are different firmware bugs. |
| `rebooting now`, then nothing, and the board is powered but silent | It reset and hung before SPL, or the console dropped | Power-cycle. If it recurs at the same point, that is a finding about the reset path and worth more than the distribution. |
| `soak-test: FAILED ...` then `[PANIC]` and the series stops there | The best possible outcome. Risk 5's decisive experiment found something | Do not restart it. The board holds the state and the log holds the census of the arrangement that produced it. |
| `U-Boot SPL` with no `soak-test: started` after it | A boot that never reached the workload | `--tally` counts these separately. Read the log around it: `MEASURED BOOT REFUSED` is a mismatched pair, `### ERROR ###` is milestone 218. |
| The watcher exits 2 (went quiet) mid-series | Three beats missed with no reboot announced | A wedge, which is what this is all for. Leave the board alone and read the last census in the log. |

## What a completed series licenses, written before it runs

One sentence, and it is narrower than it will feel:

> Over N unattended boots of this board, with this firmware and this build, the settled
> arrangement had k clean cores this many times, and the round-trip rate at each k was this.

It licenses nothing about argon or xenon. It licenses nothing about why placement lands where it
does; that is DECISIONS 138. And it licenses nothing about whether three or four clean cores are
*impossible* rather than merely unseen. Fifty draws that never show four is evidence about a
probability, and not a proof of zero. And every draw is a warm reset rather than a power cycle. So
anything that survives a warm reset is held constant across the whole series, in a way the nine
hand-cycled boots did not hold it. Those nine are the control, and the overlap is the check.
