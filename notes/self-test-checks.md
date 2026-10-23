# The boot self-test's checks: what each proves, and why it is built that way

`kernel/src/self_test.rs` runs the second rung of the boot ladder. [boot-ladder.md](boot-ladder.md)
has the contract: the verdict line, `absent`, the board profiles, and why a failure prints and the
boot carries on. This note carries the design of the checks themselves, which the module's doc
comments link to. The checks are the claims the per-port tours made, run on every port (calef,
2026-10-19 UTC, on #2434), and `absent` is calef's ruling of 2026-10-20 (UTC) on #2447.

## The verdict's mechanics

- The fixed strings come from `crates/boot_ladder`, so the kernel and `crates/serial_console` hold
  one copy. Each carries its own trailing space, so a matcher that finds one has found a boundary
  rather than the start of a longer word.
- The total is the length of `boot_ladder::SELF_TEST_CHECKS`, not a count of calls. A listed check
  that recorded nothing is named as a failure, so a kernel that ran four of five says so on every
  architecture. A check not on the list is a failure too.
- Absences print before failures, so a matcher reading the names after `FAILED: ` to the end of the
  line reads only failures.
- Nothing allocates: the name lists are fixed arrays, and the verdict is printed piece by piece.
  A self-test that could fail to report by running out of memory would report on the wrong thing.
- Each check notes its boot stage first, so the breadcrumb `sched::dump_threads` repeats survives a
  lossy serial log (notes/visionfive2.md, fifth stop). The first stage is 3, kept from the riscv64
  tour so board logs read the same, and `TOUR_STAGE` is the one after the last check.

## Injection

The feature `self_test_injection` makes `exceptions` fail and stops the counter the checks read,
so `cargo xtask boot-check --inject` can watch the gate go red. A verdict that has never been red
is untested. The stopped counter is the fault that once hung `timer` forever, so every injected run
also proves the waits are bounded. It is a `cfg!` rather than a `#[cfg]` block, so both arms compile
on every build and the injector cannot rot unnoticed.

## Bounded waits: `Counter`

Every wait is bounded by the free-running counter, which makes a number mean the same thing on a
62.5 MHz generic timer, a 10 MHz `rdtime` (4 MHz on the JH7110) and a TSC. On a broken machine the
counter is the thing in doubt, so `Counter` reports a stall rather than wait on it. A spin calls a
counter stopped after a million unchanged reads, far longer than the slowest counter's 250 ns
period. A wait that yields calls it stopped after a hundred looks, because with `preemption`'s
spinners runnable each look can cost a whole tick; a million looks would be three hours.

## The checks

- `exceptions` proves the whole trap round trip. riscv64 and `x86_64` called
  `arch::exceptions::self_test` from their ports' start; aarch64 had the machinery and gained the
  function. Returning onto the trapping instruction instead of past it would be an infinite loop.
- `mapping` maps a page at an address computed from this machine's RAM: one gigabyte above the
  direct map's last byte, past anything `mmu::init` claimed and in the kernel half on all three. A
  constant there was the third QEMU-shaped address the VisionFive 2 caught in one bench session.
  It uses `phys_to_virt` because the three architectures spell the direct map differently. On
  failure the frame is leaked, because one whose mapping state is unknown must not be reused.
- `frames` takes one frame out of the allocator and back. Every allocation comes through it, and
  it catches a bitmap whose free and used counts have drifted, a leak that reads as plenty of memory.
- `timer` makes two claims in one 20 ms window, two tick periods: the counter moves (the time
  source exists), and a tick arrives (the controller routes the interrupt, the core has it
  unmasked, and the handler runs). It spins rather than sleeping, so a counter that never advances
  and an interrupt that never arrives both end the check.
- `scheduler` spawns a thread that stores a distinctive value, so a zeroed static cannot pass. Its
  wait is bounded by the clock, not a count of yields: the thread may be placed on another core,
  and the VisionFive 2 printed "1 of 2" and "0 of 2" against a yield-bounded wait.
- `preemption` runs one more never-yielding spinner than there are cores, so some core has to be
  taken from one spinner and given to another. Two spinners on four cores could each own a core,
  which was the shape of the tours this replaced. `system_tests`'
  `a_user_program_that_never_yields_is_preempted_anyway` makes the same claim at user level.
- `userspace` has `kernel_test_subject` read the kernel's own fault counter, which is certainly
  mapped and certainly not the program's, so it takes a permission fault. It waits for the thread
  to be reaped, because the teardown returns about twenty frames and `budget`, next, brackets the
  used-frame count. Under TCG with two cores the thread was still present on 22 of 24 loaded boots.
- `budget` is the claim of milestone 11 (untyped memory): a process maps until its region is spent and
  the kernel's used-frame count does not move. The process spins after its second report so the
  count holds still, and the check then kills it.
- `blockdev` has a user-level driver read `motd` off virtio disk 0, through a nifefs parse of its
  own. The hand-over's file service takes disk 1, so they do not collide.
- `deviceirq` reads the count `x86_64` took from the PIT through the IO APIC at bring-up, the one
  window with the scheduler up and the tick not yet armed. On aarch64 and riscv64 no device
  interrupts without input or a disk, so the input driver and `blockdev` prove a line there.
- `clock` and `trng` come from one JH7110 bring-up. `clock` is the clock-and-reset controller's
  before and after (milestone 220 (this kernel drives no clock or reset controller, and the first
  real device will need one)). `trng` is two different device generations drawn through the
  confined driver, timed (milestone 306 (time the hw entropy step)). The kernel never reads a
  `RAND` register.
- `entropy` draws twice from the source the hand-over would use: the JH7110 TRNG, else the CPU's
  seed instruction. Not virtio-rng, which the progenitor drives itself after the hand-over; a
  kernel-built service on the same device would fight it for the queue.

A draw passes when it is ready, two full 32-byte buffers come back, they are not all zeros, and
they differ. One draw proves only that something was returned: a stuck register file, a driver
serving its buffer twice and a device that never started all present as a repeat.

## The TRNG's numbers

The timings print in microseconds, because a tick is a different length on every board and the
interesting half of the number is an IPC round trip. The rate comes from the device tree, so there
is no constant to go stale. The bytes-per-second figure counts everything from the readiness report
to the last byte: four `entropy_contract` round trips per draw, their context switches, the
driver's poll loop and the device. It leaves out the spawn and bring-up, which print separately. So
it is not comparable to a Linux `hwrng` figure, which reads an already-running kernel driver with no
IPC in it.

`STAT.R256` says whether the TRNG gives 256-bit answers. In 128-bit mode only `RAND0..RAND3` carry
one, so a driver assembling all eight words serves 16 real bytes in 32. The driver sets
`MODE.R256` because the width a JH7110 resets to is a build-time parameter of the silicon, and the
read-back says whether it took. An all-zero `STAT` is a register window that answered nothing,
which the `clock` line explains.

## BUGS

- Nothing proves an architecture ran the right set. The list is a compile-time fact, a check
  deleted from it takes its evidence with it, and review guards the set.
- The checks cost time on every boot: `timer` waits about 20 ms and `preemption` a fifth of a
  second, and `userspace`, `budget` and `blockdev` each spawn a process and wait for it.
- `blockdev`'s driver is left running. It holds disk 0, which nothing after it uses, and
  `virtio_service::start` returns no thread to end.
- `budget`'s helper thread can be left parked on a receive with no deadline, if the process never
  reports; the check then fails by its own deadline.
- `budget` brackets the machine's used-frame count, so it is sound only while nothing on any core
  allocates or frees in its window. Each earlier check that spawns waits for its threads to be
  reaped; a check added before it that returns with a teardown in flight reopens the red
  [load-sensitive-assertions.md](load-sensitive-assertions.md) records.
- A check can still hang the boot if the operation it measures never returns. A `sched::yield_now`
  or `mmu::map_page` that never comes back is unbounded, and `arch::timer::spin_for` still hangs on
  a stopped counter. That needs a watchdog this module does not own.
