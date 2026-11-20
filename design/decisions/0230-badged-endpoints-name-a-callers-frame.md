---
status: AMENDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 230. Badged endpoint capabilities: how a server learns which client's frame a request is in

*Section number provisional until the merge queue lands it. §221 (the boot prompt is the owner's
console) landed with #1340, and §222 to §229 are claimed by open pull requests (#1350, #1359,
#1361), so this took the next free number on 2026-09-26 and may move at merge. The file name is a
maintainer's coinage and provisional too.*

Raised 2026-09-26 by lane `milestone/fs-client-page` (pull request #1358), for its milestone, a
frame per filesystem client channel. That milestone is provisionally numbered 599 and its block is
not yet on `main`. The fork, its premise check, the analogous cases in the tree and the cost figures
are in `notes/a-frame-per-filesystem-client-channel.md`, which lands with that lane's [pull
request](https://github.com/nifeos/nife/pull/1358). It is not linked relatively because it is
not on `main` yet; whoever lands the second of the two can make it a relative link. This section
records the ruling and does not restate the note.

## The ruling

calef, 2026-09-26 (UTC), asked "A, B or C?": *"A"*. Recorded by the maintainer at 17:27Z the same
day; that is the time of recording, not of the ruling.

Option A is badged endpoint capabilities, seL4's answer:

- A new method mints a badged endpoint capability: a derived endpoint capability carrying a badge
  word that its holder cannot change. The method's name is provisional: `BADGE`, which §148 (a
  supervisor restarts by asking, and resolves by asking the kernel) refused for its own use to keep
  it free for this.
- `RECV_CAP` delivers the badge of the capability the caller invoked as a fourth return value,
  beside the first word, the Reply slot and the second word it already returns. (Amended
  2026-10-03: `RECV` delivers it too, for a plain `SEND`. See the amendment at the end.)
- The file server maps K windows and reads request `b` from its own window `b`. The progenitor holds
  a pool of K (badged endpoint, frame) pairs, hands one to each caretaker chain and takes it back at
  reap. `filesystem_protocol`'s layout and opcodes do not change.

This is a change to the system call surface under §10 (process model: capability-based, microkernel),
and it fits that model. The badge is a property of a capability, minted by a holder with the right
to derive one, and nothing about IPC gains a side effect. Each new method's semantics are recorded
here or in the section the implementing lane owes, per §10's rule, before the method ships.

## What this settles, and what it amends

- §27 (the filesystem service). §27 recorded badged endpoints as the alternative to a receive
  over a set of endpoints and did not take them. This ruling takes them for the case §27 could not
  cover, several concurrent clients of one server. §27's caretaker stands: the badge tells the
  server which window to read, and the caretaker is still what narrows a client to its subtree.
- §101 (notification objects). §101 named badged capabilities as a later, separate fork. This
  ruling decides that fork for endpoints. Whether a notification capability carries a badge too, so
  a signal's identity is kernel-stamped, is not decided here; it is the natural next use, and it
  stays §101's open item until someone raises it.
- The network stack's shared socket numbers. This also answers option 1 of
  [every-client-of-a-network-stack-shares-its-socket-numbers](../roadmap/0649-every-client-of-a-network-stack-shares-its-socket-numbers.md):
  the mechanism that proposal needed now exists. Choosing option 1 over that proposal's other
  options is still a separate ruling on `socket_protocol`, and this section does not make it.

## What it costs

From the note, computed from constants in the tree and not measured on a boot:

- The server maps K windows of 64 KiB (`fs::TRANSFER_PAGES` = 16 pages each). K = 8 is 512 KiB,
  inside the 8 MiB above `BLK_PAGE` the server does not otherwise use.
- Each concurrent client costs one 64 KiB channel of physically contiguous memory, where today the
  whole boot shares one.
- K is a fixed bound on concurrent filesystem clients per boot, and the pool is new state in the
  progenitor. Accepting windows at run time would need an attach verb, which is a wire change on top
  and is not part of this ruling.
- Per request, nothing is added: no extra rendezvous and no copy. The fourth return value is one
  more register on `RECV_CAP`'s return; the icount tripwire will price it when it is built.

## Refused

- B, the kernel remapping the receiver's window at `CALL`. It gives IPC a side effect on the
  receiver's address space, which is outside the model §10 established. It also puts a page-table
  rewrite and a TLB invalidation on the IPC fastpath at a cost nobody has measured.
