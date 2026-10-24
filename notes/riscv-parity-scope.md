# Scoping RISC-V / aarch64 feature parity

The RISC-V port proved the capability core on a second ISA: boot, MMU (Sv39), traps, the SBI
timer, the scheduler, preemption, U-mode programs and syscalls, capability invocation, IPC,
userspace-built processes, and device interrupts serviced by an unprivileged userspace driver. Rule
#1 held: a new ISA was a new `arch/` directory, not a diff across the kernel.

aarch64 is a strict superset. This note scopes the gap: what it would take to bring RISC-V to
parity, what each item proves, and in what order to do them. Every item is done now, and the
records behind each are in [riscv-parity-scope/](riscv-parity-scope/README.md).

A key framing runs through the whole list: the demonstrator's thesis is about the kernel (a
verified, portable capability microkernel). So the parity items that prove *kernel* properties on the
second arch are worth more than the ones that only port *userspace* apps.

Effort is session-sized: S = part of a session, M = one to two sessions, L = several.

---

## The gaps

### A. SMP (multi-hart): DONE.

RISC-V runs on all four harts, and the SMP test suite passes (riscv64 55, aarch64 116). Built in
four steps. A1 is the per-hart trap state (`sscratch` points at a per-hart `TrapStash` holding the
kernel `tp` and stack, replacing the single global that could not scale past one hart). A2 is
secondary bring-up via SBI HSM `sbi_hart_start` into a `secondary_boot` that replays the higher-half
transition. It is made robust to QEMU's non-deterministic boot hart by keying everything to
`arch::boot_cpu_id()` (logical id == hart id). A3 is IPIs via the SBI IPI extension (a supervisor
software interrupt, `scause` = 1, draining the inbox), lighting up `send_reschedule`. A4 is the SMP
tests un-gated and generalized off "boot core is 0". The subtle one was TLB shootdown: RISC-V has no
hardware TLB broadcast. So `flush_tlb` follows its local `sfence.vma` with an SBI RFENCE to the
other online harts, or a thread migrated to a core faults on a stale translation of its own stack.

### B. In-kernel test suite on RISC-V: DONE.

