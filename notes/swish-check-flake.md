# The swish-check CI flake, measured (name provisional)

Two `build + test (host + QEMU)` failures on the evening of 2026-09-29 (UTC), both at the
swish-check step, on pull requests touching no runtime code: #1444 (hooks only, riscv64, "could not
spawn (the progenitor is out of memory)", run 36619551608) and #1442 (ledger only, aarch64, "the
prompt never echoed `packages/greeting/0.1.0/greeting`", run 36622549563). Both were green on rerun.
This note asks whether swish-check is flaky in CI, how often and by what mechanism, or whether the
evening was unlucky; it prices remediations without enacting any. Every date is UTC. A typed line
whose input stopped mid-echo is [2026W42](swish-check-flake-2026w42.md).

## Method, and its limits

- The CI workflow (`ci.yml`) over 2026-08-30..2026-09-30, metadata only, from the Actions API. The
  filtered listing caps at 1000 rows per query, so the window was fetched per day and deduplicated
  by run id; 5380 unique runs, newest 2026-09-29T23:19Z. A run's conclusion reflects its
  latest attempt, so a failure rerun green reads as success; `run_attempt` in the listing finds
  the reruns, and each prior attempt's failed jobs were read separately. The API throttled to 502
  zero times, so the sleep-and-retry path never fired.
- Job and step conclusions for every run whose latest attempt failed (444 runs). Then 15 job
  logs, tail only: the two phenomenon attempts, the five 2026-09-29 evening failures, a spread of
  the 2026-09-26/27 block, and one each from 2026-09-28, 2026-09-25 and 2026-09-21. Check-run
  annotations carry no failure text (exit code only), so no cheaper classification exists.
- Local reproduction from the lane worktree: `helpers/qemu-bounded.sh` around every
  `script/swish-check --arch <arch>`, one QEMU at a time, 10 runs per architecture plus warm-ups,
  `pgrep -l qemu` clean after every batch.

Two lane hazards found while setting up, for the next lane:

- **`CARGO_TARGET_DIR` silently drops the file server from the archive.** `redoxfs_server_build`
  honors the variable but `redoxfs_server_elf` reads `redoxfs_server/target/<triple>/release/...`
  (xtask/src/disk.rs:104), so the packer cannot find the server and the boot runs with no
  filesystem. The archive shrinks 101 programs to 99, the progenitor prints neither
  directory-grant sentence, and the gate fails on the missing sentence rather than naming the
  cause. Symlink `redoxfs_server/target` at the same directory `CARGO_TARGET_DIR` names.
- **A target dir shared across differently-based worktrees cross-compiles.** Stale rlib metadata
  from a newer `main` made this worktree's `fixtures` fail E0425 and `components` fail E0004
  against crates this tree defines correctly. A worktree-local target dir fixed both. The
  measurement below ran entirely from a clean local target.

## Counts, with denominators

- 5380 CI runs in the window: 4278 green, 447 failed at latest attempt, 653 cancelled. Events:
  3399 pull_request, 1170 merge_group, 639 push, 172 workflow_dispatch.
- 158 runs failed `build + test (host + QEMU)`. Of those, the gate's step
  (`script/ci-build test swish-check swish-check-graphical boot-check` (the graphical row was folded into swish-check on 2026-10-03), .github/workflows/ci.yml:403)
  failed in 84, and its pre-rename `shell-check` spelling in 54 more (renamed 2026-09-23). Nineteen
  failed an older generic step name and are unclassified: no log budget was spent on them. With 16
  rerun-hidden failures found in prior attempts, 154 runs are known to have failed the gate's step
  in some attempt, and the 15 logs below cover 15 of those 154.
- 27 runs were rerun at least once. Eight of those had the gate's step fail in the prior attempt
  and stand green now, including both phenomenon runs. Rerun-green is where flakes hide: run-level
  counts undercount them by construction.
- The 84 swish-named failures concentrate: 49 on 2026-09-26, 25 on 2026-09-27 (an incident block;
  sampled logs show kernel test panics, `sched.rs:2572`, `sched.rs:2636`, `virtio.rs:740`, not
  flakes), 5 on 2026-09-29, 3 on 2026-09-25, 2 on 2026-09-28.
- Signatures in the 15 sampled logs: 7 real breakage (kernel test panics or in-flux code,
  including one `fs_service.rs:2313` panic) and 4 one-cluster package-source truncation ("the
  package source did not send a whole package", 2026-09-29 22:00, correlating with the
  `614-every-version-live` work). The rest: 2 echo-wait false reds, 1 progenitor out-of-memory,
  1 std-program illegible-output failure.