- C, private client frames and a staging token. It loses at equal cost: more moving parts, a
  copy of every byte each way, and a cooperative rule that fails silently when one holder skips the
  token. Its only case is effort (no kernel change), and this tree does not choose on effort.
- The note's listed refusals, for the reasons it gives: a channel index in the request word, a
  file server per client, copying the name out first, notification objects on their own, and
  keeping clients serialized by construction.

## What happens next

The lane's milestone block (provisionally 599) is not on `main`; it lives on the lane's branch. Updating its gate to name
this ruling is left to that lane. The lane builds the mint method and the `RECV_CAP` return under provisional names, on every
supported architecture per §19 (architectural parity is a tenet). Milestone 47 (navigation and naming)'s set grant at the prompt is unblocked once it lands.

## Amendment (milestone 613, 2026-10-03): a plain `SEND` carries its capability's badge, on `RECV` too

Recorded at merge by the maintainer. The 2026-10-03 security audit found the wire had changed and
this section had not (finding 1 of
[`2026-10-03-eight-constants-and-thirteen-components.md`](../audit-reports/2026-10-03-eight-constants-and-thirteen-components.md)). The ruling is calef's, on
[#1494](https://github.com/nifeos/nife/pull/1494) at 05:04Z on 2026-10-03 (UTC): *yes*, a plain
`SEND` delivers its capability's badge to `RECV` in `x3`. The code is commit `4feaf0dc2`
(2026-10-03 04:30Z), landed with milestone 613 (a system log service: the in-memory half).

- **The badge is delivered on every receive**, not only to `RECV_CAP` for a `CALL` or a
  `SEND_CAP`. `RECV` and `RECV_CAP` both return it in `x3` for an ordinary message, `0` when the
  capability the sender invoked was unbadged. Before this, `x3` was always `0` for a plain `SEND`.
- **It is the kernel's word on every path.** The system call layer reads the badge from the capability
  in the caller's table (`kernel/src/system_call.rs`, the `SEND` arm, into `sched::ipc_send_badged`),
  never from a register the sender filled.
- **Why.** §242 (a system log) stamps every record from the writer's badge, and a byte-sink writer
  speaks a plain `SEND`. Without this, the badge a spawner minted was dropped on the one path it
  was minted for. The second reason is confinement. A bound filesystem client that `SEND`s rather
  than `CALL`s on its badged capability arrived as badge `0`, and `crates/subtree_scope` reads that
  as an open binding. #1494's own `RECV`-consumer audit found that escape and confirmed it under QEMU.
- **A death message is told apart by its first word, as before.** Nothing in the tree reads `x3`
  before checking `x0`; the 2026-10-03 audit read every `recv_fault` caller to establish it.

No new method, number or refusal. A server written against the text above this amendment that
treats a non-zero `x3` on `RECV` as a fault or as corruption is wrong since 2026-10-03. The test is
`a_plain_send_arrives_with_its_capabilitys_badge_on_recv_and_recv_cap`, and `abi::rendezvous::RECV`'s
doc carries the same contract.

## Amendment (milestone 726, 2026-10-03): a server's window lookup refuses a nonzero badge it has no window for

Recorded by the maintainer's delegate, as the maintainer authorized. The ruling is calef's, on 2026-10-03 (UTC), option 1 of the proposal that became milestone 726 (an unknown badge fails closed in subtree_scope): *"Fail closed in the crate."*

- A badge is the index of a client's staging window, so a server that has `B` windows has no window for a nonzero badge at or past `B`. Such a badge reaches nothing. In `crates/subtree_scope`, `Bindings::<B>::of` returns `Binding::Revoked` for it, and the server answers `EBADF`. Before this, it was folded onto badge 0 and carried the whole endpoint's authority.
- Badge 0 stays the caretaker's whole authority. It is the unbadged value, it is never bound, and nothing here narrows it.
- The kernel's `BADGE` method is unchanged: it still puts no ceiling on a badge, because the kernel cannot know any server's `B`. The refusal is the server's, where the window count is known.
- The proof is `a_badge_with_no_window_is_never_open`, with a replayable falsification beside it. The server's own test is `a_badge_with_no_window_is_refused`.

No new method, number or wire format. A spawner that mints a badge past a server's pool now hands its child a capability that is refused, where it used to hand it everything.

