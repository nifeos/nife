---
status: BUILT
raised: 2026-09-26
built: 2026-10-19
milestone_dependencies: 601
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 608. Kernel tests give back their rendezvous points

*(Number and title provisional: the integrator confirms both at merge.)* Approved by calef
2026-09-26 as urgent. It comes from the ledger milestone 601 (the region table prints its peak)
built at `sched::PEAK_RENDEZVOUS`. `uuid_tests` panicked in #1344: "out of rendezvous points: 512
live at once", after only two tests were added. Every lane adding a test is at the same risk.

## Why

`sched::MAX_RENDEZVOUS` is 512. A rendezvous made with `create_rendezvous()` sits on one of the
kernel's own chunks and is never freed, so the live count only climbs across a suite run. One made
with `create_rendezvous_from(region)` goes when its region is reclaimed. A test that wires a
service and walks away from it spends the registry for the rest of the boot. The aim was to bring
aarch64's `system_tests` peak under 400 of 512 by giving those endpoints back.

## What is built

The pattern, everywhere it is applied: the endpoints a test or a test helper wires come out of a
small `memory_region::create` region, and the region goes back when the test is done with them. A
helper whose programs never exit on their own (a caretaker parked in `RECEIVE`, the NTP test
server, the clock service) returns a `user::holding::Holding` with the threads and the region in
it, and the caller releases it with `release_or_fail`. The holding's kill is what ends those
programs; reclaiming the region alone would only make the NTP test server spin on a failed
`RECEIVE`.

- The kernel's own IPC tests. Eleven tests in `kernel/src/sched.rs`'s `mod tests` retype
  their scratch endpoint from a one- or two-page region and reclaim it once their threads exit.
- The oldest userspace tests. Seven tests in `kernel/src/user/tests.rs` take their `report`
  from a region they reclaim. `date_tests`' `spawn_date` and `spawn_date_with_diagnostics` return
  their region beside their endpoints.
- NTP. `ntp_service::start_server`, `start_client` and `start_witness` retype from a region of
  `ntp_service::EXCHANGE_PAGES` and put their thread in the caller's holding. `ntp_tests` runs
  each exchange in its own holding and builds its clock with `clock_service::start_in`.
- The file service's granted clients. `start_granted_dir`, `start_granted` and
  `start_granted_set` retype the caretaker's two endpoints and the client's report from one region
  of `GRANT_ENDPOINT_PAGES` and return `(report, Holding)`. `start_granted_dir` is built on
  `narrow_dir_held`, which the provisioning test now uses too.
  `spawn_fs_client` takes its report from the caller. The shared-frame witness returns a holding
  for its two clients and three endpoints.
- The file sink and source (`start_file_sink`, `start_file_source`) retype from the caller's
  region, `FILE_SINK_PAGES` each. Both exit after `DONE`, so no holding is needed.

Name: provisional for `ntp_service::EXCHANGE_PAGES`, `fs_service::GRANT_ENDPOINT_PAGES` and
`fs_service::FILE_SINK_PAGES`, each named for the pages it sizes, the way
`clock_service::REGION_PAGES` is.

Measured on the `system_tests` image, `rendezvous:` closing line, peak live of 512 (kernel-chunk
count in parens). Before is the merge queue run 39302780056 on `main` at `0d6820c5`; after is
local, aarch64 under HVF and the other two under TCG, on this branch:

| architecture | before | after |
|---|---|---|
| aarch64 | 403 (322) | 269 (188) |
| riscv64 | 391 (310) | 260 (179) |
| `x86_64` | 280 (224) | 189 (133) |

All three are under 400 with more than 240 spare. The kernel's own unit-test image is 42 (40) on
every architecture and is not part of this milestone's target.

Left on kernel chunks deliberately, because they live for the boot or for a test that never
returns them by design:

- The FS server, block server and their IRQ endpoint (`fs_service::wire_servers`), wired once.
- `fs_service::start` and the crash pair (`start_crash`, `recover_crash`): the `fs_read` bench and
  the two-phase crash test, one report each.
- `narrow_dir`'s caretaker for `redirection_tests`' one cached shell, and `start_granted_two_dirs`'
  caretaker pair, cached per boot (its per-call report is one endpoint).
- `user::tests`' `a_user_client_moves_data_through_shared_memory` (its client spins forever) and
  `reclaim_frees_a_started_then_exited_childs_regions` (a pinned region its frame accounting
  depends on).

## What is left

Nothing for this milestone's aim. The largest remaining holders on aarch64, all outside the file,
NTP and sink helpers this block covers: `compositor_tests` (31) and `display_tests` (15) through
`compositor_service::start`, `disk_tests` (16) through `disk_service`, `credential_tests` (9),
`block_driver_tests` (8), `clock_tests` (8), and `login_tests` with `login_user_schedule_tests`
(10). Milestone 670 (test helpers give their clients a region to retype their rendezvous from)
holds them.

## Follow-on

- **Milestone 670.** Test helpers give their clients a region to retype their rendezvous from.
  The compositor and display helpers are most of what is left, and
  `display_service::start_screen_terminal` already shows the shape.
- **Recorded.** Whether the login tests' remaining ten are the login service's own boot-lived
  endpoints or per-test residue is unconfirmed. Milestone 670 checks before assuming either.
- **Recorded.** One aarch64 run on this branch failed
  `login_user_schedule_tests::suspending_a_user_ends_their_schedule_and_refuses_them_until_resumed`
  with alice `DENIED`. Nothing this milestone changed reaches the login path, the same test passed
  on the next two aarch64 runs and on riscv64, and its cause is not yet known.

## Index row

The kernel's IPC tests and the file, NTP and sink test helpers give their rendezvous back when the
test ends, taking the aarch64 `system_tests` peak from 403 to 269 of 512. Compositor, display and
disk holders remain under milestone 670.
