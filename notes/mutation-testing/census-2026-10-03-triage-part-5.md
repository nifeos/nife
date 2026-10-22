# The 2026-10-03 census's survivors, part five: the last 201

Milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time),
provisional number, 2026-10-07 (UTC). Parts [one](census-2026-10-03-triage.md) to
[four](census-2026-10-03-triage-part-4.md) hold the method. The run is still
[37108924347](https://github.com/nifeos/nife/actions/runs/37108924347); its census line numbers have
moved since. Every crate below was re-run with `script/mutation -p <crate>` on the lane's worktree
(macOS, base `fc1f425aa`), one crate at a time.

## What was left, and what was already done

Batch 6 left 201 survivors in 39 crates. 114 of them already had a row in
`notes/project-metrics/mutation-triage.csv`, written by the inflow lanes (`lane/326-survivors`, `-2`
and `-3`, 2026-10-05) after batch 6 counted. They are taken as those lanes left them, and checked:
all 100 rows marked killed are caught by the re-run, and the 14 marked equivalent are still
reported. One of the 14 is reported under a new key, because `screen_console`'s `copy_wide` is now
`copy_wide_screen`.

That leaves 87 with no row. They now have 64 rows (some mutants share a key): 10 killed by new
tests, 70 equivalent and 7 recorded gaps.

## Killed (10)

- `generational_table::Table::remove` (4). The bound `top` is documented to shrink on remove, and it
  is what keeps a sweep from visiting empty slots. A walk's values cannot see a stale bound, so
  `a_hole_below_the_bound_does_not_truncate_the_walk` and `an_emptied_table_costs_a_walk_nothing`
  now read `top` itself.
- `network_time_protocol::UNIX_LIMIT` (2). The only test of the limit compared it with itself, so `-`
  as `/` or `<<` as `>>` moved the wall to 2038 or 2036 and nothing failed. It is now pinned to
  2104-02-26T09:42:24Z, and 2100-01-01 must survive the round trip.
- `credentialer::MAX_P_COST` (2). It restates `argon2`'s bound so `Cost::new` can refuse first. The
  hostile-cost sweep paired every `p` with an oversized `m_kib`, so it could not see `p`'s bound
  move. `the_restated_bounds_are_argon2s_own` holds both constants equal to the library's.
- `calendar::Writer::byte` (1). The doc promises a dropped byte at the cap, not a panic. No format
  reaches the cap, so the test writes past it directly.
- `board_console::port::dial_in_warning`'s `cfg!` guard (1). See the next section.

## The census host is not the lane's host

The weekly census runs on `ubuntu-24.04-arm`; lanes re-run on macOS. `dial_in_warning` returns
`None` before its body unless `cfg!(target_os = "macos")`, and its only test asserted the macOS
answer. Three of its mutants were therefore caught on every lane's machine and missed by every
census. The test now asserts the off-macOS answer too, which kills the deleted `!` on the guard
(checked by hand-substituting `false` for the `cfg!`, since no Linux host was used). The other two
are equivalent on Linux, where the function is `None` by construction.

Any crate with `cfg!(target_os)` branches can differ in the same way. A lane re-deriving a crate
on macOS sees the macOS suite, and the ledger says which host it used.

## Equivalent (70)

Argued here, by shape:

| shape | mutants | where |
|---|---|---|
| `\|` as `^` on disjoint bits | 14 | `byte_sink_protocol::req`, `credential_protocol::req` (2), `generational_table::name`, `graphics_protocol` `pixel` (2), `req`, `rect` (3), `network_time_protocol` `from_unix`, the low-bit randomizer, `to_bytes` (2) |
| `1 << 0` as `1 >> 0` | 3 | `abi::rights::READ`, `c_seam::checks`, `capability::Rights::READ` |
| a boundary both arms answer the same | 9 | `block_roster::capacity_of`, `byte_sink_protocol::pack`, `calendar::UtcOffset::from_hm` (2), `calendar::Writer::offset` (only called for a non-zero offset), `page_frames` `alloc`'s hint and `largest_free_run`, `coremark::list_work`'s tie-break (equal values), `board_console::BootProgress::reach` (argued in its own comment) |
| a check repeated downstream | 1 | `calendar::DateTime::parse_rfc3339_bytes`'s `i + 6`: `number()` and `expect()` bound the same bytes and give the same `Syntax` |
| truncation | 2 | `coremark::fsm_work` (each count is at most 256, so both shifts give 0 as `u16`), `network_time_protocol::from_unix` (`%` as `+` before an `as u32`) |
| off-macOS by `cfg!` | 2 | `board_console::dial_in_warning` (see above) |

Argued in earlier appendices, and still reported by this re-run (39):

- [regressions-2026-09-14-capability-to-device-tree-blob](regressions-2026-09-14-capability-to-device-tree-blob.md): `capability::note_peak` and
  `device_tree_blob::cells`.
- [regressions-2026-09-14-clock-contract-swish-filesystem-contract](regressions-2026-09-14-clock-contract-swish-filesystem-contract.md):
  `clock_protocol`'s three. `publish` is killed by `script/interleaving-check`.
- [baseline-survivors-abi-to-ipc](baseline-survivors-abi-to-ipc.md):
  `inter_process_communication::one_queue_invariant` and `page_frames::alloc_contiguous`.
- [job-mix-and-jh7110-entropy](job-mix-and-jh7110-entropy.md): `jh7110_entropy`'s six and
  `job_mix`'s three.
- [measured-boot](measured-boot.md): `measured_boot`'s five.
- [new-crate-backlog](new-crate-backlog.md): `memory_corruption_canary_gate`'s two `pause`s.
- [jh7110-clock-and-reset-non-volatile-memory-express-screen-console](jh7110-clock-and-reset-non-volatile-memory-express-screen-console.md):
  `non_volatile_memory_express`'s seven and `screen_console`'s two.
- [census-2026-09-21-triage](census-2026-09-21-triage.md): `portable_executable`'s five and
  `stick_maker::close_tag`'s two.

## Recorded gaps (7)

- `address_space_identifier::Allocator::free` (2). The range guard runs after two `debug_assert!`s
  on the same conditions, and every `cargo test` build has debug assertions, so a bad ASID panics
  before the guard is reached. The guard is release-only, and no test here builds release.
- `board_console::port` (3): `candidates` twice and `confirm_speed`, already gaps in
  [serial-console](serial-console.md). They need the host's `/dev` or a real tty.
- `job_mix::order`'s `seed | 1` (1), the recorded `BUGS` entry on `order`.
- `network_time_protocol::PROVED_NANOS` (1). A `const` inside the `cfg(kani)` module gets no module
  path in its mutant name, so the `verification::` exclusion misses it. This is the
  `machine_discovery` `x86_64.rs:314` shape.

## Survivors this re-run saw that the census did not

35 missed mutants in three swept crates are not among the 1,004 and have no triage row. They are
code merged after the census: `board_console/src/exposure.rs` (18), `jh7110_clock_and_reset`'s
`with_parent` and `Report::every_reset_released` (4), and `screen_console`'s `Aperture::checked`,
`PixelSink::is_empty`, `ScreenConsole::clear`, `scroll` and `draw` (13). The inflow check (#1582) is
their route, and the next census lists them.

## Rows for the triage CSV

The 64 keys above are appended to `notes/project-metrics/mutation-triage.csv`. So are 8 `compositor`
rows from [batch 6's staging file](census-2026-10-03-triage-batch-6-rows.csv), which was never
appended: 34 of its 42 keys reached the CSV through the inflow lanes, and these 8 did not.

## Tally after batch 7

Of the 1,004: 621 killed, 359 equivalent, 24 recorded gaps, 0 remaining. Batch 6's 511, 275 and 17,
plus the inflow lanes' 100 and 14, plus this batch's 10, 70 and 7.
