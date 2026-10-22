---
status: BUILT
raised: 2026-07-25
built: 2026-10-19
---
# 23. A capability-routed component OS with live replacement

Every residual this block named is built, and so are the advisory warning (§231), the live swap of
`line_editor` (§232 (the line editor swap contract)) and of `display_terminal`, both under a
terminal supervisor that notices a hang on its own. Two pieces have milestones of their own: the
installer asking the terminal to swap (milestone 694), and the compositor's swap (milestone 932 (swap the compositor live)),
which waits for a boot that runs a compositor.

## The idea

Every userspace component (driver, server, app) is a swappable, vendor-shippable unit behind a
stable contract, and operators replace them live, with no reboot. A client names an endpoint, never
a peer (§12 (call/reply IPC: a one-shot reply capability)), so a component's identity is invisible
to the code that uses it. Any program that speaks the protocol and holds the right capabilities is
the component. The
kernel is the one fixed thing underneath an entirely swappable userland. This is Fuchsia's shape
(capability-routed components, stable protocol interfaces) on a verified core.

It is also the product story. The kernel confines each component to what it was granted, so
competing vendor components run safely. A Linux vendor module is ring 0, and a nife vendor component
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
swap and is its own witness. An attacker holding the client's exact capabilities cannot become the
server. The outgoing instance reads the UART after the revoke and faults at the device's own page.
And the replacement is written in C over §31 (the foreign-language seam: C holds no capabilities and
makes no syscalls), so what held across the swap is the contract.

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

