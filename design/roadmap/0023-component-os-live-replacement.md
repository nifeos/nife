---
status: PARTIAL
raised: 2026-07-25
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 23. A capability-routed component OS with live replacement

Every residual this block named is built, and so are the advisory warning (§231) and the live
swap of `line_editor` (§232 (the line editor swap contract), with a terminal supervisor). What keeps
it PARTIAL: the installer does not yet ask the terminal to swap (proposed, an architect's),
`display_terminal` and `compositor` are not swapped, and no supervisor yet ends a hung incumbent,
although the kernel can (milestone 133 (ending a permanently blocked thread, and deciding who may)).

## The idea

Every userspace component (driver, server, app) is a swappable, vendor-shippable unit behind a
stable contract, and operators replace them live, with no reboot. A client names an endpoint, never
a peer (§12 (call/reply IPC: a one-shot reply capability)), so a component's identity is invisible to the code that uses
it: any program that speaks the protocol and holds the right capabilities is the component. The
kernel is the one fixed thing underneath an entirely swappable userland. This is Fuchsia's shape
(capability-routed components, stable protocol interfaces) on a verified core.

It is also the product story. The kernel confines each component to what it was granted, so
competing vendor components run safely: a Linux vendor module is ring 0, and a nife vendor component
is a confined process that can touch only what the operator handed it. That is the payoff of the
capability model and of milestone 22 (trusted init: verify it, and shrink what a broken one can do), and it connects to the parked
competitor ambition ([competitor-question.md](../competitor-question.md)).

Two things are deliberately not swapped this way. The kernel is the verified base and changing it is
a reboot. A minimal root supervisor is the fixed point that makes swapping everything else possible;
you cannot swap the swapper infinitely.

Prior art: Fuchsia (the closest match), MINIX 3's reincarnation server (live driver replacement in
userspace), QNX (hot-swappable drivers), Erlang/OTP hot code loading and supervision.

## What is built

### Instance one: swap a console-shaped server under a talking client (2026-07-30)

DECISIONS §41 (the endpoint is the broker, and a device is revoked by taking it back),
notes/live-replacement.md. An unprivileged operator (`swapper`) builds the replacement, drains the
incumbent, takes the device back, and starts the replacement. A client (`chatty`) talks across the
swap and is its own witness; an attacker holding the client's exact capabilities cannot become the
server; the outgoing instance reads the UART after the revoke and faults at the device's own page;
and the replacement is written in C over §31 (the foreign-language seam: C holds no capabilities and makes no syscalls), so what held across the swap is the contract.

Three things the build settled. No forwarding process is needed as the broker: endpoint-only
naming makes the endpoint object the stable name, so a swap costs nothing in steady state and the
kernel's sender queue buffers the down window. The roadmap's step order (start the new server, then
revoke) does not survive contact, because revocation is by physical page; the endowment moves past
the revoke while the build stays first. And revoking a device means take-back, not destroy.

### The latency ladder, and an opt-in queue broker

`broker` is the middle rung: a queue server that buffers in its own budget while a backend is down,
so a producer never blocks on an absent consumer. It costs a second hop, priced by `broker_rtt`, so it
is opt-in per channel and passes through when both ends are up. The fast rung is the direct endpoint;
the slowest, a durable queue that writes to storage, is not built. The kernel stays synchronous
rendezvous; the queue is userspace policy bounded by the broker's own untyped.

### The component manifest (2026-08-17)

