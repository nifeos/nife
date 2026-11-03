# The inflow of 2026-10-05 and 2026-10-12, triaged

Lane maintainer/mutation-triage-2026w42, 2026-10-12 (UTC), for fatal risk 3 (the tests do not
test). Milestone 740 (the survivors a merged pull request adds are checked against a triage
record) built the weekly `inflow against the triage record` job. It failed on both runs since it
first passed: [37307759312](https://github.com/nifeos/nife/actions/runs/37307759312) (2026-10-05,
204 untriaged) and [38252397727](https://github.com/nifeos/nife/actions/runs/38252397727)
(2026-10-12, 508 untriaged). Every key on both lists now has a row in `notes/project-metrics/mutation-triage.csv`.

## What was on the lists

712 listed keys, 711 distinct. Of those, 704 were still survivors in the 2026-10-12 census, covering
1,200 mutants; one already had a row; six had been caught by 2026-10-12 (e1000e's `ulp::done` and
`ulp::has_me`, machine_discovery's `read_ivmd`), so the second run never named them again. The 704
were re-run crate by crate with `cargo mutants` on the lane's worktree (macOS, base `9ed60b258`),
after the new tests, and each row's disposition is what that run showed.

More than half came from three drivers merged in the window: `e1000e`'s I219 bring-up (#1731),
`designware_mobile_storage` (#1772) and `designware_ethernet` (#1762).

## The rows

| crate | keys | killed | equivalent | gap |
|---|---|---|---|---|
| `e1000e` | 163 | 131 | 32 | 0 |
| `designware_mobile_storage` | 119 | 97 | 22 | 0 |
| `designware_ethernet` | 111 | 89 | 20 | 2 |
| `extensible_host_controller_interface` | 55 | 30 | 25 | 0 |
| `grant_plan` | 36 | 33 | 3 | 0 |
| `system_log_protocol` | 36 | 32 | 4 | 0 |
| `paging` | 18 | 8 | 10 | 0 |
| `usb` | 16 | 12 | 4 | 0 |
| `inter_process_communication` | 15 | 13 | 2 | 0 |
| `jh7110_clock_and_reset` | 14 | 9 | 5 | 0 |
| `board_console` | 13 | 13 | 0 | 0 |
| `screen_console` | 12 | 10 | 2 | 0 |
| `package_index` | 11 | 11 | 0 | 0 |
| 20 more crates, 1 to 9 keys each | 85 | 60 | 24 | 1 |
| the six caught since 2026-10-05 | 6 | 6 | 0 | 0 |
| total | 710 | 554 | 153 | 3 |

No survivor was a bug. The kills fall into a few shapes, each already named in the note's "What
the tool has taught".

- A simulator that only answers yes. The three drivers' host models never left a cycle running,
  never held a status bit, and started every register at zero. So a sequence that wrote the wrong
  bits read back the same. Their tests now start registers with bits set, log every register
  access, and add fault knobs. A wait that only matters when hardware never answers runs against an
  inert device or under a deadline, so it fails by name.
- Constants checked only against themselves. The driver and its simulator read one `regs` module,
  so a misplaced bit agreed everywhere. The register bits are now pinned as numbers.
- A boundary no fixture met. Among them: a DNS TTL past 16 bits, a compression pointer past offset
  255, an IVRS block exactly four bytes long, a stem of `STEM_LEN` and a table of `MAX_VERBS`.
- A refusal for the wrong reason. `screen_console`'s stride refusal was tested with a zero scale,
  which was refused first.

The equivalents are mostly `|` against `^` over disjoint fields. One recurring form is worth
knowing. `^` binds tighter than `|`, so swapping the second `|` of `x | A | B` gives `x | (A ^ B)`.
With disjoint constants that is `x | A | B` again. Only the swap on the first operand is visible.

## Renames

None of the listed keys was a rename of a row that already existed. Fifteen were renames of
survivors with no row, and are triaged once, under the names cargo-mutants gives them now:

- `abi`'s eight (`cycle_counter::CycleMeaning::is_core_cycles`, `futex::admitted`). The 2026-10-12
  split moved both inline modules into files, which drops the module prefix from the mutant's
  name, so their rows are keyed `CycleMeaning::is_core_cycles` and `admitted`. Re-running the
  2026-10-12 check against this record therefore still lists those eight under their old names;
  the next census names them the new way.
- `filesystem_protocol`'s six and `line_editor`'s one. The splits #1915 and #1969 moved them out of
  inline modules, so `git blame` attributed old survivors to the move and the job listed them as
  inflow. All seven are `|` against `^` over disjoint fields.

## BUGS

- The three gaps: `direct_memory_access_validator`'s `verification::ChainMem::racing` is in the
  `cfg(kani)` module, and cargo-mutants 27.1.0 applies `.cargo/mutants.toml`'s `verification::`
  exclusion to function mutants but not to a struct-field deletion. `designware_ethernet`'s two
  `prepare` keys cover the coherence probe's read-back length, which `simulator::Memory`'s
  write-allocate cache hides; a no-write-allocate mode would close them. `package_index`'s
  `ipv4_literal` key was a fourth, differing only for a zero-padded part such as `0010.0.0.1`.
  calef ruled on 2026-10-12 (UTC) that such a host is refused outright; #2020 built the refusal,
  and its test `a_host_that_ends_in_a_number_must_be_a_canonical_address` kills the key.
- `e1000e`'s `accessible` keeps FreeBSD's branch for an identifier already known, which this port's
  only caller never reaches. Its mutants are recorded as equivalent for that reason.
- Six crate roots grew past their §282 (when a file is split, the parent becomes an index)
  baselines with the new tests. Their inline test modules moved to `tests.rs`:
  `designware_ethernet`, `designware_mobile_storage`, `e1000e` (and `pch/`),
  `extensible_host_controller_interface`, `inter_process_communication` and
  `name_resolution_protocol`.
