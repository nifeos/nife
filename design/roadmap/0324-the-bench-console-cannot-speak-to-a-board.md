---
status: BUILT
raised: 2026-09-18
built: 2026-09-19
---
# 324. The bench console cannot speak to any of the three boards

Four proposals from three lanes were about the same tool, and this block carries them as one. All
four parts are built. What no part of this
milestone could close is stated in each part and again below: **no byte of the writing mode has
reached a board, and no job-mix sweep has been watched on one.**

The working moved to three appendices on 2026-10-12 (UTC) under §212 (a prose budget). This page is
complete without them; [the directory's
index](0324-the-bench-console-cannot-speak-to-a-board/README.md) lists them.

- [`rulings.md`](0324-the-bench-console-cannot-speak-to-a-board/rulings.md): the reasoning behind
  both rulings, the two premises that had decayed before promotion, and the rename.
- [`stop-mode.md`](0324-the-bench-console-cannot-speak-to-a-board/stop-mode.md): parts 1 and 4 as
  built, including why the obvious trigger loses and the two choices calef may overrule.
- [`sweep-and-profile.md`](0324-the-bench-console-cannot-speak-to-a-board/sweep-and-profile.md):
  parts 2 and 3 as built, the recognizer break, and why three follow-on records were not built.

## The rulings, 2026-09-19

It was `DECISION` on parts 3 and 4 when this block was minted. calef answered both on 2026-09-19,
in the order this block asked for, and every part was taken the same day.

Part 4: yes, it writes, under a mode that names its purpose. The invariant in
`script/board-console`'s header was *"it reads and never writes to the board"*. It is now that it
writes only what a named mode sends, and every byte it sends is printed into the log. The log is
what a bench run is judged from, so a byte it does not show makes the capture a lie. The mode is
narrow because of an incident. The lane of milestone 249 (the boot lottery is sampled by a person
walking to the board) sent its escape byte by detaching the console. Sending it at the wrong moment
stopped U-Boot's autoboot countdown and cost a power cycle. So the shape is 249's own proposal,
`--stop` and `--stop-after <n>`. The reason offered for a "no" had expired. It
was that milestone 218 (every boot of the VisionFive 2 needs a human typing four commands into
U-Boot) might fix autoboot. 218 did, on 2026-09-16, and did not touch 249's escape.

Part 3: one tool with a board profile, and the profile is the firmware prologue only. A tool each
would fork 3,545 lines whose portable majority is already proven shared. The split was already in
`Stage`: radon's four firmware rungs, then the kernel's own ladder, reachable on all three
architectures since milestone 268 (every architecture boots the same way). Argon is deferred rather
than profiled. It has never booted nife and sits behind milestone 127 (the seL4 machine), which is
NOT-STARTED. A profile read out of vendor documentation would be an assertion shaped as a
measurement. Its prologue stays unwritten until a board prints something.

Answering part 4 first made part 1 trivial, which is the cluster earning its keep. Two premises had
decayed between filing and promotion, as two had in milestone 323 (the falsification record is
incomplete in five ways). The working is in [`rulings.md`](0324-the-bench-console-cannot-speak-to-a-board/rulings.md).

## Why a cluster

`script/board-console` and `crates/serial_console` are one tool, and four lanes filed four
limitations against it from four directions. Two came from the sweep of 216's block by milestone 247
(follow-on work named by a finished milestone goes nowhere). One came from milestone 249's
boot-lottery lane and one from the job-mix lane of milestone 168 (a multi-tasking workload
benchmark). Each is small. Together they are one statement. The bench console is a reader, for one
board, that cannot tell every finished run from a wedged one, and every one of the three boards is a
machine somebody has to walk to.

That matters because `design/fatal-risks/README.md`'s remaining experiments are hardware-gated in
the sense that means *a person has to sit at the board*. Milestones 16, 168 and 201 all wait on
hands rather than on hardware. A console that can drive a board is the difference between those
being a bench session and being a job.

## The four parts

1. BUILT 2026-09-19. It cannot stop a reboot loop. Milestone 249's self-rebooting soak is stopped
   by pressing a key, so nine draws is a whole evening at the board. Found by milestone 249's lane,
   which built the escape this automates. Part 4's answer made this the smallest part: `--stop`
   sends the byte and logs that it sent it. `--stop-after <n>` ends a series with exactly the sample
   it was asked for. Read *What landed* before extending this: the obvious trigger is the wrong one.
2. BUILT 2026-09-19. It cannot tell a finished job-mix sweep from a wedged one.
   `crates/serial_console` recognized the boot sequence and, since milestone 219 (the boot tour ends
   and the kernel halts, so there is nothing to soak), a soak's stages and heartbeat; the job-mix
   sweep had no recognizer. Found by milestone 168's lane. Two rungs close it, `sweep` and
   `sweep-done`.
