---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A fuzz window reports what it reached, not only that it was clean

Lane/fuzz-window-coverage-counters raised this on 2026-11-01 (UTC) while looking for the milestone
that names fuzz coverage counters for the continuous host. None exists. The only text is a decision
of 2026-10-31 (UTC) in [the fuzz budget proposal](the-fuzz-budget-that-counts-for-risk-7.md)
that the compiler inserts inline 8-bit counters into the kernel code that both in-guest drivers exercise. The other is a line in the BUGS list of
[the host note](../../../notes/fuzzing/continuous-host.md). Neither has a scope, so
this lane wrote none of the code and stops here.

## The gap

`windows.tsv` records, per window, the driver, slot, first seed, seeds run, time, status and commit.
The daily report can say that 12,000 seeds ran clean. It cannot say whether those seeds exercised
the whole kernel or the same few paths. Condition 2 of risk 7 (a measure of coverage exists, and
coverage has stopped growing or the growth has an explanation) has nothing to read.

## What a window counts

Reuse: the mechanism already decided is LLVM sanitizer-coverage inline 8-bit counters on the kernel code.
This proposal adds no other source.

Per window, per driver, two numbers and nothing else:

- `edges_hit`: how many counters are non-zero at the end of the window.
- `edges_total`: how many counters the kernel image has.

Both are integers. The window does not record which counters, which seeds, which inputs or which
findings produced them. A bitmap of hit edges would let a reader work backward toward an input, so
the window keeps the sum and drops the bitmap.

The kernel supplies the counter region. It also supplies a way for the driver to read it after each seed. The
driver can add the counters it saw to the window's union and print one line at the end, next to the
existing `<n> seeds, <ms> ms` line that `helpers/continuous-fuzz.sh` already parses.

## What the harness can observe today

Without the counters, almost nothing. The kernel-thread driver prints a `Census` of its own calls
(calls, refused, parked, delegated, replied, revoked and a few more) after the seed count. Those are
counts of what the driver did, not of which kernel code ran, and the confined driver prints no
census. A count of distinct syscall numbers hit would need a small change in each driver, because
neither tracks the set. It would be cheap and dependency free, but it measures the driver's
alphabet, not the kernel's reach, and it saturates within the first few seeds. It is a weaker stand-in
for condition 2, not the same measure. This lane did not build it, because building it would choose a
measure for condition 2 on the architect's behalf.

## Cost per window

Unmeasured. The fuzz budget proposal already names the measuring work (a one-architecture build that
times the counters under KVM in the nife kernel) as the second work item, and nobody has done it.
What can be said without a run: the counter increment is one byte write per edge taken, and the
read after each seed is a scan of `edges_total` bytes. At the host's measured 457 seeds per second
for the kernel-thread driver a seed has about 2.2 ms, so the scan matters only if the region is large.
The window also pays one more field to parse. The window cost stays one rebuild, one boot and one
minute of guest time.

## The format change

Append two columns to each `windows.tsv` row: column 9 `edges_hit` and column 10 `edges_total`, with
`-` in both when the window lacked them. Old rows have eight columns. The publish script
reads columns by number, so a row with eight columns reads as before. The report treats a missing
or `-` value as unknown and leaves those windows out of the coverage figure, never as zero.
The daily report gains a line per driver. It shows the largest `edges_hit` of any clean window over the
`edges_total` of the newest. It shows the same figure for the previous report so a reader sees growth.
A selftest in the style of `helpers/fuzz-daily-publish-selftest.sh` (from #3044) feeds a mix of
eight-column, ten-column and `-` rows. It checks three things: old rows still produce the same
totals, the coverage line appears and is correct, and no seed, commit or held-window field appears
in the report outside what it already carried. It joins `script/lint`.

A held window reports no coverage at all, because its counters would say something about a finding.

## Prior art

libFuzzer prints `cov:` (edges hit) and `ft:` (features) on every status line, and OSS-Fuzz publishes
per-target coverage reports (recalled). Both show how much the fuzzer reached, and OSS-Fuzz
shows the source lines too. nife differs in two ways: nife holds its findings, so the published
figure must be a bare number, and the report is one text file per day, not a browsable coverage view.
A concrete example: window `kernel-thread`, slot 1, 27,400 seeds, clean, `edges_hit` 9,310 of
`edges_total` 41,882. The report line reads "kernel-thread: 9,310 of 41,882 edges (22.2%), previous
report 9,128".

## What I need from an architect

1. Whether condition 2 reads edge counters only (this proposal's recommendation, the decided
   mechanism) or may start with the syscall-number set as an interim figure.
2. Confirm the privacy line: bare totals per driver, no bitmap. Relaxing the stricter form later is compatible, and nobody can recall a published bitmap.

The column layout is a two-way choice, so the maintainer decides it as provisional; reversing costs one
pull request.

## Proof

1. The first sweep with the counters builds and runs on x86_64 under KVM, and a window row carries
   non-`-` values.
2. The host note states the measured speed cost of the counters.
3. The selftest above fails on `main` and passes with the change.
