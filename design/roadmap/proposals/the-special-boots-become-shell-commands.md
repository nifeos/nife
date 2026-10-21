---
status: PROPOSED
raised: 2026-10-21
milestone_dependencies: 890
decision_dependencies: unwritten
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
| `disk_throughput` | NVMe write, flush, read, verify | server EL0; client is the kernel boot thread | VT-d scope; LBA size check | moves, after D2 |
| `storage_bench` | microSD and eMMC identify and read | kernel, through a kernel MMIO window | slots, clocks and resets, register snapshot | moves, after 53 |
| `job_mix` | jobs per minute against task count | tasks EL0; supervisor in the kernel | placement census; counter frequency | moves, after 523 |
| `lock_wait` | kernel lock contention under the job mix | kernel counters on every `invoke` | all of it | stays |
| `tsc_probe` | TSC rate against CMOS second edges | kernel, ring 0 | the CMOS ports themselves | decision D4 |
| `bench`, EL0 rows | `null_syscall`, `ctx_switch`, `ipc_rtt_el0`, `sink_throughput`, `map_el0`, `spawn_el0`, the `fs_*` rows, `rg_search` | `os_primitives_benchmarker` and the fs clients, EL0 | none; the kernel only spawns and grants | also runs from the shell |
| `bench`, kernel rows | `yield_switch`, `ipc_rtt`, `relay_rtt`, `call_reply`, `broker_rtt`, `spawn_reap`, `map_new`, `rfence_self`, `tss_iomap_*`, `ipc_scale_*`, `appdisp_*`, `smp_*`, `coremark` | kernel threads and kernel calls | the cost of the kernel path itself | stays |
| `icount` | timer arrival, handler bound, missed ticks | the kernel's timer interrupt path | all of it, under `-icount` | stays |
| `soak_test` and its four variants | cross-core IPC under saturation | `soaker` workers EL0; detection and tick route in the kernel | wake refusals; the `on_tick` route | stays, D5 |
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

What it loses:

- The `preflight dmar scope` line. Whether a VT-d unit this kernel programmed owns the NIC's
  requester id is `iommu::scope_of`, a kernel fact (D1).
- The PHY id and radon's bring-up report, read by the kernel at bring-up (D1).
- On xenon, the instrument that earns the I219 its way into the production boot.
  `e1000e_service::PCH_PROVEN_ON_SILICON` is `false`, and the production boot leaves an unproven
  PCH part untouched. A command typed on xenon has no NIC under it. The flip needs one bench boot
  that ends `LEASED-AND-MEASURED`, so `network_bench` stays until that run exists, then goes.

Changes: `cargo xtask network-bench` rehearses the command under OVMF on the production image;
`script/lint`'s `--features network_bench` clippy row goes; `notes/e1000e.md` and
`notes/designware-ethernet.md` give the command. Milestone 494 (a driver for the network card a PC actually has) runs its remaining items as commands.

### `disk_throughput`

The NVMe server is already EL0 and the production boot starts it. The client is not: the kernel
boot thread makes the `blk()` CALLs. A command needs a program holding a client capability on that
server, which no shell manifest can declare today. That is D2, and it is the whole gap.

What it loses: the VT-d scope line (D1). The LBA size check moves cleanly, because the server
knows its namespace's LBA size and can answer it. The write window stays a hazard. A raw-block
grant must name the window, and the program refuses to run without one.

Changes: `cargo xtask disk-throughput`'s four OVMF cases (root port, LBA 4096, LBA 8192, bypass)
boot the production image and type the command. Milestones 651 and 833 (the same benchmark against
Linux) measure through the command, which is the comparison they want: Linux is measured from a
process too.

### `storage_bench`

