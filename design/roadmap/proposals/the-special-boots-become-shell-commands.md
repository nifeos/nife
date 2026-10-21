---
status: PROPOSED
raised: 2026-10-21
milestone_dependencies: 890
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The kernel's special boots become commands typed at the production shell

*(Title and slug provisional. Every program name below is provisional too.)*

Reuse: not applicable; this is a plan, and each step's own block carries its `Reuse:` line.

Raised 2026-10-21 (UTC) by lane `lane/measurements-from-the-shell`, at calef's request. radon reached
the shell prompt on real silicon that day. A measurement can therefore be a command typed at a
prompt on the kernel that ships, rather than a card built with a cargo feature.

## What a special boot is

A special boot is a cargo feature in `kernel/Cargo.toml` that changes what the kernel does after
it boots. Five of them take the place of the progenitor hand-over in `run_measurement_boot`
(`kernel/src/boot.rs`): `job_mix`, `soak_test`, `disk_throughput`, `network_bench` and
`storage_bench`. Two diverge earlier in `kernel_main`: `icount`, then `bench`. `tsc_probe` halts
inside x86_64's timer init (`arch/x86_64/timer.rs`). The soak variants and `lock_wait` stack on
top of a boot.

Each one costs the same three things. The kernel measured is not the kernel that ships. Each
measurement takes its own card or image. And on a board, the operator rebuilds and reflashes to
change which number they want.

The test applied to every feature below: can a program the production shell starts measure the
same quantity, with authority that exists or that one named grant would add, and lose nothing a
reader of the number needs? Moving a feature never means adding a kernel-side counter to the
production build. Where the measurement needs one, the feature stays.

## The sort

"Already EL0" means the part being timed runs at EL0 in the production boot today. "Kernel-only"
names what a program cannot learn through any capability that exists.

| feature | measures | the timed part today | kernel-only facts it prints | verdict |
|---|---|---|---|---|
| `network_bench` | DHCP lease, then TCP drain Mbit/s | `net_stack` and `socket_test_client`, both EL0 | VT-d scope of the NIC; PHY id; radon's bring-up report | moves |
| `disk_throughput` | NVMe write, flush, read, verify | server EL0; client is the kernel boot thread | VT-d scope; LBA size check | moves, with the raw-disk grant |
| `storage_bench` | microSD and eMMC identify and read | kernel, through a kernel MMIO window | slots, clocks and resets, register snapshot | moves, after 53 |
| `job_mix` | jobs per minute against task count | tasks EL0; supervisor in the kernel | placement census; counter frequency | moves, after 523 |
| `lock_wait` | kernel lock contention under the job mix | kernel counters on every `invoke` | all of it | stays |
| `tsc_probe` | TSC rate against CMOS second edges | kernel, ring 0 | the CMOS ports themselves | deleted |
| `bench`, EL0 rows | `null_syscall`, `ctx_switch`, `ipc_rtt_el0`, `sink_throughput`, `map_el0`, `spawn_el0`, the `fs_*` rows, `rg_search` | `os_primitives_benchmarker` and the fs clients, EL0 | none; the kernel only spawns and grants | also runs from the shell |
| `bench`, kernel rows | `yield_switch`, `ipc_rtt`, `relay_rtt`, `call_reply`, `broker_rtt`, `spawn_reap`, `map_new`, `rfence_self`, `tss_iomap_*`, `ipc_scale_*`, `appdisp_*`, `smp_*`, `coremark` | kernel threads and kernel calls | the cost of the kernel path itself | stays |
| `icount` | timer arrival, handler bound, missed ticks | the kernel's timer interrupt path | all of it, under `-icount` | stays |
| `soak_test` and its four variants | cross-core IPC under saturation | `soaker` workers EL0; detection and tick route in the kernel | wake refusals; the `on_tick` route | stays |
| `ipc_stack_depth`, `cycle_counter_grant`, `fastpath_pad`, `single_hart` | kernel stack depth, cycle grant, layout, one hart | kernel | the kernel's own shape | stay |
| `console_flood`, `kernel_log_detached`, `kernel_log_panic_probe`, `self_test_injection`, `watchdog_probe`, `system_tests` | test fault injectors and probes | kernel | n/a | stay |

`board` and `board_th1520` select a target, not a boot, and stay. `shell` is being retired by
another lane and is out of scope here.

## The ones that move

### `network_bench`

Every timed byte already crosses EL0. The kernel starts `net_stack` over the NIC's EL0 driver,
`socket_test_client` drains the stream and reports `[code, bytes, ticks]` to the kernel. The
production boot starts the same `net_stack` through `device_starters.rs` and waits for its lease.
On radon `designware_ethernet_service::PROVEN_ON_SILICON` is `true`, so the shipping boot has a
network.

The command is a jig package holding a drain client, installed with the activation set's existing
`grant <digest> network`. The peer becomes its argument instead of the build-time
`NIFE_NETWORK_BENCH_PEER`. No new authority.

