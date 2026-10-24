# The rights-gate tests

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds the design of
`system_tests/src/user/confinement_attack_tests.rs`, whose doc comments state what each test proves.

*Name: provisional, for the stem `the-rights-gate-tests`, named for what the file attacks. calef
names files.*

## What the module attacks

The module is the kernel half of two confinement claims. Claim 26 is §41 (the endpoint is the
broker)'s "a client of a rendezvous cannot become its server". Claim 2 is §14 (the project's
direction)'s "userspace cannot forge a right out of a syscall register". Both are driven through
the real dispatcher, `syscall::invoke`, the way a program's `svc` or `ecall` reaches it. So what is
under test is the rights gate in `invoke`'s `Object::Rendezvous` arm, and nothing re-implemented
beside it. `irq_send_refusal_tests` is the pattern: grant a capability into this thread's own table
and invoke a method it must be refused. `call` is copied in shape from that file.

`invoke`, the rights check and the rendezvous are portable kernel code. The same test on each
architecture meets the parity gate of §19 (architectural parity is a tenet).

## Why it sits beside the live-swap test

`live_swap_tests` is claim 26's cited test. It drives one `READ`-gated method, `RECEIVE_CAP`, from
one fixture, and only on the two architectures with a console device page. A broken gate surfaces
there as a watchdog hang rather than a failed assertion, because a `RECEIVE_CAP` let through simply
blocks with nothing to receive.

These tests cover the other `READ`-gated methods a would-be server reaches for, `RECEIVE` and
`REAP`, and the `ENUMERATE`-gated `SURVEY`, on every architecture. Each is shaped so that a gate let
open returns promptly. Either a sender is already queued, or the method does not block. A
regression is red at an assertion, not at the watchdog.

## The write-only holder

`WRITE` lets a holder `SEND` and `CALL`, and it is the right a client holds. Becoming the server
means `RECEIVE`, `RECEIVE_CAP` or `REAP`, which take `READ`, or `SURVEY`, which takes `ENUMERATE`.
Each must answer `NotPermitted`. The right lives on the capability, and no method number or
argument register can add one.

The two blocking methods are tried with a sender already parked on the endpoint. A gate let open
would return that sender's message and drop the waiting-sender count, so the break is observable
rather than a sixty-second hang. The sender count after each refusal is the other half of the
claim: a refused receive consumes nothing. `REAP` and `SURVEY` do not block, so they need no queued
peer.

The positive control is the last step. The same endpoint, received on with a `READ` capability
through the same dispatcher, delivers the queued message. Without it, a `RECEIVE` refused for some
unrelated reason would read as the property holding. It also drains the sender so its thread exits.

## The read-only holder and the ungranted slot

A holder of `READ` alone invokes `SEND`, which takes `WRITE`. The right is not in the method number
or the argument words; it is on the capability. A receiver is parked first, so a gate let open would
deliver to it and drop the waiting-receiver count rather than block.

Then a slot this thread was never granted is invoked, and the answer is `NoSuchSlot`. That is not
"permission denied" but "there is nothing there", which is what no ambient authority means from the
inside. The test picks the second-highest table index, so the slot exists and is empty rather than
out of range. Both cases answer `NoSuchSlot`.

## The retype mint

A leaf child is handed a budget without `GRANT`, so it may spend memory but not lend it. `SPLIT`
honors that (`split_never_widens_rights`), and milestone 824 (a retype mints no right its budget
lacks) makes `RETYPE` and `RETYPE_OBJ` honor it too.

The test uses two `GRANT`-less budgets: `WRITE` alone, and `SPEND_ONLY`, which is every right but
`GRANT` and is what `memory_region_cap` hands a leaf. The frame and every object kind come back with
exactly the budget's rights. The process is retyped around the address space just made. The frame
cannot be passed on: `PageFrame::SLICE` is the one delegating method that does not block, and it is
refused `NotPermitted` for want of `GRANT`.

Two controls show that the mint is the budget's rights and not a fixed narrow set. A `READ | WRITE`
budget's frame carries exactly that. The root budget's frame carries `Rights::ALL`, so the
progenitor loses nothing.

Nothing blocks, so a regression is red at an assertion. Each capability is deleted and each region
reclaimed, so the frame and region ledgers are flat across the test.
