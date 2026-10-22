---
status: BUILT
raised: 2026-09-01
built: 2026-09-01
---
# 216. Nothing in this tree can read a board, so every hardware milestone waits on a person

Built 2026-09-01. Minted the same day by the maintainer, from calef wiring the
VisionFive 2 as a remote target and the gap becoming concrete the same hour.

What was built. `script/board-console` (name provisional), over `cargo xtask board-console`
and `crates/serial_console` (both names provisional). It opens the port at 115200 8N1, tees every
byte to a log file that is never optional, recognizes the runbook's boot sequence, and returns a
different exit status for each way a session can end. Reached the stage asked for, the board
announced a failure, it spoke and then went quiet, the time ran out, or the port would not open.
That
five-way answer is the deliverable rather than a nicety, because a bench script's whole reason to
exist is telling a hang from a refusal. See notes/board-console.md, which carries the runbook, the
marker table with each marker's source, and the BUGS.

Built with the board powered off, then checked against the board. The tool was written and
tested with no hardware. Fixtures fed one byte at a time for the recognizer, sources that block
forever for the deadline, a FIFO standing in for a port to cover a failing `stty`, and a source that
speaks and then stops. The real CH343 dongle covered everything except the board, including that
`stty` on the already-open descriptor moves it from 9600 to 115200 and reverts on exit. Then
calef powered the board on, on 2026-09-01, and captured a full boot and a full failure. Both are
committed under `crates/serial_console/tests/fixtures/captured/` as raw bytes off the wire, and
both are asserted on. The documentation was right about the seven markers it named, which is the
part worth recording: the recognizer needed no correction, only additions.

Four outcomes, three of them captured. Success, U-Boot refusing before the kernel runs, the
kernel booting and then halting on purpose at the measured-boot gate, and a genuine hang. The last
has no real sample and its fixture is synthetic and says so; the other three are bytes off the wire.

Two things the board knew that this tree's documentation did not.

The first is a third outcome. From power-on, the extlinux path ends `Moving Image from ...` /
`Device tree not found or missing FDT support` / `### ERROR ### Please RESET the board ###`,
exactly the caveat notes/visionfive2.md records about U-Boot's fallback DTB addresses. That arrives
after the image loads and before the kernel runs, so a recognizer that knew only the stages would
have called the silence after it a hang, which is the worst available answer. It sends somebody
hunting a multicore bug in a kernel that never started. Booted, hung, and refused-before-the-kernel
are three outcomes, and the tool now exits 1 for the refusal and 2 for a hang.

The second is that `nife: the capability core runs on ...` exists and is a stronger claim than
the banner, which is printed before the device tree is touched. `--until tour` is the one to want
when the question is "did it work".

And two traps that only the fourth capture exposed. The measured-boot refusal prints
`Starting kernel ...`, the whole banner, and most of a tour *before* halting, so a watcher that
returned the instant its stage arrived would report the trust boundary refusing as a successful
boot. Reaching the wanted stage now opens a two-second settle window instead of ending the session,
and a failure inside it wins. And a successful boot ends in `wfi`, so the board goes quiet and stays
quiet: treating silence as a hang would fail every good boot, so the quiet timer is suppressed once
the tour completes. Both were wrong before the captures arrived and neither would have been found by
reasoning about the runbook.

And a port-handling fact that cost a power cycle. calef's first capture was garbage, because
`stty` configured the port and then `cat` reopened it, and a new open on a macOS `cu` device puts
it back to the default rate. It looks exactly like a wiring fault. The tool opens first and
configures second, which avoids it, and now also reads the speed back and complains loudly when it
is not 115200, because an invisible wrong baud sends the next person chasing hardware.

The one finding worth carrying out of the lane, because reasoning did not produce it and a
byte-at-a-time test did: a partial line is weaker evidence than a complete one. `U-Boot ` reads as
U-Boot proper right up until the next three bytes turn out to be `SPL`, and a marker carrying a
payload captures a truncated one. So a tail may ratchet a stage and may not settle a word boundary
or capture text.

Nothing gated this, which was the point when it was minted and is now a fact rather than a
plan. No board was needed to build the thing that lets a lane use a board it cannot see, and none
was available while it was built.

In brief. `script/console` runs `cargo xtask shell`, which is QEMU. `script/board-image` builds
the VisionFive 2 payload and prints the `dd` commands for a card, deliberately running nothing
destructive itself. Between those two there is nothing. A serial console on real hardware is not
a thing this repository knows how to open, so every `Gate: HARDWARE` milestone in the sense that
means "the board is on the desk" needs a person at a terminal emulator, reading with their eyes.

