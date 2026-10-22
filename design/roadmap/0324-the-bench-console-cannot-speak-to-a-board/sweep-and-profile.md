# Parts 2 and 3 as built: the sweep recognizer and the board profile, 2026-09-19

An appendix of [milestone 324 (the bench console cannot speak to any of the three
boards)](../0324-the-bench-console-cannot-speak-to-a-board.md), moved out of it on 2026-10-12 (UTC)
under §212 (a prose budget). The block keeps what landed, the hoist, the defect another session's
merge created and what could not be tested, and is complete without this file. This file keeps the
working: the recognizer's statuses, the four exercised outcomes, the profile's two tests, the
quiet-timer sizing, the recognizer break in full, and the reasoning behind three follow-on records.

Two changes to one tool, plus the hoist that both of them turned out to need.

## Part 2, the sweep recognizer

`crates/serial_console::progress` gained two rungs, `Stage::Sweep` (`job_mix::STARTED`) and
`Stage::SweepDone` (`job_mix::DONE`). It gained one failure (`Failure::SweepFailed`, carrying the
reason the kernel gave) and the sweep's numbers (`SweepPoint`, `SweepSubrun`). So
`script/board-console --until sweep-done` now answers the question the part named. It needed no new
exit status: `0` finished, `1` refused or otherwise announced, `2` spoke and stopped, `3` ran out
with points still to print.

`cargo xtask job-mix` now judges with that recognizer instead of its own loop. That is the move
`soak_test` already made, and the reason the block of milestone 219 (the boot tour ends and the
kernel halts, so there is nothing to soak) gives: two readers drift the first time either changes.
What stood there was `starts_with("job-mix")` with no timeout at all, so a wedged sweep hung the
command forever and a bench script could not tell it from a finished one. It now tees the whole boot
to a log under `target/` that replays through `script/board-console --replay`, and returns the same
five statuses.

All four outcomes were exercised on a machine rather than argued, which is what makes the
discrimination a fact. `cargo xtask job-mix` exits 0 on a complete sweep, 2 with `--quiet-after 1s`
and 3 with `--for 30s`. The refusal path is a host test built from `job_mix::FAILED` itself, because
no kernel here has refused one. All four were re-run after milestone 168 (a multi-tasking workload
benchmark) was merged in. The windows moved because the sweep now takes 165 seconds rather than 28,
and the statuses did not.

## Part 3, the prologue as data

`crates/serial_console/src/board.rs` declares a `Profile`: ordered `Rung`s, the firmware's own
`Refusal`s, and the relocation discriminator. `Stage`'s four firmware variants became one,
`Stage::Firmware(&'static Rung)`, ordered below `Banner` because firmware runs before the kernel
whatever the board. `Failure::BadImageMagic` and `Failure::UBootRefused` became one
`Failure::FirmwareRefused`. Both were U-Boot's words rather than ours, and a board with different
firmware refuses in its own. `--board radon|xenon` selects; radon is the default, so no existing
behavior moved.

The test that says the split is real reads one capture twice. radon's own 2026-09-01 boot, read
through radon's profile, climbs SPL, OpenSBI, U-Boot and the handoff on its way to the tour. The
same bytes read through xenon's profile reach the same tour and report no firmware rung, because
none of those lines are xenon's to claim. A second test replays
`bench/xenon-2026-09-17/first-light-095500.log` through the xenon profile. It asserts the banner,
the machine line, the five-of-five verdict and the measured-boot refusal, with an empty prologue.
That pair is the ruling made mechanical.

`--until spl` against a board with no SPL is now refused rather than waited out. That is the
smallest visible benefit and the one an operator meets, instead of a two-minute watch ending in
"the time ran out".

## The hoist, which was not optional

The sweep's console markers were three private `const`s in `kernel/src/job_mix.rs` and four string
literals in `xtask/src/main.rs`. Adding a recognizer would have made a third copy, which is finding
3 of milestone 268 (every architecture boots the same way) exactly. They are now `crates/job_mix`'s
(`STARTED`, `DONE`, `FAILED`, `POINT`, `SUBRUN`, `CENSUS`, and `KIND` once milestone 168's per-kind
line was merged in). They sit beside the workload definition both halves of the instrument already
read, on `crates/boot_ladder`'s argument and with its stable-head convention. `board_console` takes
`job_mix` as a dependency for the same reason it took `boot_ladder`: ours, in this workspace,
`no_std`, no dependencies of its own, no `unsafe`.

This is the kernel-side half of the limitation `stop.rs`'s `BUGS` recorded for the soak. It was
done for the sweep because a recognizer could not be written without it. The soak's markers are
still literals in three places and that entry stands.

## The quiet timer, sized without a heartbeat

