# swish-check failures, week 2026W40, classified (name provisional)

Every failed swish-check job from 2026-09-27 to 2026-10-04 (UTC), classified by its first failing
line. Companion to [the swish-check CI flake, measured](swish-check-flake.md), which carries the
mechanisms; this file carries the week's rows. Classified by a maintainer session on 2026-10-04,
checked against milestone 342 (the kernel and the `console` server drive one UART from two address
spaces) by lane `lane/echo-splice-recurs` the same day.

## The question this answered

The week's count read as if the echo wait / splice failure had survived milestone 342: 6 of 18
failures, and 4 of the 6 merge-group evictions. **It had not.** Milestone 342 merged as #1498 at
2026-10-03T09:52:00Z. All six echo failures ran before that, and none of their trees contained it.
Five head SHAs were checked with `git merge-base --is-ancestor`. The sixth, `01a862cdc`, is a
deleted merge-group ref; it ran at 03:29Z, before a 04:13Z group of the same pull request that
lacked it.
The counts were right and the week boundary hid the fix inside them.

After the merge, 118 merge-group swish-check jobs ran to 2026-10-04T01:55Z. One failed, on the
`noteless` empty answer, not on an echo. Across every failed run in that span (145 runs, any
event) there were two swish-check failures, both that same `noteless` signature. Before 342 the
splice took 3 of 29 merge-group jobs in one day. At that rate, 118 clean jobs would happen by chance
about 4 times in a million (0.9 to the 118th).

## Rows

MG is a merge-group run, whose failure evicts the pull request. "342 in tree" is filled for the echo
rows only.

| Run | Event | Leg | First failing line | Verdict | 342 in tree |
|---|---|---|---|---|---|
| 37167978481 | MG #1573 | aarch64 | `noteless` answered "" | flake | |
| 37153714653 | PR milestone/323 | aarch64 | `noteless` answered "" | flake (same SHA green) | |
| 37112286561 | MG #1516 | riscv64 | never echoed `packages/noteless/0.1.0/noteless` | splice | no (09:12Z) |
| 37095806100 | MG #1489 | aarch64 | never echoed `package install downloads/uptime.nifepkg` | splice | no |
| 37093381682 | MG #1489 | aarch64 | same | splice | no (ref gone; by time) |
| 37086229708 | MG #1478 | aarch64 | same | splice | no |
| 37085138162 | PR claim-only gate | aarch64 | same | splice | no |
| 37082721025 | PR milestone/152 | riscv64 | never echoed `echo hello world \| wc` | splice | no |
| 37094606658 | MG #1486 | riscv64 | `std_exerciser`: progenitor out of memory | flake (#1444) | |
| 37081228603 | PR milestone/624 | x86_64 | `noteless` killed, invalid opcode, rip 0x60001748 | real (624's code) | |
| 36808261077 | PR milestone/624 | x86_64 | same, same rip | real | |
| 36815059506 | PR milestone/152 | aarch64 | capability slots 24 of 32, above the recorded 23 | real (budget gate) | |
| 36771759801 | PR milestone/152 | aarch64 | same | real | |
| 36815044632 | PR milestone/614 | aarch64 | package source did not send a whole package | real (614) | |
| 36771736550 | PR milestone/614 | aarch64 | same | real | |
| 36756713459 | PR ci-split-build-test | aarch64 | no `std_exerciser` built for this architecture | real (CI config) | |
| 37100843406 | PR milestone/632 | aarch64 graphical | `graphical_terminal` launched, no prompt | unclear | |
| 37095306042 | PR milestone/632 | aarch64 graphical | no prompt within 120s | unclear | |

Both riscv64 rows are the gauge splice, not a riscv-only path the fix missed. Their transcripts show
the kernel's gauge inside the echo, as in the parent note's aarch64 example:

```
$   capability slots: 24 of 32 at peak
echo  progenitor sta hck: 20576 of 49152 bytes at peak, 28576 spare
ello world | wc
```

Run 37112286561 spliced two gauges into each other (`260.164 spare`) as well as into the echo.

## Counts

- By signature: echo splice 6 (uptime 4, riscv64 2); `noteless` empty answer 2; progenitor OOM 1;
  x86_64 invalid opcode 2; slots budget 2; package truncation 2; CI split config 1; graphical no
  prompt 2.
- By leg: aarch64 12 (2 of them graphical), riscv64 4, x86_64 2.
- Merge-group evictions: 6. Echo splice 4, `noteless` 1, OOM 1. After 342 merged: `noteless` 1.

## BUGS

- The week boundary is the failure this file records, and nothing gates it: a weekly count that
  straddles a fix reads as a recurrence. Whoever counts next should split rows at a fix's merge time
  and say which side each falls on.
- The `noteless` empty answer is now the only flake seen at merge-group since 342. It has its own
  note and proposal (PR #1594), not this one. Resolved by milestone 748 (the noteless launch that
  prints nothing); see below.

## The noteless empty answer, measured (2026-10-04 UTC)

Lane `lane/noteless-flake-chase`. Both failing transcripts show the line typed once (the leg's
summary counts 8 lines) and echoed twice, a gauge between the copies and the program's sentence
after the second. That is `system_log_contract::console::Inserter::flush`: a line end, the queued
kernel lines, then the partial line again. The flush timer is `FLUSH_NS` (250 ms) in
`components/src/system_log.rs`. Only a flush landing after the last typed character and before
Enter leaves an exact copy; one mid-typing leaves a prefix, which the gate already read correctly.
That is why the line was always whole.

Widened window: `NIFE_SWISH_ENTER_PAUSE_MS=400 script/swish-check --arch aarch64` on patagonia,
alternating builds, each boot bounded by `helpers/qemu-bounded.sh`.

| Harness | Boots | Boots failed | Empty answers |
| --- | --- | --- | --- |
| without `without_redraws` | 4 | 4 | 12 (3 a boot: `echo hello world \| wc` and two `package install` lines) |
| with it | 3 | 0 | 0, both reboot segments run |

The empty answers fall on whichever lines a gauge follows, so the signature was never about
`noteless`; after the reboot that is its first line. A unit test feeds run 37167978481's bytes
through the gate's filter and parser: red before the fix with CI's own `""`, green after.

One system-side finding, recorded in `components/src/system_log.rs`'s BUGS: the flush deadline is
armed by the first forwarded line, not the queued one, so a queued line can be redrawn well inside
250 ms. Cosmetic; whether it widened the CI window is unmeasured.
