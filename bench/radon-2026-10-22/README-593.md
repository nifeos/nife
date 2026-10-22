# radon, 2026-10-22 (UTC): milestone 593's watchdog bench step

Milestone 593 (a wedged kernel resets itself)'s "radon's bench step", run from 17:25 to 17:54
UTC. Both images were built from this branch with `script/board-image --soak --tftp` and served
by `script/board-netboot`; neither was written to a card. Each printed `SEALED` and each boot
printed `payload came from net`. Captures were taken with `script/board-console --until none`
and cleaned with `briefs/bench-session.md` step 6's `tr`.

| file | boot | image |
|---|---|---|
| `593-armed.log` | 1 | `--extra-features watchdog_soak_test` (features `board,soak_test,watchdog_soak_test`) |
| `593-wedge.log` | 2 | `--extra-features wedge_soak_test` |
| `593-wedge-loop.log` | 3 to 6 | the same wedge image for 3 to 5, then the healthy image of boot 1 for 6 |
| `593-wedge-loop-times.txt` | 3 to 6 | host UTC time at which each key line reached the capture, polled every 0.2 s |

## Boot 1: an armed soak survives and the escape works

Discovery, clocks and resets passed in the order the block lists them:

    soak-test-watchdog: found the JH7110 watchdog: starfive,dskit-wdt at 0x13070000, its clocks and resets in the SYS CRG at 0x13020000 (named by this machine's tree)
    soak-test-watchdog: 2 of 2 reset(s) released, 0 step(s) rejected, 0 specifier(s) skipped
    soak-test-watchdog: the counter runs at 23999850 Hz, measured against this kernel's timer. ...
    soak-test-watchdog: ARMED. THIS BUILD RESETS THE MACHINE IF THE SOAK STOPS BEATING: 720000000 ticks ...

The soak ran to `t=608s` with no reset. Every `petted` line read between 594,358,858 and
599,367,957 of 720,000,000 ticks left: one beat's worth gone, and the reload taking each time.
One byte written to the console at 17:36:15 UTC gave `DISARMED at t=618s ... The watchdog is
halted`, and the soak kept beating. `wakerate` held at 401 to 402 and crossings rose every beat.

## Boots 2 to 5: a wedge resets itself

calef power-cycled radon once, onto the wedge image. Six beats in it printed `WEDGING NOW at
t=30s`. The watcher of `593-wedge.log` stopped about 45 s later, on its own three-missed-beats
rule (`went quiet`), so that capture holds the wedge and not the reset. The next capture, started
ten minutes later with `--quiet-after 0`, found the board already on a later boot of the same
image: it had been resetting, netbooting and wedging again with nobody at it.

`593-wedge-loop.log` holds three whole cycles:

| `WEDGING NOW` | next `U-Boot SPL` | gap | next `soak-test: started` |
|---|---|---|---|
| 17:48:37.180 | 17:49:38.995 | 61.8 s | 17:49:48.976 |
| 17:50:19.564 | 17:51:21.284 | 61.7 s | 17:51:32.406 |
| 17:52:03.061 | 17:53:04.843 | 61.8 s | 17:53:15.188 |

60 s of each gap is the load's arithmetic, counted from the last pet at `t=30s`, which came just
before the wedge. The rest is the time from reset to SPL's banner and the host's 0.2 s poll.

## Boot 6: the loop ends without a power cycle

Before the third reset, the served kernel was put back to boot 1's. Boot 6 fetched it, armed, and
soaked healthy, so the wedge loop ended with nobody at the board.

The JH7110 keeps no record this kernel knows of how a boot ended, so the timing is the evidence
that the watchdog did these resets. Nothing else in the build resets the board: it carries no
`reboot_soak_test`.
