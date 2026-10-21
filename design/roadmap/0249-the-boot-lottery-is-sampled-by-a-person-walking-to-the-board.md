---
status: PARTIAL
raised: 2026-09-03
milestone_dependencies: 592
decision_dependencies: none
machine_requirements: none
specific_machine: radon (the boot lottery being sampled is radon's)
needs_person: yes
---
# 249. The boot lottery is sampled by a person walking to the board, so nine draws is a whole evening

Minted 2026-09-03 by calef, who asked during the boot series whether the soak
could reboot itself, and built the same day on `milestone/249-self-rebooting-soak`. *(Number
provisional until the merge queue lands it.)*

It is the second kind the roadmap README distinguishes: the board is here and
this needs a person at it. The mechanism, the escape and the reader are built and are green on
what a host can gate; radon's first answer, from 2026-09-04, is recorded below. See *What was
built, with the board powered off* below, and notes/soak.md's procedure, whose first four steps are the ones a lane cannot take.

`PARTIAL` rather than `IN-PROGRESS`, deliberately. Building the mechanism shipped; running the
series is a bench session, with nothing a lane could pick up and no branch to name, which is what
`IN-PROGRESS` is for. Milestone 218 (every boot of the VisionFive 2 needs a human) answered
`NOT-STARTED` a day earlier, correctly, because it has one phase and none of it had happened. Here a
real artifact exists, and saying otherwise would make the column lie in the other direction.

In brief. On 2026-09-03 nine boots of radon produced a fifteenfold throughput range, and
milestone 240's census explained it: rate tracks the number of cores free of grinders and carrying an
IPC group. Every one of those nine draws cost calef a walk to the board and the maintainer a capture.

The distribution is the thing that is missing, and nine draws barely sketch it. Counting by rate,
the nine landed at two clean cores six times, one clean core twice, and zero once. **Three and four
clean cores have never been drawn at all**, so the top of the curve is unmeasured and it is unknown
whether it is rare or structurally unreachable. A hundred draws would answer that; nine cannot.

## The mechanism already exists and is called with the wrong argument

`kernel/src/arch/riscv64/semihosting.rs` calls SBI's `system_reset` today, under the `board` feature,
to power the board off at the end of a test run:

```rust
const SRST_RESET_TYPE_SHUTDOWN: usize = 0;
```

SRST defines reset type 0 as shutdown and 1 as cold reboot. So a self-rebooting soak is one
constant and a timer, over an `ecall` this project already knows radon's OpenSBI honors, because the
shutdown path is in use. Milestone 218's `boot.scr` then drives the next boot with nothing typed at
it, which was proven on this board for the first time the same evening.

## The hazard, which is the part to design first

A board that reboots itself on a timer is a board nobody can get back. Every boot runs the same
image and reboots again, so short of pulling power and rewriting the card there is no way to reach a
normal boot. That is worse than the problem being solved, and it is the reason this is a milestone
rather than a one-line change somebody makes in an afternoon.

The escape suggested by the situation: before rebooting, check whether a byte has arrived on the
UART. A console is attached for the whole series anyway, the kernel already reads that device, and
it makes stopping the loop as cheap as pressing a key. Other shapes exist (a bounded count, a
build-time cap) and the choice is this milestone's, but something must make an unattended loop
stoppable without physical access.

## What was built, with the board powered off

**radon was unpowered on the evening of 2026-09-03 and this lane could not touch it**, which is the
same condition milestone 87's UEFI lane worked in and is why the deliverable is shaped the way that
one's was. The mechanism, and a bench procedure detailed enough that the person who *can* reach the
board spends their time on the machine rather than on reconstructing what to type.

- `--features reboot_soak` (name provisional), on all three architectures since 2026-09-24. After a fixed window the soak calls
  SBI SRST `system_reset` with reset type 1 instead of beating forever.
  `arch::semihosting::reboot` returns `sbiret.error`, so a firmware that refuses says so on the
  console instead of being assumed either way.
- **The escape**, four mechanisms deep, argued in the next section.
- `script/board-image --soak --reboot`, which prints what the card will do and how to stop it
  before it builds anything.
- `script/board-console --tally <log>` and `board_console::lottery` (names provisional): read a
  capture of many boots and report what the lottery drew each time, plus the distribution. Eight
  host tests, two of them over real captures, including one taken for this milestone because the
  tree had no capture carrying a placement census.
- **notes/soak.md** gains the procedure, in order, with a table mapping each observable outcome to
  what it means and what to do, and four `BUGS` entries.

The build without the feature is byte-for-byte the build that was there before, and that was
measured rather than argued. The release riscv64 `--features board` kernel was disassembled at this
branch and at its base commit, with the four edited kernel files swapped and nothing else touched.
The two disassemblies differ on 216 lines and every one of them is an LLVM local-symbol
disambiguator (`....llvm.<hash>`, which is a hash of the module's text and changes when a comment
does). **Not one instruction differs**, so the plain board build, the plain soak build and every
QEMU run are untouched by construction and by measurement:

```console
$ diff before.asm after.asm | grep -E '^[<>]' | grep -vc '\.llvm\.'
0
```

The one edit that a `board` build without `reboot_soak` even compiles is `sbi_system_reset` taking
its reset type as a parameter and returning `sbiret.error`. Its existing caller passes the same
constant it always did, and the disassembly above is what says the compiler agrees.

**What is not built, and the reason it is not**: a distribution. That is this milestone's stated
proof and it requires the board.

## The hazard, designed first, and what each layer buys

Four things make the loop stoppable without physical access (the requirement above), listed
strongest-first in AGENTS.md's ladder because the ordering is the argument.

1. The loop exists only in a build that asked for it, by a name carrying the word `reboot`.
   `boot_lottery` was the other candidate and was refused: it names the finding, and the flag is met
   by an operator about to write a card that makes their board reset itself. The name has to say
   what the machine will do.
2. **A build that cannot reboot must not pass for one.** It would look exactly like a board that
   drew the same placement fifty times. `script/soak-test --reboot` fails any build that does not come back.
3. The kernel polls the console UART's data-ready bit, every beat and again through the five
   seconds before each reset. The bit is sticky (set while a byte is unread, cleared only by
   reading it, and nothing in a soak boot reads it), so a five-second poll cannot miss a keypress:
   the question is *"has anyone typed since this armed"*, not *"is anyone typing now"*. Any byte
   counts, so nothing has to agree on a character.
4. Stopping disarms the reboot and leaves the soak running, rather than halting. This is the
   half of the design worth arguing for, because the obvious answer is to halt. `Stage::Soak` has
   already told `board_console` that silence after a soak starts is a hang, so a halting escape
   would report itself as the exact failure this instrument exists to detect. It would throw the
   run away to get the board back. Disarming leaves the board in milestone 219's understood state,
   still beating, and costs nothing.

And one that needs no cooperation from this kernel: U-Boot's autoboot countdown runs on every
one of these boots, and anything typed at it lands the board at `StarFive #`. That is a two-second
window rather than a five-second one, which is why it is the fallback rather than the design, and it
is exactly the escape a person had before milestone 218 removed the need for it.

### What was refused, and why

A bounded reboot count. The obvious rung-one answer, and it cannot be built here. A cold reset
takes the RAM, and the only persistent store on this path is the U-Boot environment in the SPI flash
of the only board of its kind this project owns. Milestone 218 already refused to write to it, for
the same reason. What is bounded instead is the wall clock *per draw*, which is weaker and is stated
as such: the board never wedges in the loop, it only stays in it.

A magic stop character. Refused because it is a thing two programs would have to agree on, for
no gain: any byte is strictly easier to send and cannot be got wrong by a person at a terminal.

Rebooting on a failure. The reset is asked for *after* the beat's verdict, and `panic!`
diverges, so a soak that found something keeps its evidence.

### The weakest link, named rather than hidden

Nothing in this kernel can prove the escape is reachable, because a UART cannot receive a byte
it sends. A receive path that is miswired, unpowered at the adapter, or held by something else reads
"nobody typed" forever, which is indistinguishable from nobody typing. What closes that is step 4 of
the procedure: press a key on the first boot and confirm `soak-reboot: DISARMED` before walking
away. That is rung four wearing a procedure's clothes, it is the lowest rung in the design, and it
is in the `BUGS` sections of both `kernel/src/soak.rs` and notes/soak.md as well as here.

## The one fact this milestone could not check

Whether radon's OpenSBI implements SRST reset type 1 as a reset. SBI permits any subset of the
reset types, so the kernel prints `sbiret.error` rather than assuming. A refusal (`-2`) or a dark
board would each close this route and point at a power-cycled series; notes/soak.md's outcome table
separates the two. The bench answered on 2026-09-04, below.

## What it must not become

**This is not milestone 248** (a placement can only be drawn, never constructed, so the strongest
finding on this machine rests on two boots), and the two should not be folded together. 248 builds an
arrangement on purpose; this draws many more of them at random. They have different risks, and this
one can ship in an afternoon while 248 needs a design. Keeping them apart is what stops the cheap one
waiting on the expensive one.

## The proof that this milestone worked

A distribution over at least fifty unattended boots, recorded in notes/soak.md beside the nine
hand-drawn ones: how often each clean-core count comes up, and whether three or four ever does.

Not a kernel that reboots, which is the mechanism rather than the result. By this test the
milestone is not finished, and what remains is a bench session, not a phase somebody could build. `script/board-console --tally` is what turns that session's log into the row this section
asks for, and it prints the caveats beside it so a count of draws is not quoted as a probability.

## The bench answered it, 2026-09-04, and closed the route

**radon's OpenSBI accepts SRST reset type 1, and the board does not come back.** Corrected
2026-09-24: no reset happens. OpenSBI's reset is an I2C write to the PMIC, the write fails, and
OpenSBI hangs. The transcript is `target/board/radon-2026-09-04-srst-reset-pmic.log` and notes/soak.md
carries the line-numbered extract.

That is a third outcome, and neither this block nor the note predicted it. Both priced a refusal
(`sbiret.error == -2`) against silence at the `ecall`. What happens is that the firmware accepts
and then fails before any reset, in its own PMIC write.

The kernel half is sound and was verified the same evening. The reboot fires when it says it
will, prints what it is about to do, and the escape works: a byte sent mid-soak produced
`soak-reboot: DISARMED at t=75s` and the soak carried on past the 120s mark it would otherwise have
rebooted at. That is step 4 of the bench procedure, the one thing no mechanism in the kernel can
prove for itself, done. The mechanism is not what failed.

What it retires. This block refused a smart-plug series as *"a lane spent on a guess until the
firmware has actually refused reset type 1"*. It has now effectively refused, so milestone 224
(nothing can power-cycle radon, so a hung soak needs a person) is no longer a convenience. It is the
only remaining route to an unattended series on this board, and the guess it was waiting on is spent.

And the escape's own limitation showed up in use. `script/board-console` holds the port and
cannot write, so the byte had to be sent by detaching it, which is exactly the proposal this lane
filed (`design/roadmap/proposals/board-console-cannot-speak-to-the-board.md`). Sending it at the
wrong moment also stopped U-Boot's autoboot countdown and cost a power cycle, which is the fallback
this block names working as an obstacle.

## The draws so far, read 2026-10-21

`script/board-console --tally` over the five radon captures in `bench/` counts **41 unattended
draws of the rebooting soak: 38 at one clean core, 3 at two, none at zero, three or four.** Nine
more reach the fifty. Four builds disagree (the two later ones drew one clean core 39 times in 39),
so the pooled count is not one distribution. notes/soak/boot-lottery.md has the table, what was
left out, and why these do not compare with the nine hand-drawn power cycles.

## All three architectures, 2026-09-24

aarch64 and `x86_64` now reboot through the same soak path, each proven under QEMU by a second boot.
The routes, the proofs, each board's first bench step and the corrected radon diagnosis are in
notes/board-reboot.md.

## Follow-on

- **Milestone 592.** Milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write), awaiting
  one bench reset.
