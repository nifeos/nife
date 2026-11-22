---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
---
# 603. An interrupt's endpoint refuses every send

Built 2026-09-26. *(Number provisional: minted by the lane, to be confirmed at merge.)* The lane is
`milestone/603-irq-send-refused`, pull request #1383.

## The ruling

calef, 2026-09-26, recorded at 21:33 UTC: "B", on the IRQ migration §101 (notification objects)
left open. An endpoint bound with `bind_irq` refuses every deposit from userspace, so an interrupt's
`w0 = 1` is unforgeable by kernel rule rather than by wiring. Moving interrupts onto notification
objects is decided later, driver by driver. Refused: C, a `BUGS` entry that relies on wiring. The
record is §101's second amendment.

Milestone 151 (notification objects) found the gap. All fifteen `bind_irq` endpoints are created
for their interrupt and never granted as a `Rendezvous` capability, and `soak::bind_tick_routes`
binds endpoints its caller supplies. Nothing refused a program granted `WRITE` on one.

## What was built

| piece | where |
|---|---|
| the mark: `Rendezvous::bind_to_interrupt`, one-way, in the padding after `pending` | `crates/inter_process_communication` |
| the refusal: `send` answers `Send::Refused` and touches nothing, tested before a receiver is popped | the same |
| `bind_irq` marks the endpoint before it publishes the route, so the soak's caller-supplied endpoints are covered too | `kernel/src/sched.rs` |
| `SEND`, `SEND_CAP` and `CALL` return `NotPermitted` without blocking; a §26 (the fault endpoint) death message sent there is dropped, since `EVENT_FAULT` is also `1` | `sched.rs`, `system_call.rs` |
| `NotPermitted`'s doc names the case | `crates/moho_abi` |

The refusal is a variant of `send`'s result rather than a check at each caller. Every path that
deposits into a rendezvous has to match it or the kernel does not compile, so a fifth deposit
method cannot forget the rule. That is the top rung of the ladder, where a per-method check
would have been the second.

The error is the tree's existing `NotPermitted`, not a new one. The system call layer reads the reason
only after `take_ipc_aborted` has already said `true` (`aborted_send_error`, `#[cold]`), so a send
that went through pays nothing at that layer.

Provisional names, all calef's to rule on: `Rendezvous::bind_to_interrupt`,
`is_bound_to_interrupt`, `Send::Refused`, `sched::take_ipc_refused`, `set_ipc_refused`,
`system_call::aborted_send_error`, the `Thread::ipc_refused` field, and the test module
`user::irq_send_refusal_tests`.

## Proof

- Kani: `send_rendezvous_iff_a_receiver_waited` now also proves that a marked rendezvous refuses
  and changes nothing, and `seed` makes the mark symbolic, so all seven harnesses in the crate cover
  a marked rendezvous. All seven pass. Folded in rather than added, because it is the same
  decision's other branch. Removing the check turns it red at both `!bound` assertions (run
  2026-09-26); its replayable record stays the older defect, one patch per harness.
- Kernel, all three ISAs: `a_send_to_an_interrupts_endpoint_is_refused_and_the_driver_never_sees_it`.
  The test grants itself `Rights::ALL` on a routed endpoint, so the kernel's rule is the only thing
  in the way. Each of the three methods is refused, the driver stays parked in `Irq::WAIT` with no
  sender queued, and then `irq_notify` wakes it with `w0 = 1`.
- Falsified once on aarch64, with the check removed from `send`: the `SEND` assertion got `Ok(0)`
  where it wanted `NotPermitted`. The patch is under `kernel/falsifications/`.

## Measurement

The check is one load and one branch on `send`'s common path. `script/fastpath-footprint`, measured
on patagonia with the same nightly, the base tree against this one:

| ISA | `ipc_send_recv` | `ipc_call_reply` | `system_call_entry` |
|---|---|---|---|
| aarch64 | 5500 to 5544 (+44) | 7172 to 7236 (+64) | 1508 to 1516 (+8) |
| riscv64 | 4778 to 4820 (+42) | 6082 to 6102 (+20) | 1892 to 1910 (+18) |
| `x86_64` | 6464 to 6496 (+32) | 8398 to 8414 (+16) | 1701 to 1717 (+16) |

The bytes are mostly each `Refused` arm's call into the cold helper, which counts because the gate
sums whole symbols. CI's footprint job reported the same nine numbers. All three ISAs stay inside the 5% drift band.

`script/bench` in CI, this branch against the merge-queue group for #1367, whose base was this
branch's base and which touches no IPC code. Same host type, same nightly, instructions per 1,000
or 5,000 iterations:

| ISA | `ipc_rtt` | `call_reply` | `ipc_rtt_el0` |
|---|---|---|---|
| aarch64 | 1034642 to 1032327 (-0.22%) | 1057883 to 1058321 (+0.04%) | 10994673 to 11025956 (+0.28%) |
| riscv64 | 171469 to 170697 (-0.45%) | 177104 to 177158 (+0.03%) | 1861550 to 1860234 (-0.07%) |
| `x86_64` | 17254582 to 17261272 (+0.04%) | 17788619 to 17789069 (+0.00%) | no row |

Every move is under half a percent and they go both ways, so the check is below what this
instrument resolves. riscv64 `ipc_rtt` is the row milestone 598 (a nightly bump restamps the
floors it proves it did not move) records as bimodal, and its -0.45% is that, not a speed-up.

The footprint deltas leave `ipc_send_recv` on riscv64 at +4.1% and `system_call_entry` on `x86_64` at
+4.9% against the saved baselines, inside the 5% band but close to it. Milestone 151 measured its own
growth on the same closures, so whichever of the two lands second may cross the band. That is a
baseline to re-record at merge with both numbers in hand, not a regression in either.

## BUGS

- The mark is one-way, because `bind_irq` has no unbind. A route re-bound to a new endpoint leaves
  the old one refusing sends. That is the safe direction, and nothing rebinds a live driver's
  interrupt today except the riscv64 tests, which share the UART's source.
- A kernel thread that calls `sched::ipc_send` on an interrupt's endpoint returns at once with its
  abort flag set, as it does for a stale endpoint. No kernel caller does this. The flag is the
  system call layer's to read, so a kernel caller would not learn why.
- The test's route to interrupt 250 outlives it, like milestone 151's route to 251. Reclaiming the
  region makes the name stale, and `irq_notify` drops a signal to a stale name.

## Follow-on

- **Recorded.** The one-way mark, the silent kernel-side abort and the test's leftover route are in
  this block's `BUGS`, and the first is beside the method in `crates/inter_process_communication`.
- **Done.** Milestone 151's `BUGS` entry on the forgeable `w0 = 1` is removed by this change, its
  note says the gap is closed, and §101 (notification objects)'s first amendment now points at the
  second.
- **Decision.** Moving each driver's interrupt onto a notification object is deferred, driver by
  driver, by `design/decisions/0101-notification-objects.md`.

## Index row

An endpoint bound to an interrupt now refuses every send, so a driver's `w0 = 1` can only mean its
device fired. calef ruled it on 2026-09-26 (§101 (notification objects), option B), closing a
forgery milestone 151 (notification objects) found that had held only because nobody was granted
`WRITE` on such an endpoint. The refusal is a variant of `Rendezvous::send`'s result, so any future
deposit method must handle it to compile.
