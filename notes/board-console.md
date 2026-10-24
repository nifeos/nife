# Reading a board, and the one thing this tool says back

Milestone 216. `script/console` boots a shell in QEMU. `script/board-image` builds the VisionFive
2 payload and prints the `dd` commands for a card. Between those two there was nothing, so every
milestone gated on real hardware needed somebody at a terminal emulator reading with their eyes.
`script/board-console` is the missing middle: it opens the port, logs every byte, recognizes how
far the boot got, and stops on a deadline.

The last clause is the only hard part. Opening a serial port is a `screen` invocation. Knowing
when to stop reading is what kept this a milestone. The detail behind this note is in
[board-console/](board-console/README.md).

And since milestone 324 it writes, under one rule: *it writes only what a named mode sends,
and every byte it sends is printed into the log.* calef ruled that on 2026-09-19, replacing this
note's original *"it reads and never writes to the board"*. There is exactly one named mode today
and it sends exactly one byte: `--stop`, which ends the self-rebooting soak of milestone 249 (the boot lottery). The second
clause is the half that makes the first safe. The log is the artifact a bench run is judged from,
and a byte the board received that the capture does not show makes the capture a lie about the run.
**This is not a keyboard**, and the narrowness is the decision rather than caution about it;
"Writing to a board" below has the argument.

## The commands

```
script/board-console                                # watch until the kernel banner, 120s cap
script/board-console --for 30m --until none         # sustained watching, for a stress run
script/board-console --port /dev/cu.usbmodemXXXX    # when two adapters are plugged in
script/board-console --replay target/board-console-1756744000.log   # re-read a capture
script/board-console --stop                         # end a rebooting soak at its next draw
script/board-console --stop-after 50                # end one with exactly fifty samples in it
```

| flag | default | what it does |
|---|---|---|
| `--port <dev>` | the only USB serial adapter in `/dev` | Which device to open. `NIFE_BOARD_PORT` sets it too. Two adapters with no `--port` is an error, not a guess. |
| `--log <file>` | `target/board-console-<epoch>.log` | Where the bytes go. There is no way to turn it off. |
| `--for <duration>` | `120s` | The hard cap. `90`, `90s`, `30m`, `2h`. |
| `--board <name>` | `radon` | **Which firmware prologue to expect** (milestone 324). `radon` is the VisionFive 2's chain; `xenon` has none, because it boots through PVH straight into our banner. argon has no profile on purpose. |
| `--until <stage>` | `banner` | Stop early at one of this board's firmware rungs (`spl`, `opensbi`, `uboot`, `handoff` on radon), or at a shared rung: `banner`, `machine`, `selftest`, `tour`, `prompt`, `soak`, `sweep`, `sweep-done`. `none` watches the whole duration. A word this board has no rung for is refused, not waited out. |
| `--quiet-after <duration>` | `15s` | Give up if the board speaks and then stops. `0` disables it. Suppressed once the tour completes, always. |
| `--stop` | off | `--stop-after 1`. Send the byte that ends milestone 249's rebooting soak, at the next draw. |
| `--stop-after <n>` | off | Send it at the n-th armed draw this session sees, so the series has exactly n samples. |

