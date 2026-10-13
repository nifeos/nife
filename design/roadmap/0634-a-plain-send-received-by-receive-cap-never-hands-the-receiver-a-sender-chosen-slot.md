---
status: BUILT
built: 2026-10-03
raised: 2026-10-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 634. A plain SEND received by RECEIVE_CAP never hands the receiver a sender-chosen slot

*(Title provisional. It drives fatal risk 7, the confinement claim.)* The `RECV_CAP` path a server
uses to receive a `CALL`'s reply slot hands a plain `SEND`'s sender a free choice of what lands in
`x1`, the register a server reads as a capability slot. An audit of the tree's `RECV` consumers
found it, for calef's ruling that a plain `SEND` carries its capability's badge (milestone 613 (a
system log service: the in-memory half), PR #1494). Two escapes were confirmed under QEMU before any fix. This is
within the established capability model: a semantics fix to one delivery path, no new method and no
new syscall number.

## Why: what each arrival order left in the registers

`RECV_CAP` returns five words. For a message that carries a capability (a `CALL`'s reply, a
`SEND_CAP`'s delegation) every word was right on both orders. For a plain `SEND` (three data
words, no capability), `x1` was wrong, and which way depended on who reached the rendezvous first.
Code references are `kernel/src/sched.rs` on this branch's base (origin/main `d6cb7049c`); the path
is shared scheduler code, identical on all three ISAs (no `arch::`).

- Receiver parked first (the exploitable order). `ipc_recv_cap` found no sender, parked, and
  read its mailbox at the bottom of the function (~line 3748). The later `ipc_send` took the
  `Send::Rendezvous(receiver)` arm and dropped its three words straight in: `r.mailbox = msg`, where
  `msg = wide([w0,w1,w2]) = [w0,w1,w2,0,0]` (`wide`, line 3425). So `RECV_CAP` returned `x0=w0`,
  `x1 = w1`, `x2 = w2`, `x3 = 0`, `x4 = 0`. `x1` was the sender's second word, a number the
  sender chose.
- Sender parked first. `ipc_recv_cap` took the `Recv::FromSender` arm (~line 3698) and built
  `[msg[0], slot, msg[1], msg[3], 0]` with `slot = NO_CAP`, because a plain sender parked no
  `outgoing_cap`. So `x1 = NO_CAP` already on this order.

A `CALL` server reads `x1` as the slot of the one-shot `Reply` capability the kernel minted
(`abi::rendezvous::RECV_CAP`). On the receiver-first order a plain `SEND` handed that server a
sender-chosen number where it expects a reply slot. Two sibling paths carried the same
order-dependence with kernel-chosen values. An interrupt signal returned `x1 = 0` on the
receiver-first order (line 2643) but `NO_CAP` on the other. A death message returned the dead
thread's id in `x1` on the receiver-first order.

## Why it is a confinement defect: two escapes, confirmed under QEMU

The milestone 613 audit found **no `RECV_CAP` consumer in the tree that checks the kind of object in
the slot it received**, and the ABI has no call that would let one. The only guard anywhere is
`x1 == NO_CAP`, which did nothing on the receiver-first order. Two attacks were written against the
kernel and run under QEMU on aarch64 on 2026-10-03 UTC, each driving the real kernel objects and the
real `sched::delete_current_cap`.

### Escape 1: a server deletes its own capability (net_stack-shaped)

`net_stack` (`components/src/net_stack.rs:227`) is reachable by any program whose manifest declares
network (the progenitor grants `WRITE` at `grant_plan::NETWORK_SLOT`). Its `OP_ATTACH_PAGE_FRAME`
does `map_page_frame(N, ...)` then `cap_delete(N)`, `N` the received slot, with no guard. Its own
capabilities sit at low fixed slots (report, NIC IRQ, virtio, budget, the STACK endpoint). The
attack test modeled this exactly: a server granted itself a victim capability at a fixed slot,
received with `RECV_CAP`, and unconditionally deleted `x1`; an attacker sent a plain `SEND` whose
second word was the victim's slot number.

```
ATTACK1 receiver-first: x1 delivered = 7 (chose 7), victim survived = false
ATTACK1 sender-first:   x1 delivered = NO_CAP, victim survived = true
```

**VERDICT: ESCAPE on the receiver-first order** (the server deleted its own capability at the
attacker-chosen slot). Near miss on the sender-first order (`x1 = NO_CAP`, the delete found nothing).
Against the real `net_stack` the same move deletes STACK, the NIC IRQ, the transport or the budget
and kills networking for every client.

### Escape 2: a bound client is seen as root (redoxfs_server-shaped), a separate defect

`redoxfs_server` reads the badge in `x3` to pick a client's window and subtree scope. On `main`, the
`SEND` syscall carries no badge: it calls `sched::ipc_send(ep, [a0,a1,a2])` with no badge argument
and `wide` fills word 3 with 0. So a client that `SEND`s on its per-window badged `FILE` capability
instead of `CALL`ing arrives as badge 0, and `subtree_scope::Bindings::of(0)` returns
`Binding::Open` unconditionally (`crates/subtree_scope/src/lib.rs:243`). The server then admits any
handle with root authority. The attack confirmed the kernel half:

```
ATTACK2: CALL delivered badge 0x5a5a, plain SEND delivered badge 0x0
```

**VERDICT: ESCAPE, by construction and confirmed.** This defect is independent of the `x1` finding
and of arrival order, and this milestone does not fix it: calef's ruling on #1494 is its fix.
Once a plain `SEND` carries its capability's badge, a bound client's `SEND` arrives bound and the
scope holds. Risk 7's record carries it as an open defect on `main` closed by #1494 (the §216 (fatal-risk
facts are correctable, and verdicts are the architect's) facts,
dated 2026-10-03). The throwaway attack programs stay in the lane's worktree
(`recv_cap_attack_tests.rs` as it stood before it was reduced to the property test, saved aside);
the fix lane for #1494 already pins the badge property with
`a_plain_send_arrives_with_its_capabilitys_badge_on_recv_and_recv_cap`.

