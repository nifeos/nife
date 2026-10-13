---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
promoted_from: a-call-server-can-tell-a-reply-from-a-delegation
milestone_dependencies: none
decision_dependencies: 245
machine_requirements: none
specific_machine: none
needs_person: no
---
# 706. A `CALL` server can tell a Reply from a delegation

*Title and slug are drafts.*

Raised by the 2026-10-03 security audit's follow-up (finding 11, item (b) of
`design/audit-reports/2026-10-03-eight-constants-and-thirteen-components.md`'s reconciliation). An
input to fatal risk 7 (the confinement claim). Ruled by calef, 2026-10-03 (UTC): "Option 1 with
the typed runtime helper." Recorded as DECISIONS §245 (a `CALL` server tells a Reply from a
delegation), number provisional, which holds the three options and why each won or lost.

## The gap

Milestone 634 (a plain SEND received by `RECEIVE_CAP` never hands the receiver a sender-chosen slot)
made `x1` on `RECV_CAP` `NO_CAP` unless the kernel installed a capability. So the guard every
`CALL` server had was `x1 != NO_CAP`, and a real capability passes it.

Any client with `WRITE` on a server's endpoint and `GRANT` on a capability of its own could
`SEND_CAP` it there. The server invoked `reply(x1, ...)`, which is method `0` on whatever sits in
the slot. On a rendezvous nobody receives on, that is `SEND`, and the server parked for the life of
the machine with every client behind it. A delivery that was not a Reply also left its slot behind,
one of 32. Severity medium: a denial of service on any `CALL` server, not an escape.

## What was built

- The kernel tag. `RECV_CAP` returns `abi::rendezvous::REPLY_DELIVERED` (`1`, name provisional)
  in `x4` exactly when `x1` is the Reply the kernel minted for a `CALL`. `ipc_call_badged`'s
  rendezvous arm and `ipc_recv_cap`'s collect of a parked caller write it through one helper,
  `reply_tag`, in `kernel/src/sched.rs`. A `const` assertion in `crates/abi` keeps it apart from
  `notification::BOUND`, which shares the register.
- The typed helper in `crates/user_mode_runtime` (all names provisional). `recv_request(slot)`
  returns a `Request { w0, w1, badge, delivered }`, where `delivered` is a `Delivered`:
  `Reply(Reply)`, `Delegation(slot)` or `Nothing`. `recv_request_bound` is its twin for a thread
  with a bound notification. `Reply` has no public constructor, and `reply(to: Reply, r0, r1)`
  consumes one. `Delivered::into_reply()` returns the Reply and deletes a delegation.
- Every server converted. The `components` are `net_stack`, `compositor`,
  `fs_file_caretaker`, `fs_nameset_caretaker`, `fs_subtree_caretaker`, `display_terminal`,
  `framebuffer_driver`, `gpu_driver`, `entropy`, `jh7110_entropy`, `non_volatile_memory_express`,
  `line_editor`, `clock`, `credentialer`, `broker`, `terminal_supervisor` and `swapper`. Outside
  them: `redoxfs_server`, `crates/virtio`'s block server, `crates/swap_protocol`'s two services,
  and the fixtures `call_server`, `job_mix_task`, `network_time_test_server`, `soaker` and
  `window`. `reply` accepts only a `Reply`, so the compiler is the census: a slot-typed call left
  anywhere would not build. Outside the runtime, a grep for `abi::reply::REPLY` finds one raw use,
  in `fixtures/call_server`, on purpose: it proves the kernel refuses a second answer.
- How each converted. `net_stack` takes a delegation for `OP_ATTACH_PAGE_FRAME` only and a
  Reply for every other op; a `CALL` naming `ATTACH` is refused with `REP_ERR`. Servers on a
  `CALL`-only contract that skipped `NO_CAP` now skip any non-Reply. Servers that did their work
  for a message with no Reply (the compositor, the display drivers, the file service, virtio-blk)
  still do, and answer only a Reply. `line_editor`'s parked reads hold `Option<Reply>`.

## What proves it

`system_tests::user::recv_cap_attack_tests::a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders`.
A client `SEND_CAP`s a rendezvous nobody receives on, and another `CALL`s, on both arrival orders.
The server trusts only the tag. It must answer the caller and end holding what it held before.

It tests the tag, not the hang, and that is a choice the brief allowed. A test of the hang
itself would have to end a thread blocked in `SEND`, which needs the timed receive milestone 417 (a
usurper that reports instead of hanging) waits on. The replayable falsification zeroes the two
stores. Replayed on aarch64 on 2026-10-03, it fails at the predicted assertion: the server deletes
the Reply as a delegation, and the caller is never answered.

## BUGS

- The check is in the runtime, not the kernel. A program that issues `RECV_CAP` through its own
  `invoke` and reads `x1` raw is as exposed as before, and no gate catches a new server written
  that way. Every server in the tree uses the runtime today. The follow-on below is the answer
  that does not depend on the server.
- A plain `RECV` sees the tag on one arrival order only. When the server is parked, the Reply
  is installed and tagged whichever receive it made; when the caller is parked, a plain `RECV`
  installs nothing. Serving `CALL` endpoints with `RECV_CAP` is the standing advice, and no server
  in the tree does otherwise.

## Architectural parity

All three ISAs at once. The tag is written in `kernel/src/sched.rs`, shared scheduler code, and the
runtime helper reads it through `invoke5`, which each ISA already implements. The test is in the
system-test image every leg boots; the falsification is replayed on aarch64 only, as
`script/falsifications` does for every kernel test.

## Follow-on

- **Recorded.** Option 2, an endpoint that refuses `SEND_CAP`, is the rung-one answer: a server opts
  in and the kernel refuses a delegation at the `SEND_CAP` arm, the shape milestone 603 (an
  interrupt's endpoint refuses every send) took for `SEND`. A wire-format change in the rights word
  or a new attribute, so an architect's call; it waits until a server asks for it. Recorded in §245
  and in the `RECV_CAP` BUGS in `crates/abi`.

## Index row

`RECV_CAP` tags a `CALL`'s Reply in `x4` (`abi::rendezvous::REPLY_DELIVERED`), and every `CALL`
server receives through `user_mode_runtime::recv_request`, whose typed Reply is the only thing
`reply` accepts and which deletes an unwanted delegation (DECISIONS §245). A client can no longer
park a server or leak its slots by `SEND_CAP`ing where it should `CALL`.