Most of this one is kernel code by necessity today. The card is identified and timed through
`crates/designware_mobile_storage::bench` on a kernel MMIO window, and only the last step goes
through the EL0 server. That server is started by `storage_bench` and by nothing else. The booted
system has no block device on radon (the module's own `BUGS`).

So the move is the work of milestone 53 (the board's own peripherals: network and storage on real silicon), not this plan's. Once the production boot starts the EL0
microSD server, the timed read is a client of it under the same grant as D2. The register snapshot
and the clock and reset lines belong in the server's own bring-up log. The radon bench session of
2026-10-21 (pull request #2485) shows why they matter: the timed CMD18 read fails with a CRC error.
Until 53 lands, `storage_bench` is the instrument debugging that read, and it stays.

### `job_mix`

Milestone 523 (moving the job-mix supervisor into userspace) is this move, investigated end to end. Its five blockers are this plan's, and they
are not restated here. One is narrower than 523 says: blocker 5, the riscv64 counter frequency. A
riscv64 process reads the real rate from the timebase page `counter_frequency_contract` defines.
`kernel::user::timebase_page_phys` publishes it and `build_child_space` maps it. A child the
progenitor builds therefore gets radon's 4 MHz. 523's lane should confirm that against a
shell-launched child before counting the blocker closed.

`lock_wait` does not move. Its numbers are counters inside every `invoke`, and a production build
must not carry them.

### `tsc_probe`

It reads the CMOS second edge at ports `0x70` and `0x71`. A `PortRange` capability exists
(milestone 299 (the x86 port-range capability)) and is minted at boot for COM1's ports only. `clock_service.rs` records why the CMOS
is different: DECISIONS §121 (what a device capability is when the device has no page) keeps that port range kernel-resident, and the kernel's own RTC reader
uses it. The CMOS is an index register followed by a data register. Two parties driving it race
each other. The x86 TSC is ambient at ring 3, so the TSC half is free. The CMOS half is D4.

### The `bench` EL0 rows

These already measure a userspace program: `os_primitives_benchmarker` self-times with
`user_mode_runtime::now`, and the kernel only spawns it and hands it a report rendezvous. The
proposal is not to delete the bench boot. It is to make the same program also a command, so a board
on the production kernel gives the same rows.

Two rows need more than a shell child holds. `spawn_el0` builds and destroys a process, which is
523's blocker 1 (construction authority). `map_el0` may need a region larger than the 64-page `--mem` ceiling.
The rest fit what the shell grants. The fs rows need the filesystem client page, which the shell
already endows.

The baseline does not move. `script/bench --check` gates against `bench/baseline-*.txt`, taken on a
quiet machine under `-icount`. A production boot has the console, `net_stack` and the shell
competing for the core. Its numbers are a different measure, and they print under their own row
prefix so nobody compares them with the baseline (D6).

## What stays, and why

- `soak_test` and its variants. The detection reads `sched::wake_refusals()`, and the tick route
  is `signal_waiters()` called from `sched::on_tick`. §138 (how a saturated workload is made to hand threads across cores) approved option D on the condition that
  "nothing of it exists in a production build". A shell command would need that hook in the shipping
  kernel. There is a second reason. In the production boot, the services the progenitor starts
  already occupy cores, so a draw there comes from a different placement distribution than the
  225 draws so far. One radon boot is one draw (milestone 225 (run the soak on radon, argon and xenon)), and those draws are only comparable
  with each other. D5 puts the choice to calef.
- `icount` and the kernel `bench` rows. What they time is the kernel path. A program can time a
  syscall, but not `sched::yield_now` between kernel threads.
- `lock_wait`, `ipc_stack_depth`, `cycle_counter_grant`, `fastpath_pad`, `single_hart`. Each
  changes the kernel's code or layout to see itself. A production build carrying any of them would
  be a different kernel from the one being measured.
- The test probes. They inject faults a production kernel must not be able to produce.

## Order

Cheapest and most useful first. Each step is a milestone of its own, and none starts on radon before its input works under
milestone 890 (riscv64 and x86_64 join the shared boot path). Today radon's prompt echoes the first keystroke and no later one
(pull request #2485), so nothing can be typed there yet. QEMU and xenon can start sooner.

1. The `bench` EL0 rows as a command. The program exists; it needs a catalog entry or a jig
   package and a manifest. It proves the shape on every architecture, and it settles D6.
2. `network_bench` on radon. No new authority, and radon's production boot has the network. It
   answers D1 for the dmar and PHY lines.
3. `disk_throughput`, after D2. xenon and the OVMF rehearsal.
4. `storage_bench`, after milestone 53 starts the microSD server in the production boot. It reuses
   step 3's grant.
5. `network_bench` on xenon, after the bench boot proves the I219. Then the feature is deleted.
6. `job_mix`, when 523's forks are answered.
7. `tsc_probe`, by D4.

## End state

The kernel's measurement features are the ones that measure the kernel: `bench`'s kernel rows,
`icount`, the soak family, `lock_wait`, `ipc_stack_depth`, `cycle_counter_grant`, `fastpath_pad`
and `single_hart`, plus the test probes. Everything that measures a device or a userspace workload
is a program, typed at the prompt of the kernel that ships. It ships as a jig package from basalt like
every other program (§297 (software reaches a nife system as a package `jig` installs)). `run_measurement_boot` holds `soak::run` alone. `script/board-image`
keeps `--bench` and `--soak`, and `--job-mix` goes.

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

- `kernel/Cargo.toml` says `script/bench`, `script/icount` and milestone 25 (cross-OS performance comparison) turn on
  `cycle_counter_grant`. Nothing in `xtask/` or `script/` does. Only `--extra-features` can.
- `kernel/src/arch/x86_64/port.rs` says no user program has been given a port. COM1's `PortRange`
  has been one since milestone 299.
- Milestone 268 (every architecture boots the same way) says `self_test_injection` fails CI. `script/ci-build` runs `script/boot-check`
  without `--inject`, and nothing in `.github/` passes it.

## Open decisions

Each is calef's. One per item, each with its options and a recommendation.

### D1. Where the kernel-only preflight lines go

The VT-d scope of a device, the PHY id and radon's Ethernet bring-up report. Today only the special
boots print them.

- A. The production kernel prints them once, at device bring-up, in the boot log the board console
  already reads. No syscall and no per-operation cost.
- B. A read-only method on the device's existing capability, so a program asks "who confines you"
  and prints the answer beside its number.
- C. Drop them from the measurement. Confinement stays a kernel self-test fact.

Recommendation: A. The confinement of a DMA device is a fact every production user is owed, not
only an operator measuring. B adds a method to the syscall surface for one reader. C separates a
number from the condition that makes it quotable.

### D2. A raw block grant for a shell-launched program

`disk_throughput` and `storage_bench` need a client capability on a block server. No shell manifest
declares one, and the manifest is a published contract between the shell and the progenitor.

- A. A new manifest grant naming one block device. It is read-only by default, and writing requires
  a declared window the server enforces.
- B. Measure through the filesystem instead. Nothing new, but it times RedoxFS over the disk rather
  than the disk, which is a different number from fatal risk 6's.
- C. Keep both as special boots.

Recommendation: A. The window belongs in the server, not the client. A client that asks for the
whole disk is refused there.

### D3. Whether `job_mix` leaves the kernel

Milestone 523 holds five forks: construction authority, initrd access, a per-program memory
ceiling, placement visibility and the riscv64 frequency. This plan adds none.

- A. Promote 523 as step 6 and answer its forks there.
- B. `job_mix` stays a special boot, and the end state names it among the kept features.

Recommendation: A. The number exists to decide the event-kernel question (§96 (process kernel or event kernel)) for programs on the
shipping kernel. Placement visibility (523's blocker 4) decides whether the moved number is
quotable, so it is the fork to answer first.

### D4. `tsc_probe`, given that §121 keeps the CMOS in the kernel

- A. Mint a `PortRange` over `0x70..=0x71` for one program, and stop the kernel's RTC reader while
  it holds it. This reverses §121 for the CMOS.
- B. Keep `tsc_probe` as a kernel feature. It measures the kernel's own boot calibration, at the
  moment that calibration runs.
- C. Delete it. It is in no gate. `notes/tsc-under-tcg.md` answered its question (the rate is
  sound; the calibration is the defect) and milestone 571 (the x86 boot calibrates the TSC once) is built on that.

Recommendation: B, unless calef reads `notes/tsc-under-tcg.md` as closed, and then C. A is the most
work and the most authority, to move an instrument whose subject is a kernel step.

### D5. The soak and §138 option D

- A. No change. The soak stays a special boot, and option D's condition holds.
- B. Move the workers to a command and keep the `on_tick` route in the production kernel behind a
  runtime switch. This breaks D's condition: some of it then exists in a production build.
- C. Move the workers to a command without the tick route. It loses the wake path, which is half
  of fatal risk 5's experiment.

Recommendation: A. B buys a reflash-free soak at the cost of the one property D was approved for.
C measures less, on a placement distribution 225's draws were not taken from.

### D6. Whether the bench program's production-kernel numbers are published

Step 1 makes the EL0 rows runnable on a production board, with services competing for the core.

- A. Print them under their own prefix, record them in `bench/` beside the boot's numbers, and
  never gate on them.
- B. Run them only for diagnosis, and publish nothing from the production kernel.

Recommendation: A. They are the numbers a program on nife actually sees, which is what a stranger
comparing nife with Linux will measure. The quiet-machine baseline stays the tripwire.
