# The first loaded acceptance run, 2026-08-17

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## The acceptance run, 2026-08-17: 45 loaded runs, 36 green, and what the nine reds were

Milestone 62 (tests that assert on time) has said since 2026-08-01 that running the suite once
cannot verify its own fix. A flake that fires one run in six is indistinguishable from a fixed one
until you have run it many times. So the evidence it asks for is a repeat count under load. Four
rounds of the old note produced such a count three times, by hand, and each time the number lived in
a report. This is that count taken by an instrument that records its own conditions,
`script/repeat-under-load`.

The result is not the green the milestone hoped for. Nine of forty-five runs went red. Eight were
two assertions, in perfect ISA symmetry, both already named in the old BUGS section as known
residuals that only the icount instrument could close. The ninth was a real kernel bug that nothing
in this tree had recorded.

### The instrument and the conditions, stated before the result

`script/repeat-under-load -n 45 -s 8` starts one busy loop per host core and runs `script/test`
(both ISA legs plus the host crates). Per run it records the elapsed seconds, the one-minute load
average sampled every ten seconds, and how many QEMU processes were up.

- Host: Mac15,3, 8 cores, Darwin 25.5.0 arm64. Tree `d9f0d151`.
- 45 runs, 108 minutes of wall clock, 2026-08-17 22:46Z to 2026-08-18 00:34Z.
- One-minute load average across the whole loop: 26.1 low, 63.0 peak. That is at and above the ~22
  the eight-spinner recipe puts an eight-core machine at.
- The machine was not this lane's alone. Two other lanes were gating on the same laptop for the
  first third of the loop, one of them running its own emulator under `-icount`. Their emulators are
  the `QEMUs seen` column: this run has at most one alive at a time, so any sample reading two or
  more is a neighbor.
- A pilot run of the same command was cut short after one run and is not counted here. The eight
  spinners were stopped between every run, and the loop left no QEMU behind.

### Every run, in order

Reporting only the interesting ones would be the dishonesty this milestone exists to remove. That
is the second round's rule, and it still holds.

| run | result | seconds | 1-min load average (min/mean/peak) | QEMUs seen | what failed |
|---|---|---|---|---|---|
| 1 | pass | 171 | 28.9 / 39.7 / 50.8 | 2 |  |
| 2 | **FAIL** | 47 | 31.5 / 34.9 / 39.2 | 1 | aarch64 `timer.rs:414`, the eight-attempt retry budget |
| 3 | **FAIL** | 100 | 39.0 / 41.5 / 44.0 | 3 | riscv64 `timer.rs:509`, the miss taxonomy |
| 4 | pass | 160 | 40.4 / 45.4 / 49.0 | 3 |  |
| 5 | **FAIL** | 104 | 38.6 / 42.3 / 45.4 | 3 | riscv64 `timer.rs:509`, the miss taxonomy |
| 6 | pass | 151 | 33.4 / 38.6 / 42.4 | 2 |  |
| 7 | pass | 151 | 35.7 / 38.4 / 43.2 | 4 |  |
| 8 | **FAIL** | 113 | 32.5 / 36.7 / 39.8 | 3 | riscv64 `timer.rs:455`, the eight-attempt retry budget |
| 9 | pass | 157 | 37.1 / 39.5 / 44.6 | 3 |  |
| 10 | pass | 152 | 31.8 / 36.1 / 43.2 | 4 |  |
| 11 | **FAIL** | 51 | 31.8 / 32.7 / 33.8 | 2 | aarch64 `timer.rs:470`, the miss taxonomy |
| 12 | pass | 158 | 34.3 / 39.3 / 41.8 | 4 |  |
| 13 | **FAIL** | 51 | 36.7 / 40.3 / 43.9 | 3 | aarch64 `timer.rs:414`, the eight-attempt retry budget |
| 14 | **FAIL** | 102 | 39.2 / 41.6 / 45.1 | 3 | riscv64 `timer.rs:455`, the eight-attempt retry budget |
| 15 | pass | 167 | 38.2 / 42.4 / 48.3 | 3 |  |
| 16 | pass | 164 | 48.0 / 51.1 / 56.5 | 3 |  |
| 17 | pass | 175 | 31.4 / 40.4 / 46.9 | 2 |  |
| 18 | pass | 149 | 26.9 / 30.6 / 33.6 | 1 |  |
| 19 | pass | 164 | 26.1 / 30.2 / 35.8 | 1 |  |
| 20 | pass | 145 | 28.1 / 30.6 / 35.8 | 1 |  |
| 21 | pass | 160 | 33.8 / 41.8 / 50.7 | 1 |  |
| 22 | pass | 160 | 43.0 / 46.7 / 52.0 | 1 |  |
| 23 | pass | 148 | 33.6 / 40.0 / 50.9 | 1 |  |
| 24 | pass | 148 | 29.0 / 39.0 / 52.4 | 1 |  |
| 25 | pass | 159 | 27.1 / 35.8 / 42.0 | 1 |  |
| 26 | pass | 161 | 29.8 / 35.1 / 41.7 | 1 |  |
| 27 | pass | 151 | 35.5 / 40.3 / 45.5 | 1 |  |
| 28 | pass | 172 | 31.9 / 37.6 / 41.3 | 1 |  |
| 29 | **FAIL** | 43 | 36.6 / 39.8 / 41.7 | 0 | aarch64 `timer.rs:470`, the miss taxonomy |
| 30 | pass | 164 | 41.7 / 50.1 / 54.0 | 1 |  |
| 31 | **FAIL** | 117 | 33.6 / 39.7 / 49.8 | 1 | riscv64 `frames/src/lib.rs:315`, **double free of a frame** |
| 32 | pass | 161 | 34.6 / 39.7 / 42.3 | 1 |  |
| 33 | pass | 162 | 33.6 / 37.8 / 40.6 | 1 |  |
| 34 | pass | 170 | 31.1 / 39.6 / 47.5 | 2 |  |
| 35 | pass | 161 | 30.5 / 37.6 / 52.2 | 1 |  |
| 36 | pass | 150 | 32.5 / 37.0 / 45.6 | 1 |  |
| 37 | pass | 159 | 34.1 / 40.2 / 45.4 | 1 |  |
| 38 | pass | 160 | 43.6 / 47.2 / 51.7 | 1 |  |
| 39 | pass | 161 | 40.0 / 46.2 / 48.5 | 1 |  |
| 40 | pass | 157 | 33.7 / 36.4 / 40.1 | 1 |  |
| 41 | pass | 160 | 34.6 / 40.7 / 45.6 | 1 |  |
| 42 | pass | 175 | 41.9 / 53.5 / 63.0 | 1 |  |
| 43 | pass | 161 | 31.3 / 40.7 / 54.1 | 1 |  |
| 44 | pass | 163 | 29.6 / 34.4 / 38.9 | 1 |  |
| 45 | pass | 174 | 28.5 / 40.4 / 52.7 | 1 |  |

