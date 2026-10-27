# Can a userspace process hold a timer?

*(Written 2026-09-05 for milestone 263, a spike. This note prices a design and does not build one:
nothing here adds a syscall or an object, and the fork it informs is an architect's. Name
provisional: `timer-capability.md` is a sibling of `timed-wait.md`
rather than a second copy of it, and the two answer different halves of it.)*

*Since then, verified 2026-10-11 (UTC): the fork was decided as §147 (a timer a userspace service
cannot hold), and milestone 151 (notification objects) is built. Milestone 106 (a wait that ends on
either the interrupt or the deadline) built `objtype::TIMER` on 2026-09-26 on all three
architectures. [timer.md](timer.md) has what was built. This note stays as the pricing record, and
it describes the tree of 2026-09-05 and 2026-09-13.*

The answer, first. The userspace-timer-service answer to milestone 106 does not survive
[§19 (architectural)](../design/decisions/0019-architectural-parity.md) parity, and the reason is not the one the
milestone block predicted.

Two clauses, and both are needed:

1. On riscv64 no *architected* timer can be granted to U-mode, on any machine, by the privileged
   architecture rather than by a gap in this kernel. There is no U-mode timer-compare CSR, no enable
   bit that would create one, and no route to SBI from U-mode.
2. The only mechanism that works on all three is per-board MMIO, and it is not uniformly
   available. The two real boards have generous MMIO timer blocks (argon fourteen channels, radon
   four, each with its own interrupt line). But QEMU's aarch64 `virt` has no MMIO timer device at
   all, and that is the machine every gate in this tree runs on. So a userspace timer service would
   be a different driver per board, absent on the aarch64 CI machine, which is §19's own definition
   of the bug rather than a parity story.

The correction worth having is on the third architecture the block wrote off. aarch64 can grant a
timer comparator to one thread rather than to all of EL0, using machinery this tree built for a
different reason three days ago.

## What each specification actually says

Three specifications, read rather than recalled, because milestone 263's own `BUGS` section says the
table it was scoped from is this tree's doc comments and *"the maintainer's reading is the thing most
likely to be wrong here."* It was wrong about one of three. The register fields, the quoted text and
the code behind each verdict are in [specifications.md](timer-capability/specifications.md).

- aarch64: the block's negative is refuted. `CNTKCTL_EL1.EL0PTEN` (bit 9) opens the physical
  timer's comparator to EL0, and this kernel has only ever written bit 1. The grant can be per
  thread. §139 (who may read the cycle counter) says the tree has no per-thread system-register bit
  kept across a context switch, and milestones 229 and 237 built exactly that, so the sentence is
  stale inside its own decision. What is scarce is comparators. There is exactly one spare, the
  physical timer, and it traps under a hypervisor, which is why the tick owns the virtual one.
- riscv64: the block's negative is confirmed, and it is total. Sstc's every enable gates S-mode
  and VS-mode only, `stimecmp`'s CSR address makes it supervisor-only regardless, and a U-mode
  `ecall` never reaches SBI. The one comparator per hart is already the kernel's tick.
- x86_64: the HPET is memory-mapped with several comparators, so it could be handed to a process
  with the existing `DeviceFrame` capability. QEMU q35 has one. Whether xenon has one is not
  recorded anywhere in this tree.

## The other route: an MMIO timer the kernel already knows how to delegate

An MMIO timer block is a `DeviceFrame` plus an `Object::Interrupt`, which is how every userspace
driver
here already owns a device, so a machine with a spare one can give it to a userspace service today.
argon has fourteen channels and radon four, each with its own interrupt line. xenon's HPET is
presumed and not confirmed. QEMU's aarch64 `virt` has none, and its riscv64 `virt` has only the
goldfish RTC's single alarm. The table and its sources are in
[mmio-timers.md](timer-capability/mmio-timers.md).

Why this route does not rescue the userspace-service answer. It is a per-board driver rather than
a portable capability, so §19's *"a kernel capability ships on every supported architecture, proven by
the same suite"* is not met by it: the suite runs on QEMU, and QEMU's aarch64 `virt` has nothing to
drive. A userspace timer service would exist on argon, exist differently on radon, exist a third way
on xenon, and not exist at all on the machine this tree gates against. That is three drivers and a
hole, against one syscall method.

