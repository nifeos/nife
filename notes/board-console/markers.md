# What board-console recognizes, and where each marker came from

*Name: provisional.*

An appendix to [notes/board-console.md](../board-console.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). It holds the marker tables with their sources, the traps
in the four outcomes, and how a job-mix sweep is judged.

Every marker was first quoted from `notes/visionfive2.md`'s bench runbook ("What appears, in order,
on a good day" and the failure-triage ladder) or from this tree's own source. They were then
checked against the board, on 2026-09-01, against a captured success and a captured failure that
now live in `crates/serial_console/tests/fixtures/captured/` and are asserted on by the tests.

The first four rows are radon's and live in a board profile (milestone 324 part 3,
`crates/serial_console/src/board.rs`); the rest are the kernel's and are shared by every board.

| stage | board | marker | source |
|---|---|---|---|
| `spl` | radon | `U-Boot SPL` | runbook, confirmed on the board |
| `opensbi` | radon | `OpenSBI v` | runbook ("record the version line"), confirmed: `OpenSBI v1.2` |
| `uboot` | radon | `U-Boot ` followed by a word that is not `SPL`/`TPL`, or `StarFive #` | runbook, confirmed both ways |
| `handoff` | radon | `Starting kernel ...` | runbook, confirmed |
| `banner` | every | `boot_ladder::BANNER` (`nife on `) | `kernel/src/main.rs`, confirmed |
| `machine` | every | `boot_ladder::MACHINE` | milestone 268 |
| `selftest` | every | `boot_ladder::SELF_TEST` | milestone 268 |
| `tour` | riscv64 | `boot_ladder::TOUR` | `kernel/src/main.rs`, confirmed |
| `prompt` | every | `boot_ladder::PROMPT` | milestone 268 |
| `soak` | every | `soak-test: started` | milestone 219 |
| `sweep` | every | `job_mix::STARTED` | milestone 324, confirmed under QEMU 2026-09-19 |
| `sweep-done` | every | `job_mix::DONE` | milestone 324, confirmed under QEMU 2026-09-19 |

xenon's profile is an empty prologue, and that is a measurement rather than a gap.
`bench/xenon-2026-09-17/first-light-095500.log` shows nothing before `nife on ` that this tool
matches, because the machine boots through PVH straight into our banner. A test replays that exact
file through the xenon profile and asserts the banner, the machine line, the five-of-five verdict
and the measured-boot refusal, all with no firmware rung climbed at all. That is what the split is
for: the same radon capture read through xenon's profile reaches the same tour and reports no
firmware rung, because none of those lines are xenon's to claim.

One more thing is reported and is deliberately not a stage: `init/build`, meaning userspace init
built its child. It cannot be a stage without breaking the ladder, because a card with no archive
runs the whole tour and never reaches it, so putting it below `tour` would make reaching the tour
imply something that did not happen. It is a detail of a successful boot, like `Moving Image from`,
and it is the only difference between the two successful captures.

Since milestone 295 it reads captured logs and nothing else. calef retired
`components/src/builder.rs` on 2026-09-14, so no kernel this tree builds prints `init/build` and
`userspace_ran()` is `false` on every live board. The matcher stays because
`tests/fixtures/captured/vf2-2026-09-01-userspace.log` carries the line, and that capture is
evidence off real VisionFive 2 silicon that cannot be re-taken with a different kernel; deleting the
recognizer to tidy the code would throw the evidence away. What to ask of a board booted today is
`reached() >= Stage::Prompt`, and it is a stronger question. `init/build` meant userspace built one
child out of two capabilities, where the prompt cannot appear unless userspace built the console
server, the line discipline, the input driver and the shell. The two successful captures are still
the two successful captures; what distinguishes them is now a fact about 2026-09-01 rather than a
test to run.

And five things that end a session early rather than waiting the clock out:

| what | marker | source |
|---|---|---|
| U-Boot giving up, with the reason on the line before | `### ERROR ### Please RESET the board ###` | captured 2026-09-01; no documentation in this tree named it |
| a stale or wrong card | `Bad Linux RISCV Image magic!` | triage ladder row 3, never seen at a bench |
| the trust boundary refusing | `MEASURED BOOT REFUSED` | `kernel/src/trust.rs`, captured 2026-09-01 |
| our own panic, with its message | `[PANIC] ` | `kernel/src/panic.rs` |
| U-Boot relocating (a note, not a stage) | `Moving Image from` | triage ladder row 4, confirmed |

The captures of 2026-09-01 turned this from a two-way question into a four-way one, and three of
the four are things a board actually did.

| outcome | what it looks like | exit |
|---|---|---|
| Success | the tour reaches `nife: the capability core runs on RISC-V.`, then the board goes quiet because the kernel halts in `wfi` | 0 |
| U-Boot refused before the kernel ran | the image loads and relocates, then `### ERROR ### Please RESET the board ###` | 1 |
| The kernel booted and then halted on purpose | `MEASURED BOOT REFUSED`, after the banner and most of a tour | 1 |
| A genuine hang | it starts, says a few lines, and stops before the tour | 2 |

Three of those four are traps for a naive recognizer, and each got a fix:

Both successes and the measured-boot refusal contain the banner. The refusal prints
`Starting kernel ...`, the whole nife banner, and most of a tour before halting. So a watcher that
returned the moment `--until banner` was satisfied would report it as a success, in the case a
bench script most needs to be right about. So reaching the wanted stage starts a settle window
(`settle`, two seconds) rather than ending the session, and a failure arriving inside it wins.

Silence after the tour is how a good boot ends. The kernel halts in `wfi`, so the board goes
quiet and stays quiet. Treating silence alone as a hang would fail every successful boot, so the
quiet timer is suppressed once the tour completes, whatever `--quiet-after` says. The synthetic hang
fixture stops *before* the tour, which is exactly the difference.

A measured-boot refusal is the gate working, not a crash. It exits 1, because the board did not
boot and a script asking "did it boot" needs a no. But the message says what actually happened and
what to do. A report that read like a crash would send somebody debugging the boot mechanism, when
what is wrong is that two files on the card came from different builds.

## The extlinux outcome, which documentation did not have

The capture that mattered most is the one that failed. From power-on, the extlinux path ends:

```
Moving Image from 0x40200000 to 0x80200000, end=802ff000
Device tree not found or missing FDT support
### ERROR ### Please RESET the board ###
```

That is exactly the caveat `notes/visionfive2.md` records about U-Boot's fallback DTB addresses,
and it arrives after the image has loaded and relocated and before the kernel has run a
single instruction. A recognizer that knew only the stages would have watched the image load, seen
nothing more, and called the silence a hang. That is the worst available answer: it sends somebody
hunting a multicore bug in a kernel that never started. Booted, hung, and refused-before-the-
kernel are three outcomes, not two. The tool exits 1 for the refusal and 2 for a hang, and the
difference is the difference between resetting the board and opening a debugger.

## And `tour` is a stronger claim than `banner`

`nife on RISC-V (rv64, S-mode, Sv39)` is printed before the device tree is touched, so it says the
console works and nothing else. `nife: the capability core runs on RISC-V.` is the last line of the
boot tour, so it says paging, traps, the timer, the frame allocator, SMP and the scheduler all came
up. `--until tour` is the one to want when the question is "did it work"; the default stays
`banner` because only the milestone-tour build prints the other, and a shell build would wait for a
line that is never coming.

## The card's U-Boot environment is degraded, and that is not ours

Both captures carry `*** Warning - bad CRC, using default environment`, several
`** Invalid partition 3 **` / `Couldn't find partition mmc 1:3` / `Can't set block device`
complaints, and `## Error: "boot2" not defined`, before U-Boot finds `mmc 1:1` and gets on with it.
The board boots through all of it. Nobody should read those lines as a defect in our payload, and
whether the environment is worth repairing is somebody else's milestone.

## Telling a finished job-mix sweep from a wedged one

Milestone 324 part 2, found by milestone 168's lane. `script/job-mix` boots a `--features job_mix`
kernel whose boot tour ends in a sweep over task counts rather than in a halt. Until this milestone
nothing recognized any of it. A finished sweep and one that stopped partway both ended as the clock
running out, so they shared an exit status. And `cargo xtask job-mix` had no timeout at all and hung
forever on a wedge.

Two rungs close that. `sweep` is `job_mix::STARTED`, meaning the pool spawned and the workload
announced itself. `sweep-done` is `job_mix::DONE`, meaning every point printed and the kernel is
parking. So:

```
script/board-console --until sweep-done --for 30m     # at a board
script/job-mix --arch riscv64 --smp 4                 # under QEMU, same judging, same statuses
```

returns `0` for a sweep that finished, `1` for one the kernel refused to start (`job-mix: FAILED`),
`2` for one that spoke and then stopped, and `3` for one still printing when the clock ran out.

The quiet timer is the hard part, and a sweep is harder than a soak. A soak beats on the wall
clock every five seconds whatever the workload is doing, so a missed beat is a missed deadline and
fifteen seconds is three of them. A sweep speaks only when a subrun ends. The longest subrun is the
top of `job_mix::TASK_SWEEP`, measured at 4.0 seconds
(`crates/serial_console/tests/fixtures/captured/qemu-2026-09-19-aarch64-job-mix-medians.log`,
249,234,771 ticks on a 62.5 MHz counter), so `script/job-mix` defaults `--quiet-after` to sixty
seconds, fifteen times that. A board outside that margin will be called wedged when it is merely
slow; `--quiet-after 0` is the answer and it gives up the wedge detection. This is in both `BUGS`
sections because it is the one number here a bench operator may have to change.

And the margin is spent by changes nowhere near it. It was twenty to one against a 2.6-second
subrun when this paragraph was first written, earlier on 2026-09-19. Milestone 168 landed the same
day and took twenty-one repeats of a seven-kind mix where there had been three of a five-kind one.
That made the slowest subrun half again as long and cost a quarter of the headroom without anybody
touching `--quiet-after`. A number quoted from a capture is only as current as the capture.

And `job-mix: done` joins the quiet exemption, with `tour` and `prompt`, because the kernel
halts in `wfi` after it. `sweep` deliberately does not: silence during a sweep is the wedge.
