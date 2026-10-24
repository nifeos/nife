# The test suite reaches RISC-V: the follow-ons of §19 (architectural parity)

*Name: provisional.*

An appendix to [notes/riscv-parity-scope.md](../riscv-parity-scope.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It is the record of making `kernel::sched::tests` and
`kernel::user::tests` run on both ISAs: what moved, the three defects the port surfaced, the
assertions broken on purpose, and what stays aarch64-only.

Follow-on (§19 (architectural parity), 2026-07-31): the last two gated portable tests are gated no longer.
`kernel::sched::tests::an_interrupt_becomes_a_message` and
`an_interrupt_that_arrives_before_the_wait_is_not_lost` were the "two SGI interrupt tests" named
above. Neither property is architectural (IRQ-to-IPC delivery, and a lost-wakeup race); only the
*trigger* was, and it is now behind three per-arch functions in the test module. RISC-V raises the
console UART's own transmit-empty line into the PLIC, because it has no SGI. Its two other
candidates are worse. The SBI IPI lands on the software-interrupt arm, which never reaches
`irq_route`. The PLIC's pending block is read-only, and QEMU ignores writes to it. The legs are
not twins in what the trigger costs, and notes/interrupts.md says which way each one is wider.
That leaves `kernel::sched::tests` fully portable, and the remaining aarch64-only tests are the ones
that genuinely need aarch64 machine code or a GIC.

Follow-on (§19 (architectural parity), 2026-08-01 00:08 UTC, commit 3b6eb2254): `kernel::user::tests` runs on both ISAs. It was the last
whole module gated to one architecture, and the reason turned out not to be the tests. The module
comment said "every test drives a hand-written aarch64 program through `exec` and reads aarch64 fault
registers", which was true and was the wrong thing to fix. The *scaffolding* was aarch64, and the
tests came along unchanged once three things moved.

1. A portable last-fault record. `arch::UserFault` (Permission / Translation / Other, each
   carrying Read / Write / Fetch) plus the address, in place of two public `ESR`/`FAR` statics every
   test decoded inline. This is what keeps the assertion "a permission fault at exactly this
   address" rather than softening it to "a fault happened", which would have been a test converted
   into something that passes for the wrong reason. The two ISAs are not symmetric here, and the
   asymmetry is documented in the code. aarch64 is *told*: `ESR_EL1`'s fault status code
   distinguishes permission from translation in silicon, at the instant of the fault. RISC-V is not:
   `scause` has one code per access kind and never says why the walk refused. So its classifier
   walks the tables the hardware just walked. That walk happens after the fault, which is a real gap
   and is written up as a `BUGS` note on `riscv64::exceptions::classify`. aarch64's answer is a
   measurement; RISC-V's is an inference.

2. The hand-assembled programs became real ELFs. Five `global_asm!` blobs (three aarch64, two
   RISC-V) are gone, along with `exec`, the one-page raw-machine-code loader they needed. Their
   behaviors are ordinary. So they are the `outlaw` binary (two roles: read a forbidden address,
   round-trip through user mode) and the `interrupt_ignorer` that §24 (interrupting the foreground
   process)'s interrupt work already built. Every program the kernel runs now arrives as an ELF. The privilege-boundary test hands the forbidden *address* to
   the program in a register rather than baking a constant into machine code. That is the trick that
   makes one program serve two ISAs with different kernel address spaces.

3. `hello` builds for RISC-V, and always could. It carries the role catalog of milestones 7 (user mode)
   through 19: printing client, the untyped demo, the granter and receiver, the call server, the
   aspace builder, the init roles. xtask's comment claimed it was "aarch64-wired". Three quarters of
   that claim was already false: console, input and shell were in the riscv build list directly
   below it. The last quarter was six syscalls hand-rolled in aarch64 `asm!` naming x0/x2/x3/x4/x8.
   On RISC-V those are the zero register, sp, gp, tp and fp. `user_mode_runtime` had had portable versions of
   all six since 19f.6 lifted the runtime out; the duplicates never got deleted. A stale
   comment stood in for a real blocker for a year, which is this note's recurring lesson in a new
   costume.

Three defects the port surfaced, all invisible until the tests ran on the second machine.

- init built the wrong program. Several init roles in `hello` build a child out of *this
  binary's own* ELF and re-enter it at another role. They found that ELF by reading the archive
  entry literally named `"init"`. Right on aarch64, where hello *is* the boot program; wrong on
  RISC-V, where `init` is the portable `builder` demo. init built a child out of `builder` and started
  it at a role `builder` does not have. The child reached for an initrd mapping it did not own
  and died. Nothing said "wrong program": the test waiting on that child's report just never got one
  and the 90 s ceiling fired. `hello::ROLES_ENTRY` now names the entry per ISA, matching the kernel's
  `HELLO_ENTRY`, and the two must agree.

- The fault record was published before it was written. Every test that reads the last-fault
  record watches `USER_FAULTS` rise and then calls `last_user_fault()`. So the counter is the
  record's publication flag; it was being bumped *first*, on both ISAs, all relaxed. A reader
  therefore gets either an earlier fault's record (the assertion satisfied by the wrong evidence) or,
  on the boot's first fault, a zero that decodes as "nothing faulted". The tests only passed because
  something earlier in the suite had already faulted at the same address for the same reason. Fixed
  by storing the record first and making the counter's `fetch_add` the `Release`, with an `Acquire`
  fence in the accessor.

- A reap wait that was really waiting for the whole machine.
  `reclaim_frees_a_started_then_exited_childs_regions` waited for `thread_count()` to return to a
  baseline it sampled at the top of the test. `thread_count()` is the size of the *entire* thread
  table. The top of the test is exactly when the previous tests' processes are still tearing
  down, so the baseline was a number the system would move on its own. It failed that way once on
  RISC-V, where the slower machine leaves more teardown in flight, and passed on a re-run. That is
  the signature of a wait written against something wider than the property.
  `sched::is_thread_present` asks whether *this* child was reaped. Third time for this shape: the
  wait was a yield count until the cross-core placement of §28 (SMP placement) broke it, then a
  clock-bounded headcount until this. Widening the timeout would have hidden it each time.

The assertions were broken on purpose to check they still bite (a ported test that has never
failed is not evidence it still catches what the original caught). Four representative properties,
one per category, each broken in the kernel or in the user program and each confirmed red on
both ISAs before being restored:

| Category | What was broken | What happened, on both ISAs |
| --- | --- | --- |
| Fault assertions | `outlaw`'s `READ_KERNEL` reads an *unmapped* address instead of the kernel's | `a_user_program_cannot_read_a_kernel_address` fails on `Translation(Read)` versus `Permission(Read)`. This is the one worth having: it proves the permission-versus-translation distinction is load-bearing on RISC-V too, where it is *derived* by walking the tables rather than read out of a register. |
| ELF rejection | `paging::Mapper::map`'s `WrongHalf` guard disabled | `an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused` fails with `left: None`: the load fully succeeded, mapping a user program on top of the kernel. |
| Capability rules | the `GRANT` check on `SEND_CAP` disabled | `a_capability_can_be_delegated_over_ipc_and_grant_gates_re_delegation` fails as a lost-wakeup hang, identically on both. The receiver's re-delegation is no longer refused, so it blocks in the send it expected to fail and its verdict never arrives. Red, but through the watchdog rather than the assertion. |
| `userspace_init_*` | hello's `child()` reports `CHILD_WORD ^ 1` | `the_report_child_builder_parses_an_elf_and_builds_a_running_child` fails on the exact word. |

The first of those is also what found the publication-ordering defect above. Breaking a test
and running it *alone* puts it in a state the full suite never reaches.

What stays aarch64-only, and why. Each is written at the test, not in a blanket module comment,
because a blanket comment is exactly how the old claim survived past being true.

| Test | Reason |
| --- | --- |
| `el1_runs_on_sp_el1` | No RISC-V analogue exists. RISC-V does not bank `sp` by privilege level; there is one `sp`, swapped with `sscratch` on trap entry. The hazard (two names for one register, silently) cannot arise, so a twin would have nothing to assert. |
| ~~`asid_tagging_keeps_address_spaces_apart_without_flushes`~~ | Closed by milestone 58; it runs on both ISAs now. It was here because the property was not true on RISC-V: `write_satp` issued an unconditional `sfence.vma` on every root switch, so a twin would have read the right byte because everything was just flushed, not because the tagging works. The row stays, struck through, because the *reason* is the useful part: this is what a test that cannot fail for its stated reason looks like before anyone notices. |
| `the_hardware_says_el0_cannot_read_the_kernels_memory` | Twin exists: `riscv_virtio_tests::the_page_tables_say_u_mode_cannot_read_the_kernels_memory`. Kept separate on purpose, because the *mechanism* is the subject: aarch64 asks the silicon (`AT S1E0R`), RISC-V has no such instruction and walks in software. Merging them would assert only what both can say. |
| `the_interrupt_child_builder_delegates_an_interrupt_to_its_child` | RISC-V has no second interrupt to raise. Its only hand-assertable line is the console UART's, which `spawn_elf_child_builder` is already routing for the input driver, so a twin would prove delivery through whichever route was bound last rather than through the delegated capability. The property is covered by `riscv_virtio_tests::a_userspace_driver_reads_a_file_from_a_virtio_disk` (which asserts `ROUTED_IRQS` rises while a userspace driver waits on its own Irq cap) and by `sched::tests::an_interrupt_becomes_a_message`. |
| `the_device_child_builder_builds_a_driver_that_reads_real_hardware` | The assertion is `0xB105F00D` in the PL011's PrimeCell identification registers, and RISC-V `virt` has no PL011. That constant is what makes the test exact rather than "the read did not fault"; substituting a virtio magic number would be a different test wearing this one's name. Device delegation to a userspace driver is proved on RISC-V by the virtio-blk driver test, which is a stronger version of the same claim. |
| 24 device / filesystem / network tests | Twins already exist in `riscv_virtio_tests`, which drives the same properties through the dedicated `blk` and `net_stack` binaries. Running both copies would double the suite's slowest tests (including the ~300 s `std_net`) to prove nothing new. This duplication is itself worth revisiting: see the open gap below. |