RISC-V now boots the kernel test harness and passes 51 portable tests (`cargo xtask test` runs
both arches: aarch64 116, riscv64 51). The gap is the aarch64-specific tests, gated off riscv: the
userspace-exec suite in `system_tests/src/user/tests.rs` (37 tests driving hand-written aarch64 programs through `exec`), the
SMP tests (workstream A), and the two SGI interrupt tests. Three real things surfaced. The
`sifive_test` finisher (`0x10_0000`) had to be mapped device-typed and reached through the direct
map (the boot tour halts via `wfi` and never exercised `semihosting::exit` under paging). The timer
watchdog needed wiring into riscv `timer::tick`. And a genuine race: a timer tick landing inside
`sched::init` ran the deferred `schedule()` before the idle thread was registered ("nothing runnable
and no idle thread"). It was fixed by masking interrupts across init, which aarch64 gets for free by bringing
the scheduler up before enabling interrupts.

Follow-on (§19 (architectural parity)): the suite was at parity on the portable tests and not on the arch ones.
`arch/aarch64` carried 21 unit tests and `arch/riscv64` carried zero, across the same three files, so
the properties that differ between the ISAs were asserted on one side only. The twins are now
written, and they found three defects nothing else would have. The timer delivered 80 Hz against a
configured 100 (the relative-re-arm drift bug aarch64 shipped and fixed back at milestone 5, arrived
at here by a different route). Then `missed_ticks` a stub returning 0, and a single global tick counter
for a per-hart timer. The lesson is B's own lesson repeated one level down. A parity claim is only
as wide as the suite that checks it, and "both ISAs run the same suite" was true of the portable half
and not of the arch half. See notes/riscv-arch-tests.md, which also sizes the remaining
`kernel::user::tests` port.

Follow-on (§19 (architectural parity), 2026-07-31, and 2026-08-01 00:08 UTC for the
`kernel::user::tests` half): `kernel::sched::tests` and `kernel::user::tests` run on both ISAs. The two SGI interrupt tests now trigger through three per-arch functions, with RISC-V
raising its console UART's own transmit-empty line (notes/interrupts.md). `kernel::user::tests` was
gated by its scaffolding, not its tests: a portable `arch::UserFault` record replaced the aarch64
`ESR`/`FAR` statics, the hand-assembled programs became real ELFs, and `hello` turned out to have
built for RISC-V all along. RISC-V infers permission-versus-translation by walking its tables where
aarch64 is told by `ESR_EL1`, and `riscv64::exceptions::classify`'s `BUGS` says so. The port
surfaced three defects: init built the wrong program, the fault record was published before it was
written, and a reap wait was waiting for the whole machine, "a wait written against something wider than the
property". Four assertions were broken on purpose
and went red on both ISAs. What stays aarch64-only, and why, is written at each test. The full
record is in [riscv-parity-scope/user-tests-port.md](riscv-parity-scope/user-tests-port.md).

### Open gap: `tests` and `riscv_virtio_tests` overlap by 24 tests

The 24 are gated to aarch64 only because the twin exists. The overlap was measured: nine bodies are
byte-identical and no behavior diverges
in any of the 24. The merge (use `blk` on both ISAs and delete the copies) is deliberately not done,
because it changes what the disk and network tests prove.

### Closed and corrected gaps

The other gaps are closed or corrected. Milestone 58 (RISC-V TLB shootdown) made RISC-V stop
flushing its whole TLB on every
address-space switch, after the SBI RFENCE shootdown landed. The leak police, `no_leaked_threads`,
sorted before `user::tests` and so never policed it; it is `thread_leak_police` now, runs last, and
the two spinners it found are reaped. `every_secondary_runs_scheduled_work` was a placement bug, not
host load: a wider deadline changed nothing, which is what ruled out slowness. And
`kernel::smp::tests` is genuinely host-load sensitive on `main`, so run the same load against `main`
before blaming a diff. Each is in [riscv-parity-scope/suite-gaps.md](riscv-parity-scope/suite-gaps.md).

### C. virtio-blk + on-disk filesystem: DONE, over BOTH transports

Booting the riscv kernel with the runner's exact flags (QEMU 11.0.2, `-global
virtio-mmio.force-legacy=false -device virtio-blk-device`) finds a modern virtio-mmio block device
at slot 7 (0x10008000, PLIC IRQ 8); `find_block_device` reports it and the kernel registers the
transport. Both runners fail loudly on a missing disk file. A runner that silently drops the disk
when `NIFE_DISK` names a file that does not exist boots disklessly, every slot honestly reads
device-id 0 ("empty"), and it looks like QEMU preferring PCIe for riscv, which it does not. The
PCIe transport keeps its own justification (notes/pcie-transport-scope.md: the door to NVMe and
real hardware) but is not a parity-C prerequisite.

Over mmio: the `blk` dedicated binary (the shared
`virtio` module behind the 19f skeleton), `virtio_service` wiring unchanged, and the three disk
tests (read, DMA-escape attacker, indirect attacker) green on riscv. The PLIC is programmed for
the boot hart's own context, not a hardcoded context 1, because `sie.SEIE` is set on whatever hart
OpenSBI elects.
Over PCIe as well: the PCIe transport (DECISIONS §18 (the PCIe transport: one driver, two buses),
notes/pcie.md) runs the byte-identical driver against the same image attached as virtio-blk-pci,
with the completion arriving as INTx through the PLIC. Every parity workstream (A-E) is done;
aarch64 and riscv64 are at feature parity, DMA confinement included.

### D. Full integrated boot + interactive shell: DONE.

The interactive shell runs on RISC-V, over the serial: `echo` echoes, `run 9` spawns a worker that
computes 81. A new portable `system_initializer` (the counterpart of hello's aarch64-tied
`init_boot`) is loaded as the boot process. From an untyped budget plus the NS16550 device cap and
the UART Irq cap, it builds the console server, input driver, and shell out of its own budget and
wires them. The kernel parks. The three shell programs became arch-neutral -- the UART register
layout is gated per arch
(PL011 vs NS16550, including that the NS16550 clears its RX interrupt on read, with no ICR). The
wiring is arch-neutral: `system_initializer` maps whatever device cap the kernel grants and delegates whatever
Irq cap it is handed, so the same binary would drive a PL011. Every boot reaches it after the
tour, as on aarch64.

### E. Benchmarks: DONE.

All eleven primitives plus CoreMark run on RISC-V, single-hart and SMP. `bench.rs` moved its timing
to `arch::timer::now`/`frequency` (rdtime on riscv), the boot reaches `bench::run` under `--features
bench`. `initial-ramdisk-riscv` packs `os_primitives_benchmarker` + `coremark`. Two fixes fell out
of `spawn_el0` (the fast
userspace spawn+reap loop): os_primitives_benchmarker's 9-instruction `CHILD_STUB` was aarch64
machine code (added the riscv `li`/`ecall` version). And the `MAP_CODE` syscall never synced the
icache on the userspace
map-executable path (a correctness fix for both arches, latent until a spawn loop stressed it).

And the numbers are now comparable. `cargo xtask bench --riscv` runs the same suite on riscv under
the same deterministic `-icount` instrument (single hart, so an idle `wfi` cannot jump virtual time to
the timer and inflate the spawn primitives), with its own baseline (`bench/baseline-riscv64.txt`) for
`--check`/`--save`. The comparable metric is ns/iter: under `-icount shift=0` virtual time advances
~1 ns/instruction, and ns/iter divides out each arch's timer frequency (aarch64 CNTFRQ vs riscv's
10 MHz), so it is frequency-normalized instructions-per-primitive. CoreMark lands within ~7% across
the two ISAs (same workload), which is the sanity check that the comparison is sound.

## Post-parity correction (2026-07-27, milestone 32): user faults did not kill on riscv

Parity was declared with a hole nothing had tested: the riscv trap dispatcher could not kill a
faulting user thread. A U-mode `ebreak` was counted and stepped over, so a userspace panic
handler (every one of which ends in `ebreak`, expecting to die) resumed into its own spin loop and
lived forever. Any other U-mode fault fell into an arm that panicked the whole kernel. A comment
claimed no user thread could run on RISC-V yet, which had been false since milestone 20 (a portable HAL, proven on a second architecture).
DECISIONS §10's "a driver bug is a crashed process, not a dead machine" was an aarch64-only
property, and the parity record above did not say so.

Nothing noticed because no riscv test ever made a user thread fault. All the parity-C drivers
either succeed or are refused politely; the fault path had no witness. Milestone 32's
kill-mid-write test is the first riscv test that *requires* a user thread to genuinely die, and it
flushed the gap out within minutes of being written.

The fix mirrors aarch64's `user_fault`: a U-mode breakpoint or fault increments `USER_FAULTS`,
prints the legible kill line, and `sched::exit()`s the thread from the trap handler (the same
context the blocking-syscall path already schedules away from). The S-mode `ebreak` self-test
keeps its step-over. Proven by `a_faulting_user_thread_is_killed_and_the_kernel_survives` (spawn
the blk binary at a bogus role: panic, `ebreak`, killed, reaped) and by the kill-mid-write pair.

Two lessons, both repeats: a stale comment is misinformation with authority (the teardown note's
"a TODO that outlives the decision that resolved it" rule, hit again); and a parity claim is only
as wide as the suite that checks it. The fault path was in the suite's blind spot on both counts.

## The aarch64-only user tests

Two tests in `system_tests/src/user/elf_child_builder_tests.rs` run only on aarch64. Each names this
section, and each property they cover is proved on RISC-V by a different test.

The interrupt-delegation test (`interrupt_child_builder`) needs an interrupt the test can raise by
hand. RISC-V has no software-generated interrupt a test can assert on itself: the SBI IPI arrives
down the software-interrupt arm and never reaches `irq_route`. The only line it can raise by hand is
the console UART's own, and `spawn_elf_child_builder` already routes that as the UART receive
interrupt. A twin would share that one source between the builder's UART capability and the test's
delegated one. It would then prove delivery through whichever route was bound last, not through the
delegated capability, which is the whole claim. On RISC-V the property (an interrupt arriving as a
message through a delegated Irq cap) is proved by
`riscv_virtio_tests::a_userspace_driver_reads_a_file_from_a_virtio_disk`, which asserts that
`ROUTED_IRQS` rises while a userspace driver waits on its own Irq cap, and by
`sched::tests::an_interrupt_becomes_a_message`.

The device-delegation test (`device_child_builder`) asserts a PL011 register, and RISC-V `virt` has
no PL011. The constant `0xB105F00D` in the PrimeCell identification registers is what makes the test
exact rather than "the read did not fault". The NS16550 on the other machine has no equivalent
constant, and a virtio magic number in its place would be a different test under this one's name.
On RISC-V, device delegation is proved by the same virtio-disk test, which is a stronger form of the
claim: device MMIO, a DMA region and an interrupt, all delegated.