The kernel-only lines move into the production boot log (D1). The `preflight dmar scope` line is
`iommu::scope_of`, whether a VT-d unit this kernel programmed owns the NIC's requester id. The PHY
id and radon's Ethernet bring-up report are read by the kernel at bring-up. Each prints once there.

What it loses is xenon, for now. `e1000e_service::PCH_PROVEN_ON_SILICON` is `false`, and the
production boot leaves an unproven PCH part untouched. A command typed on xenon has no NIC under it.
The flip needs one bench boot that ends `LEASED-AND-MEASURED`, so `network_bench` stays until that
run exists, then goes.

Changes: `cargo xtask network-bench` rehearses the command under OVMF on the production image;
`script/lint`'s `--features network_bench` clippy row goes; `notes/e1000e.md` and
`notes/designware-ethernet.md` give the command. Milestone 494 (a driver for the network card a PC
actually has) runs its remaining items as commands.

### `disk_throughput`

The NVMe server is already EL0 and the production boot starts it. The client is not: the kernel
boot thread makes the `blk()` CALLs. The command holds the raw-disk grant (D2) instead, and that is
the whole gap. The VT-d scope line moves to the boot log (D1). The LBA size check moves cleanly,
because the server knows its namespace's LBA size and can answer it.

The benchmark writes. Its command names a write window, and the server refuses a write outside it.

Changes: `cargo xtask disk-throughput`'s four OVMF cases (root port, LBA 4096, LBA 8192, bypass)
boot the production image and type the command. Milestones 651 and 833 (the same benchmark against
Linux) measure through the command, which is the comparison they want: Linux is measured from a
process too.

### `storage_bench`

