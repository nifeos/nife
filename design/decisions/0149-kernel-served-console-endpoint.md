---
status: DECIDED
raised: 2026-09-09
decided: 2026-09-15
ratified_by: calef
---

# 149. May the kernel answer on an endpoint, where §121 leaves no userspace holder?

2026-09-15, and the answer is no. Raised 2026-09-09 by the maintainer. Resolved
not by picking one of its three options but by dissolving its premise. This section asked what to
do *given that §121 leaves no userspace holder on x86*, and on 2026-09-15 calef reopened §121 to give
x86 exactly that holder (the port-range capability). So the kernel does **not** answer on an endpoint;
x86's console is a userspace driver like the other two. See the resolution at the end. *(Number
provisional until the merge queue lands it.)*

What is blocked: x86_64 reaching an interactive shell at all, and therefore milestone 182, and
therefore the whole "every architecture boots to swish" half of the parity target.

## The question, in one sentence

On aarch64 and riscv64, `swish` does not talk to a UART. It talks to a console server over an
endpoint, and that server is `components/src/console.rs`, an unprivileged process holding the UART's
page as a capability. On x86_64 there is no such server and there cannot be one, so the question
is whether a kernel thread may park on the same rendezvous and answer the same protocol.

## Why there cannot be one, which is settled rather than open

DECISIONS §121 ratified option 2 permanently: x86's legacy port-I/O devices stay in the kernel,
and only MMIO devices get the mapping-based capability. COM1 on xenon is the built-in port at
`3F8h`, IRQ 4 (`notes/xenon-firmware.md`, read off the firmware screen), so it is port I/O, so it is
kernel-resident, and that is a decision rather than a gap.

**This section does not reopen §121 and must not be read as doing so.** §121 decided *where the
driver lives*. This decides *how a program reaches it*, which §121 raised and did not answer: its
own text says what is blocked is "a userspace console or input driver on x86", and leaves the
consequence for the shell unstated.

## What the tree already provides, measured rather than assumed