It is a good answer to a different question. If a *particular* workload on a *particular* board
wants a private high-resolution timer, this is how it gets one, and it needs nothing built. It is not
how a kernel serves `thread::sleep`.

## What the fourth shape costs

`Timer::ARM(deadline, notification)` -> the kernel signals that notification at the deadline.

The deadline bookkeeping was already priced in `notes/timed-wait.md` and is not re-measured: one
comparison per idle tick and a per-thread deadline at zero bytes. What was unpriced is the object.
A throwaway scaffold built it on 2026-09-05, passed `script/test` on all three architectures, and
was deleted. Three figures came out of it:

- The object is free in bytes. `size_of::<Object>()` and `size_of::<Cap>()` do not change, so a
  `Timer` capability costs nothing per slot, per capability table or per TCB.
- `ipc_fastpath` does not move on any architecture. `syscall_entry` grows by 12 bytes on aarch64,
  158 on riscv64 and 96 on x86_64, because one more `Object` arm is one more decode step for every
  syscall.
- The kernel grows by under a quarter of a per cent: 738 bytes on aarch64, 2,266 on riscv64, 1,386
  on x86_64.

The scaffold used the crudest expiry table, built no revocation, measured nothing in time and
priced no kernel-side consumer. The method, the tables and those gaps are in
[first-scaffold.md](timer-capability/first-scaffold.md).

## What the second scaffold priced, 2026-09-13

A second scaffold, page-resident like a real implementation, priced what the first left out. It
was compiled on all three architectures and deleted, never run. The reasoning, the four builds and
the symbol-level working are in [second-scaffold.md](timer-capability/second-scaffold.md).

- The holder dying with a timer armed costs 268 bytes of code on aarch64, 212 on riscv64 and 276 on
  x86_64, and zero data. It is not deferrable bookkeeping. Without it the timer interrupt reads and
  writes a freed page on every core, a use-after-free with no syscall involved. The fix is the
  sweep `sched::reap_region_objects` already runs for rendezvous, run against the timer table.
- Serving a kernel thread costs 312, 282 and 400 bytes of code and zero data. A kernel thread cannot
  issue a syscall, so it needs something smaller than the fourth shape: a deadline word on the TCB
  and a walk that wakes it. Applied to `kernel/src/soak.rs`'s supervisor it builds clean; nothing
  was run.
- The error bar is zero bytes: a rebuilt build came out byte-identical. `ipc_fastpath` did not move,
  reproducing the first scaffold.
- One finding is not about timers. Growing `IpcTables` made riscv64 alone materialize a 25,680-byte
  `.rodata` template, so the same source cost it 37,061 data bytes against aarch64's 10,980. It is a
  cliff rather than a slope, and it prices every future addition to the scheduler's tables. It also
  makes `MAXIMUM_TIMERS` a real decision rather than a constant copied from `MAXIMUM_RENDEZVOUS`.

## The dependency on milestone 151, stated

The fourth shape signals a notification, and notification objects are
[§101 (notification objects)](../design/decisions/0101-notification-objects.md), decided 2026-08-20 and unbuilt when this
was written (milestone 151, BUILT since; see the note under the title). What the pricing above
assumes about it:

1. That the signal target is a `Rendezvous`, not a separate object. The scaffold signaled through
   `sched::interrupt_notify`, which takes a `RendezvousId`, because that is what exists today. §101's whole
   argument is that a notification should be *its own object with its own queue*, separate from the
   endpoint. If 151 builds that, `Timer::ARM`'s second argument names a `Notification` and not a
   `Rendezvous`, and the dispatch arm's capability check changes shape. The byte figures do not
   move much; the syscall's meaning does.
2. That binding to a TCB is what makes the shape useful. Milestone 106's title is met only if a
   thread blocked in `RECEIVE` on an endpoint wakes on *either* a message or the deadline, and §101 says
   that is what TCB binding is for. Without 151 the fourth shape gives a thread a timer it can block
   on and no way to block on a timer *and* a message at once, which is milestone 106's actual
   complaint. So the fourth shape is not independently useful: it is 151 plus one object.
