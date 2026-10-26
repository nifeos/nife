# The `uefi-test` exit-1-after-green rate, and the exit-path protocol

*(Written by the lane executing milestone 516 (`uefi-test` can exit 1 after its own suite has
passed, and nobody has taken a rate), 2026-10-16 (03:23 to 03:47 UTC). Name provisional: the lane
mints it, the name-ratification queue owns it. The block this serves is
[design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md](../design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md);
the measurement recipe it followed is notes/load-sensitive-assertions.md's.)*

On 2026-09-19 a stranger's `cargo xtask uefi-test` printed `test result: ok. 215 passed, 71
skipped` and then failed with `qemu exited Some(1), not 3`. Everything else on screen looked like
a fault, so the only theory available was a wrong one (QEMU version, firmware). This page records
the rate the block asked for, the analysis that says what a `1` after a green summary can and
cannot be, and the failure message `xtask` now prints.

## The measurement

Twenty separate `cargo xtask uefi-test` invocations on patagonia, 8 cores, Homebrew QEMU
11.1.2, sequential, one warm-up run first (not counted; it carries the cold cargo caches). Each
row records the 1/5/15-minute load average before and after the run, the wall-clock seconds, the
exit status, and how many `qemu-system` processes were visible afterwards. Window: 03:23 to
03:47 UTC, 2026-10-16. Runs took 66 to 81 s; every run printed both suites' green summaries
(kernel: `133 passed, 13 skipped`; system_tests: `277 passed, 101 skipped`) and exited 0.

**The machine was never idle during this window, and that is a recorded condition, not a
cleanup.** Milestone 412's lane was measuring `cargo xtask uefi-boot` rates on this machine
concurrently, and lanes cannot see each other. The block asked for idle and loaded batches; this
window produced a load-gradient batch instead. The idle batch was deliberately not forced:
generating load while a sibling lane measures on the same machine would have poisoned both rates
(the load recipe's own warning about running it while other lanes gate here). The two lanes'
rates reconcile through the load columns below.

| run | start (UTC) | load before | rc | s | load after | other qemus |
|---|---|---|---|---|---|---|
| 00 | 03:23:37 (warm-up, not counted) | 11.75 9.51 8.12 | 0 | 81 | 5.44 7.99 7.66 | 0 |
| 01 | 03:24:58 | 5.44 7.99 7.66 | 0 | 68 | 4.28 7.11 7.35 | 0 |
| 02 | 03:26:06 | 4.28 7.11 7.35 | 0 | 70 | 4.67 6.57 7.12 | 0 |
| 03 | 03:27:16 | 4.67 6.57 7.12 | 0 | 66 | 3.35 5.79 6.79 | 0 |
| 04 | 03:28:22 | 3.35 5.79 6.79 | 0 | 67 | 3.07 5.24 6.50 | 0 |
| 05 | 03:29:29 | 3.07 5.24 6.50 | 0 | 70 | 3.50 4.97 6.30 | 0 |
| 06 | 03:30:39 | 3.50 4.97 6.30 | 0 | 67 | 2.71 4.44 6.00 | 0 |
| 07 | 03:31:46 | 2.71 4.44 6.00 | 0 | 67 | 3.35 4.28 5.82 | 0 |
| 08 | 03:32:53 | 3.35 4.28 5.82 | 0 | 67 | 3.96 4.36 5.73 | 0 |
| 09 | 03:34:00 | 3.96 4.36 5.73 | 0 | 69 | 4.23 4.36 5.63 | 0 |
| 10 | 03:35:09 | 4.23 4.36 5.63 | 0 | 68 | 4.15 4.32 5.51 | 0 |
| 11 | 03:36:17 | 4.15 4.32 5.51 | 0 | 67 | 3.71 4.15 5.36 | 0 |
| 12 | 03:37:24 | 3.71 4.15 5.36 | 0 | 67 | 4.18 4.15 5.26 | 0 |
| 13 | 03:38:31 | 4.18 4.15 5.26 | 0 | 72 | 5.62 4.72 5.39 | 0 |
| 14 | 03:39:44 | 5.62 4.72 5.39 | 0 | 67 | 5.63 4.94 5.42 | 0 |
| 15 | 03:40:51 | 5.63 4.94 5.42 | 0 | 70 | 4.75 4.90 5.37 | 0 |
| 16 | 03:42:01 | 4.75 4.90 5.37 | 0 | 70 | 4.63 4.90 5.34 | 0 |
| 17 | 03:43:11 | 4.63 4.90 5.34 | 0 | 70 | 4.92 4.98 5.33 | 0 |
| 18 | 03:44:21 | 4.92 4.98 5.33 | 0 | 68 | 4.31 4.75 5.22 | 0 |
| 19 | 03:45:30 | 4.31 4.75 5.22 | 0 | 67 | 3.69 4.53 5.10 | 0 |
| 20 | 03:46:37 | 3.69 4.53 5.10 | 0 | 68 | 5.14 4.82 5.16 | 0 |

Load values are the 1/5/15-minute averages `sysctl -n vm.loadavg` reports. Every green run also
carried the four VT-d fault lines (the masked fault-event interrupt and the translation failure,
twice per boot pair), which is the point the escape test's expectation makes below.

## The verdict

Zero failures in twenty runs; the defect did not reproduce at this sample size. By the rule of
three, zero in twenty bounds the rate below 1 in 7 at 95% confidence, which is consistent with
the one sighting on 2026-09-19 and proves nothing tighter. The rate's honest reading is
therefore: rarer than one gate in seven, cause not reproduced, and the next sighting now meets a
message that names its exit path rather than one that invites a QEMU-version theory. The load
spread over the window was 2.71 to 5.63 on the 1-minute average (8 cores), and the 15-minute
average declined from 8.12 to 5.16. The concurrency source was milestone 412 (`script/test`'s
UEFI leg asserts two cores online) measuring `uefi-boot` rates beside this lane's own boots. Run
duration moved only 66 to 81 s across that spread.

## What exits 1, and what cannot

`isa-debug-exit` is the whole protocol, and its arithmetic decides everything downstream
(`kernel/src/arch/x86_64/semihosting.rs`): the guest writes `code >> 1` to the device's port and
QEMU reports `(written << 1) | 1`. Every status the device can produce is odd. The guest's two
codes are 3 (success, writes 1) and 1 (failure, writes 0). **A reported 1 is therefore the
guest's failure write and also QEMU's own error exit**, and the number alone cannot say which
path it was. A reported 0 or any even status is never the device: QEMU was ended from the host
side. The `NIFE_UEFI_TIMEOUT` bound lands as a 0, because QEMU exits 0 on the SIGTERM that ends
it (measured on Homebrew QEMU 11.1.2, 2026-10-15).

Which guest code wrote a failure, if it was the guest: every in-tree failure writer prints a
marker line first, so the transcript attributes it.

| writer | marker in the transcript | where |
|---|---|---|
| the panic arm | `[PANIC]` | `kernel/src/panic.rs` |
| the frame ledger's two gates | `FRAME LEDGER:` | `kernel/src/testing.rs` `report_page_frame_ledger` |
| the no-progress watchdog | `WATCHDOG: no progress for ~60 s` | `kernel/src/testing.rs` `watchdog_tick` |
| the per-test budget watchdog | `WATCHDOG: test exceeded its budget` | `kernel/src/testing.rs` `check_test_ceiling` |
| a filter that selects nothing | `no test matches the filter` | `kernel/src/testing.rs` `test_main` |

The ordering in `test_main` is what makes a green summary evidence: the ledger reports, the peak
tables and the timing table all print **before** the summary line, and the summary's next
statement is the success write. Between `test result: ok.` and `semihosting::exit(EXIT_SUCCESS)`
nothing in-tree runs but the panic arm. So **a 1 after a green summary is either a panic on the
way out (the transcript has a `[PANIC]`) or QEMU's own error exit (its reason is on stderr)**.
It cannot be a test failure or a ledger gate; both exit before any summary exists.

That ordering is also why the VT-d lines were never the cause. The escape test's masked
fault-event interrupt and its translation failure are what a passing confinement test looks like
from outside; milestone 215 (a PCI function's interrupt reaches nothing on x86_64) named the
family. They appear in green runs and red runs alike: this window's twenty green runs each
carried four of them. They were the only fault-looking lines on the stranger's screen.

## What the failure message now says

`xtask/src/uefi.rs`'s `exit_verdict_lines` (unit tests pin it in `exit_verdict_tests`) classifies
the status against the protocol above and prints, at every failure:

- first, that the run's `qemu-system-x86_64` VT-d lines (`Interrupt Mask set, irq is not
  generated`, `vtd_iommu_translate: detected translation failure`) are the escape test's own
  expected faults (milestone 215), not a cause;
- then the reading. A 1 after a green summary points past the verdict: the panic arm, or QEMU's
  own error exit on stderr. A 1 with no summary is the failure write before one, or QEMU failing
  under the guest. A 0 is never the device, and carries the elapsed seconds that separate a
  180 s bound from an early stop. Other odd statuses decode the value the guest wrote; even
  statuses say QEMU was ended from the host side. A watchdog kill defers to the watchdog's own
  line, which names the stage that overran.

The host-load line notes/load-sensitive-assertions.md describes prints beside a red leg
independently, so a red run says both what the host was doing and which exit path produced the
status.

## BUGS

- The rate is one host, one QEMU (Homebrew 11.1.2), and a window that never reached idle
  (1-minute load 2.71 or above throughout). An idle-machine batch is still unmeasured.
- The classifier's readings are unit-tested, not nature-tested: no natural failure happened in
  the window, so no path has yet been exercised against a real red boot. The first real sighting
  is the test.
- A 1 after a green summary with no `[PANIC]` in the transcript names QEMU's own error exit and
  points at its stderr, but cannot name the cause when QEMU dies without one. The line says
  where to look, not always what it was.

## See also

- [design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md](../design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md):
  the block this page serves
- [notes/load-sensitive-assertions.md](load-sensitive-assertions.md): the measurement recipe, the
  host-load line, and the register of the assertion family
- [design/roadmap/0215-x86-64-pci-interrupt-routing.md](../design/roadmap/0215-x86-64-pci-interrupt-routing.md):
  why the VT-d lines are the escape test's own expected faults