Most of this one is kernel code by necessity today. The card is identified and timed through
`crates/designware_mobile_storage::bench` on a kernel MMIO window, and only the last step goes
through the EL0 server. That server is started by `storage_bench` and by nothing else. The booted
system has no block device on radon (the module's own `BUGS`).

So the move is the work of milestone 53 (the board's own peripherals: network and storage on real
silicon), not this plan's. Once the production boot starts the EL0 microSD server, the timed read
holds the same raw-disk grant as `disk_throughput`. The register snapshot and the clock and reset
lines belong in the server's own bring-up log. The radon bench session of 2026-10-21 (pull request
#2485) shows why they matter: the timed CMD18 read fails with a CRC error. Until 53 lands,
`storage_bench` is the instrument debugging that read, and it stays.

### `job_mix`

Milestone 523 (moving the job-mix supervisor into userspace) is this move, investigated end to end.
It is step 6, and its five forks are not restated here. One is narrower than 523 says: blocker 5,
the riscv64 counter frequency. A riscv64 process reads the real rate from the timebase page
`counter_frequency_contract` defines. `kernel::user::timebase_page_phys` publishes it and
`build_child_space` maps it. A child the progenitor builds therefore gets radon's 4 MHz. 523's
lane confirms that against a shell-launched child before counting the blocker closed.

`lock_wait` does not move. Its numbers are counters inside every `invoke`, and a production build
must not carry them.

### `tsc_probe` is deleted

`notes/tsc-under-tcg.md` answered its question: the TSC's rate is sound and the boot's calibration
is the defect. Milestone 571 (the x86 boot calibrates the TSC once) is built on that answer, and the
probe is in no gate (D4). A calibration check on real x86 hardware belongs to a gate on 571, not
to this probe. Deleting it follows the procedure below, minus the command it would have become.

### The `bench` EL0 rows

These already measure a userspace program: `os_primitives_benchmarker` self-times with
`user_mode_runtime::now`, and the kernel only spawns it and hands it a report rendezvous. The bench
boot stays. The same program also becomes a command, so a board on the production kernel gives the
same rows.

Two rows need more than a shell child holds. `spawn_el0` builds and destroys a process, which is
523's blocker 1 (construction authority). `map_el0` may need a region larger than the 64-page
`--mem` ceiling. The rest fit what the shell grants. The fs rows need the filesystem client page,
which the shell already endows.

The baseline does not move (D6). `script/bench --check` gates against `bench/baseline-*.txt`, taken
on a quiet machine under `-icount`. A production boot has the console, `net_stack` and the shell
competing for the core. Its numbers are a different measure. They print under their own row prefix
and are recorded in `bench/` beside the quiet boot's numbers. Nothing gates on them.

## What stays, and why

- `soak_test` and its variants (D5). The detection reads `sched::wake_refusals()`, and the tick
  route is `signal_waiters()` called from `sched::on_tick`. §138 (how a saturated workload is made
  to hand threads across cores) approved option D on the condition that "nothing of it exists in a
  production build". A shell command would need that hook in the shipping kernel. In the
  production boot, the services the progenitor starts also occupy cores, so a draw there comes from
  a different placement distribution. One radon boot is one draw (milestone 225 (run the soak on
  radon, argon and xenon)), and those draws are only comparable with each other.
- `icount` and the kernel `bench` rows. What they time is the kernel path. A program can time a
  syscall, but not `sched::yield_now` between kernel threads.
- `lock_wait`, `ipc_stack_depth`, `cycle_counter_grant`, `fastpath_pad`, `single_hart`. Each
  changes the kernel's code or layout to see itself. A production build carrying any of them would
  be a different kernel from the one being measured.
- The test probes. They inject faults a production kernel must not be able to produce.

## Order

Cheapest and most useful first. Each step is a milestone of its own. No step on radon starts before
its input works under milestone 890 (riscv64 and x86_64 join the shared boot path). Today radon's
prompt echoes the first keystroke and no later one (pull request #2485), so nothing can be typed
there yet. QEMU and xenon can start sooner.

1. The `bench` EL0 rows as a command. The program exists; it needs a catalog entry or a jig
   package and a manifest. It proves the shape on every architecture.
2. `network_bench` on radon, with the production kernel printing D1's lines at bring-up. No new
   authority, and radon's production boot has the network.
3. The raw-disk grant, then `disk_throughput` through it. xenon and the OVMF rehearsal.
4. `storage_bench`, after milestone 53 starts the microSD server in the production boot. It holds
   step 3's grant.
5. `network_bench` on xenon, after the bench boot proves the I219. Then the feature is deleted.
6. `job_mix`, as milestone 523. Its five forks come to the architect here, placement visibility
   first. 523 waits behind the partial milestones and starts after them.
7. Delete `tsc_probe`. It has no precondition, so it may land before any step above.

## End state

The kernel's measurement features are the ones that measure the kernel: `bench`'s kernel rows,
`icount`, the soak family, `lock_wait`, `ipc_stack_depth`, `cycle_counter_grant`, `fastpath_pad`
and `single_hart`, plus the test probes. Everything that measures a device or a userspace workload
is a program, typed at the prompt of the kernel that ships. It ships as a jig package from basalt
like every other program (§297 (software reaches a nife system as a package `jig` installs)).
`run_measurement_boot` holds `soak::run` alone. `script/board-image` keeps `--bench` and `--soak`,
and `--job-mix` goes.

## How a retired feature is deleted

In one pull request, once the command has produced the same verdict on the machine the feature
was proven on:

1. Delete the feature from `kernel/Cargo.toml`, its module, its arm in `run_measurement_boot`, and
   every `cfg` naming it. `boot.rs`'s `hand_over` and `sched::exit` conditions lose it too.
2. Delete its `script/lint` clippy row, its `script/board-image` flag and its xtask build. Keep the
   xtask rehearsal pointed at the production image.
3. Rewrite the notes page's procedure as the command. A `bench/` log stays as evidence, unedited.
4. Cite the bench log that matched in the commit message. The comparison is the proof, and the
   tree states only the command.

## Found while sorting

These need homes of their own and are not this plan's.

- `kernel/Cargo.toml` says `script/bench`, `script/icount` and milestone 25 (cross-OS performance
  comparison) turn on `cycle_counter_grant`. Nothing in `xtask/` or `script/` does. Only
  `--extra-features` can.
- `kernel/src/arch/x86_64/port.rs` says no user program has been given a port. COM1's `PortRange`
  is one, minted by milestone 299 (the x86 port-range capability).
- Milestone 268 (every architecture boots the same way) says `self_test_injection` fails CI.
  `script/ci-build` runs `script/boot-check` without `--inject`, and nothing in `.github/` passes it.

## Decisions

calef ruled each of these on 2026-10-21 (UTC), on pull request #2488.

### D1. The production kernel prints the preflight facts at bring-up

The VT-d scope of each DMA device, the PHY id and radon's Ethernet bring-up report print once, at
device bring-up, in the boot log the board console reads. No method and no per-operation cost are
added. A measurement quotes the boot log's line beside its number.

### D2. One raw-disk grant serves every raw-access program

A single shell-manifest grant delivers the existing one-disk block-service endpoint
(`filesystem_contract::blk`) to a shell-launched program. The server enforces one of three modes:
read, write within a declared window, or write the whole disk. The person typing the command picks
the disk, per command.

Every program that needs raw access holds this grant: `disk_throughput` and `storage_bench`, and
the disk utilities (`disk_surveyor`, `disk_partitioner`, mkfs, fsck, the installer, imaging). No
second raw-access mechanism exists. calef: "I don't want to build two mechanisms to enable that
unless necessary."

### D3. `job_mix` leaves the kernel as milestone 523, at step 6

523's five forks (construction authority, initrd access, a per-program memory ceiling, placement
visibility and the riscv64 frequency) come to the architect at step 6, placement visibility first.
The number exists to decide the event-kernel question (§96 (process kernel or event kernel)) for
programs on the shipping kernel. Nothing starts now: 523 waits behind the partial milestones.

### D4. `tsc_probe` is deleted

Its question is answered and 571 is built on the answer. A real-hardware calibration check
belongs to a gate on 571.

### D5. The soak stays a special boot

It stays `--features soak_test`, and §138 option D's condition holds: nothing of the `on_tick`
hook exists in a production build.

### D6. Production-kernel bench numbers are recorded, never gated

They print under their own prefix and are recorded in `bench/` beside the quiet boot's numbers.
The quiet-machine baseline stays the tripwire.
