# The boot self-test's checks: what each proves, and why it is built that way

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-syscall-selftest.*

The second rung of the boot ladder runs in two halves: `kernel/src/self_test.rs` checks the
kernel's own internals, and `components/src/boot_self_test.rs` runs the checks that need a user
program. [boot-ladder.md](boot-ladder.md) has the contract: the verdict line, `absent`, the board
profiles, and why a failure prints and the boot carries on. This note carries the design of the
checks themselves, which the modules' doc comments link to. The checks are the claims the per-port tours made, run on every port (calef,
2026-10-19 UTC, on #2434), and `absent` is calef's ruling of 2026-10-20 (UTC) on #2447.

## The two halves

The kernel runs `exceptions`, `mapping`, `frames`, `timer`, `scheduler`, `preemption` and
`device_interrupt`, prints their lines and then `boot_ladder::TOUR`'s. No check there spawns a user
program or starts a service, so a kernel that leaves services to userspace keeps its self-test. The
other eight, `userspace`, `budget`, `supervised`, `teardown`, `blockdev`, `clock`, `trng` and
`entropy`, run in `boot_self_test`, which the progenitor builds before the shell.

- The tally is code both halves share, `boot_ladder::verdict`: one bit per check in three masks, and
  the line and verdict formats. The kernel writes its tally into the page `boot_ladder::kernel_half`
  lays out. The hand-over grants that page to the progenitor. The progenitor maps it read-only into
  the program, which adds its own outcomes and prints the one verdict.
- The page also carries what the program cannot learn at user level: a kernel address to fault on,
  whether the build injects, and what the JH7110 bring-up answered. `boot_ladder::self_test_program`
  fixes the program's slots, its addresses, the image table and its two messages.
- The program cannot read the archive, so the progenitor measures `kernel_test_subject`,
  `memory_region_depleter` and `block_driver` and copies them in. The program builds each child
  from a budget of its own, supervises it, and collects or destroys it before the next check.
- The program writes each line into a transcript page and tells the progenitor where it ends. The
  progenitor can print only once the shell's output page is mapped. So the transcript and the
  verdict reach the console after the tour line, between the hand-over's lines and the prompt.
  `serial_console`'s `has_reached` waits for the verdict line itself, and `boot-check` requires it.
- The verdict reaches a screen the kernel still paints, aarch64 and riscv64 with a `ramfb`. The
  progenitor's console reaches the UART alone there, so the program sends its tally with `DONE`,
  packed into one word (`Tally::to_packed`). The progenitor sends that word on an endpoint the
  hand-over grants only on a machine with a screen, and then deletes it. A kernel thread takes the
  one message, renders the verdict with `boot_ladder::verdict`, and paints it on the screen alone,
  since the UART already has the program's copy. Nothing the progenitor sends is painted as text.
  On `x86_64` the screen is the shell's terminal by then, and the verdict reaches it as every
  console line does. The architect ruled on this path on 2026-11-11 (UTC).
- Before a check takes the death of a child it kills on purpose, the program names the child's
  thread in a `boot_ladder::SELF_TEST_SUBJECT` line, in decimal, as the kernel's kill report
  prints it. `userspace`, `supervised` and both rounds of `teardown` do so. `swish-check` removes
  exactly those threads' kill reports and fails every other kill after the hand-over, as the
  architect ruled on 2026-11-11 (UTC).
- A boot that cannot reach the program still prints a verdict. With no archive or no progenitor, the
  kernel prints one that fails the eight by name. A program that dies before its verdict makes the
  progenitor say so. That boot has no `nife self-test:` line, and `boot-check` fails it.

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

The feature `self_test_injection` makes one check in each half fail, `exceptions` in the kernel
and `userspace` in the program, which reads it from the kernel's half. It also stops the counter
the kernel's checks read, so `cargo xtask boot-check --inject` can watch the gate go red, and it
requires both names. A verdict that has never been red
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
  direct map's last byte, past anything `mmu::initialize` claimed and in the kernel half on all
  three. A constant there was the third QEMU-shaped address the VisionFive 2 caught in one bench
  session. It uses `phys_to_virt` because the three architectures spell the direct map differently.
  On failure the frame is leaked, because one whose mapping state is unknown must not be reused.
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
- `userspace` has `kernel_test_subject` read the kernel's own fault counter, at the address the
  kernel's half carries. That page is certainly mapped and certainly not the program's, so it takes
  a permission fault. Its death must reach its supervisor naming that address, and its corpse must
  be collected before `budget` brackets the free-frame count.