This is a choice calef may want to overrule, recorded where a reader meets it rather than only
here. A sweep has no wall-clock heartbeat, so its quiet timer cannot be three missed beats the way a
soak's is. It is sized against the slowest subrun instead: 4.0 seconds measured (249,234,771 ticks
on a 62.5 MHz counter) in the capture at
`crates/serial_console/tests/fixtures/captured/qemu-2026-09-19-aarch64-job-mix-medians.log`.
`script/job-mix` defaults `--quiet-after` to sixty seconds, fifteen times that. A board outside that
margin reads as wedged when it is merely slow; `--quiet-after 0` is the escape and it costs the
detection.

The margin was twenty to one when this was written, and milestone 168 spent a quarter of it the same
day. It took twenty-one repeats of a seven-kind mix instead of three of a five-kind one. That is an
argument for the heartbeat rather than for a larger default. The alternative is a heartbeat in
`kernel/src/job_mix.rs`, which is a kernel change and is recorded in the block's follow-on rather
than taken.

## The recognizer was broken by another session before it was merged

The mechanism is the finding, not the bug. While parts 2 and 3 were being built, another session's
lane finished milestone 168 and landed on `main`. That lane changed what `kernel/src/job_mix.rs`
prints for a sweep point. `ticks=<t> jpm=<r>`, the best of three, became `repeats=21 ticks_min=
ticks_median= ticks_max= jpm_median=`, the median of 21 with its two ends. The head, `job-mix:
tasks=`, did not move.

So the hoist this lane made did its job and the recognizer still broke. Every marker matched;
`SweepPoint`'s parse read nothing, because it looked for `ticks=` and `jpm=` and the line no longer
carried either. `ticks_min=` does not start with `ticks=`.

Both branches were green, and that is the part to keep. The parser and the committed fixture had
been made from the same pre-168 kernel, so they agreed with each other and neither agreed with the
kernel. No gate compares a recognizer against a kernel; nothing could have. It was found by reading
the merged source, which is rung zero of AGENTS.md's ladder.

The fix changed three things. `SweepPoint` carries the seven fields the line now prints (`tasks`,
`jobs`, `repeats`, `ticks_min`, `ticks_median`, `ticks_max`, `jpm_median`). They are spelled exactly
as the wire spells them, so the struct can be diffed against a line by eye. The fixture was
re-captured from a current kernel and the old one deleted. `crates/job_mix`'s `KIND` joined the six
markers, because 168's lane added a seventh printed line (`job-mix-kind:`) in the style the hoist
had just retired.

The limitation this leaves is recorded rather than fixed, in
`crates/serial_console/src/progress.rs`'s `BUGS` where the next reader of the parser meets it. The
markers are shared through `crates/job_mix`; the field names inside the line are not. A kernel that
renames or adds a field still prints a line this recognizer matches and still parses to nothing,
silently. Sharing the field names the way the heads are shared is the fix and is not taken here. It
wants a decision about what shape that sharing takes.

## Why three follow-on records were not built

### Sharing the field names

The shape is not obvious, and that is why it is not built. A shared `&str` per field would make the
kernel's `println!` a format string assembled from constants. That is less readable at the place it
matters most. A shared parser in `crates/job_mix`, which both the kernel's printer and the
recognizer are written against, is the stronger version. It is a bigger change than this lane's
remit. The cheap partial measure is already taken: the struct's fields are spelled as the wire
spells them, so a diff by eye works. The fixture being a real capture means a re-capture catches it
the moment somebody re-captures.

### A heartbeat for the sweep

This is the one follow-on item a reader may most want minted as a milestone. The lane that found it
says so plainly rather than claiming a milestone number, which is the integrator's to give.
`kernel/src/soak.rs` prints every five seconds whatever the workload is doing. That is what lets a
watcher call a hang after three missed beats and be right about it. `kernel/src/job_mix.rs` prints
only when a subrun ends, so the sweep's wedge timer is fifteen times the slowest subrun anybody has
measured, and a slow board reads as wedged. It is a kernel change (the supervisor would have to print from
a timer rather than between subruns) and it is outside parts 2 and 3. It is recorded in
`script/job-mix`'s and `notes/board-console.md`'s `BUGS` as well, where a reader meets the number
that would stop being a guess.

### Milestone 439's figure

Milestone 439 (a job-mix sweep goes quiet for a whole subrun) is another lane's block, and a
developer does not edit one, so the correction lives here, where a reader
will meet it. 439's block and its index row quote *2.6 seconds (163,224,570 ticks)* for the slowest
subrun. That is this lane's own figure, from the capture that milestone 168 invalidated. Re-measured
on the merged tree it is 4.0 seconds (249,234,771 ticks on a 62.5 MHz counter). The ratio against
the 60-second default is fifteen to one rather than twenty. It is corrected everywhere this lane
owns. The correction strengthens 439's own argument rather than weakening it: the number moved by
half again in a day, without anybody touching the watcher.