### The eight timing reds are two assertions, twice each, on both ISAs

The symmetry says this is a property of the assertions rather than of either architecture:

| assertion | aarch64 | riscv64 | what it said |
|---|---|---|---|
| `ticks_arrive_at_the_configured_rate`, the eight-attempt retry budget | `timer.rs:414`, runs 2 and 13 | `timer.rs:455`, runs 8 and 14 | "no miss-free measurement window in eight tries" |
| `the_handler_keeps_up_when_no_lock_is_held`, the taxonomy's cut | `timer.rs:470`, runs 11 and 29 | `timer.rs:509`, runs 3 and 5 | "the timer handler is taking longer than a whole tick period, with no lock held" |

Both were predicted in the old note, and neither had ever been observed. Its BUGS section said the
drift test "can still go red on a pathological host, by design", and called that "rarer by orders".
It also said the taxonomy's cut leaves a one-period-wide window in which a host deschedule "still
fails, wearing the message that blames this kernel". Only a lock-holding probe had measured that, at
0.83 of an interval. This run is the first time either was seen in the wild. It corrects the first
claim: at these loads the retry budget is not rare. It fired four times in forty-five runs, and one
of those was the third run of the loop.

The taxonomy red carries its own numbers, which is the 2026-08-15 fix working even as the assertion
fails. Run 3 reported `last miss re-armed 55810 counter ticks late against an interval of 100000`.
That is 0.56 of an interval, inside the "slow handler" bucket, and the handler was not slow. It is
the documented window, observed, at a lateness lower than the probe that first measured it.

Neither is fixable by widening, and the run is evidence for that. The retry budget has a second,
implicit claim: "a host that cannot give eight clean windows is pathological or the handler is
slow". The sibling assertion carrying the taxonomy answers that properly, and it passed immediately
before this one panicked in run 2's log. Widening the taxonomy's cut trades the flake for a slow
handler going unreported, which BUGS already refuses. Both need the instrument that milestone 78 (the
load-sensitive assertions) built the same day. Under `-icount shift=0,sleep=off` virtual time is a
function of instructions executed, so a contended host cannot move it at all.

That changes what these two are for. `script/icount` now asserts zero missed ticks on both ISAs. That
is a strictly stronger claim than either assertion makes, and one a loaded host cannot falsify. It
is a separate boot mode, so `script/test` still carries the wall-clock pair. The pair still fails at
roughly one run in six under this load, and it no longer learns anything the instrument does not
assert better. So this wants a deliberate disposition, not a wider bound. The question for whoever
takes it: does the wall-clock pair keep a claim of its own on a machine where `script/icount` is not
run? Not whether eight attempts should have been sixteen.