3. BUILT 2026-09-19. It serves two boards of three, not one, and the third is not ready to be
   served. Filed on 2026-09-03 as *"it serves radon only"*, which was true then. On 2026-09-17 it
   captured xenon's boot (`bench/xenon-2026-09-17/first-light-095500.log`), and a `--replay` on
   2026-09-19 reports the banner, the machine line, the five-of-five self-test and the measured-boot
   refusal. Nothing was added for xenon: it boots through PVH straight into our banner. What was
   left was radon's prologue being hard-coded rather than declared, now
   `crates/serial_console/src/board.rs`. Found by milestone 247's sweep, from the block of
   milestone 216 (nothing in this tree can read a board).
4. BUILT 2026-09-19. It cannot type at a board, and it should be able to. A named mode, its bytes
   logged, not a keyboard: `--stop`, `--stop-after <n>`, and the rewritten invariant.
   `notes/board-console.md` states each limitation as the ruling has it. Found by milestone 247's sweep, from milestone 216's
   block.

## What landed, 2026-09-19: parts 1 and 4

`--stop` and `--stop-after <n>` on `script/board-console`, and the rewritten invariant in its
header, in `crates/serial_console/src/lib.rs` and in `notes/board-console.md`. The engine is
`crates/serial_console/src/stop.rs`; the reading loop took one parameter (`watch::watch_with`)
rather than being copied. `Escape::observe_line` is the only writer in the crate. It logs and
flushes the announcement before the byte goes out, and the log shows the byte as `0x0d`.

The obvious trigger loses, silently. `soak-test: started` comes about a kilobyte before
`arm_reboot` drains the receive buffer, so a byte sent there is discarded on almost every run and
looks like a miswired board. The trigger is the arming announcement (`THIS BUILD REBOOTS THE
BOARD`), printed after the drain. That one gate sends nothing in every state the ruling did not
cover, and by then U-Boot's countdown is long over. Its price: a session attaching mid-draw waits
up to two minutes.

`--stop` waits fifteen seconds for the board's own `DISARMED` line. That makes it the first
end-to-end check of the soak escape, turning `kernel/src/soak.rs`'s manual procedure into a command
with an exit status. No exit status was added: confirmed is `0`, and sent-but-unconfirmed or
never-sent is `3`. A board-announced failure (`1`) or a hang (`2`) still wins.

Two choices calef may want to overrule, both cheap: the byte is a carriage return (`NUL` was
refused beside the constant), and `--for` defaults to 150 seconds per draw plus two minutes, a
default duplicated rather than shared. The reasoning for all of this is in
[`stop-mode.md`](0324-the-bench-console-cannot-speak-to-a-board/stop-mode.md).

## What landed, 2026-09-19: parts 2 and 3