`crates/capability_plan`, notes/component-manifest.md. `swapper` holds no endowment literals: each
contract declares its capability half in `hot_swap_contract`, and the operator says only which of its
own objects answers to which role name, per child. A manifest is a request and the provisions are the
authority (Fuchsia's `use`/`offer` split), so a vendor's declaration cannot widen its own authority.
The role name is the component's and the object is the supervisor's, which makes a component's peer
substitutable too. A manifest is still compiled in rather than shipped beside a binary; see
Follow-on.

### The hung component (2026-08-17)

notes/hung-component.md. Against an incumbent that stops answering without dying, three of the four
steps are unchanged: the one that needs its cooperation, the drain, is the one a hang makes
redundant. So a supervisor restores the service with no authority it did not already hold, which
corrects the sentence in §32 (a supervisor may collect a corpse) that restarting a hung child needs
the stronger right. Reclaiming its memory needs the region capability, which the builder holds if it
keeps it. The domain reports a hang as `BLOCKED`, exactly what a healthy idle server reports, and
`Endpoint::REAP` answers `StillAlive` for every member, so a supervisor notices only by asking: its
quiesce goes unanswered within a bound. Destroying the hung component's region then finishes it in
place and answers its stranded caller `abi::Error::Gone`.

### Dependency-aware orchestration (2026-08-23)

`component_plan::depends_on` and `dependents`, notes/dependency-orchestration.md. A component names
the contracts it cannot silently tolerate the absence of, and a supervisor asks the graph who to warn
before a swap. Only a component that forwards synchronously while serving others ever needs an entry:
a pure consumer degrades for free on the endpoint's sender queue, §41's mechanism doing a second job.

### State handoff (2026-09-26)

notes/state-handoff.md, to the ruling in DECISIONS §209 (state handoff is an opaque blob over a
granted frame, and it is optional). `Requirements` gained the optional `handoff` field (provisional,
as §209 asked), `hot_swap_contract` a stateful contract, and `swapper` a fourth role, `ROLE_HANDOFF`,
that tries the swap twice. First against a replacement that cannot read the incumbent's blob, which
refuses, is reaped, and leaves the incumbent serving with its state. Then against one that can,
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

### The terminal, swapped live (2026-10-19)

calef ruled four questions on 2026-10-19 (UTC), and `display_terminal` is swapped to them.

- **A component's spawner supervises its swaps** ("spawner supervises"), with no new supervisor
  program and no central swap service. `terminal_supervisor`, in display mode, holds
  `display_terminal` beside `line_editor`, builds both through `supervision_protocol`, and
  `SWAP`'s second word names the child.
- **The replacement comes from a dedicated swap reserve** ("D"): one per machine, apart from the
  job pool (`JOBS_BUDGET_PAGES`), sized to the largest swappable instance plus its handoff
  (`supervisor::SWAP_RESERVE_PAGES`, 396 pages). Whoever drives a swap splits it whole and lends
  it with `LEND`, a `SEND_CAP` before the `SWAP`. The supervisor builds the replacement and the
  handoff run in it, moves the terminal into it and then home to its own bay with the same image,
  and deletes its capabilities before answering. The lender's `DESTROY` then revokes whatever was
  made from it. A second swap while the reserve is lent finds nothing to split and is refused
  until the first is answered; a display swap with nothing lent is answered `NEEDS_RESERVE`. The
  holder is milestone 694's trigger. That is not built, so the reserve is held where today's
  trigger lives, the kernel's test.
- **A swap carries everything** ("everything"): the whole `Vt`, grid and scrollback, with the
  cursor, the rendition and the parser's state, as a §209 blob (`video_terminal::handoff`). It is
  twelve bytes a cell, so the largest is 132 pages, not the 176 the sixteen-byte cell gave.
  **A replacement reads its predecessor's layout** (calef, 2026-10-19, "b"): a build that changes
  the layout bumps `LAYOUT` and keeps the old reader, and `video_terminal`'s host tests absorb a
  checked-in blob of every earlier layout. A blob for another grid shape is refused, and the swap
  rolls back; that limit is in `video_terminal::handoff`'s `BUGS`.
- **The compositor is its own milestone** ("Split"): 932.

Measured on 2026-10-19 (UTC) with `MemoryRegion::USAGE`, debug images: an instance commits 207 to
212 pages and its map budget 4. `system_tests::user::terminal_display_swap_tests` swaps a
supervised `display_terminal` twice on the kernel's display stand-in, on all three architectures. It
compares every scanout pixel with an engine the kernel runs, and the blob carried home with the
one that engine writes, byte for byte. A 24-bit color cut in half by the swap finishes on the
other side. A second lend while the first is out is refused, and the retried swap commits.

The test found a fault in the scroll fast path: a write that scrolled several rows left the tail
of a wider row on the screen. `video_terminal::Vt::take_damage` now renders the rows such a move
exposes, and a host test plays the painter.

## Follow-on

- **Done.** A handoff is a run of pages, 2026-09-26: `Handoff::pages`, minted with
  `MemoryRegion::RETYPE`'s page count, which calef ruled the same day
  (`design/roadmap/0659-a-region-retypes-a-frame-run.md`, option A).
- **Recorded.** A manifest is compiled in rather than shipped beside a binary. The ELF-note manifest
  work, milestone 597 (a program carries its manifest in an ELF note), carries a program's grant
  manifest rather than a component's declaration; notes/component-manifest.md's `BUGS` carries the history.
- **Done.** The timed wait a supervisor needs to notice a hang exists: milestone 106 (a wait that
  ends on either the interrupt or the deadline) is BUILT 2026-09-26. Milestone 263 (can a userspace process hold a timer, on all
  three architectures) was RECORDED rather than a blocker. `swapper` and `terminal_supervisor`
  use it for their bounded quiesces (below).
- **Done.** What may be done to a component that never cooperates? Its region holder `DESTROY`s it.
  Milestone 133 (ending a permanently blocked thread, and deciding who may), ruled by calef on
  2026-09-03 (proposal A), finishes a `Blocked` resident in place without waking it
  (`sched::finish_blocked_resident`). Milestone 254 (a caller stranded by a server that died) wakes
  every caller such a server held a reply capability for with `abi::Error::Gone`
  (`sched::strand_callers_of`). Each is tested on all three architectures: 133 in
  `system_tests/src/user/force_kill_tests.rs`, 254 in `kernel/src/sched.rs`, and their composition
  in the hung test below. A holder of only the supervision endpoint still cannot end a child. That
  is 133's refused proposal B, open behind a customer, and every supervisor here is also its child's
  builder.
- **Done.** A supervisor reclaims a hung incumbent, 2026-10-18. `swapper`'s `ROLE_HUNG` keeps the
  incumbent's region capability and `DESTROY`s it after `STARTED`, with no release: milestone 133
  finishes the `Blocked` incumbent in place and milestone 254 answers the client whose request it
  swallowed with `Gone`. `live_swap_tests::a_supervisor_notices_a_hung_component_and_reclaims_it`
  asserts that the budget's live-children pages fall by exactly one instance, that the incumbent
  leaves no death, and that the client saw `Gone` and was served by the replacement. On all three
  architectures: `ROLE_HUNG_WITHOUT_DEVICE` runs the same system on `x86_64`, which has no device
  page. notes/hung-component.md.
- **Done.** A supervisor notices a hang on its own, 2026-10-18. The incumbent's wedge is silent,
  and `swapper` asks for the drain with milestone 106's timed wait, through a stand-in because a
  `CALL` takes no deadline; no answer within `QUIESCE_BOUND_MS` (250 ms) is a hang. The bound is
  about forty times the slowest healthy answer measured on three ISAs, in wall time, since
  `script/test` runs TCG without `-icount`; the measurements are beside the constant in
  `components/src/swapper.rs`. The same test asserts the verdict, its order before the survey and
  the reclaim, and the absent drain.
- **Done.** `terminal_supervisor` notices a hung `line_editor` on its own, 2026-10-19. Its quiesce
  is made by a stand-in built from the supervisor's own image, which signals the answer on a
  notification the supervisor's timer signals at `QUIESCE_BOUND_MS` (250 ms). On no answer it starts
  the replacement without the state and destroys the incumbent's region, which answers every caller
  it held with `Gone`. Only a spawner that passes a swap endpoint copies the image and grants the
  extra budget (`line_editor::component::supervisor::OWN_IMAGE_ADDRESS`), so a boot pays nothing.
  `terminal_swap_tests::a_hung_terminal_is_noticed_within_the_bound_and_replaced_without_its_state`
  hangs a real `line_editor` behind a console that stops acknowledging. It asserts the verdict, a
  wait of at least the bound, `Gone` to the parked reader and the typist, a replacement that serves
  empty, and a budget that reclaims whole. A healthy quiesce took 2 to 8 ms across the three
  ISAs, so the bound is about thirty times the slowest; the measurements are beside it. That `swish` and `rmle` treat `Gone` as a failed read is the supervisor's `BUGS`: which
  terminal requests may be repeated is a contract question, and an architect's.
- **Done.** The advisory warning, 2026-09-27, as ruled in DECISIONS §231 (a swap's warning to a
  dependent is advisory, and the supervisor never waits for it). `broker` reads a warning page on
  every wakeup, and `swapper` signals a notification bound to it and never waits. So a dependent that
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
- **Milestone 694.** Milestone 694 (the installer asks the terminal to swap).
  `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md`, NOT-STARTED, raised
  2026-09-27: the trigger. Nothing on a real boot asks the supervisor to swap yet, and nothing
  on a boot holds the swap reserve.
- **Done.** `display_terminal` is swapped live, 2026-10-19, through the swap reserve, carrying
  everything; "The terminal, swapped live" above has the rulings and the test.
- **Milestone 932.** The compositor's swap, `design/roadmap/0932-swap-the-compositor-live.md`,
  split out by calef on 2026-10-19. It runs only in the tests of milestone 33 (a compositor: one
  screen, mutually distrusting clients), and its swap waits for a boot that runs it.
- **Recorded.** A `graphical_terminal` session still builds its `display_terminal` itself and
  hands `terminal_supervisor` only its endpoint, so a session's `display_terminal` is not
  supervised. Until something on a boot can ask for a swap, holding it would cost the session
  the image's 220 pages for nothing. `components/src/terminal_supervisor.rs`'s `BUGS` carries
  it.

## Index row

the flagship payoff, and a product ambition