The two stop flags change two other defaults, because both answer "when does this session
end" and a stop mode is the answer. `--until` becomes `none` (the escape ends the session, not a
stage), and an explicit `--until <stage>` alongside a stop is refused rather than overridden: a
soak is reached long before its reboot loop arms, so `--until soak --stop` would return before
sending anything. `--for` defaults to `150s` per draw plus two minutes, derived from
`kernel/src/soak.rs`'s two-minute draw and the twenty-odd seconds a boot takes, so `--stop-after
50` gets the two unattended hours milestone 249 prices it at. Both are overridable and neither is
an agreement with anything; getting them wrong costs a re-run. `--stop` with `--replay` is refused
outright, because a file has no board on the other end of it.

And one mode that opens no port at all (milestone 249): `--tally <log>` reads a capture of many
boots and reports what the thread-placement lottery drew on each. It is `serial_console::lottery`
rather than the recognizer. It answers a question no single-boot reader can be asked (how often does
each arrangement come up). Its exit statuses are only `0` and `4`, because an analysis of a finished
log has no board left to have gone quiet. See notes/soak.md.

## The exit statuses, which are the point

A bench script needs to tell a hang from a refusal, and a tool with two exit codes cannot.

| status | meaning |
|---|---|
| 0 | Reached the stage asked for. With `--until none`, watched the whole duration and the board announced nothing. |
| 1 | The board announced a failure: a bad image header, a measured-boot refusal, or a kernel panic. |
| 2 | It spoke and then went quiet. **This is what a hang looks like from this end**, and it is the one the multicore risk cares about. |
| 3 | The time ran out with the requested stage unreached. |
| 4 | The port could not be opened, or the arguments were wrong. No session happened. |

## Writing to a board, which is one byte and one reason

Milestone 324, parts 1 and 4, on calef's ruling of 2026-09-19. Everything above this heading
reads. This section is the only part of the tool that writes, and it is worth reading before
anyone extends it, because the shape is the decision.

### What it sends, and what it will not

One byte, `0x0d`, a carriage return. `kernel/src/soak.rs`'s escape accepts any byte at all, so
this is a choice inside a contract that is already fixed rather than anything two programs agree
on. Carriage return because the kernel's own instruction is *"press any key on this console"* and
Enter is what a person presses when told that: a tool whose byte is the one the documented manual
procedure produces is running the same experiment the procedure runs. `NUL` was the other
candidate, refused narrowly, and the reasoning is in
[board-console/modules.md](board-console/modules.md).

It will not send anything else. There is no mode that types a line, no mode that drives U-Boot,
and no keyboard. **The hazard is not writing; it is an open keyboard beside a countdown a stray
byte consumes.** Milestone 249's lane sent this same escape by detaching the console, hit U-Boot's
autoboot countdown with it, and paid a power cycle. That incident is the argument, not a
preference about tidiness.

### When it sends, which is the whole of the safety

Only after the board has printed its arming announcement, the `THIS BUILD REBOOTS THE BOARD`
line `kernel/src/soak.rs` prints from `arm_reboot`. Nothing else in a session can cause a byte to
go out. Four things follow from that one gate:

- The kernel's own drain is already behind us. `arm_reboot` calls `console::discard_rx` and
  *then* prints, so a byte sent any earlier is a byte the kernel deliberately throws away. This is
  the trap worth knowing about: `soak-test: started` is printed **before** `arm_reboot`, with four
  more lines between them, about a kilobyte, roughly ninety milliseconds at 115200 baud. A sender
  triggered on that line would lose the race nearly every time, silently, and the failure would look
  exactly like a board whose receive path is miswired.
- There is a reboot loop to stop. A plain `--features soak_test` kernel never prints the banner
  and never polls for an escape.
- The board is long past its firmware, because a kernel that has printed this has been running
  for the length of a boot tour. The countdown hazard is closed by the gate rather than by the
  byte.
- It counts draws. One banner per boot of a rebooting build, one-to-one with the
  `soak-test: started` lines `script/board-console --tally` opens a draw on, so a log produced by
  `--stop-after n` tallies as exactly n draws.

Say `--stop` is given and the board is powered off, or at a U-Boot prompt, or running a kernel with
no soak in it. The answer is the same in all three cases: nothing is sent, the session ends on its
deadline, and the tool says no armed reboot loop announced itself. The cost of the gate is one
sentence: a session that attaches to a board already mid-draw waits out the rest of that draw, up to
two minutes, before the next banner arrives.

### What confirms it, and why that is new

The kernel prints `soak-test-reboot: DISARMED` when the escape lands. The tool waits fifteen
seconds for it (three of the kernel's five-second polls) and the exit status says whether it came:
`0` confirmed, `3` sent and unacknowledged, `3` never sent. No new exit status was added.

**That closes a gap `kernel/src/soak.rs` states in its own `BUGS` and cannot close from where it
stands**: *"the escape is a poll of one bit and nothing verifies that the bit can ever be set
[...] Nothing in this kernel can prove otherwise, because a UART cannot receive a byte it sends."*
A host holding the far end of the cable is not under that limit, so a confirmed `--stop` is the
first end-to-end evidence that a board's receive path works. The procedure that kernel BUGS entry
names as the substitute (press a key on the first boot, confirm `DISARMED` before walking away)
becomes something a script does and records in the log.

### What the log shows

Three lines, and the ordering is deliberate: the announcement is written and flushed **before** the
byte goes to the port, so a write that fails reads correctly rather than leaving a log that claims
a byte nobody sent.

```
soak-test-reboot: THIS BUILD REBOOTS THE BOARD. It soaks for 120s, then asks the firmware ...