So the two phenomenon signatures are real, repeat across days, and are a minority of red runs at
this step: most reds are real breakage arriving through the same step. Both signatures are kin to
the load-sensitive family
([the register](load-sensitive-assertions.md)), with a difference worth naming: the host being
slow is only the window-opener here, and each failure needs its own two-writer race or reap race
to close inside it.

## Each signature, mechanically

**The echo wait (signature of #1442).** Each typed line is waited for as an exact string,
`wait_after(at, "{echoed}\n", 30s)` (in `swish_check::swish_check_boot`; 30s is
`SWISH_CHECK_LINE_SECS`, boot 120s `SWISH_CHECK_BOOT_SECS`, x86_64 90s `SWISH_CHECK_X86_LINE_SECS`).
The failure is usually not slowness. Run 36622549563's transcript shows the kernel's
progenitor-stack gauge spliced inside the echo itself:

```
$ packages/greeting/0  progenito.1.0/r stack: 27400 of 49152 bytes at peak, 21752 spare
greeting
```

The command ran (its output follows); the exact string can never match because two processes write
one UART with no arbiter (script/swish-check's first BUGS entry). `degauge`
(`swish_check::gauge::degauge`) was built for this after #1371 and #1420, but it strips a gauge it
can find whole; when the splice lands inside both strings at once, neither is recoverable. The same
signature hit the riscv64 reboot leg on 2026-09-28 (run 36362665042, "never echoed
`packages/greeting/0.1.0/greeting`" beside an uptime line). The gate's own constant doc already
records this class (`SWISH_CHECK_BOOT_SECS`'s doc) and names the remedy: rerun the leg before
reading the transcript.

**The echo splice's base rate, measured 2026-10-03 (UTC) by lane 629.** 3 of 29 merge-group
swish-check jobs that day failed with the aarch64 gauge-versus-echo splice, about one in ten. Every
one landed on the echo of `package install downloads/uptime.nifepkg`, right after the tampered
install: runs 37086229708, 37093381682 and 37095806100. That line is where the progenitor stack
reaches a new peak (the gauge prints `after "package install downloads/uptime.nifepkg"` on every
aarch64 leg), so the gauge and the next echo meet there on every run and splice when the timing
lines up.

**Fixed by milestone 342 (the kernel and the `console` server drive one UART from two address
spaces), 2026-10-03.** The kernel no longer writes the UART once the log service has attached: its
lines go through the service to the console, which writes them only at the start of a terminal
line. The proof is `script/swish-check --flood`, which makes the kernel print a line every
100 ms over the whole session. With the service attached, every flood line arrived whole and the
gate stayed green: 254 lines on aarch64, 263 on riscv64 and 180 on x86_64 (one a second there), none
spliced. The detached control
(`--flood-detached`, the kernel printing for itself as before 342) spliced 83 of 466 on aarch64
and 11 of 323 on riscv64, and failed the gate both times. The aarch64 run failed at
`uuid 2> ent.txt`, and the riscv64 run on this note's own signature (*the prompt never echoed
`caps wc doc/kernel/ipc-naming.md`*). The two signatures below this one are not this
fix's.

**A recurrence was reported, and it was false (2026-10-04).** The week 2026W40 count put the echo
splice at 6 of 18 swish-check failures and 4 of 6 merge-group evictions, which read as 342 not
holding. Every one of the six ran before #1498 merged (2026-10-03T09:52:00Z) on a tree without it.
Since then, 118 merge-group swish-check jobs ran with no echo failure; at the old one-in-ten rate
that is about four in a million. The fix stands. Rows, ancestry checks and the riscv64 transcripts
are in [2026W40](swish-check-flake-2026w40.md).

**The progenitor OOM (signature of #1444).** The shell prints the sentence when the progenitor
answers with the `SPAWN_FAILED` sentinel (components/src/swish.rs:3053; the sentinel is
`u64::MAX`, crates/grant_plan/src/spawnproto.rs:633). A `std` program needs one contiguous
384-page region (`STD_REGION_PAGES`, crates/grant_plan/src/lib.rs:1713) carved from a job pool of
exactly six native regions plus one std region (`system_initializer::JOBS_BUDGET_PAGES`).
`split_job` waits for an in-flight reap by yielding 1024 times, and its own doc calls that
"kept on effort, and a foot gun", recording the same CI failure on 2026-09-27 (its doc in
`crates/system_initializer` at `402e122d3`). `7fdd76557` later removed `split_job`, when the pool
began waiting on the undertaker. Reclamation is asynchronous (`components/src/job_undertaker.rs`),
and its BUGS note a mode where a parked fault-report SEND wedges the collecting loop entirely
(job_undertaker.rs:38-51). QEMU memory is not implicated: both runners give 256 MiB
(helpers/qemu-runner-aarch64.sh:537, helpers/qemu-runner-riscv64.sh:273), and the constraint is the
progenitor's own bounded pool. Whether the 2026-09-29 instance was the yield budget or
fragmentation of the pool is not determined; the in-tree precedent says yield budget.
Corrected 2026-10-03 UTC: it was the pool. A region returned out of order stayed a hole for the
rest of the boot, and the pool (672 pages) is never destroyed. Both pipelines strand a region, since
a producer ends first, so `free` read 576 KiB used where it alone holds 192, on every leg of every
run. A job carved before the reaper reached its predecessor stranded one more, and on CI's riscv64
runner that added up until `std_exerciser`'s 384 pages no longer fit (runs 37094606658 and
37110045386). A temporary kernel trace of the pool's holes showed the two pipeline holes on every
local run. Holding the reaper back on one job in three reproduced the CI failure on the old kernel
and passed on the fixed one, with `free` back at 192. The fix is in `RegionTable::return_to_parent`,
and swish-check now fails a run whose `free` reads above two regions.

## Local reproduction: zero

Ten `--arch aarch64` and ten `--arch riscv64` runs (plus warm-ups), each a full leg with two boots,
from a clean worktree-local target: 20/20 green, no failure of either signature. Zero is a result:
on this fast, unloaded host the races do not close. CI's runner is measured 1.5x to 1.8x slower than
this machine on these legs (`SWISH_CHECK_X86_LINE_SECS`'s doc), which widens both windows: the
gauge-versus-echo interleave and the reap-versus-yield race.

## What the data supports, and refuses

Supports: both phenomenon failures are timing races the tree already knows about, not regressions
in the PRs that carried them; each has an in-tree record predating this evening (#1371/#1420 for
the interleave, 2026-09-27 for the OOM). The step is red for real reasons far more often than for
these two signatures; the flake is a minority mode that reruns cure.

Refuses: a per-run flake probability. Signatures need logs, the log budget was 15 of 154
gate-step-failing runs, and the sample was targeted at the phenomenon and the clusters, not drawn
at random. It also refuses "slow echo" as the echo failure's cause: the waited-for string never
arrives because it was destroyed, 30 seconds is not too few, and #1442's command had already
answered.

## Remediation candidates, priced; none enacted

- Raise the echo wait (30s to 60s). Fixes a mode absent from the sample (genuine slowness);
  the observed echo reds never see the string at all. Cost: a real hang reports twice as late.
  Not recommended on this data.
- Retry the failing leg once inside the gate. Mechanizes what the constant doc already tells a
  person to do (`SWISH_CHECK_BOOT_SECS`'s doc). Cures both signatures as observed (both rerun
  green). Cost: one extra leg of CI time only on red, and a deterministic interleave defect would
  surface as intermittent rather than red, trading loudness for throughput.
- Raise QEMU `-m`. Buys nothing: both signatures are bounded by the progenitor's internal pool
  and by a two-writer race, not by machine RAM. Not priced further.
- **The root fixes, both already tracked.** The interleave class dies when the kernel stops writing
  the UART once userspace owns it (DECISIONS §175 (where the kernel's own output goes once
  userspace owns the console), ruled 2026-09-27, unbuilt; the degauge doc points there). The OOM
  race dies when the undertaker tells the progenitor a job's memory is back
  (`design/roadmap/0685-a-job-is-finished-when-its-memory-is-back.md`). Both are architect
  territory, not this lane's.
- Runner variance as the finding. Both confirmed signatures are races whose windows scale with
  runner speed; the local host cannot reproduce them in 20 legs and CI's slower runners can in one
  evening. If the flake needs one sentence: it is the gate meeting CI's machines, not a defect in
  what the gate measures.

## The truncation cluster, read out of the code (2026-09-30)

**Corrected 2026-10-02 UTC: this section's conclusion is wrong.** The cluster was milestone 614's
defect, not a host-side race. Its 0.2.0 recipe put a second `greeting` stem in the image's
catalog, the lookup took the first line (0.2.0, by recipe filename order), and the source
served only 0.1.0, so the "fast refusal" was a 404. The two merge-queue runs below batched #1443
itself. The code reading of the fetch path below still holds; the slirp hypothesis does not. The
fix and its record are in milestone 614's roadmap block.

The count above left the four truncation runs with no mechanism. A static lane (no QEMU, reading
the tree and the CI logs) followed the sentence into the fetch path and answered it. The lane ran
on the branch of milestone 614 (two installed versions of one program, each runnable, and a caller
granted the one it needs).

**Where the sentence comes from.** `swish` renders `ActivationStatus::FetchFailed` as that one line
(`write_activation`, crates/swish/src/lib.rs). Fourteen sites in the progenitor produce the status.
They cover the socket page's carve, retype and map; the open and the connect; the request
write; the receive length; the head parser; the status check; the length bound; and the staging
carve and map (`fetch`, `receive_body` and `stage_pages`, crates/system_initializer/src/lib.rs).
One sentence answers for all fourteen, so a transcript cannot say which fired.

**No partial-read defect exists in that path.** `receive_body` loops until `http_response` says the
body is whole, folding every read through `Response::feed`, a byte-stream reader that holds only the
head. A host test feeds one real response at every split length, and the fuzzer asserts on 46 million
inputs that status, body and verdict never depend on where the reads fell
(fuzz/fuzz_targets/http_response_feed.rs). No read assumes it fills anything: `OPERATION_RECEIVE` returns
whatever smoltcp had buffered, at most the 2,048-byte socket buffer, and any length from one byte to
`DATA_MAX` is accepted. The one length prefix, `Content-Length`, is parsed from the accumulated head
only after the blank line, so a split head cannot misread it. The declaration bounds the staging
carve, the copy, and completion itself: `Ok` only at exactly that many body bytes, more refused,
zero or oversized refused before anything is carved.

The timing bounds are real and were not hit. Each `OPERATION_RECEIVE` and the connect wait inside
`service_until`'s 15 s bound (components/src/net_stack.rs), under the gate's 30 s per line. Both
transcripts that kept their timings rule a timeout out more cheaply: the failing line is absent from
the gate's own slowest-three (floors 1.0 s and 0.6 s) and the legs ran 26.3 s and 25.8 s in total.
The refusal answered fast.

What the six runs pin down. Runs 36635825171, 36635855388, 36637445269, 36637505183, 36662722651
and 36667068219, 2026-09-29T21:49 through 2026-09-30T04:02, every one at `package install greeting`,
always the second fetch of the boot. The first fetch, the lying `uptime`, had finished a whole
exchange seconds earlier: its catalog refusal needs the whole body and its digest. Two of the six
were the merge queue of pr-1452 and pr-1443 together, so no pull request's diff is the trigger. The
run before the cluster, 21:41, was green; the next failure on the branch, 15:56 on 2026-09-30, is a
different signature (an inbound-prober leg).

One premise corrected: the failure rolled no generation. A refused fetch writes nothing, so
generation 1 stays live, and the off-by-one cascade arrives later, when the 0.2.0 disk install makes
generation 2 where the transcript wants 3.

Why the second fetch, from the code. Everything the guest feeds the second fetch is identical to
the first. The ephemeral port rotates, so the second connection is a fresh 4-tuple
(`socket_contract::ephemeral`'s own reason: reusing a port whose slirp flow had not cleared stalls). The socket
page's region and the staging region are destroyed in reverse carve order at request end, so the
image pool returns to empty between commands. The destroy is a synchronous kernel call that revokes
the shared page out of `net_stack` before it returns (kernel/src/memory_region.rs), so the next
attach maps a clean page.

What remains is the host side: slirp, forking `helpers/package-http-peer` per connection while the
first connection's teardown is still settling. The tree has met this peer family twice, both
recorded: the stale 4-tuple stall above, and the echo peer that blocks its next connection behind a
half-closed predecessor (the reason `OPERATION_CLOSE` drains the FIN handshake). A fast refusal on a
back-to-back second connection is the same family's third face, at a speed only CI's runners have
shown.

Nothing in this tree distinguishes the fourteen sites today. One word per refusal in the answer, or
a progenitor-side trace the shell's vocabulary leaves alone, would localize the next occurrence
from its transcript. Priced, not enacted.

One asymmetry found on the way, recorded here because the fetch path has no BUGS of its own and
notes/packages.md is at its prose cap. `fetch` carves its socket page and staging with one-shot
`memory_region_split` calls. The disk-install path yield-retries the same pool through `split_job`,
because CI once made it transiently short (2026-09-27, that function's own doc). The discipline is
clean between commands, so this cannot explain the cluster; it is the one place the fetch path
lacks the retry its sibling learned to need.

## The noteless signature (2026-10-04, UTC)

`packages/noteless/0.1.0/noteless` answered "", evicting #1573. It answered; the gate misread.
Both runs (37167978481, 37153714653) were the first launch after the reboot, not after `package
rollback`. A gauge waited in the console's queue mid-line, the log service's 250 ms flush fell
between the echo of the last typed character and the echo of Enter, and the console redrew the line
beneath the gauge (milestone 342's design). Filtered, `$ line` appeared twice and the first copy
read as silent.

Fixed in the harness by `without_redraws`, milestone 748 (the noteless launch that prints nothing).
A 400 ms pause before Enter (`NIFE_SWISH_ENTER_PAUSE_MS`) failed 4 of 4 boots without it and 0 of 3
with it; rows in [the 2026W40 file](swish-check-flake-2026w40.md).
