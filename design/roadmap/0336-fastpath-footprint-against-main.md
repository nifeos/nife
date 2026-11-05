---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 336. Should the fastpath footprint gate compare against `main` instead of a stored baseline

Premise re-checked 2026-09-19: the build is still owed. DECISIONS §144 (the fastpath footprint gate
gets a delta and a ceiling) ruled both shapes, with the ceiling at 16 KiB on
`max(ipc_send_receive, ipc_call_reply) + syscall_entry`. Nothing has been built against it.
`script/fastpath-footprint` still compares against `bench/fastpath-aarch64.txt`,
`bench/fastpath-riscv64.txt` and `bench/fastpath-x86_64.txt` at a 5% tolerance, with no delta against
`main` and no absolute ceiling. The two designs share little code, because the delta half replaces
the baseline files and the ceiling half is new.

In brief. `script/fastpath-footprint` measures the IPC fastpath's code size against a stored
baseline file, which somebody has to remember to re-record when growth is understood and accepted.
The alternative is to measure against `main` at the time the pull request runs, so the comparison
has no stored state and cannot go stale. Milestone 237 fixed one instance of the stored baseline
going stale and explicitly did not touch the mechanism. The work is to decide which shape the gate
takes, and then to build it.

## Why this matters

The stored baseline has already failed in the way it fails. Milestone 237 records it: two lanes
each measured "within bound" against the same stale baseline and neither re-saved it, so aarch64
headroom fell from 3.9 points to 1.5 with nothing firing. The gate was green throughout. Nothing
in the mechanism can distinguish "this change is small" from "this change is small and the last four
were not."

Leaving it undecided is the worse outcome of the two, because it means the failure recurs and gets
patched per instance. Milestone 237 was that patch. There are two more baselines waiting to do the
same thing.

## The costs on each side, which is the material calef needs

**A stored baseline** is an absolute budget. It answers "how big is the fastpath allowed to be",
which is the question that matters for a claim about this kernel, and it catches slow accumulation
across many small changes because the reference does not move. Its failure mode is the one already
observed: the reference goes stale, growth is absorbed, and re-recording is a manual act nobody
owns.

**Comparing against `main`** is a delta budget. It cannot go stale and needs nobody to remember
anything, which is a full rung up AGENTS.md's ladder. Its failure mode is the inverse and is worse
in the long run: every individual change is within bound and the absolute size grows without limit,
because the reference moves with the tree. It also costs a second build per run, since `main` has to
be built to be measured.

The third shape, which neither the block nor this proposal has priced, is both: a delta check
against `main` for the pull request plus an absolute ceiling that only calef raises. That is more
machinery, and whether the extra check earns its keep is part of the same decision.

## One data point from 2026-09-04, which cuts against `main` alone

PR #716 is the daily toolchain bump: one line of `rust-toolchain.toml`, no kernel source. It failed
the `syscall_entry` check at 10.4% on riscv64 because the newer compiler inlined a device-interrupt
helper into the trap handler (the full account is in the
`a-flat-entry-set-counts-bytes-no-syscall-fetches` proposal, which milestone 433 numbers 368).

**A delta-against-`main` gate would have failed this too, and for a worse reason.** `main` would
have been built under the *old* pin, so the comparison would have attributed the entire 194 bytes to
a pull request whose diff cannot contain them, with no stored file for anyone to inspect. The stored
baseline at least made the reference explicit and stable enough to diff against by hand. This does
not decide the question, and it is one case rather than a pattern; it is offered because it is the
kind of case the delta option is supposed to be immune to and is not.

The same lane's measurements on the other side are in the `unattributed-fastpath-residuals`
proposal, which milestone 433 numbers 361. All
three `ipc_fastpath` baselines are now stale, aarch64 having gone stale again within a week of
milestone 237 re-recording it. That is this proposal's own failure mode recurring on schedule.

## Index row

`script/fastpath-footprint` measures the IPC fastpath's code size against a stored baseline file that
somebody must remember to re-record. It has failed that way: two lanes each measured "within bound"
against the same stale baseline, and aarch64 headroom fell from 3.9 points to 1.5 with nothing
firing. DECISIONS §144 ruled on 2026-09-04 that the gate gets both shapes: a delta check against
`main` that cannot go stale and replaces the baseline files, and an absolute ceiling of 16 KiB per
architecture that only calef raises. They catch different failures, because a delta alone lets
absolute size grow without limit. Nobody has built it.
