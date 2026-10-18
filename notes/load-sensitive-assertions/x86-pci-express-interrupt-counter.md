# The x86_64 PCIe interrupt counter, 2026-09-04

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## A counter that could not have been late, only sampled late: 2026-09-04 (the x86_64 MSI-X flake)

`user::tests::a_userspace_driver_reads_a_file_over_the_pcie_transport` failed three times in two
days on the `x86_64` leg. Twice it cost a lane a `script/test`, and once it evicted PR #734 from the
merge queue. The shape was always the same. The first assertion passed: the driver read the right
bytes off the `virtio-blk-pci` disk and reported them. The second did not.

```
[PANIC] panicked at system_tests/src/user/tests.rs:2061:5:
the read completed but the device's interrupt was never delivered to this kernel
```

The proposal that asked for this work named two hypotheses with opposite fixes: the interrupt is
genuinely lost, or the counter is read before it lands. It is the second. The tree can be read for
the answer rather than raced for it, which is unusual for a family that usually has to be measured.

### The chain that settles it

Three links, none of them timing-dependent:

1. `Irq::WAIT` is not a poll. `syscall::irq_wait` is `sched::irq_route(intid)` followed by
   `sched::ipc_receive(ep)`. It returns only when that endpoint is signaled or has a counted pending
   signal, and both come from `sched::irq_notify`.
2. Every architecture's trap handler bumps `ROUTED_IRQS` immediately before calling `irq_notify`,
   never after (`arch/aarch64/exceptions.rs`, `arch/riscv64/exceptions.rs`,
   `arch/x86_64/exceptions.rs`).
3. `crates/virtio`'s `complete_block` cannot leave its loop without a `WAIT` returning. The driver
   cannot reach its `send` without `complete_block` returning, twice.

So a report on the wire proves that `ROUTED_IRQS` has already moved. The interrupt cannot be lost in
a way that produces this failure. A lost interrupt leaves the driver blocked in `WAIT` forever, so it
never reports, and the leg dies at the QEMU watchdog with no output from this test at all. It does
not come back with the right bytes and a still counter. The only remaining way to observe a zero
delta is a baseline sampled after the increments, and the test sampled it in exactly the wrong place:

```rust
let Some(report) = virtio_service::start_pci(init_image()) else { ... };

let irqs_before = ROUTED_IRQS.load(Ordering::Relaxed);   // <- after the driver is spawned
let word = sched::ipc_receive(report)[0];
```

`start_pci` ends by spawning the driver. Suppose a tick preempts the test thread after that spawn,
and it does not get the CPU back before the driver has finished both reads and reported. Then
`irqs_before` is sampled past both completions, `ipc_receive` returns the already-queued report at
once, and the delta is zero. The window looks like a handful of instructions when you read it, and
it is not. The measurements below show the sample point drifting by tens of milliseconds from run to
run.

### The direction, in this register's vocabulary

The main page's first diagnostic sorts failures by direction. This one is the negative family (*"the
assertion was written against something wider than the property"*) seen from its mirror. Nothing
arrived from outside the measured window. The property's own evidence fell before the window opened,
because the window opened late. Load makes it likelier for the ordinary reason: an oversubscribed
host lets more of the guest's work fit inside one preemption.

### What was measured