`crates/component_plan`, notes/component-manifest.md. `swapper` holds no endowment literals: each
contract declares its capability half in `swap_protocol`, and the operator says only which of its
own objects answers to which role name, per child. A manifest is a request and the provisions are the
authority (Fuchsia's `use`/`offer` split), so a vendor's declaration cannot widen its own authority.
The role name is the component's and the object is the supervisor's, which makes a component's peer
substitutable too. A manifest is still compiled in rather than shipped beside a binary; see
Follow-on.

### The hung component (2026-08-17)

notes/hung-component.md. Against an incumbent that stops answering without dying, three of the four
steps are unchanged: the one that needs its cooperation, the drain, is the one a hang makes
redundant. So a supervisor restores the service with no authority it did not already hold, which
corrects §32's sentence that restarting a hung child needs the stronger right; that sentence is right
about reclaiming its memory. The harder half is measured: the domain reports a hang as `BLOCKED`,
exactly what a healthy idle server reports; `Endpoint::REAP` answers `StillAlive` for every member;
and `abi::Error::Gone` never reaches a caller stranded mid-`CALL`, because freeing it needs the
cooperation whose absence is the hang.

### Dependency-aware orchestration (2026-08-23)

`component_plan::depends_on` and `dependents`, notes/dependency-orchestration.md. A component names
the contracts it cannot silently tolerate the absence of, and a supervisor asks the graph who to warn
before a swap. Only a component that forwards synchronously while serving others ever needs an entry:
a pure consumer degrades for free on the endpoint's sender queue, §41's mechanism doing a second job.

### State handoff (2026-09-26)

notes/state-handoff.md, to the ruling in DECISIONS §209 (state handoff is an opaque blob over a
granted frame, and it is optional). `Requirements` gained the optional `handoff` field (provisional,
as §209 asked), `swap_protocol` a stateful contract, and `swapper` a fourth role, `ROLE_HANDOFF`,
that tries the swap twice: against a replacement that cannot read the incumbent's blob, which
refuses, is reaped, and leaves the incumbent serving with its state; then against one that can,
which commits. The client's unchanged sequence check is the continuity witness, because the
component answers with its tally. It runs on all three architectures, the first swap system to,
since it has no device; the queued system lost the same inherited x86 skip.

Two findings. The handoff page cannot be deferred past a revoke like a device, because
`PageFrame::REVOKE` is symmetric and would take the operator's own capability, so it is mapped at
build in both instances and sequenced by the drain. And §209 needs a refusal it does not state: a
supervisor that routes no handoff page is refused a component that declares one, since wiring it
anyway is a kill-and-replace that looks like a successful swap.

§209 replaces §116 (live component state handoff is declined, for want of a customer).

### The unwarned dependent, measured (2026-09-26)

`ROLE_UNWARNED` swaps the queued system's backend without ever warning `broker`. The producer loses
nothing and only waits for the down window. So a dependent that will not answer its warning needs
nothing done to it; the supervisor only has to not block on it, which today it does, because the
warning is a `CALL`. notes/non-cooperative-fallback.md.

## Follow-on

- **Done.** A handoff is a run of pages, 2026-09-26: `Handoff::pages`, minted with
  `MemoryRegion::RETYPE`'s page count, which calef ruled the same day
  (`design/roadmap/0659-a-region-retypes-a-frame-run.md`, option A).
- **Recorded.** A manifest is compiled in rather than shipped beside a binary. The ELF-note manifest
  work, milestone 597 (a program carries its manifest in an ELF note), carries a program's grant
  manifest rather than a component's declaration; notes/component-manifest.md's `BUGS` carries the history.
- **Done.** The timed wait a supervisor needs to notice a hang exists: milestone 106 (a wait that
  ends on either the interrupt or the deadline) is BUILT 2026-09-26. Milestone 263 (can a userspace process hold a timer, on all
  three architectures) was RECORDED rather than a blocker. No swap supervisor uses it yet:
  `components/src/swapper.rs` and `terminal_supervisor.rs` hold no deadline wait (checked
  2026-10-03), so nothing notices a hang on its own yet: `ROLE_HUNG`'s wedge announces itself.
- **Done.** What may be done to a component that never cooperates? Its region holder `DESTROY`s it.
  Milestone 133 (ending a permanently blocked thread, and deciding who may), ruled by calef on
  2026-09-03 (proposal A), finishes a `Blocked` resident in place without waking it
  (`sched::finish_blocked_resident`). Milestone 254 (a caller stranded by a server that died) wakes
  every caller such a server held a reply capability for with `abi::Error::Gone`
  (`sched::strand_callers_of`). Each is tested on all three architectures: 133 in
  `system_tests/src/user/force_kill_tests.rs`, 254 in `kernel/src/sched.rs`. Their composition is
  argued in 133's block and run nowhere. By the code, a hang costs no unreclaimable region. A holder
  of only the supervision endpoint still cannot end a child. That is 133's refused proposal B, open
  behind a customer, and every supervisor here is also its child's builder.
- **Outstanding.** No supervisor demonstrates it. `swapper`'s `ROLE_HUNG` deletes the incumbent's
  region capability in `start_child`. It frees the stranded client by a cooperative `NOTE_RELEASE`.
  The task: keep the incumbent's region and `DESTROY` it after `STARTED`, with no release. Assert
  that the region returns and that the swallowed request's caller sees `Gone`, on all three
  architectures. Recorded 2026-10-08.
- **Done.** The advisory warning, 2026-09-27, as ruled in DECISIONS §231 (a swap's warning to a
  dependent is advisory, and the supervisor never waits for it): `broker` reads a warning page on
  every wakeup and `swapper` signals a notification bound to it and never waits, so a dependent that
  does not answer no longer hangs the supervisor. Tested on time, never, and late
  (`ROLE_LATE_WARNING`) on all three architectures. notes/non-cooperative-fallback.md.
- **Done.** A handoff is a run of pages, 2026-09-26: `Handoff::pages`, minted with
  `MemoryRegion::RETYPE`'s page count, DECISIONS §233 (`MemoryRegion::RETYPE` takes a page count).
- **Done.** `line_editor` is swapped live, 2026-09-27, by `terminal_supervisor` (name provisional,
  calef's to name), which `system_initializer` now builds in its place. Rulings: DECISIONS §232 (the
  `line_editor` swap contract: a quiesce opcode on the served endpoint, and a retry flag), whose
  `FLAG_RETRY` `swish` and `rmle` handle, and option A of `design/roadmap/0661-a-terminal-supervisor-holds-the-line-editor.md`
  on 2026-09-27. `kernel::user::terminal_swap_tests` swaps it twice under a typist on all three
  architectures, carrying the half-typed line and the history. notes/interactive-stack-swap.md.
- **Milestone 694.** Milestone 694 (the installer asks the terminal to swap). `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md`, PROPOSED
  2026-09-27: the trigger. Nothing on a real boot asks the supervisor to swap yet.
- **Outstanding.** `display_terminal` and `compositor` are not swapped. Milestone 600 (the
  graphical terminal stack is built in userspace) moved `display_terminal`'s construction into
  `system_initializer`, so a supervisor could now hold it; what it lacks is a declaration and a
  handoff for a grid of several hundred KiB. The compositor runs only in the tests of milestone 33
  (a compositor: one screen, mutually distrusting clients). Checked 2026-09-27.

## Index row

the flagship payoff, and a product ambition