That is the binding constraint on the fatal risks rather than a convenience. Four of the five
unrun entries in `design/fatal-risks/README.md` are hardware-gated, and risk 5 (it cannot be made reliable
on multicore, and the bugs appear only on silicon) is the one this hurts most. Its own decisive
experiment is *sustained* stress with the load-sensitive assertions live, and its text is honest
that the class "produces a confidence rather than a verdict". Sustained is exactly what a person at
a bench is worst at.

## What exists as of today, so nobody re-derives it

calef wired the rig on 2026-09-01. The UART is a WCH CH343 dongle, presenting as CDC-ACM, at
`/dev/cu.usbmodem*` on the development Mac; it enumerates whether or not the board is powered.
Board power is on a Kasa smart strip, and `~/.local/bin/vf2-power` on that machine drives it, with
the outlet addressed by alias and no way for a caller to name a different one. Another
outlet on that strip feeds an external drive that must never be switched off.

**The plug was not reachable from either machine when this was written** (discovery found nothing
on two subnets, by broadcast and by unicast to every host), so treat remote power as unproven and
do not build anything that assumes it. This milestone's deliverable stands without it: a console
that reads and logs is useful the moment somebody presses power by hand, and gains reset later.

## What it needs

**A way to open the board's serial port, log what comes back, and know when to stop.** The last
clause is the whole difficulty and is why this is a milestone rather than a one-line `screen`
invocation:

- Knowing a boot succeeded means recognizing the sequence `notes/visionfive2.md`'s bench
  runbook already writes down: the SPL banner, OpenSBI's, U-Boot's, then `Starting kernel ...`,
  then ours. That note is the specification; read it rather than inventing markers.
- Knowing a boot hung is the case that matters, because a hang is what a multicore defect looks
  like. A timeout with the captured log is the answer, not a hang of the tool itself.
- **Never leaving the port open**, which is this tree's `Never leave QEMU running` rule wearing
  different clothes. A held serial port locks out the next run and the failure surfaces far from
  the cause.

## BUGS

- This block does not decide whether the tool drives power, and it should not until the plug is
  reachable. A tool that power-cycles is a different and more dangerous object than one that reads.
  **The built tool reads and never writes**, to the port or to the outlet, so this stayed undecided
  rather than being settled by an implementation.
- Reading alone cannot boot this board, and the capture proves it. The extlinux path fails, so
  reaching nife means interrupting autoboot and typing four commands at `StarFive #`. That is not a
  gap in the tool as specified; it is this block's specification being incomplete, and it is
  written up as a proposal below rather than fixed here, because it is a scope decision.
- It says nothing about the aarch64 or x86_64 boards, which will want the same thing with
  different banners. Whether that is one tool with a board profile or three is a real design
  question and is not answered here.
- A captured log is not a test result. Deciding pass or fail from console text is how a harness
  ends up asserting on a vendor's boot message; where that line sits is worth naming before it is
  crossed. The tool names its outcome `Reached`, for what it observed, rather than `Passed`; that
  is a wording, not a mechanism, and the line is still uncrossed rather than defended.
- The markers are checked against one board, on one day, in two states. Better than
  documentation alone, and not the same as proven. Uncovered: other firmware builds, the two
  synthetic fixtures nobody has yet seen at a bench, and the aarch64 and x86_64 boards. The
  fixtures split into `captured/` and `synthetic/` so the provenance is a path rather than a claim
  in a README.
- **The card's U-Boot environment is degraded** (`bad CRC, using default environment`, several
  `Invalid partition 3`, `"boot2" not defined`) and the board boots through all of it. Recorded
  beside the fixture so nobody reads those lines as a defect in our payload. Whether to repair the
  environment is not this milestone.

## Driving U-Boot: mostly answered elsewhere, and a small residue that is not

This block proposed a milestone for making the console drive U-Boot. **Milestones 217 and 218 were
minted the same day and take most of it.** 217 (the card
carries a kernel and an archive from different builds, and the gate is the only thing that noticed).
218 (every boot of the VisionFive 2 needs a human typing four commands into U-Boot). The proposal is
withdrawn rather than left duplicating them.
218 is the better framing: a rig that works by typing at U-Boot's prompt is recreating in a script
what the boot loader is supposed to do, and depends on catching a two-second window. Fix the boot
path and nothing needs to type.

