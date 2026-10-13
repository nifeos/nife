# The rebooting soak on radon: why, the hazard, and the reset

An appendix to [notes/soak.md](../soak.md). It is milestone 249 (the boot lottery is sampled by a
person walking to the board): `--features reboot_soak_test`, `script/board-image --soak --reboot`
and `script/board-console --tally`. The procedure and its outcomes are in
[rebooting-soak-procedure.md](rebooting-soak-procedure.md).

This was written on 2026-09-03 with the board powered off and no bench session available. That is
the same condition `notes/x86-uefi-boot.md` was written in, and the same reason its procedure is as
detailed as it is. Every claim below is either about code in this tree, which was built and
host-tested, or is a question for the bench, marked as one. The first four steps of the procedure
answer questions nobody here could answer. The bench answered the reset question on 2026-09-04, and
the answer is below.

## Why a rebooting soak, in one paragraph

[radon-2026-09-03.md](radon-2026-09-03.md) records four runs on radon whose rates span fifteenfold.
The census of milestone 240 (the soak reports what happened and not where, so an eightfold difference cannot be explained) explains them: the rate tracks the number of cores that hold an IPC
thread and no grinder. Counting the nine boots of 2026-09-03 that way, six landed on two clean cores,
two on one, and one on none. Three and four clean cores have never been drawn. Nothing says whether
that is rare or structurally impossible. The distribution is the missing thing, and it is missing
because every draw cost a person a walk to the board.

## The hazard, and the four things that answer it

A board that reboots itself on a timer is a board nobody can get back. Every boot runs the same
image and reboots again. So without an escape, the only way back is pulling power and rewriting the
card. That is worse than the problem being solved, and it is why this is a milestone rather than a
one-line change. Four mechanisms answer it, strongest first, in AGENTS.md's own ladder:

1. The loop exists only in a build that asked for it, by a name with `reboot` in it:
   `--features reboot_soak_test`, or `script/board-image --soak --reboot`. An ordinary card, a
   `--soak` card, and every QEMU run are untouched. So the failure cannot arrive by accident.
2. Any build of it for a non-riscv64 target is a compile error, not a card that quietly never
   resets. The reset is SBI's, and the escape is the NS16550's line-status register; neither exists
   elsewhere. A card that silently never rebooted would look exactly like a board that drew the same
   placement fifty times.
3. The kernel polls the console UART's data-ready bit every beat (five seconds), and again through
   the five seconds before each reset. The bit is sticky. It is set while a byte sits unread, and is
   cleared only by reading that byte, and nothing in a soak boot reads it. So the question being
   asked is *"has anybody typed since this armed"*, not *"is anybody typing right now"*. A poll every
   five seconds cannot miss a keypress. Any byte counts, so no character has to be agreed on between
   the board and whoever is at the terminal.
4. Stopping disarms the reboot and leaves the soak running. It does not halt the kernel. That is
   deliberate, and it is the better half of the design. A halted kernel is silence, and `Stage::Soak`
   has already told `board_console` that silence after a soak starts is a hang. So stopping the loop
   would have reported itself as the failure this whole instrument exists to detect. Disarming leaves
   the board in the well-understood state of milestone 219 (the boot tour ends and the kernel halts, so there is nothing to soak), still beating, and the run is not thrown away
   to get the board back.

And there is a fallback that needs no cooperation from this kernel at all. U-Boot's autoboot
countdown runs on every one of these boots, and anything typed into it drops the board at
`StarFive #`. That is the escape a person had before milestone 218 (every boot of the VisionFive 2 needs a human typing four commands into U-Boot) removed the need for it. It is a
two-second window rather than a five-second one. It is what remains if the kernel's own escape turns
out not to work. The card is the one after that.

A bounded reboot count was considered, and cannot be built here. A cap of fifty would be the obvious
rung-one answer, and there is nowhere to keep the count. A cold reset takes the RAM. The only
persistent store on the path is the U-Boot environment in the SPI flash of the only board of its kind
this project owns. Milestone 218 already refused to write to it, for the same reason. What is bounded
instead is the *wall clock per draw*, which is a weaker property honestly stated: the board never
wedges in the loop, it only stays in it.

## Verifying the reset before anything is left unattended

Two facts this tree did not have, and the bench gets both in the first four minutes.

### Does the reset work? Answered 2026-09-04, with a third outcome

radon's OpenSBI accepts SRST reset type 1 and never returns, and the board does not come back.
There is no reset: OpenSBI's reset is an I2C write to the PMIC, it fails, and OpenSBI hangs
(notes/board-reboot.md). From `target/board/radon-2026-09-04-srst-reset-pmic.log`, in
file order:

```
line   3: U-Boot SPL 2021.10 (Feb 12 2023 - 18:15:33 +0800)     <- the power-on boot
line 241: nife on RISC-V (rv64, S-mode, Sv39)
line 399: soak-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot).
line 401: i2c read: write daddr 36 to
line 403: i2c read: write daddr 36 to            (repeating)
     ...: cannot read pmic power register
```

So the outcome table in the procedure was incomplete, and the missing row is the one that actually
happened. The firmware neither refuses with `-2` nor goes dark at the `ecall`. It accepts, resets,
and the *firmware on the way back* fails. Something the PMIC needs is not reinitialized by a warm SoC
reset the way it is by removing power. U-Boot 2021.10's SPL has no recovery for it.

What it settles: an unattended series is not available on radon by this route. The escape works,
and was verified the same evening. `soak-reboot: DISARMED at t=75s` was sent mid-soak, and the soak
carried on past the 120s mark it would otherwise have rebooted at. So the mechanism is sound, and the
firmware is the wall.

And it retires a guess. Milestone 249's block refused a smart-plug series as *"a lane spent on a
guess until the firmware has actually refused reset type 1"*. The firmware has now effectively
refused, in a way no amount of reading could have predicted. So milestone 224 (nothing can
power-cycle radon, so a hung soak needs a person) moves from a convenience to the only remaining
route to an unattended series.

A correction, recorded because it cost the architect an hour of a late evening. The maintainer
reported this working, twice. The evidence was the `U-Boot SPL` banner at line 3 of that log, read
out of `grep` output whose order is the file's rather than the event's. That banner is the power-on
boot. Nothing in the transcript ever showed a boot on the far side of a reset. The tell is that the
reboot line is at 399 and the banner at 3. The only reliable reading is line order within one
segment.

### Does the escape work on this cable?

Nothing in the kernel can prove it. A UART cannot receive a byte it sends. A receive path that is
miswired, unpowered at the adapter, or held by something else reads "nobody typed" forever. That is
indistinguishable from nobody typing. This is the one mechanism in the design that rests on a
procedure rather than on a machine, and the procedure is step 4 of
[rebooting-soak-procedure.md](rebooting-soak-procedure.md). Do not skip it because the first boot
looks healthy. A healthy boot is exactly what a board with a dead receive line looks like.
