# board-console: the BUGS list in full

An appendix to [notes/board-console.md](../board-console.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). The main page lists each limitation in a line; this is each one in
full.

## The evidence the markers rest on

The markers are checked against one board, on one day, in four states. That is much better than
where this started, which was documentation only, and it is not the same as proven. Not covered:
every other way this board can behave, a different vendor firmware build with differently worded
banners, the two synthetic cases nobody has yet seen at a bench, and argon, which has never printed
a byte to this tool. x86_64 is no longer in that list. xenon was captured on 2026-09-17
(`bench/xenon-2026-09-17/first-light-095500.log`), and the markers that matched were the portable
ones. That is a second board's worth of evidence for exactly the half of `Stage` that claims to be
portable, and none at all for the half that is radon's firmware.

There is no real sample of a hang, which is the outcome this tool exists for, since a hang is what a
multicore defect looks like from the far end of a serial cable. The synthetic fixture is a real
capture truncated before the tour. If risk 5 ever produces a genuine one at a bench, capture it; it
would be worth more than every other fixture here. A marker whose real text differs by a word is
missed, and a missed marker reports a healthy board as having got less far than it did.

A missed marker fails toward pessimism; a matched one does not. Matching is `contains` anywhere in a
line, so a console that echoed `Starting kernel ...` back would be read as having handed over.
Nothing guards against that. This bullet used to rest on the tool never writing. Since milestone 324
(the bench console cannot
speak to any of the three boards)
landed it rests on what the writing mode sends instead. `--stop` sends one byte, which cannot spell a
marker, and the mode logs what it sent, in hex, so a reader can rule it out by hand. A host test
asserts the same of the mode's own log annotations, which are the other text this tool adds to a
capture. A future mode that typed whole lines would put this hazard back, which is a reason to keep
the write surface at named commands rather than a keyboard.

It does not recognize an OpenSBI trap dump, which the triage ladder lists as a real and specific
outcome (the kernel started the S7 and vendor firmware died in its own handler). The dump's exact
first line is written down nowhere in this tree. Guessing it would put text in a recognizer that no
machine has ever printed. Such a boot is caught as silence or as time running out, with the dump
sitting in the log, which is one step worse than being named.

A captured log is not a test result. The roadmap block says this first, and it is worth repeating
where the tool is. This reports how far a boot got. Deciding a milestone passed on the strength of a
vendor's boot message is a line nobody has agreed to cross. `Reached` is named for what was observed
rather than for a verdict.

## Firmware, writing, and power

It drives no firmware, and the reason that was fatal on this board is gone. The captured failure was
the proof: the extlinux path from power-on ended at `### ERROR ###`, so reaching nife meant
interrupting autoboot and typing the four `StarFive #` commands. Milestone 218 (every boot of the
VisionFive 2 needs a human typing four commands) closed that on
2026-09-16, confirmed by a boot whose countdown expired with nobody typing
(`bench/radon-2026-09-16/tour-083200.log`). So a reader is now enough to get radon from power-on to
the kernel, and nothing here types at U-Boot. What is not gone is the next entry, which arrived from
a different direction and is what the writing mode answers.

Stopping a rebooting soak no longer needs a person, and no byte of it has reached a board yet
(milestones 249 and 324). `--features reboot_soak_test` makes a board cold-reboot every two minutes.
Its escape is a byte on the console UART: any byte, checked every five seconds. Until milestone 324
this tool held the port and could not send one. So the escape was reached by a person typing, either
into this session's terminal or by detaching it first. Detaching a console is not free
(notes/soak.md records a 6% rate change from doing it mid-run). `--stop` and `--stop-after <n>` are
now that escape from a script. What is not yet proven is the wire. Milestone 324's lane had no board
attached to it. So the decision to send is tested against captures, and the bytes against a buffer
standing in for the port. Nothing has tested that a write to the descriptor reaches the UART, or that
the kernel's poll finds it. The first real `--stop` is that experiment and its `DISARMED` line is the
result.