(Correction, 2026-08-18: "strictly stronger" was true of the handler-latency assertion and false of
the drift one. The instrument was blind to the drift bug until it gained a fourth claim; see
[the timer disposition](timer-assertion-disposition.md).)

### The ninth red is not a timing assertion, and it wants its own lane

Run 31, riscv64, during
`force_kill_tests::destroy_reclaims_a_region_whose_resident_is_blocked_in_receive`:

```
[PANIC] panicked at crates/frames/src/lib.rs:315:9:
double free of frame 0x82a3e000
```

That is `Frames::free`'s deliberate loud failure, and its doc comment is right that both cases it
covers are kernel bugs. Nothing in `notes/` or `design/` recorded this; a grep for "double free"
found only the assertion itself. It occurred once in forty-five loaded runs, and zero times in the
quiet run that preceded the loop. It is recorded in the BUGS section of notes/object-revocation.md,
beside the `DESTROY` path it fired in.

Fixed 2026-08-18 in PR #316. The diagnosis corrects this section rather than confirming it. The
last section of this appendix read the sighting as a rate question ("the next lane on it should assume the window
is narrower than that"), and that framing was wrong. It is not a rare race. It is an ownership
defect that reproduces deterministically on aarch64, and riscv64 is only where the timing exposed
it. `user_aspace_create` builds an address space inside a region the caller already holds an
`Untyped` to, where `AddressSpace::new` carves its own. Both stored a bare `u64` whose `Drop` called
`untyped::destroy` unconditionally: two names for one run of memory. The pin argument meant to make
that safe holds only for a drop under `SCHED`, and `sched::finish_switch` releases `SCHED` before
dropping. Beside it, `untyped::destroy` released `REGIONS` between deciding and removing the slot, so
two callers could both pass the refusal check.

The lesson is about the instrument rather than the bug. A loaded repeat count is good at surfacing a
defect and actively misleading about characterizing one. This run produced a frequency (1 in 45) for
something that had no frequency. A lane budgeted against that number would have gone looking for a
narrow race instead of an ownership question. Treat a sighting from this instrument as "there is
something here", never as "here is how often".

Two details from the fix. The write-up lives in the "Two names for one region" section of
notes/object-revocation.md. And the bug was found by reading, not by reproducing. The lane that took
it never saw the panic again and did not need to, because its deterministic test stages the window
by hand rather than racing for it. The instrument said where to look and was no use at all for
saying what was there.

This is the whole point of the milestone: a red run that means something. Eight of these nine reds
are noise the instrument cannot yet remove, and the ninth is a memory-safety bug in the kernel. A
suite that fails for reasons unrelated to the change trains everyone to re-run rather than read.
Here the one red worth reading arrived wearing the same color as eight that were not.

### Load average did not predict the failures; a second emulator did

The most useful number in the table is the one this instrument added, and it is not the load
average:

| condition | runs | red |
|---|---|---|
| a neighboring lane's emulator seen during the run | 17 | 6 |
| this run's emulator alone | 28 | 3 (one of which is the double free) |

The load average separates them not at all. Run 42 passed at a peak of 63.0, the highest in the
table. Run 11 failed at a peak of 33.8, near the lowest. Eight steady spinners raise the load
average a great deal and apparently do not, on their own, reliably deschedule the vCPU across a
250 ms measurement window. Another TCG emulator competing for the same cores does.

The honest caveat cuts against the table above. A run that dies in 43 seconds gives the ten-second
sampler only four looks. So the neighbor count for exactly the runs that failed fastest is the
least reliable figure here. Runs 2 and 29 are recorded at one and zero, and neither can be trusted
to mean no neighbor was up. Read the split as a lead worth instrumenting properly, not as a
measured ratio.

### What this run proves, and three things it does not

It proves the suite is not load-insensitive at the loads the recipe manufactures, and it names
exactly where: two assertions, both ISAs, both awaiting icount rather than a margin. That is a real
answer to milestone 62's acceptance question, and it is a "no".

It does not prove the other assertions are correct. Thirty-six green runs are thirty-six draws from
one host at one load band. That is the fifth round's lesson again: an injection that fires proves
only that an assertion can fail, and a loop that passes proves only that it did not fail here. The
lane for milestone 124 (a thread is born where it lives) did 45 loaded full-suite runs without
reproducing a fault it was hunting.

It does not establish a rate for the double free. One in forty-five is a sighting, not a frequency.
The lane that took it assumed the window was narrower than that, correctly. A thirty-run attempt on
2026-08-18 never reached the test: at a host load of 63 the suite died first on the riscv64 timer
assertion two sections up. The bug was diagnosed by reading the reclaim path instead.

And it does not transfer to CI. This is one laptop, one QEMU build, one load shape. GitHub's runners
are a different machine with a different contention profile, which is the caveat notes/cpu-models.md
attaches to its own matrix.