The flake did not fire in 23 executions. `script/repeat-under-load -n 12 -- --arch x86_64` ran on
patagonia with 8 spinners, at a one-minute load average between 12 and 91. It was 11 of 12 green.
The one red was a different, already-recorded defect: the UEFI tour's two-core AP bring-up
(`ap_boot`'s BUGS entry 1, `smp: cpu 1 did not start (firmware returned -1)`). This test passed every
time.

What the same runs did show, on every one, is the race made visible. A temporary diagnostic sampled
`ROUTED_IRQS` three times: on entry to the test, at the old baseline, and at the assertion.

```
[DIAG] pcie read: entry=544 before=545 after=549
[DIAG] pcie read: entry=884 before=886 after=890
[DIAG] pcie read: entry=893 before=897 after=899
[DIAG] pcie read: entry=554 before=557 after=560
```

Two numbers matter. One to four interrupts always landed between the test's entry and its old
baseline. So the sample point drifts by tens of milliseconds run to run: the test thread does not
hold the CPU across `start_pci`, and how much ground it loses varies. And the window the assertion
actually measured got as narrow as two, which is the device's own completions and nothing else. The
margin the test was passing on was between zero and three counts wide, and both ends of that
distribution are the same thing moving.

### The control, which is what a rare failure can be given instead of a red run

A window that a preemption opens by luck can be opened on purpose. The baseline was left where it
was, and 20,000 `yield_now()` calls were inserted between `start_pci` and the sample. The test fails
on the first try, with the field's panic verbatim:

```
[DIAG] pcie read: entry=43 before=51 after=51
[PANIC] panicked at system_tests/src/user/tests.rs:2071:5:
the read completed but the device's interrupt was never delivered to this kernel
```

`before` is eight counts past `entry`, and `after` equals `before`. The driver's completions were
counted, then the baseline was taken, then the already-queued report came back at once. Then the
baseline was moved ahead of `start_pci` and nothing else changed, with the same 20,000 yields still
in place. It passes (`entry=43 before=43 after=50`). That is the hypothesis chosen twice: once by
reading the chain, once by making the window wide enough to see.

### The acceptance run

`script/repeat-under-load -n 12 -- --arch x86_64` ran again on the fixed tree: the same host, the
same eight spinners, a one-minute load average between 4 and 40. It was 12 of 12 green. That is not
proof the flake is gone, and this register has said so before about greener numbers. The proof is the
control above and the ordering the fix rests on. The run confirms the two changes broke nothing else
on the leg that carries them.

### The other half: `ROUTED_IRQS` did not mean the same thing here

Chasing this showed why the margin existed at all. On aarch64 and riscv64 the counter is bumped only
in the branch that found a userspace endpoint for the interrupt. On x86_64 it was also bumped by the
local APIC timer, by the reschedule IPI, and by device vectors nothing had claimed. So on this
architecture, a timer tick satisfied a test asserting that a device's completion reached the kernel,
most of the time. It failed when no tick happened to land. That is not a stricter test failing
occasionally. It is a weaker one, passing for the wrong reason.

It matters for the fix as well as for the record. Sampling the baseline earlier, on its own, would
have widened the window until a timer tick was guaranteed. That makes an assertion green by giving it
more of something unrelated. Milestone 62 (tests that assert on time) forbids that by name, and
§61 (a lint is adopted on evidence) records three lints dropped for the same shape. So both went in
together, and `arch::x86_64::exceptions::ROUTED_IRQS` now counts what its two siblings count.
`arch::timer::ticks()` already counted the ticks the bring-up tour prints, and
`DEVICE_IRQS`/`SPURIOUS_IRQS` already counted the rest, so no number was lost.

Two `sched` tests read this counter through the same portable name
(`an_interrupt_becomes_a_message`, `an_interrupt_that_arrives_before_the_wait_is_not_lost`). Both
were vacuous on x86_64 for the same reason: a timer tick could satisfy a `spin_until` waiting for a
routed test interrupt. They are real on all three architectures now, without their own text changing.

### The verdict

Both halves are ordering arguments rather than margins, which is what this register keeps asking for.

- The baseline moves to before `virtio_service::start_pci`. Until it runs there is no queue, no route
  and no driver, so nothing this device does can be counted before that line. No wait was added,
  because there is nothing to wait for. The increment is upstream of the wakeup, which is upstream of
  the report.
- The assertion stays. It is now implied by the first one rather than racing it, and that is the
  reason to keep it: it guards the implication. A driver rewritten to poll the used ring breaks the
  chain, and so does a handler that counted after `irq_notify` instead of before it; either way this
  fires. The first assertion alone would pass on a polled driver, which is the
  confinement-adjacent failure this test exists to catch.

The failure message now carries the two numbers, so a recurrence arrives as data rather than as a
mystery. If this ever fires again, it will say how many interrupts were routed before the device was
wired and how many after the report. Given the chain above, a delta of zero would then mean the chain
itself is broken: a driver completing without an interrupt, the confinement-adjacent failure worth
waking up for.

The same one-line move went into the three sibling tests with the identical shape. They are
`a_userspace_driver_reads_a_file_from_a_virtio_disk` here and its riscv64 twin, and the riscv64 pcie
twin. None was ever observed to flake, and all had the same window.

### BUGS

`ROUTED_IRQS` is still a global, and the assertion is still not device-specific. It says an
interrupt became some driver's message in this window, not that this device's completion did. The
ordering argument above makes it sound for this test. It would stop being sound if a second driver
were running concurrently in the same window. Saying exactly the narrow thing wants a per-intid
delivery count. That cannot simply live in `sched::irq_route`, because `syscall::irq_wait` and
`soak.rs` both call it outside the handler, so counting there would count lookups rather than
deliveries. So it wants the three arch handlers to route through one portable call. That is a
refactor rather than a patch, and it is not done here.