The `--stop` gate costs a draw when a session attaches mid-run. The byte goes out only on the board's
arming announcement, which is printed once per boot. So a `--stop` against a board already two
minutes into a draw waits for that draw's reboot before it can act. Attaching at power-on has no
such gap. The gate is what makes every other state safe (see "Writing to a board" on the main page),
and this is its price, stated rather than hidden.

An unconfirmed send is three faults wearing one report. The byte may not have left the host. The
board's receive path may be dead (`kernel/src/soak.rs`'s own `BUGS` says nothing in the kernel can
tell). Or the kernel may be wedged in a way the beat has not yet shown. The tool says the byte went
out and was not acknowledged, and cannot say which. The surrounding beats in the log separate them
by hand: beats still arriving with no `DISARMED` points at the receive path.

It does not touch power. The board's Kasa strip was not reachable from either machine when this was
written, and the roadmap block declines to decide whether this tool should ever drive it. A tool
that power-cycles is a different and more dangerous object than one that reads.

## Board profiles

The board profile is real now, and exactly one board has ever been checked against one. calef ruled
on 2026-09-19 that this is one tool whose profile is the firmware prologue and nothing else.
Milestone 324 part 3 built it. `crates/board_console/src/board.rs` declares radon's four rungs, its
two refusals and its relocation note as data. `--board` chooses, and `Stage::Firmware` is what a
boot reaches while climbing one. What that does *not* do is add evidence. radon's rungs are asserted
against bytes off the wire on 2026-09-01. xenon's profile says it has no prologue, which rests on one
capture on one day (`bench/xenon-2026-09-17/first-light-095500.log`). It would miss a xenon that fell
over inside its own firmware. argon has no profile on purpose. A Jetson boot chain read out of vendor
documentation and never watched on a wire is the assertion-shaped-as-measurement failure this tree
keeps catching. Its prologue stays unwritten until a board prints something.

Nothing gates a profile against the board it claims to describe. It is the same gap
`crates/boot_ladder` records against the kernel, one level out, with the same mechanism: review, plus
a capture in `tests/fixtures/captured/` for every rung anybody asserts on. A rung declared with a
marker no machine prints fails in the direction that looks like success.

## Sweeps, timing, and the port

A sweep has no heartbeat, so its wedge timer is a guess with headroom. The sixty-second default on
`script/job-mix` is fifteen times the longest subrun measured on one host under TCG. A board slower
than fifteen-to-one is called wedged when it is merely slow, and `--quiet-after 0` gives up the
detection to avoid that. Giving the sweep a real wall-clock heartbeat, the way `kernel/src/soak.rs`
has one, is a kernel change and was not made here. The longest subrun is 4.0 seconds since milestone
168 (a multi-tasking workload benchmark)'s mix, and `script/job-mix`'s own header says fifteen.

No sweep has been watched on a board. The recognizer was proved against QEMU on 2026-09-19. A
finished sweep exits 0, one wedged by a one-second quiet window exits 2, and one cut off by a
twelve-second cap exits 3. A refusal is a host test built from `job_mix::FAILED` rather than from a
capture, because no kernel here has refused one. radon has never run a sweep this tool watched; that
is milestone 168's own HARDWARE gate and not something part 2 could close.

The settle window is two seconds, and two seconds is a guess. It is long enough for the captured
measured-boot refusal, which follows the banner within a tour's worth of printing, and there is no
principle behind it beyond that. A failure that a board announces three seconds after the awaited
stage would still be reported as a success. `--until none` has no early exit at all and sees
everything up to the cap, which is the answer when being right matters more than being quick.

No test opens a real serial device. A pseudo-terminal pair would be the honest stand-in. Making one
needs `posix_openpt` and its `ioctl`s, which is `libc`, which is §46 (thin primitives or whole
subsystems)'s decision and not a lane's. So
the port layer is tested as pure logic plus a regular file, and the one claim that leaves unproven is
named in [design.md](design.md). If this crate ever takes a serial dependency, a pty test should
arrive with it.

Neither the tool nor this note knows whether a session was interrupted. Ctrl-C kills the process,
the descriptor closes, and the log holds every byte that had arrived. But no summary line is written,
and no exit status distinguishes it from a crash. The log is the record; the summary is a
convenience.