board-console: sending the soak escape to the board now: 1 byte, 0x0d. Draw 50 of 50 armed its
reboot at +7412.3s and this is the sample the series was asked for.
board-console: sent 1 byte, 0x0d. Waiting up to 15s for the kernel to say it found it.
soak-test: t=5s beat=1 rounds=... rate=.../s workers=24 refused=0 mismatch=0 stalled=0
soak-test-reboot: DISARMED at t=5s: a byte arrived on this console. This board will not reboot
itself again.

board-console: the board acknowledged the escape and will not reboot itself again. Its own line is
above; the soak keeps running.
```

The byte is rendered in hex and never written raw, so the capture stays greppable, cannot move a
reader's cursor, and cannot be mistaken for something the board said. A host test asserts that no
line this mode writes into a log reads as a boot marker when that log is replayed.

## What it recognizes

Every marker was quoted from `notes/visionfive2.md`'s bench runbook or this tree's source, then
checked on 2026-09-01 against a captured success and a captured failure that now live in
`crates/serial_console/tests/fixtures/captured/`. The firmware rungs (`spl`, `opensbi`, `uboot`,
`handoff`) are radon's and live in a board profile (`crates/serial_console/src/board.rs`); xenon's
profile is empty, because it boots through PVH straight into our banner. The kernel's rungs are
shared by every board: `banner`, `machine`, `selftest`, `tour`, `prompt`, `soak`, `sweep` and
`sweep-done`. Five markers end a session early: U-Boot's `### ERROR ###`, a bad image magic,
`MEASURED BOOT REFUSED`, our own `[PANIC] `, and U-Boot relocating, which is a note. `init/build`
is reported and deliberately not a stage; it is the only difference between the two successful
captures, and since milestone 295 (retire `components/src/builder.rs`) only those old captures print it.

Four outcomes, three of them seen on a board:

| outcome | what it looks like | exit |
|---|---|---|
| Success | the tour reaches `nife: the capability core runs on RISC-V.`, then the board goes quiet because the kernel halts in `wfi` | 0 |
| U-Boot refused before the kernel ran | the image loads and relocates, then `### ERROR ### Please RESET the board ###` | 1 |
| The kernel booted and then halted on purpose | `MEASURED BOOT REFUSED`, after the banner and most of a tour | 1 |
| A genuine hang | it starts, says a few lines, and stops before the tour | 2 |

Reaching the wanted stage starts a two-second settle window, so a refusal that follows the banner
still wins. Silence after the tour is how a good boot ends, so the quiet timer stops there. Booted,
hung, and refused-before-the-kernel are three outcomes, not two. `--until tour` is the one to want
when the question is "did it work". A job-mix sweep is judged the same way, with `sweep-done` as its
finish and a sixty-second quiet timer. The marker tables, their sources, and each trap are in
[board-console/markers.md](board-console/markers.md).
## The design, and testing it with no board

A partial line may ratchet a stage but not settle a word boundary or capture text. The read runs on
its own thread, so the deadline holds whatever the port does. The device is opened first, then
configured with `stty`, then the speed is read back, because opening a macOS `cu` device resets it.
calef's first capture was garbage for exactly that reason. `cu.`, never `tty.`. There is no serial
dependency: `stty(1)` makes the one call needed, and §46 (thin primitives or whole subsystems) makes
taking a crate a decision. The recognizer, the deadline, the port layer and the writing mode are all
host-tested with no board; the one claim no host test reaches is that `tcsetattr` took, and
`confirm_speed` checks it on every session. The detail is in
[board-console/design.md](board-console/design.md).

## EXAMPLES

A boot check, the ordinary case:

```
$ script/board-console --for 90s
--- /dev/cu.usbmodem5C7B0104661 at 115200 baud, logging to target/board-console-1756744000.log, up to 90s ---
U-Boot SPL 2021.10 (Feb 12 2023 - 20:24:34 +0800)
...
Starting kernel ...

nife on RISC-V (rv64, S-mode, Sv39)

board-console: reached kernel banner (1437 bytes in 6.2s)
board-console: banner: nife on RISC-V (rv64, S-mode, Sv39)
board-console: log at target/board-console-1756744000.log
$ echo $?
0
```

A hang, which is what a multicore defect looks like:

```
$ script/board-console --for 90s --quiet-after 10s
...
Starting kernel ...

board-console: went quiet after kernel handoff (491 bytes in 14.1s)
board-console: log at target/board-console-1756744100.log
$ echo $?
2
```

Nothing at all, with the triage ladder's first row said out loud:

```
board-console: time ran out after nothing recognizable (0 bytes in 90.0s)
board-console: not one byte arrived. Check TX/RX are crossed, that the board has power, that the
DIP switches are on QSPI, and that this is the cu.* device.
```

Re-reading a capture, which needs no board and no adapter:

```
$ script/board-console --replay target/board-console-1756744100.log
```

A job-mix sweep watched under QEMU, with its three endings (done, quiet, out of time), all real
runs, is in [board-console/job-mix-examples.md](board-console/job-mix-examples.md). At a board the
same question is `script/board-console --until sweep-done --for 30m`, which returns the same statuses.

Ending a rebooting soak from a script, which is the one thing this tool writes for. No run of
this against a board exists yet; what follows is what the code produces, from the transcripts the
host tests feed it:

```
$ script/board-console --stop
--- /dev/cu.usbserial-A28FR8LZ at 115200 baud, logging to target/board-console-1758290000.log, up to 270s ---
--- stopping after 1 armed draw(s); one byte will be sent to the board and printed into the log ---
...
soak-test: started 4 groups of one responder, 3 callers, 1 grinder and 1 tick waiter ...
soak-test-reboot: THIS BUILD REBOOTS THE BOARD. It soaks for 120s, then asks the firmware ...

board-console: sending the soak escape to the board now: 1 byte, 0x0d. Draw 1 of 1 armed its
reboot at +24.8s and this is the sample the series was asked for.
board-console: sent 1 byte, 0x0d. Waiting up to 15s for the kernel to say it found it.
soak-test-reboot: DISARMED at t=5s: a byte arrived on this console. ...

board-console: the board acknowledged the escape and disarmed its reboot loop (2914 bytes in 30.1s)
board-console: log at target/board-console-1758290000.log
$ echo $?
0
```

A series with exactly fifty samples in it, which is what milestone 249's distribution wants:

```
$ script/board-console --stop-after 50 --log target/radon-lottery-$(date +%s).log
...
$ script/board-console --tally target/radon-lottery-....log     # reports fifty draws
```

And the same command against a board that is not running a rebooting soak, which is every case the
ruling did not cover arriving as one answer:

```
board-console: time ran out after soak running (184213 bytes in 270.0s)
board-console: no byte was sent: no armed reboot loop announced itself before the session ended
$ echo $?
3
```

## BUGS

Each is in full in [board-console/limitations.md](board-console/limitations.md).

- The markers are checked against one board, on one day, in four states; argon has printed nothing.
- There is no real sample of a hang, the outcome this tool exists for.
- A matched marker does not fail toward pessimism: an echoed `Starting kernel ...` would count.
- It does not recognize an OpenSBI trap dump.
- A captured log is not a test result.
- No byte of `--stop` has reached a board yet; the wire is unproven.
- The `--stop` gate costs a draw when a session attaches mid-run.
- An unconfirmed send is three faults wearing one report.
- It does not touch power.
- Nothing gates a board profile against the board it describes, and only radon's has evidence.
- A sweep has no heartbeat, so its sixty-second wedge timer is a guess with fifteen-to-one headroom.
- No sweep has been watched on a board.
- The two-second settle window is a guess.
- No test opens a real serial device.
- Neither the tool nor this note knows whether a session was interrupted.