- `budget` is the claim of milestone 11 (untyped memory): a process maps until its region is spent and
  the machine's free-frame count, read from the machine statistics page, does not move. The process
  spins after its second report so the count holds still, and the check then destroys its region.
- `supervised` builds `kernel_test_subject` in a region of its own with its supervision endpoint in
  the reserved fault slot, so `START` makes it supervised (DECISIONS §26 (the fault endpoint)). The
  child loads from an address nothing maps. The check waits for the endpoint's survey to list the
  child dead. It does not wait on a receive, because a receive has no deadline. A corpse parked as
  the endpoint's sender then makes the receive return at once. The message must say `EVENT_FAULT`
  and name the child, a pc inside its executable segment and the address, and the supervisor reaps
  the corpse with its region. It reads the thread id from the survey, since the program never sees a
  TCB's.
- `teardown` runs `supervised`'s round twice and judges the second: the machine's free-frame count
  must not move across a spawn, a death and a reap. A first round pays first-use carves that are
  not leaks (a kernel stack the recycler has not held yet), so one before-and-after over a single
  round cannot tell those from a leak. Under QEMU both rounds cost zero, because `supervised` has
  already paid them.
- `blockdev` has a user-level driver read `motd` off virtio disk 0, through a nifefs parse of its
  own. The hand-over sets disk 0's confined transport, interrupt and DMA page aside for the program,
  and the file service takes disk 1, so they do not collide. The driver exits once it has reported,
  and the program collects it.
- `device_interrupt` reads the count `x86_64` took from the PIT through the IO APIC at bring-up, the
  one
  window with the scheduler up and the tick not yet armed. On aarch64 and riscv64 no device
  interrupts without input or a disk, so the input driver and `blockdev` prove a line there.
- `clock` and `trng` come from one JH7110 bring-up. `clock` is the clock-and-reset controller's
  before and after (milestone 220 (this kernel drives no clock or reset controller, and the first
  real device will need one)). `trng` is two different device generations drawn through the confined
  driver, timed (milestone 306 (time the hw entropy step)). The hand-over brings the controller up
  and wires the TRNG's service. It records what both answered in the kernel's half and grants the
  service's endpoint for the program to draw from. The kernel never reads a `RAND` register.
- `entropy` draws twice from the JH7110 TRNG where the tree names one, else from the progenitor's
  own entropy service, whatever it draws from: the CPU's seed instruction or a virtio-rng device.

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
read-back says whether it took; nothing has ever read it on hardware. An all-zero `STAT` is a register window that answered nothing,
which the `clock` line explains.

## BUGS

- Nothing proves an architecture ran the right set. The list is a compile-time fact, a check
  deleted from it takes its evidence with it, and review guards the set.
- The checks cost time on every boot. `timer` waits about 20 ms and `preemption` a fifth of a
  second. The progenitor builds `boot_self_test` before the shell, and it spawns five processes and
  waits for them.
- `budget` and `teardown` bracket the machine's free-frame count, so each is sound only while
  nothing on any core allocates or frees in its window. Each earlier check collects its children
  before it returns; a check added before them that returns with a teardown in flight reopens the
  red [load-sensitive-assertions.md](load-sensitive-assertions.md) records.
- The hand-over, not userspace, starts the JH7110 TRNG's service and sets disk 0 aside. A TRNG that
  never reports ready leaves a kernel thread parked on the receive. Milestone 811 (the boot services
  leave the kernel) moves those starters out.
- `trng` and `entropy` draw with calls that have no deadline, so a service that never answers holds
  the boot before the shell.
- `supervised` checks that the faulting pc lies in the child's executable segment, not which
  instruction it was.
- A check can still hang the boot if the operation it measures never returns. A `sched::yield_now`
  or `mmu::map_page` that never comes back is unbounded, and `arch::timer::spin_for` still hangs on
  a stopped counter. That needs a watchdog this module does not own.
