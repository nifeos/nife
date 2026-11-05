# How much memory each proof takes, and what the Kani pool reserves for it

An appendix to [notes/verification.md](../verification.md). It holds the measured peak memory of
every row of `script/verify`'s crate table, and how `helpers/heavy_slot.py` turns it into a weight.

The machine's Kani memory is a pool of 2 GiB units ([heavy-job-slots.md](../scripts/heavy-job-slots.md)).
A run takes the units of every harness it runs at once. With no measurement a harness weighs 10 GiB,
so `script/verify --only <row>` at the default `VERIFY_JOBS=4` takes 40 GiB, all of a 56 GiB host's
pool. Most proofs use a small fraction of that. A `generational_table` run held all 20 units while
its two solvers used about 4 GiB each, and every other proof on the machine waited.

## How the numbers were taken

`python3 helpers/kani_peaks.py measure <row>` runs `script/verify --only <row>` at `VERIFY_JOBS=1`
in a transient systemd scope and reads the scope's `memory.peak`. That counts everything the run
did: the compile, every solver and the page cache the build wrote. So a row's peak is the largest
single harness plus the compile, and it errs high. The script writes the row to
[`helpers/kani-peaks.tsv`](../../helpers/kani-peaks.tsv) with the crate directory's git tree and
the Kani version, and `python3 helpers/kani_peaks.py show` prints the table as the checkout sees it.

Measured on 2026-11-05 (UTC), Kani 0.68.0, on an x86_64 Linux host, heaviest first:

| row | peak GiB | units |
|---|---:|---:|
| `glob` | 8.37 | 5 |
| `subtree_scope` | 5.80 | 3 |
| `machine_discovery` | 5.23 | 3 |
| `file_allocation_table` | 5.02 | 3 |
| `capability_manifest_note` | 3.60 | 2 |
| `elf` | 3.46 | 2 |
| `calendar` | 3.30 | 2 |
| `usb` | 1.91 | 1 |
| `measured_boot` | 1.81 | 1 |
| `domain_name_system` | 1.72 | 1 |
| `globally_unique_identifier_partition_table` | 1.65 | 1 |
| `interfaces_note` | 1.41 | 1 |
| `jh7110_clock_and_reset` | 1.35 | 1 |
| `components` | 0.99 | 1 |
| `block_roster` | 0.93 | 1 |
| `extensible_host_controller_interface` | 0.78 | 1 |
| `direct_memory_access_validator` | 0.76 | 1 |
| `driver_process_contract::extensible_host_controller_interface` | 0.75 | 1 |
| `capability_plan` | 0.70 | 1 |
| `pci` | 0.62 | 1 |
| `capability` | 0.56 | 1 |
| `device_tree_blob` | 0.54 | 1 |
| `generational_table` | 0.52 | 1 |
| `network_time_protocol` | 0.45 | 1 |
| `designware_mobile_storage` | 0.45 | 1 |
| `non_volatile_memory_express` | 0.42 | 1 |
| `timetable` | 0.40 | 1 |
| `nifefs` | 0.38 | 1 |
| `package_archive` | 0.37 | 1 |
| `inter_process_communication` | 0.32 | 1 |
| `page_frames` | 0.30 | 1 |
| `filesystem_contract` | 0.29 | 1 |
| `intrusive_queue` | 0.22 | 1 |
| `periodic_deadline` | 0.19 | 1 |
| `driver_process_contract::e1000e` | 0.18 | 1 |
| `credential_contract` | 0.17 | 1 |
| `address_space_map` | 0.13 | 1 |
| `paging` | 0.10 | 1 |
| `address_space_identifier` | 0.09 | 1 |
| `designware_ethernet` | 0.09 | 1 |
| `driver_process_contract::non_volatile_memory_express` | 0.08 | 1 |
| `e1000e` | 0.08 | 1 |
| `memory_regions` | 0.07 | 1 |
| `driver_process_contract::designware_mobile_storage` | 0.07 | 1 |
| `kernel` | not measured | 5 |

## What a row weighs

A row weighs its peak rounded up to whole units, at least one. There is no further margin: the peak
already holds the compile and the page cache, and CBMC uses the same memory for the same program
and version. Thirty-seven of the 44 measured rows weigh one unit, so a single-row run at
`VERIFY_JOBS=4` takes 8 GiB where it took 40. `glob` is the largest row and weighs 10 GiB, the same
as the default, so a run over the whole table (`--shard`, `--parallel`, no flag) weighs what it did.

A row counts only for the code it measured. When the crate's directory differs from the recorded
tree, has uncommitted or untracked files, or the Kani pin moved, the row weighs the default again.
The reason is the same `generational_table`: its harnesses on main peak at about half a GiB, and
a lane that reworked them ran solvers near 5 GiB each, ten times the measured row. A measured weight
applied to that lane's run would have under-reserved it.

`kernel` stays unmeasured. It proves different harnesses on each host architecture, so one number
cannot stand for it.

## What the pool still does not catch

A weight is a reservation, not a limit. Between 2026-11-04 and 2026-11-05 (UTC) the host's
out-of-memory killer ended nine solver processes at 18 to 52 GiB resident. No row on main comes
near that at `VERIFY_JOBS=1` (the largest is 8.4 GiB), so those came from edited harnesses or
mutants, and the default weight did not hold them either. Only the host stops a run that exceeds its
weight. `helpers/kani_peaks.py`'s `BUGS` section lists this and the other limits: a stale row
after any change to its crate, a fingerprint that misses changes in a crate's dependencies, and
peaks taken on one x86_64 Linux host.
