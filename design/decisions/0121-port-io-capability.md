---
status: AMENDED
raised: 2026-08-23
decided: 2026-08-25
ratified_by: calef
---

# 121. What a device capability is when the device has no page: x86 port I/O

Reopened and reversed 2026-09-15. The original ruling (calef, 2026-08-25, *"Ratify option 2
permanently"*) stood on a premise the boot-to-a-prompt goal has since falsified; calef reversed it to
option 1, the port-range capability, on 2026-09-15, on this file's own named trigger. The
reasoning below is kept as the record of what was decided and why it held while it held; the reversal
is the last section. What a capability *is* is the centre of this system's claim, not an
implementation choice a lane makes on the way past.

What is blocked: nothing today, and one thing soon. Milestone 161's boot is entirely in ring 0,
so the kernel drives COM1 directly and no grant is needed. What is blocked is a userspace console
or input driver on x86, which is the arrangement both other architectures already have and one of
the things "parity" means (DECISIONS §19). `user::UART_PHYS` is zero on x86_64 today, and that
zero is a marker for this file rather than an address.

## The rule this collides with

Moved to [`rule-and-options.md`](0121-port-io-capability/rule-and-options.md): the device-is-a-page
rule, and why x86's legacy devices break it.

## What x86 actually provides

Two mechanisms, and only one has usable granularity.

- `RFLAGS.IOPL`, two bits in the flags register: ring 3 may use `in`/`out` on *every* port, or on
  none. All-or-nothing. Granting it is granting the whole machine, including the interrupt
  controller and the CMOS clock, which is not a capability in any sense this project means.
- The TSS I/O permission bitmap: one bit per port, 8 KiB for the full 64 Ki ports, at an offset
  the TSS names. The CPU consults it on every `in`/`out` from ring 3 and faults when the bit is set.
  This has exactly the right granularity, and it is the only thing that does.

The bitmap's awkwardness, and the reason this is a decision rather than a task, is that it is
**per-task state, not per-address-space state**. A page mapping lives in the address space and
travels with it; the I/O bitmap lives in the TSS, and the TSS is per-CPU. So a thread's port
rights have to be re-established on every context switch that changes which thread is running, on
whichever CPU it lands on. That is a cost and a synchronization problem the mapping model does not
have.

## The options

Moved to [`rule-and-options.md`](0121-port-io-capability/rule-and-options.md): option 1, a
port-range capability; option 2, legacy devices stay in the kernel; option 3, an I/O-port service.

## The decision: option 2, permanently

**x86's legacy port-I/O devices, the console included, stay kernel-resident. This is not an interim
stance to be revisited on a schedule; it is a permanent architectural boundary.** Only memory-mapped
devices get a userspace driver on x86 -- which is everything on the machine that matters to
this project's own ranking function. Milestone 87's real target (a Dell OptiPlex 7050 Micro with the
Dell C4PDJ serial module, already on calef's desk) almost certainly exposes its serial port through
the traditional Super I/O / COM1 legacy interface rather than a memory-mapped UART. True MMIO UARTs
are rare outside embedded and server hardware. Nobody has checked the module's own datasheet to
confirm this, so it is stated as inference from general PC architecture, not a verified fact. But it
is the reason this reads as a permanent property of x86 legacy serial rather than a QEMU emulation
artifact: real hardware likely has the identical shape.

Nothing on the actual customer path is affected. Every device the Time Machine backup-server
thesis touches -- network, NVMe/disk, everything SMB needs -- is already PCI/PCIe, already
memory-mapped, and already gets the full userspace-driver treatment identically on all three
architectures under the existing mapping-based capability model. The gap this decision accepts is
confined entirely to a debug/developer serial console a customer never interacts with.

The measured cost (below) supports closing this rather than leaving it open. ~2,682 ns per context
switch, a 423% overhead on the naive always-write implementation. It was refined to note a
lazy/conditional write would cost far less but still needs real, currently unbuilt engineering with
no present motivation to build it. That is exactly the kind of cost this tree measures rather than
argues, and the number argues for the status quo.

This is the same posture DECISIONS §19 already accepts elsewhere in this tree: a documented scope
note is a legitimate answer to a parity gap, not a failure to work around. An honest, recorded
exception is worth more than an overclaimed parity, which is this project's own stated culture.

<!-- writing-standards: exception. 2026-10-18 (UTC), sentence-split lane. Reason: the next
     sentence is 47 words, and the "Reopened" section quotes it verbatim. -->
**The reopening trigger, named so this does not get revisited out of habit**: if a real userspace
console on x86 ever becomes something calef actively wants demonstrated, rather than something the
other two architectures merely happen to have, that is what justifies building option 1 for real. At
that point the number to get first is the lazy/conditional write's actual cost (see "Refined
2026-08-25" below), not the naive always-write number already measured here, since that number prices
the wrong implementation.

**This was not a lane's call, and still is not retroactively**: option 1 would have added an object
type to the capability surface and option 2 permanently declares a parity gap. Both are calef's
under the tenets: the system call surface is a boundary, and a parity gap needs a recorded plan rather
than a silence. That recorded plan is exactly what this decision now is.

### Amended 2026-08-24: option 2 now, but not argued blind against the alternatives

calef asked whether option 2 can be built so there is something to measure against if a later call
goes to option 1 or option 3, rather than deferring the whole question to argument again when it
comes back. It can, and neither half needs the feature it is pricing.

**Option 3's cost is already on record, just never cited here.** The dominant cost of an I/O-port
broker is one IPC round trip per register access. This kernel's own IPC round trip is measured: ~337
ns (release, HVF, aarch64), against a null system call at ~27 ns, both from `notes/benchmarks.md`'s
cross-OS table. Neither number is x86-specific, and citing it here is not a claim that it transfers
unchanged to different silicon; it is the order-of-magnitude confirmation this file's own "obviously
wrong for a UART" already asserted without a number. A raw `in`/`out` instruction is single-digit
cycles; hundreds of nanoseconds per register access, on a device a driver polls in a tight loop, is
the gap that makes option 3 wrong regardless of which architecture supplies the exact figure.

**Option 1's cost is not measurable yet, and the reason is narrower than "nothing runs in ring 3".**
The dominant cost is the TSS I/O bitmap write on every context switch, and that does not need the
capability type built to measure. A micro-benchmark that writes an 8 KiB bitmap into the current
CPU's TSS on every switch, timed against a switch that does not, is a far smaller thing than option
1 itself. It is in the same shape `script/bench`'s existing icount-based measures already take. What
it needs that does not exist yet is real context switching between two threads on x86_64. That is
downstream of ring 3 (this milestone's item 3, in a lane as of 2026-08-24) but not necessarily
delivered by it: that lane's own proof may run only one program in ring 3, not switch between two.
Whether it does is this decision's own next input, not something to guess now.

**So the plan, not just the recommendation:** option 2 stands. Once x86 has two threads actually
switching, a small follow-on benchmark (unscoped as its own milestone; whoever picks it up mints the
number) measures the TSS-bitmap-write cost the same way `ipc_rtt` and `null_system_call` already measure
theirs. That turns the next 1-vs-3 call into one made on both sides' numbers, not one side's.

### Measured 2026-08-24: the write costs more, not less, than the prose above guessed

Milestone 161 item 4 landed real two-thread context switching on `x86_64` this session, so the
precondition above is met. `tss_iomap_switch` (`kernel/src/bench.rs`) is `yield_switch` with one
`arch::segments::bench_write_io_bitmap` call added on every switch-in, in both threads. That call is
a real 8192-byte write (`65536` ports `/ 8`, the exact size, not the round "8 KiB" this file used
loosely), into a CPU-owned static, never wired to the live TSS's `iomap_base`. It measures the write
option 1 would add, not the enforcement. Full methodology and five-run medians for both a debug and
a release kernel: `notes/benchmarks/x86-tss-iomap.md`, "2026-08-24: the TSS I/O-bitmap switch cost".

No icount leg exists on this ISA (`icount()` already refuses `--arch x86_64`, and this port's runner
attaches no image to pin QEMU's virtual clock to). So every number below is plain TCG on this Apple
Silicon host: no KVM, no HVF for `x86_64`, statistical, and not a stand-in for real x86 silicon
cycle counts. `cargo xtask bench --x86` runs it and refuses `--check`/`--save` for the same reason.

| build | `yield_switch` (bare switch) | `tss_iomap_switch` (switch + write) | delta | per 8192-byte write | overhead |
|---|---|---|---|---|---|
| debug, median of 6 | 12,320 ns/iter | 15,360 ns/iter | 3,040 ns/iter | ~1,520 ns | +25% |
| release, median of 5 | 1,267 ns/iter | 6,769 ns/iter | 5,363 ns/iter | ~2,682 ns | +423% |

The debug row is the reassuring one and the release row is the honest one, and they disagree by 17x
on "how bad is this." A debug build's baseline switch carries so much fixed cost (unelided checks,
unoptimized bookkeeping) that the write is a modest ~25% addition to a slow number. Strip that in
release and the baseline switch itself gets ~10x faster while the write's own cost barely moves,
both being close to plain memory bandwidth. On the switch path option 1 would run on, the
write does not add a quarter of the cost; it is most of the cost. That is a stronger statement than
this file's own prose made before there was a number to check it against: "the dominant cost"
undersold it. Correcting the record rather than the prose that was already qualified: nothing above
was wrong, the amendment already named this the dominant cost from architecture alone; the number
says the margin is wider than "dominant" implied.

**This does not change the recommendation.** Option 2 still stands for the reason it always did.
Nothing today asks for a userspace console on `x86_64` specifically, and option 1's cost, now
measured rather than assumed, is exactly as unattractive as the prose already argued, arguably more
so. What changes is that the next 1-vs-3 call is now made on both sides' numbers: option 3's ~337 ns
per IPC round trip (already on record above) against option 1's ~1.5-2.7 us per switch for the write
alone. That figure is before the capability type, the revocation shootdown, or anything else option
1 would cost. Whether that gap is worth paying for a userspace `x86_64` console is still
calef's call, per this file's own closing question.

### Refined 2026-08-25: the number prices the naive write, not the write option 1 would actually ship

calef and the maintainer read the measured table above together and flagged a gap in what it prices.
`tss_iomap_switch` writes the bitmap on *every* switch, unconditionally, for both threads, whether
either one holds a port capability or not. That is not how the real OS this decision keeps citing as
precedent, Linux, actually does it. `ioperm`/`iopl` give a thread its own I/O bitmap only if it asks
for one. The TSS's `iomap_base` on switch-in is set to "no bitmap" for every thread that never has,
so the write is paid only when a thread that actually holds port permission is on either side of the
switch. On a system where realistically one process would ever hold a port capability if option 1
were built (the console driver), that is the overwhelming majority of switches paying nothing and
one rare case paying the measured cost. It is not every switch paying it.

**So the table above is a confirmed worst-case upper bound, not the number option 1 would actually
cost in a system that implemented the lazy version.** It does not change today's call: option 2
still costs nothing and nothing today asks for the alternative, so there is nothing to trade the
measured cost against yet. It changes what the *next* measurement should be if this decision is
revisited seriously. That is the lazy/conditional write's real cost, not a second run of the same
always-write benchmark. The lazy write needs a per-thread "does this thread hold any port
capability" bit checked on switch-in rather than an unconditional write. Recorded here so a future
reader prices option 1 against the write it would actually pay, not the one this file happened to
measure first because it was the smaller thing to build.

## What answered it

The question this file closed on was never technical: **is a userspace console driver on x86 part of
what the demonstrator claims, or is "userspace drivers, demonstrated on two architectures, with the
third's legacy serial port a recorded exception" enough?** calef answered the second, permanently,
once the cost of the alternative was measured rather than assumed. Option 2 is the scope note, and it
is this decision's own deliverable.


## Reopened 2026-09-15: the trigger this file named has fired, and the call is option 1

This file wrote its own reopening condition, and it is now met. The "reopening trigger" section
above said, verbatim: *"if a real userspace console on x86 ever becomes something calef actively wants
demonstrated, rather than something the other two architectures merely happen to have, that is what
justifies building option 1 for real."* Two changes since 2026-08-25 met it exactly:

- The customer path the original ruling leaned on is gone. Option 2 rested on *"the gap this
  decision accepts is confined entirely to a debug/developer serial console a customer never interacts
  with,"* which was true while the customer path was the Time Machine backup server (journey 2, all
  MMIO). Journey 2 was retired 2026-08-30. The serial console is no longer beside the customer path.
- A prompt on every architecture became the goal. Milestones 268 and 182 made x86 boot into
  userspace and stop one step short of a shell, because §121 left the console in the kernel. On a
  headless x86 machine (xenon, driven over serial by design), the serial console is not a debug
  side-channel; it is the only interface anyone operates the machine through. By principle 1's own
  rule, the operator at that prompt is the first customer of x86 nife. So the serial console is the
  customer this file assumed did not exist.

The call is option 1: the port-range capability, enforced by the TSS I/O bitmap (calef,
2026-09-15). x86's serial console and input drivers become userspace processes holding a `(base, count)`
port capability, exactly as the other two architectures' UART drivers hold a memory-mapped device
capability. This restores "every driver is a userspace process" on real x86 hardware, with no
exception, which is the choice seL4 (this project's own benchmark), Genode and L4Re all made with the
same TSS-bitmap mechanism.

calef chose this over §149 option 1 (a kernel thread serving the console) deliberately, and over
sequencing through it. The maintainer's first framing was "kernel thread now, port capability later,
the console endpoint hides the migration." calef ruled straight to the port capability, on the tenet
that this project is not built out of convenience. The endpoint would have let userspace stay
unchanged, but the driver would have sat in the kernel until someone did the expensive work anyway.
And the only thing "later" bought was a faster prompt and a chance to measure the switch cost first.
He accepted both give-ups. See DECISIONS §149 (may the kernel answer on an endpoint), now resolved
by this reversal, and milestone 299 (the x86 port-range capability), which builds it.

Build the lazy write, not the naive one this file measured. The "Refined 2026-08-25" section is now
load-bearing rather than a footnote. The ~2,682 ns/switch release figure is the worst-case
always-write. The real cost is the lazy version, where the TSS `iomap_base` is set to a real bitmap
only when a thread that actually holds a port capability is on either side of a switch. In a system
where realistically one process (the console driver) ever holds one, that is near-zero for the
overwhelming majority of switches. Milestone 299's own first measurement is the lazy write's real
cost, taken the way this file already specified, not a second run of `tss_iomap_switch`.

What this does not reopen. Option 3 (the I/O-port broker, a system call per register access) stays
refused on its measured ~337 ns-per-round-trip cost against single-digit-cycle `in`/`out`; nothing
here revisits it. And the mapping-based capability for MMIO devices is unchanged: this adds a second
kind of device capability for the one class that has no page, it does not replace the first.

## A port range grants only with WRITE (calef, 2026-10-05)

A `PortRange` capability opens I/O ports only when its rights include `WRITE`. calef's ruling of
2026-10-05 16:33 UTC, quoted: "grant nothing without WRITE since we can't grant READ without write"
([comment on #1687](https://github.com/nifeos/nife/pull/1687#issuecomment-5998739348)).

The TSS I/O bitmap has one bit per port and cannot permit `in` without `out`, so a `READ`-only grant
has no honest meaning. `thread_control_block_insert_from` (`kernel/src/sched/thread_control_block.rs`)
grants only on `WRITE`; `component_plan` maps `Serve` to `READ`, so an observe-only view gets no port.

Refused: a rights-free port range, which makes `READ` on it a statement a reader must disbelieve; and
separate `in`-only and `out`-only kinds, which need every port access trapped. Milestone 768 (a
read-only port range grants nothing) builds it, with
`x86_port_tests::a_read_only_port_capability_must_not_grant_port_output` and a replayable falsification.