## What is built

The receive side decides what `x1` means. `ipc_recv_cap`, on the receiver-first mailbox read,
returns `NO_CAP` in `x1` unless a capability was actually installed for this delivery. A one-bit
`Thread::cap_delivered` flag records it: the delivery paths that insert a capability
(`ipc_send_cap`, `ipc_call_badged`) set it, parking in `RECV_CAP` clears it, and a bound-notification
delivery is exempted by its `x4 == BOUND` tag. This makes `NO_CAP` the default for every delivery
that carried no capability, which settles the two sibling paths above for free. The change is in
shared scheduler code, so it is one patch for all three ISAs (DECISIONS §19 (architectural parity is a tenet)).

Two things it deliberately does not change: `x2` and the third word (no consumer reads `x2` as a
capability, so it is not a confinement concern), and the badge, which is #1494's.

### The options, and why this one

Both candidates were prototyped and measured on all three ISAs against this branch's base, same
compiler.

- Option D (chosen): the receive side writes `NO_CAP` unless a cap was installed. One place owns
  what each register means, the two sibling paths get the same guarantee without a second edit, and
  a future delivery path that forgets the rule fails closed to `NO_CAP`. It leaves `ipc_send`, the
  plain SEND/RECV hot path, untouched.
- Option B (refused): `ipc_send` checks whether the parked receiver is in `RECV_CAP`. Fixes only
  the one case, grows the SEND/RECV footprint on every ISA, and perturbs `ipc_rtt` on aarch64
  (+0.6%). It argued no better on the merits and cost about the same work, so the choice was not
  about effort.

Footprint (`script/fastpath-footprint`, bytes) and icount (`script/bench`, ticks/iteration) after
the fix, deltas against the base:

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `ipc_send_recv` | 5640 (+0) | 4922 (+0) | 6608 (+0) |
| `ipc_call_reply` | 7032 (+44) | 6086 (+34) | 8291 (+16) |
| `ipc_rtt` | 1056406 (+0) | 174034 (+0) | 17603719 (+0) |
| `call_reply` | 1084816 (+0.3%) | 180793 (+0.09%) | 18177369 (+0.09%) |
| `yield_switch` | +0 | +0 | +0 |

Every move is inside the 5% footprint band and the 10% icount band, so no baseline floor is
re-saved. The cost lands on `ipc_call_reply`, the shape that already carries the capability
machinery, and `ipc_rtt` (the cross-OS headline) does not move.

### The proof

`system_tests/src/user/recv_cap_attack_tests.rs` (provisional module name), two tests that fail on
the receiver-first order before the fix and pass after, on all three ISAs:

- `a_plain_send_to_recv_cap_delivers_no_cap_whichever_side_parks_first` drives both orders and
  asserts `x1 == NO_CAP` each.
- `a_server_that_deletes_the_received_slot_deletes_nothing_of_its_own` is escape 1 reduced to its
  property: the server's own capability survives on both orders.

A replayable falsification,
`system_tests/falsifications/user.recv_cap_attack_tests.a_plain_send_to_recv_cap_delivers_no_cap_whichever_side_parks_first.patch`,
reverts the fix and turns the first test red. These began as the throwaway attack programs that
demonstrated the two escapes above.

## Follow-on

- **Milestone 714.** Milestone 714 (the sibling RECEIVE_CAP paths get a receiver-first test). `design/roadmap/0714-the-sibling-receive-cap-paths-get-a-receiver-first-test.md`. The two sibling paths (an interrupt signal's `x1`, a death message's
  `x1`) now also return `NO_CAP` on the receiver-first order through the same `cap_delivered` default.
  No test drives either through that order, so their correctness is reasoned from the code. A lane that wants the
  measured grade should add one, the grade `notes/confinement-claims.md` already asks of an
  unmeasured verdict.
- **Recorded.** Escape 2 (the badge-0 root-scope bypass) is closed by #1494, not here;
  risk 7's record carries it. If #1494 is reworked or
  reverted, that escape reopens; risk 7's record is where it lives until #1494 lands.

## Index row

| 634 | a plain SEND received by RECEIVE_CAP never hands the receiver a sender-chosen slot | BUILT |