- **Milestone 593.** A watchdog for a wedged kernel, milestone 593 (a wedged kernel resets itself):
  `design/roadmap/0593-a-wedged-kernel-resets-itself.md`.
- **Milestone 653.** Milestone 653 (xenon may carry Intel AMT, which would power-cycle it and give it a console with nobody at the desk). xenon's possible AMT: `design/roadmap/0653-xenon-may-carry-amt.md`.

- **Milestone 324.** *The watcher reads a board and never speaks to it, so stopping a reboot loop needs a
  person at the keyboard.* `script/board-console` holds the port and cannot send the byte that is
  this milestone's escape, so both the escape and its verification are rung four. A `--stop` mode
  and a `--stop-after <n>` would make each a command. Not done here because it overturns an
  invariant stated in that script's own header, which makes the shape of it an architect's call. It
  is recorded in the meantime in `notes/board-console.md`'s `BUGS`.
- **Recorded.** *The escape rests on a procedure and not on a mechanism*, in the `BUGS` of
  `kernel/src/soak.rs` and notes/soak.md, and in *The weakest link* above. A UART cannot receive a
  byte it sends, so no mechanism in this kernel can close it; the proposal above is the nearest
  thing.
- **Recorded.** *Nothing about the reboot has run on radon, including whether the firmware
  implements SRST reset type 1*, in notes/soak.md's `BUGS` and in *The one fact this milestone could
  not check* above. It is the bench's first question and the kernel prints the answer.
