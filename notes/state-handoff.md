# State handoff

*The last residual of milestone 23 (a capability-routed component OS with live replacement), built
2026-09-26 to the ruling in DECISIONS §209 (state handoff is an opaque blob over a granted frame,
and it is optional). `crates/component_plan`'s `Handoff` and `Requirements::handoff`,
`crates/swap_protocol`'s `TALLY` and `serve_with_state`, `components/src/swapper.rs`'s `handoff()`,
and the guest test
`a_component_keeps_its_state_across_a_swap_and_a_swap_that_cannot_absorb_it_does_not_commit`. Read
notes/live-replacement.md first; this builds on its four steps.*

## What §209 asked for, and where each part landed

§209 ruled three things. Each has one home.

1. An opaque blob over a granted frame. One frame, retyped out of the operator's budget and
   routed under the role name `component_plan::HANDOFF_ROLE` to every instance of the contract. The
   operator never maps it and never reads it. The blob's layout is `swap_protocol`'s business,
   versioned there beside the only code that reads it (`LAYOUT_1`, `state_blob`, `absorb`).
2. Optional, declared beside `depends_on`. `Requirements::handoff: Option<Handoff>`, a required
   field with no default, so a contract cannot be declared without answering the question. Every
   declaration in the tree says `None` except `TALLY`.
3. Failure does not commit. The operator retires the outgoing instance only after the incoming
   one sends `NOTE_ABSORBED`. An incoming instance that cannot read the blob sends `NOTE_REFUSED`,
   exits without ever receiving a request, and is reaped; the operator then sends the incumbent
   `POKE_RESUME`, and it goes back to serving with the state it never lost.

## Three things the build settled

The handoff page cannot be taken back, so it is not deferred. The obvious design copies the
device: map the page into the incumbent, revoke it, map it into the replacement. It does not work.
`PageFrame::REVOKE` on ordinary memory is symmetric (`kernel/src/revoke.rs`,
`revoke_page_frame_run`): it deletes the invoker's capability too, because frame revocation exists to
make reclamation safe. Only a `DeviceFrame` revoke spares the invoker. So the page is mapped into
every instance at build time, like the witness page, and exclusivity comes from sequencing rather
than from revocation. The incumbent writes the blob inside its `OPERATION_QUIESCE` handler, before the
reply, and then blocks on its control endpoint. The replacement is started after that reply and
reads the blob first. `Plan::handoff`'s doc comment carries this. It is the reason `configure`
consuming the address-space capability did not matter either: nothing is installed after build.

A supervisor that cannot carry state is refused a component that has some. §209 does not say
this and needs it. A supervisor that routes no handoff page, wiring a stateful component anyway,
performs a kill-and-replace that looks exactly like a successful swap while the state goes to zero.
So `plan` treats the handoff role like any other need and refuses with `Unprovided`. The guest test
makes that its opening control: the client's routing table is asked for `TALLY` and refused before
anything is built.

Fresh and absorbing are told, not inferred. The first instance on a channel is started with
`START_FRESH`, a replacement with `START_ABSORB`. The tempting shortcut is to treat an empty page as
a fresh start, and it is the silent loss again: a zeroed page reads as a tally of zero. `absorb`
refuses a page with no marker on it, and a host test pins that.

## How "no state was lost" is proven

The stateful component answers each request with its tally, the count of requests the *component*
has served across instances, in the reply word the other channels use to echo the sequence number.
The client (`chatty`, unchanged) checks that word against the sequence number it sent. The two agree
on all 64 requests only if nothing was lost and no state was, so `SEQ_ECHOED` is the continuity
witness without the client knowing which system it is in. A replacement that restarted from zero
would answer request 40 with 0.

Two more witnesses agree independently. The operator's `ABSORBED` step carries the tally the
replacement took over, and the test asserts it equals the tally the incumbent reported at its second
drain, and equals the sequence number at which the client first saw version 2. The witness page
shows one version change, no gap, never backwards, so the refused attempt left no trace in the
conversation.

**It runs on all three architectures.** The stateful system has no device, so the x86 gap the direct
and hung systems skip on (`NO_UART_PAGE`) does not apply. The queued system never needed that skip
either and has lost it in the same change.

## EXAMPLES

A contract that carries state declares where its page goes, and a supervisor routes one frame to
both instances:

```rust
pub const TALLY: Requirements = Requirements {
    contract: "tally",
    caps: CONSOLE.caps,
    maps: BACKEND.maps,
    pages: INSTANCE_PAGES,
    depends_on: &[],
    handoff: Some(component_plan::Handoff { va: STATE_VA }),
};

let to_incumbent = Provisions { held: &[/* ... */, (component_plan::HANDOFF_ROLE, state)] };
let plan = component_plan::plan(&TALLY, &to_incumbent)?;
assert_eq!(plan.handoff(), Some(STATE_VA)); // wait for NOTE_ABSORBED before retiring anyone
```

Run the guest test alone. The filter does not narrow the architectures, per DECISIONS §19
(architectural parity is a tenet):

```sh
script/test --test a_component_keeps_its_state_across_a_swap
```

## BUGS

A handoff is a run of pages, not one page. `Handoff` carries a page count, and the supervisor mints
the run as one frame with `MemoryRegion::RETYPE`'s count (calef, 2026-09-26 UTC), because
`line_editor` with its history needs a little over 4 KiB. The
stateful fixture declares two pages and writes its tally on the second, so a run that stopped at
its first page would lose the state and fail the test.

A refusing replacement leaves because it chooses to. The operator cannot tear down a live child
(notes/hung-component.md, question 4), so "revoke the new grant" is the refuser's own exit plus the
reap that returns its region. A replacement that neither absorbs nor refuses is the hung case one
level out, and the operator, blocked in `receive` on its coordination endpoint, hangs with it. That is
the detection half of the non-cooperative fallback (notes/non-cooperative-fallback.md), and nothing
here answers it.

A hung incumbent cannot serialize. §209 says so and this does not change it: the blob is written
inside the incumbent's `OPERATION_QUIESCE` handler, so handoff recovers a planned swap and not the failure
it is most wanted for.

The handoff page stays writable in the outgoing instance until it exits. Revocation cannot take
it (see above), so an incumbent that ignored `POKE_QUIT` and woke later could overwrite a blob the
next swap reads. The witness page has the same exposure (notes/shared-page-audit.md). Closing it
wants a take-back for ordinary frames, which §132 (what `PageFrame::REVOKE` owes an overlapping run) declined to widen
revocation into.

The refusal is staged. No build in the tree writes `LAYOUT_2`; the refusing replacement is
`c_swappable` started claiming to understand only that layout. The operator's behavior on refusal
is real. The incompatibility is configured.

Every new name here is provisional: `handoff` (§209's own), `Handoff`,
`HANDOFF_ROLE`, `Plan::handoff`, `TALLY` and its contract string `tally`, `ROLE_HANDOFF`,
`START_FRESH`, `START_ABSORB`, `STATE_VA`, `LAYOUT_1`, `LAYOUT_2`, `STATE_MAGIC`, `state_blob`,
`absorb`, `serve_with_state`, `start`, `POKE_RESUME`, `NOTE_ABSORBED`, `NOTE_REFUSED`,
`step::ABSORBED` and `step::ROLLED_BACK`.

## See also

- DECISIONS §209 (state handoff is an opaque blob over a granted frame, and it is optional) and
  §116 (live component state handoff is declined, for want of a customer)
- notes/live-replacement.md, notes/component-manifest.md, notes/hung-component.md
- notes/non-cooperative-fallback.md for the case a handoff cannot reach