Part 2: `crates/serial_console::progress` gained `Stage::Sweep` and `Stage::SweepDone`,
`Failure::SweepFailed`, and the sweep's numbers (`SweepPoint`, `SweepSubrun`). `script/board-console
--until sweep-done` answers the part's question with no new exit status. `cargo xtask job-mix`
judges with the same recognizer instead of a `starts_with("job-mix")` loop with no timeout, and tees
the boot to a replayable log under `target/`. It exits 0 on a complete sweep, 2 with `--quiet-after
1s` and 3 with `--for 30s`; a host test covers the refusal. All were re-run after milestone 168
merged.

Part 3: `crates/serial_console/src/board.rs` declares a `Profile` of firmware `Rung`s, `Refusal`s
and the relocation discriminator. `Stage`'s four firmware variants became `Stage::Firmware(&'static
Rung)`, and two U-Boot failures became `Failure::FirmwareRefused`. `--board radon|xenon` selects,
with radon the default. One radon capture read through both profiles reaches the same tour, with
and without a prologue. A xenon capture replays through xenon's empty one. `--until spl` on a board
with no SPL is now refused rather than waited out.

One hoist was not optional. The sweep's markers were private `const`s in `kernel/src/job_mix.rs`
and literals in `xtask/src/main.rs`; a recognizer would have made a third copy (milestone 268's
finding 3). They are now `crates/job_mix`'s, which `board_console` depends on as it does on
`crates/boot_ladder`.

The sweep has no wall-clock heartbeat, so its quiet timer is sized against the slowest subrun: 4.0
seconds measured, with `--quiet-after` defaulting to sixty, fifteen times that. calef may want to
overrule that sizing. The detail is in
[`sweep-and-profile.md`](0324-the-bench-console-cannot-speak-to-a-board/sweep-and-profile.md).

## Another session's merge broke the recognizer, 2026-09-19

While parts 2 and 3 were being built, milestone 168 landed on `main` and changed what a sweep point
prints: `ticks=` and `jpm=` became `repeats=21 ticks_min= ticks_median= ticks_max= jpm_median=`.
The head did not move, so every marker matched and `SweepPoint` parsed nothing. Both branches were
green, because the parser and its fixture came from the same pre-168 kernel. It was found by reading
the merged source. The fix spells `SweepPoint`'s seven fields as the wire does, re-captured the
fixture, and added `KIND` to the shared markers. The field names inside the line are still not
shared, and that is recorded in `crates/serial_console/src/progress.rs`'s `BUGS`.

## What could not be tested

No byte of the writing mode has reached a board, and no sweep has been watched on one. Neither lane
had a board attached, and neither radon nor xenon was reachable. Parts 1 and 4 rest on eleven host
tests in `stop.rs` asserting the exact bytes a board would receive, and two in `watch.rs` through
the real loop. Parts 2 and 3 rest on QEMU captures and host tests. radon's rungs are asserted
against bytes off the wire. Xenon's empty prologue rests on one capture on one day, and argon has
no profile at all.

## Follow-on

- **Done.** Parts 2 and 3, on `milestone/324-sweep-recogniser-and-board-profile` on 2026-09-19,
  including the fix for the defect the merge with milestone 168 created.
- **Recorded.** *The block of milestone 439 (a job-mix sweep goes quiet for a whole subrun) quotes a
  figure this lane has since re-measured.* It quotes 2.6 seconds (163,224,570 ticks) for the slowest
  subrun; on the merged tree it is 4.0 seconds (249,234,771 ticks on a 62.5 MHz counter), fifteen to
  one against the default rather than twenty. A developer does not edit another milestone's block,
  so the correction lives here.
- **Recorded.** *A marker is shared and the fields inside the line are not*, in
  `crates/serial_console/src/progress.rs`'s `BUGS` and in `crates/job_mix`'s marker header. A reader
  may want it minted as a milestone; its shape is not obvious.
- **Recorded.** *The job-mix sweep has no wall-clock heartbeat, so its wedge timer is a guess with
  headroom*, in the `BUGS` of `script/job-mix` and `notes/board-console.md`, and beside
  `Stage::Sweep`. This is the item a reader may most want minted. It is a kernel change, outside
  parts 2 and 3.
- **Recorded.** *No job-mix sweep has been watched on a board*, in the `BUGS` of `script/job-mix`,
  `crates/serial_console/src/lib.rs`, `notes/board-console.md` and
  `crates/serial_console/tests/fixtures/README.md`. radon running one is milestone 168's own
  HARDWARE gate; the first bench sweep closes it.
- **Recorded.** *Only radon's profile has been checked against a machine, xenon's emptiness rests on
  one capture, and argon has none*, in the `BUGS` of `crates/serial_console/src/board.rs` and
  `notes/board-console.md`.
- **Recorded.** *Nothing gates a board profile against the board it claims to describe*, in
  `crates/serial_console/src/board.rs`'s `BUGS`. It is the gap `crates/boot_ladder` carries against
  the kernel, one level out, with the same mechanism: review, plus a capture for every rung asserted
  on.
- **Recorded.** *The soak's console markers are string literals agreeing by a reader having checked,
  in three places*, in the `BUGS` of `crates/serial_console/src/stop.rs`. They were never hoisted
  into `crates/boot_ladder`. It is a kernel change, outside parts 1 and 4, and the soak's are now
  the last set in that state.
- **Recorded.** *No byte of the writing mode has reached a board*, in the `BUGS` of `stop.rs` and of
  `notes/board-console.md`. The first bench run closes it, and its `DISARMED` line is the result.
- **Recorded.** *A `--stop` against a board already mid-draw waits that draw out*, in the same two
  `BUGS` sections. It is the price of the gate that makes every other state safe.
- **Refused.** A general typing mode, or `--send <text>`. An open keyboard beside U-Boot's autoboot
  countdown cost milestone 249's lane a power cycle. A second named command with its own purpose is
  the shape to add when one is wanted.
- **Refused.** Hoisting `--stop`'s confirmation window and the draw length into a crate shared with
  the kernel. They are a timeout and a default, not an agreement, and §46 (thin primitives or whole
  subsystems) says a shared crate is for what two binaries must agree on.

## Index row

`script/board-console` watches a board's serial port and could not type at it, hard-codes radon's
firmware chain, and cannot tell a finished job-mix sweep from a wedged one. Four lanes filed those
separately between 2026-09-03 and 2026-09-04; read together they say the bench console is a reader.
That is load-bearing rather than cosmetic, because the remaining fatal-risk experiments (milestones
16, 168, 201) are gated on a person sitting at a board rather than on hardware existing. A
console that can drive one is what turns those from a bench session into a job. calef decided both
gated parts on 2026-09-19. It writes, under a named mode whose bytes are logged, and the three boards
share one tool whose board profile is the firmware prologue alone, with argon deferred behind
milestone 127 rather than guessed from vendor documentation. All four parts landed the same day.
`--stop` and `--stop-after <n>` end milestone 249's rebooting soak from a script, the byte goes out
only on the board's own arming announcement and never before it, and every byte sent is printed into
the log in hex. `--until sweep-done` tells a finished job-mix sweep from a wedged one, with no new
exit status and with `cargo xtask job-mix` judging through the same recognizer rather than a second
copy that had no timeout at all. And the firmware prologue is data: `--board radon|xenon`, with
radon's four rungs and two refusals declared in `crates/serial_console/src/board.rs` and everything
from the kernel banner up shared. It is proved by reading one radon capture through both profiles and
getting the same tour with and without a prologue. What none of it proves is hardware: no
`--stop` has reached a board and no sweep has been watched on one.