- The endpoint object is `Object::Rendezvous`, and the kernel's is `type Rendezvous =
  inter_process_communication::Rendezvous<Thread>` (the alias in `sched/ipc_tables.rs`).
- `crates/inter_process_communication`'s rendezvous is generic over `T: Node` and
  host-testable, with no privilege level anywhere in it. `ThreadControlBlock` is what implements
  `Node`.
- The kernel already has kernel threads, which run a closure on their own stack
  (`kernel/src/thread.rs`, "starts a kernel thread's closure").
- `crates/inter_process_communication`'s own doc names the parked-server case as the ordinary one:
  a thread queued on a rendezvous is permanently `Blocked`, "and the commonest such thread is a
  server".

So the mechanism needs no new object type, no new system call number, and no change to the surface
(§10, §16). That is the strongest argument here and it is worth stating plainly: this is the option
that does not grow the boundary.

## The options

Option 1: a kernel thread parks on a rendezvous and answers the console protocol. `swish` is
byte-identical on all three architectures and never learns where its console server lives. The
divergence §121 requires stays entirely below the interface.

- *For*: no surface growth; parity at the level that a program can observe; the client code, the
  protocol and the tests are one thing rather than three.
- *Against*: a kernel thread that clients can block is a denial-of-service and priority-inversion
  surface that none of §40's subtree-death supervision reaches, because the thing that wedges is
  the kernel. It puts a message parser inside the trusted computing base, which is code that has to
  be verified rather than argued about. And it is a real, if narrow, concession against §14's
  minimal-core claim: seL4's position is that the kernel provides mechanism and no services.

Option 2: a console system call. The kernel exposes a write/read pair and `swish` calls it directly
on x86.

- *For*: smaller to build; no kernel thread to schedule or wedge.
- *Against*: it is system call surface, the expensive kind under the *move fast on what can be undone*
  tenet, and every future program is written against it. It is also worse on security grounds in a
  capability system, and this is the part that is easy to get backwards. A system call is reachable by
  every thread in the system, where an endpoint is reachable only by whoever was handed the
  capability, and can be revoked. Option 2 asks for a rule about who should call it; option 1 makes
  most programs unable to.
- It also puts a permanently x86-shaped hole in the client: `swish` would have to know which
  architecture it is on, which is the divergence this is trying to remove.

Option 3: reach the shell through the graphical stack instead, which is what milestone 182
currently assumes. The framebuffer is MMIO, so the existing mapping model works untouched.

- *For*: no new mechanism of any kind.
- *Against*: it makes milestone 177, the largest piece in the tree, a prerequisite for x86 having a
  shell, which inverts the agreed order (software parity first). It also leaves x86 with no serial
  path for a bench session, and `board_console` reads a wire.

## Recommendation

Option 1, and the effort test (§92 (a caretaker is supervised), and the seventh question) comes out
the same way. If all three cost the same, option 1 is still the one to pick, because it is the only
one where the interface does not diverge. It is not the cheapest to build; option 2 is.

The denial-of-service objection is real and is the part that wants designing rather than assuming.
The shape that answers it is a service thread that **never blocks on a client**: it may block
waiting for a message, which is the ordinary parked-server case, but must not park inside handling
one.

## What this deliberately does not decide

Whether the kernel should serve IPC generally, to expose facts only it knows (preemption counts,
scheduler state, the machine description). calef raised that in the same conversation and it is a
better question than this one, but it is a different question. The two were deliberately
separated so that the general case is not decided on the momentum of the narrow one:

| | This section (console) | Kernel introspection |
|---|---|---|
| Kernel holds a device | yes | no |
| On the data path | every byte | one question, then done |
| Blocks a kernel thread indefinitely | yes | no |
| `swish` works without it | no | yes |
| Architectures | x86 only, a divergence | all three, parity |
| Why we would do it | **forced** by the hardware | **chosen**, because it beats a system call |

That last row is why they are separate. One is a concession with no alternative; the other is a
design we would pick freely. A reader citing this section for the general case is misreading it,
and the introspection question should get its own section when there is an actual fact to expose
rather than in the abstract.

## BUGS

- The kernel-thread-parks-on-a-rendezvous mechanism is verified at the data-structure level
  only. `crates/inter_process_communication` is generic and privilege-free, and kernel threads
  exist, but nothing here has proved the kernel's own receive path is free of an assumption that the
  receiver entered from EL0. A lane will find that, and if the assumption exists this becomes a
  larger fork than it reads as.
- Nothing in this section prices the denial-of-service work, which is named as a shape and not as
  a design.
- This is the first kernel-resident IPC server in the tree, so there is no in-tree precedent to
  follow and no second instance to check a convention against.


## Resolved 2026-09-15: the premise was removed, not one of the options chosen

This section's question was conditional: *given* that §121 keeps x86's serial driver in the kernel, is
a kernel thread serving the console the least-bad way to reach a prompt? It listed a fourth path only
implicitly, by citing §121: change §121 so there is a userspace holder. That is what calef did.

§121 was reopened and reversed to its own option 1 (the port-range capability); x86's console and
input become userspace drivers holding a `(base, count)` port capability. So every option this section
weighed is moot: there is no kernel-resident console driver to serve behind an endpoint, no console
system call, and no need to reach the shell through graphics. `swish` talks to a userspace console server
on x86 exactly as it does on aarch64 and riscv64, and this section's recommended option 1 (the kernel
thread) is **not** built.

Why the recommendation here lost, recorded because it was argued for. This section recommended the
kernel thread on the strength of the interface-does-not-diverge test, and it was right that the
client stays identical. What it undervalued is that the kernel thread keeps a device driver in the
kernel, which is the one thing §14 (the project's direction)'s thesis says userspace should hold. It
also undervalued that the endpoint's ability to hide the driver's location is a reason to *migrate*
later, not a reason to build the kernel version first. calef ruled straight to the userspace driver
(DECISIONS §121 (what a device capability is), milestone 299 (the x86 port-range capability)),
accepting a slower path to the x86 prompt in exchange for not shipping a kernel-resident driver at
all. The BUGS below, about a kernel-resident IPC server's denial-of-service surface and its place in
the trusted computing base, are retired by that choice rather than answered: there is no
kernel-resident server.