**What is left over, and it is an architect's**, is smaller than the proposal was: *should this tool
ever be able to write to the port at all?* 218 may remove the need entirely, in which case the
answer is no and this stays a reader. If 218's routes all fail, something has to type, and then the
question is whether it is this tool with an explicit mode or a second one. The argument for one tool
is that the port can only be held once and a driver contains a reader. The argument for two is that
a tool which writes to a board is a more dangerous object than one that reads, which is the
reasoning this block already applies to power.

**Nothing is blocked on the answer**, which is why it is a paragraph and not a milestone: the tool
as built serves every read-only use, and 218 is the thing to do first either way.

## Follow-on

- **Milestone 218.** The board's own autoboot fails, so reading alone cannot boot it: reaching nife
  means interrupting the two-second countdown and typing four commands at `StarFive #`. This block
  proposed a milestone for making the console drive U-Boot and withdrew it, because 218 is the
  better framing: fix the boot path and nothing needs to type. 218 also owns the degraded U-Boot
  environment (`bad CRC, using default environment`, the `Invalid partition 3` lines, `"boot2" not
  defined`), which the board boots through and which this milestone deliberately did not repair.
- **Milestone 217.** The card carrying a kernel and an archive from different builds, minted the
  same day out of the same bench session and taking the other half of the withdrawn proposal.
- **Recorded.** `design/roadmap/0216-board-console.md` BUGS: a captured log is not a test result.
  Deciding pass or fail from console text is how a harness ends up asserting on a vendor's boot
  message. The tool names its outcome `Reached`, for what it observed, rather than `Passed`, and
  that is a wording rather than a mechanism, so the line is still uncrossed rather than defended.
- **Recorded.** `notes/board-console.md` records the provenance of the markers: one board, on one
  day, in two states. Better than documentation alone and not the same as proven. Uncovered are
  other firmware builds and the two synthetic fixtures nobody has yet seen at a bench, which is why
  the fixtures split into `captured/` and `synthetic/` so provenance is a path rather than a claim.
- **Recorded.** `design/roadmap/0216-board-console.md` records that remote power is unproven: the
  Kasa plug was not reachable from either machine when this was written, discovery found nothing on
  two subnets by broadcast and by unicast, so nothing should be built assuming it. The tool stands
  without it and gains reset later.
- **Refused.** Making the tool drive power. A tool that power-cycles is a different and more
  dangerous object than one that reads, and the outlet next to the board's on that strip feeds an
  external drive that must never be switched off. The built tool reads and never writes, to the port
  or to the outlet, so the question stayed undecided rather than being settled by an
  implementation.
  This refusal is milestone 472 (design/roadmap/0472-a-bench-console-that-drives-power.md), which
  carries it with the condition that would change it.
- **Recorded.** `design/naming.md` holds the naming backlog this milestone added to:
  `script/board-console`, `cargo xtask board-console` and `crates/serial_console` all shipped
  provisional, as a lane's names are, and `script/names --unratified` is the worklist.
- **Milestone 324.** Whether `script/board-console`
  should ever be able to write to the serial port at all, and if so whether that is this tool with
  an explicit mode or a second tool. It is an architect's call. Milestone 218 (a human typing four
  commands into U-Boot) may remove the need by fixing autoboot. The answer is then no, and while it
  is open a bench session facing a board that will not boot has no sanctioned way to type
  at it.
- **Milestone 324.** Whether argon
  (aarch64) and xenon (x86_64) get this console with a board profile or a tool each. Same behavior,
  different banners and a different boot sequence, and the choice is an architect's. Until it is
  made the other two boards have no console tool at all, so the bench workflow this milestone built
  exists for one board out of three.

## Index row

`script/board-console` (name provisional) opens the port at 115200 8N1, logs every byte to a file
that is never optional, recognizes the boot sequence, and returns a different exit status for each
way a session ends. Built with no hardware, then **checked against four real captures** from
2026-09-01, all committed as raw bytes and asserted on: two successful boots, U-Boot refusing
before the kernel ran, and the kernel halting at the measured-boot gate. The documentation was
right about every marker it named and silent about two the board prints. **The captures broke two
things reasoning had got wrong**. A refusal that prints the whole banner before halting was
reported as a success (fixed by a settle window), and silence after the tour was reported as a
hang when it is how a good boot ends in `wfi`. A genuine hang is the one outcome with no real
sample. It reads and never writes; 217 and 218 took the driving question.
