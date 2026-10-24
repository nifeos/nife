# Where the kernel suite's time goes

Read from the `time-record-*` artifacts of 30 green merge-group runs of `ci.yml`, 2026-10-16 16:43 to 2026-10-17 16:28 UTC. Milestone 807 (the kernel suite reports what each test cost, and the riscv64 suite's time is explained) wrote the record. Every figure is guest time under QEMU TCG unless it says phase, which is host time.

Full medians are in two files beside this page: `kernel-suite-time/per-test-medians.tsv` (654 tests, the median over the runs that recorded each) and `kernel-suite-time/per-module-medians.tsv` (110 `system_tests` modules; each run's module sum, then the median).

## The runs

39049533667, 39047563611, 39046882948, 39044362921, 39044296578, 39042260802, 39041012695, 39031958289, 39026018093, 39024963403, 39022703071, 39008645871, 39004167669, 39003594045, 39002671195, 39002638565, 39000567580, 38999427449, 38997281221, 38997118765, 38994400601, 38994366541, 38993833063, 38976866787, 38976812950, 38975708023, 38973117762, 38970563790, 38969976716, 38934238212.

These are the 30 newest successful runs that kept all four artifacts (`time-record-test-aarch64-riscv64`, `-test-x86_64`, `-cpu-matrix-1`, `-cpu-matrix-2`). The artifacts start on 2026-10-15, so no older run can be read.

## How long each suite takes

Median seconds of host time per phase, from the `test` job's legs. The range over the 30 runs is in the record.

| leg | image | build | boot | suite |
|---|---|---|---|---|
| aarch64 | `system_tests` | 6.8 | 0.1 | 149.8 |
| riscv64 | `system_tests` | 6.7 | 0.2 | 184.1 |
| x86_64 | `system_tests` | 7.0 | 0.7 | 129.5 |
| aarch64 | kernel unit tests | 2.7 | 0.1 | 7.5 |
| riscv64 | kernel unit tests | 3.2 | 0.2 | 8.9 |
| x86_64 | kernel unit tests | 3.7 | 0.7 | 9.4 |

The five `cpu-matrix` models run the same riscv64 suite: 183.8 to 184.1 s for four of them and 193.1 s for `rva23s64`. The suite phase is 184 s of a leg that costs about 200 s, so the suite is nearly all of it.

## Where the riscv64 system tests spend it

Modules by riscv64 median, in seconds. The ratio is riscv64 over aarch64 where both ran the module.

| module | aarch64 | riscv64 | x86_64 | ratio |
|---|---|---|---|---|
| `compositor_tests` | 21.5 | 31.7 | 34.6 | 1.47 |
| `riscv_virtio_tests` | none | 30.3 | none | none |
| `package_index_tests` | 19.5 | 19.9 | 18.9 | 1.02 |
| `display_tests` | 9.2 | 14.9 | 14.8 | 1.63 |
| `login_user_schedule_tests` | 8.4 | 12.8 | 0.0 | 1.53 |
| `net_confinement_tests` | 10.9 | 11.1 | 11.0 | 1.02 |
| `login_tests` | 3.9 | 4.8 | 0.0 | 1.24 |
| all 110 modules | 149.6 | 185.0 | 131.2 | 1.24 |

The riscv64 suite runs 391 `system_tests`, aarch64 394 and x86_64 378. Over the whole suite riscv64 is 1.24 times aarch64.

Two kinds of test sit at a ratio near 1.0. `package_index_tests` and `net_confinement_tests` wait on wall time or the network, so the emulated CPU does not matter. The modules with a ratio of 1.5 or more (`compositor_tests`, `display_tests`, `login_user_schedule_tests`) compute under TCG, where riscv64 is the slower emulation.

The ten slowest tests on riscv64, median seconds:

| s | test |
|---|---|
| 19.9 | `package_index_tests::a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest` |
| 15.2 | `riscv_virtio_tests::an_accept_nobody_answers_ends_at_the_stacks_bound` |
| 10.1 | `display_tests::a_bitmap_font_and_a_vt_engine_put_readable_text_on_the_scanout` |
| 7.8 | `compositor_tests::three_clients_compose_into_one_scanout_and_the_host_sees_it` |
| 6.3 | `login_user_schedule_tests::suspending_a_user_ends_their_schedule_and_refuses_them_until_resumed` |
| 5.3 | `net_confinement_tests::a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic` |
| 5.3 | `riscv_virtio_tests::a_host_process_connects_to_the_guest_and_is_answered` |
| 5.2 | `net_confinement_tests::a_socket_holder_cannot_receive_on_its_socket_capability` |
| 4.7 | `confined_fuzzer_tests::a_confined_fuzzer_reaches_nothing_it_was_not_granted` |
| 3.9 | `nth_retype_tests::login_gives_back_everything_when_any_retype_of_a_login_fails` |

## What the growth since 2026-09-20 consists of

The comparison points are two scrapes recorded in milestone 807's block (the riscv64 section of `test` went from 358 to 457 tests and from about 47 to 110 s between 2026-09-20 and 2026-10-06) and this page's record. They are different instruments. The block measured two scrapes of one suite 17% apart, so treat any difference under that as noise.

Between the 2026-10-06 scrape (320 `system_tests`, 129.0 s) and these runs (391 tests, 185.0 s), the suite gained 71 tests and about 56 s. What the data attributes:

- **`package_index_tests`, about 20 s.** One test, added on 2026-10-09 (`git log` for the file). It did not exist at the scraped commit. It takes 19 to 20 s on all three architectures, so it is wall time, not emulation. It is the largest single cause and it is not riscv64-specific.
- **`riscv_virtio_tests`, about 15 s.** The module went from 14.8 s (31 tests at the scraped commit) to 30.3 s (32 tests). One added test cannot account for a doubling, and the record does not say which test grew. Unexplained.
- **`compositor_tests`, about 7 s.** 24.5 s over 11 tests became 31.7 s over 12.
- `login_user_schedule_tests`, 12.8 s. Commit 642f021d4 (2026-10-12) moved login's schedule tests out of `login_tests` into this module. The two modules together are 17.6 s against the scrape's 18.0 s, so this is a move and not growth.
- Falls. `timetable_tests` fell from 12.2 s to 4.2 s, which is consistent with the calendar test's two five-second waits removed in PR #1779. `login_tests` fell from 18.0 s to 4.8 s because of that move.

## What this does not explain

- Nothing before 2026-10-15 was recorded. The growth from 358 to 457 tests and from 47 to 110 s that began on 2026-09-20 cannot be split by test or module from this data. Only the 10-06 to 10-17 slice above can, and only against a scrape of a different kind.
- About 22 s of the 56 s is unattributed. The named modules net to about +34 s: +20, +15 and +7 for the modules above, less the 8 s fall. The two login modules net to zero. The rest is spread across the other modules and visible in none.
- Thirty runs span 24 hours. They share runner pool conditions that a different day may not. The `rva23s64` model's median is 9 s above its siblings, though its fastest run (183.6 s) matches them, so some of its runs are slow and the record does not say why.
- The x86_64 suite varies most. Its suite phase ranged from 105 s to 139 s. The riscv64 suite ranged from 183 s to 205 s.

## How the record is taken

`xtask/src/time_record.rs` writes the record. The child's stdout passes through byte for byte as it
arrives, and each whole line is stamped. Four phases are cut from the stamps. `build` runs from the
spawn to the guest's first line, which is cargo building the image and QEMU starting. `boot` runs to
the runner's `running` line, `suite` to `test result:`, and `exit` to the child's exit. A phase whose
end marker never came is left out rather than guessed.

The record is whole when every test announced on a `test <path> ... ` line has a time line, every
time line names such a test, and their count is the `running` line's.

The clock check compares the guest's sum of per-test times with the host's span for the same tests.
On x86_64 the guest's clock is a TSC calibrated once at boot, which milestone 571 found can be wrong
by 4x, and nothing else in the tree would notice. The span holds every test body plus the overhead
between tests: the name, the frame ledger's charge, and the reports after the last one. So the sum
sits below the span and exceeds it only on a fast clock. Under TCG the guest's counter follows the
host's clock and stops when the VM stops, so it cannot honestly run ahead. The upper slack covers
the host's stamp landing a pipe read late. Half the span, the lower bound, is far below any measured
share and far above a 4x error. Its three seconds of absolute slack keep a short image (137 kernel
unit tests in about six seconds on riscv64) from failing on fixed costs. A repeated run is not
judged, because the runner scans the frame bitmap twice between runs.

The host watchdog ends a boot 60 s past the running test's guest budget. The guest's ceiling fires
on the first tick past its budget, prints the test's name and a thread dump, and exits; 60 s leaves
that exit room, with margin for a guest clock a little behind the host's. Boot and exit get the
default budget plus the grace, 150 s, and the slowest of either in CI on 2026-10-11 was 1.8 s. The
slowest honest test in about 27,000 per-test records was 20.5 s, measured over three merge-group CI
runs on 2026-10-11 and eight lane runs on patagonia, a shared and busy host. That test was
`package_index_tests::a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest`
on aarch64, and no test reached 30 s. CI's `test` job runs every leg in a median of about 17 minutes,
against milestone 721's 20-minute budget and a 25-minute `timeout-minutes`. A hang costs a run 2.5
minutes and fails with the test's name. The hang that motivated it is in [scheduler.md](scheduler.md).