3. That §101's carve-out still stands. §101 anticipated a userspace timer process and named a
   kernel timed wait as the alternative for kernel needs. This spike's finding is that the userspace
   process cannot exist on riscv64, which moves the fourth shape from "an option nobody listed" to
   "the only one of the four that is buildable on all three architectures". That is a finding for
   calef, not a recommendation from this lane.

## BUGS

- The 2026-09-13 scaffolds were never run, not even under QEMU. The 2026-09-05 one was gated
  green with `script/test` before it was deleted; these three builds were only *compiled*, on all
  three ISAs and with `--features soak_test`. So every figure here is a size, and nothing on this page is
  evidence that a timer fires, that a sleeping kernel thread wakes, or that the reap sweep runs at
  the right moment. A byte count is the cheapest half of a pricing and it is the half that was
  bought. Running them is perhaps an hour and would turn "it compiles" into "it works".
- The kernel-side pricing measures the cheapest correct shape, which may not be the one wanted.
  `sleep_until` parks on no rendezvous with a sentinel, wakes directly, and returns nothing, so a
  kernel thread woken this way cannot tell a deadline from an abort. Every kernel consumer beyond a
  watchdog (§101 names a scheduling deadline and an in-kernel retransmit) plausibly wants more than
  that, and more than that was not priced.
- The reap sweep was priced, not proved. There is no test that destroys a region holding an
  armed timer, which is precisely the use-after-free the second-scaffold summary above says the sweep exists to
  close. Whoever builds this owes that test before the sweep is believed.
- `MAXIMUM_TIMERS = 512` is this lane's guess and it drives the only figure here that is large.
  See the `IpcTables` finding above.
- Nothing here was run on hardware. The aarch64 refutation is a register specification plus this
  tree's own code, not an EL0 program that armed `CNTP_CVAL_EL0` and took the interrupt. The cheap
  version of that experiment (set `EL0PTEN`, have a user program write the comparator, see whether
  INTID 30 arrives) was not run, and it is the thing that would turn this section from a reading into
  a measurement. Under a hypervisor it is *expected* to fail, for the reason milestone 9 recorded.
- The Tegra X1 TRM was not read. NVIDIA's download redirects to a developer login. The argon row
  is mainline Linux's devicetree binding and DTS, which NVIDIA's own maintainers wrote, so the channel
  count and the fourteen SPIs are solid; anything register-level is not established here.
- The JH7110 timer has no mainline driver and this project has not driven it. Four channels with
  four PLIC lines is what the TRM says; nothing in this tree has touched the device.
- The Arm citation is a rendering of Arm's machine-readable system-register description, not the
  Architecture Reference Manual PDF. developer.arm.com's register pages render their content in
  JavaScript and returned no field text to a fetch. The rendering used is generated from Arm's own
  published XML and agrees with this tree's existing `EL0VCTEN`-is-bit-1 fact, but it is a secondary
  source and is marked as one.
- xenon's HPET is unconfirmed. See the x86_64 section of
  [specifications.md](timer-capability/specifications.md). Reading it needs either the first-light
  photograph transcribed or the boot tour re-run with the table list captured, and neither was done
  here.
- The "one spare comparator" claim for aarch64 is about EL1/EL0 only. `CNTHP_*`, `CNTHV_*` and
  `CNTPS_*` exist at EL2 and EL3 and are not this kernel's to give.
- The scaffold is gone. The variant, the `abi::timer` module, the retype arm, the dispatch bodies,
  the expiry table and the tick hook were built to obtain the numbers and deleted. Rebuilding them is
  an hour; shipping them would have settled a syscall-surface fork by accident, which is what
  milestone 106's lane refused to do and what §10 (process) and §16 (revocation) reserve to an
  architect.
- **`Timer`, `Timer::ARM` and `Timer::CANCEL` are the milestone block's provisional coinages**, minted
  so the measurement could exist. Names are an architect's (§75 (provenance)).
