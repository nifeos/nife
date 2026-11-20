---
status: DECIDED
raised: 2026-10-04
decided: 2026-10-04
ratified_by: calef
---

# 246. A plain `RECEIVE` never takes a capability, whichever side reaches the rendezvous first

*Section number PROVISIONAL: the maintainer mints it at merge. Written 2026-10-04 (UTC) by the lane
on PR #1611. The finding, the seven questions and the first cost estimate are in the block of
[milestone 752 (a seeded system call driver with a shadow model)](../roadmap/0752-a-seeded-system-call-driver-with-a-shadow-model.md),
section "A real finding".*

calef, 2026-10-04 (UTC), ruling option A of that section.

## The ruling

A plain `RECEIVE` (`abi::rendezvous::RECEIVE`) never installs a capability in the receiver's table.
The outcome no longer depends on arrival order.

- **A `SEND_CAP` reaching a plain `RECEIVE`** delivers its data word in `x0`, `0` in `x1` and `x2`,
  and the sender's badge in `x3`. The capability copy is dropped. The sender keeps its own, since
  `SEND_CAP` narrows a copy and never moves the source.
- **A `CALL` reaching a plain `RECEIVE`** delivers its two words and the badge. The caller's `CALL`
  returns `Gone`. The one-shot Reply lives in the receiver's capability table (§12 (call/reply
  IPC)), and a plain `RECEIVE` names no slot to fill, so nobody could ever answer it.

`RECEIVE_CAP` is unchanged. There is no new method, system call number or error. This narrows what an
existing method returns, within §10 (the capability-based microkernel process model).

## How the kernel knows

`Thread::receiving_cap` (name provisional) is `true` only while the thread is parked in
`RECEIVE_CAP`. `ipc_receive_cap` sets it beside its `cap_delivered` reset and clears it when that
receive resumes. So a plain `RECEIVE` always parks with it `false`.

The two receiver-first arms read it. `ipc_send_cap` installs only for a `RECEIVE_CAP`. `ipc_call_badged`
hands a plain receiver to an out-of-line `call_meets_plain_receive`.

The sender-first order already dropped a `SEND_CAP`'s copy (milestone 633 (an outside agent attacks
the confinement claim)). Its `CALL` arm used to leave the caller parked on a Reply that existed
nowhere. Now `collected_without_serving` answers it `Gone`. That function also holds the corpse arm,
which was inline before.

All of it is shared scheduler code, so one change covers aarch64, riscv64 and x86_64.

The clear sits at the `RECEIVE_CAP` resume because of the footprint gate. The first cut cleared it
at a plain `RECEIVE`'s park and kept the corpse and caller arms inline. On `main` after the
overflow-checks change, that put x86_64's `ipc_send_receive` at 6,880 B. The baseline is 6,544 B, so
that was 5.1%, past the band.

## Why

Before this, a receiver that parked first had the capability installed and its slot in `x1`. One
that arrived second did not. Every plain-`RECEIVE` server in the tree ignores `x1`: swish draining a
child's output, `wc`, `mdr` and the terminal sink. So each delegation that found one parked cost it
one of its 32 slots for good.

A std program, including an unvouched one, can retype its own region into a frame carrying `GRANT`.
It can then `SEND_CAP` that frame to its output. So a confined program could fill the shell's table.
That is fatal risk 7.

seL4 has the receiver opt in. A capability moves only into the slot the receiver names, and the
message arrives regardless (Reference Manual, `manual/parts/ipc.tex` at seL4 commit `c6ce4d2a0c`,
"Capability Transfer"). Its Reply lives outside the capability space, so a plain `seL4_Recv` can
answer a `Call`. Nife's Reply lives in the table, so `Gone` is the honest answer.

## What else was considered

From milestone 752's block, in one line each.

- (B) A plain `RECEIVE` always takes the capability. Refused: a receiver that did not ask gains
  authority.
- (C) Refuse a `SEND_CAP` at a plain receiver. Refused: the sender-first order has already parked and
  cannot be refused, so the orders would still differ.
- (D) seL4's sender-side gate, `GRANT` on the endpoint capability itself. A later rung, not a
  substitute.
- (E) Leave it recorded. Refused: it was exploitable.

## What it costs, measured

`script/fastpath-footprint` against `main` at the merge, on patagonia:

| closure | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `ipc_call_reply` | 6,912 to 6,948 B (+0.5%) | 6,094 to 6,136 (+0.7%) | 8,467 to 8,547 (+0.9%) |
| `ipc_send_receive` | 5,612 to 5,600 (-0.2%) | 5,002 to 4,978 (-0.5%) | 6,832 to 6,832 (0) |

All are inside the 5% band. The block's throwaway measurement of the receiver-first half gave
`call_reply` +0.02% under icount and `ipc_rtt` +1.4%.

## Tests

- `system_tests::user::receive_cap_attack_tests::a_send_cap_to_a_plain_receive_installs_nothing_whichever_side_parks_first`.
- `..::a_call_to_a_plain_receive_is_answered_gone_whichever_side_parks_first`.
- Each has a replayable falsification, replayed red on aarch64.
- The seeded driver's model (`system_call_fuzzer_tests`, `Model::plain_receive`).

## What is still open

- (D), the sender-side gate, is not built. Any holder of `WRITE` on an endpoint and `GRANT` on
  anything may still `SEND_CAP` there. What changed is that a plain receiver no longer keeps it.
- §245 (a `CALL` server tells a Reply from a delegation) has a "still open" note. It says a
  plain-`RECEIVE` `CALL` server sees the tag on one order only. This section answers it: neither
  order delivers a Reply now.