- **Refused.** A power-cycled series over radon's smart plug, as an alternative to SRST. It is the
  better experiment (a power cycle is what the nine control boots were) and it is a lane spent on a
  guess until the firmware has actually refused reset type 1. notes/soak.md's outcome table is where
  that finding would arrive; raise it then.
  This refusal is milestone 477 (design/roadmap/0477-a-power-cycled-boot-series.md), which carries it
  with the condition that would change it.
- **Outstanding.** The tally's clean-core definition is `board_console::lottery`'s, host-tested
  against the one settled arrangement radon has printed. If a series shows the rate does not follow
  it, that is this milestone's result rather than a defect in the reader, and the definition is what
  a second board would test. Checked 2026-09-03.

## BUGS

- It samples radon and nothing else. The lottery is this board's, with four startable harts and
  this firmware; a distribution measured here says nothing about argon or xenon.
- **Cold reboot is not power cycling**, and the two may not draw from the same distribution. Anything
  that survives a warm reset (firmware state, cached tables) is held constant across every draw in a
  way a hand-cycled series did not hold it. The nine hand-drawn boots are the control, and the check
  is whether the automated distribution matches them where they overlap.
- An unattended board can fail unattended, and what was built reports it rather than preventing
  it. `script/board-console --tally` counts U-Boot SPL banners as boot attempts beside `soak: started`
  as draws, so "three boots and then a wedge at 2am" reads as attempts exceeding draws instead of
  as a short series. The watcher's own silence judgment is still per run rather than per boot, which
  is the weaker half and is unchanged.
- Nothing here explains the lottery, only measures it. Why placement lands where it does is
  DECISIONS 138's territory and is not answered by more samples.
- A rebooting series and a long run are different experiments. Fifty two-minute draws measure the
  distribution over placements; the three-hour run in notes/soak.md measures what one placement does
  over time and is the only evidence that a slow draw is stable rather than a warm-up. Neither
  replaces the other.

## Index row

SBI SRST is already called with reset type 0; type 1 is a cold reboot, and 3 and 4 clean cores
have never been drawn
