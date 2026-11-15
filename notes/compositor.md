# The compositor

Milestone 33, rung two of the display ladder. One screen, several mutually distrusting clients, each
holding a capability to its own surface. The code halves are `crates/compositor` (the contract and
the pixel arithmetic), `components/src/compositor.rs` (the compositor), and `fixtures/src/window.rs`
(a client); this is the prose half, the same split
[framebuffer-contract.md](framebuffer-contract.md) makes for rung one and
[terminal-contract.md](terminal-contract.md) for the terminal.

Rung one asked whether an unprivileged process could put pixels on a screen without holding a
device. Rung two asks the question that makes a compositor interesting: **can several processes
share one screen without being able to reach each other?**

## The shape

```text
  virtio-gpu ──virtio (PCIe, IOMMU)──► display ──gfx FLUSH(damage)──► compositor ◄──doorbell CALL── clients
                                        │      (rung one's contract)    │  │                        (a surface each)
                                        └───────── the scanout, shared ─┘  ├── input ring (shared with the
                                                                           │   input source only)
                                                                           └── one input endpoint per
                                                                               focusable client
```

The compositor is rung one's client, unchanged at that seam. It holds the display endpoint and the
scanout frames exactly as `painter` did, and `gpu_driver` cannot tell the difference; three of the
four kernel tests replace `gpu_driver` with the kernel itself and the compositor does not notice
that either. That was the promise the framebuffer contract made when it said routing was by
endpoint. It cost nothing to keep: `gpu_driver` and `graphics_contract` needed no change for this
milestone beyond one new kernel-side wiring entry point (`display_service::start_driver`, the driver
with no client).

What each party holds:

| | compositor | an honest client | a capture client |
|---|---|---|---|
| slot 0 | report endpoint | report endpoint | report endpoint |
| slot 1 | display endpoint, WRITE | the doorbell, WRITE | the doorbell, WRITE |
| slot 2 | the doorbell, READ | its input endpoint, READ (focusable only) | *(empty)* |
| slots 3.. | one input endpoint per focusable client, WRITE | | |
| mapped RW | the scanout, the window list, the input ring, every client's control page and surface | its own control page and surface | its own control page and surface |
| mapped RO | | | **the screen and the window list** |
| knows | nothing about any device | nothing about the screen, its own position, or its neighbors | |

## The idea the whole design rests on: the doorbell carries no authority

Every client rings one shared endpoint, and every request on it is content-free. There are two
verbs, `HELLO` ("I have started") and `COMMIT` ("look at the surfaces"), and neither takes an
argument. That is not minimalism for its own sake.

A shared endpoint carries no sender identity. There are no badged capabilities here. Item 5 of
DECISIONS §26 (the fault endpoint) records that decision and what would bring it back. So a server
receiving on an endpoint that several clients hold cannot tell which one sent a message. A protocol
that named a surface, a window, or a rectangle in its *message* would therefore be forgeable by any
client: `flush(window 2)` from the holder of window 0 would be indistinguishable from the real
thing. That is the vulnerability this rung exists not to have.

So the design inverts it. The message says nothing; the memory says everything.

- Every per-client fact lives in per-client memory. A client's geometry, its window id, its damage
  rectangle and its sequence counter are fields in a control page that only it and the compositor
  map (`compositor::proto::ctl`). The only surface a client can describe is its own, because the
  only control page it can write is its own.
- Every privileged answer travels through privileged memory, never through a reply. A screenshot is
  a read-only mapping of the screen. The window list is a read-only page the compositor publishes.
  There is no enumerate verb and no screenshot verb, so there is nothing for a hostile client to
  call and nothing for the compositor to guard.
- Keystrokes arrive in memory too, an input ring the input source shares with the compositor and
  nobody else. A keystroke carried in a message word would let any client inject input into the
  focused client; a keystroke in a page no client maps cannot be forged at all.
- The reply words carry status only, and the kernel's one-shot Reply capability routes them to
  whoever called (DECISIONS §12). So a request is answered correctly without the compositor ever
  learning who asked.

The result is a compositor with no authorization code in it. It never asks "may you?" about
anything, because there is no request it could receive that would need the question.

That is the difference in kind from Wayland: a Wayland compositor decides in code what each
identified client may do, and here the authority to read anything is a mapping the kernel
made at spawn ([compositor/wayland.md](compositor/wayland.md)).

## What a frame is

1. A client paints its own surface, writes its damage rectangle into its own control page, bumps its
   sequence, and `CALL`s `COMMIT`.
2. The compositor drains the input ring (below), then walks every client's control page. A sequence
   that changed means new pixels: clip that client's rectangle to that client's surface, place it on
   the screen, and union it into this frame's damage.
3. Composite the damage: background first, then every window in stacking order, each clipped to the
   damage. Opaque windows, so the last writer wins and the stacking order is visible in the result.
4. `gfx FLUSH(damage)` to the display, and reply to whoever rang.

The caller is blocked in `CALL` for the whole of composition, which is what makes reading a client's
pixels safe with no lock and no double buffering: the client that rang cannot be writing while the
compositor reads. That is flow control by rendezvous rather than by trust, and it is the same
argument [line-discipline.md](line-discipline.md) makes for the terminal. A client that rings on
*another* client's behalf gains nothing, since it is still only its own control page it can write.

A client's rectangle is untrusted input, so it is clipped, not believed. Rung one *refuses* an
out-of-surface rectangle rather than clamping it, on the grounds that the caller is the only party
who can tell a coordinate bug from intent. Rung two clips, and the reason is the same identity-free
doorbell: the compositor scans every surface, so it cannot attribute a bad rectangle to a caller in
order to refuse *that caller's* request. And the worst a lie can do is make the compositor re-copy
the liar's own pixels. So it clips and records `STATUS_CLIPPED` in the liar's own control page,
which is per-client feedback through the only channel that can carry it.

## Focus is a capability, not a variable

Three questions Unix conflates, separated here:

- Who may deliver input? Whoever maps the input ring. That is the input driver, and in the tests it
  is the kernel playing the driver's part (`Wiring::type_bytes`). No client maps it, so no client
  can inject a keystroke into another. There is no "grab the keyboard" verb to guard because there
  is nothing a message could say that would do it.
- Who may receive it? The focused client, because it *holds an input endpoint*. A client without one
  cannot be sent a keystroke by anyone, and its attempt to receive on the slot where one would be is
  refused by the kernel with `NoSuchSlot`. Holding the endpoint is what makes a client eligible for
  focus at all.
- Who decides? The compositor, in userspace, on policy of its own (TAB moves focus to the next
  window). The kernel routes the message and knows nothing about focus. The decision is *published*
  in the window-list page, so a holder of that page, and the kernel test, can witness a focus change
  rather than ask about it.

Input reaches the focused client as `terminal_contract::OPERATION_BYTES` over a `CALL`: the
terminal contract's driver half, verbatim (notes/terminal-contract.md). So a terminal is a client of
this compositor without either contract changing, which is what rung three needs and the reason the
framing was reused rather than reinvented.

That claim was cashed on 2026-07-30: two display terminals side by side, with a keystroke typed at
each and TAB between, and the kernel compares every pixel of the composed screen. It taught one rule
worth stating here: a client must not ring the doorbell in response to input, because the compositor
is blocked in its `CALL` to that client and the pair deadlocks. It does not need to, since the input
source rings `COMMIT` itself. The test and the confined keyboard driver it brought are in
[compositor/terminal-focus.md](compositor/terminal-focus.md).

## No ambient display, in two dialects

The roadmap's requirement is that window enumeration, screenshots, and screen sharing be **grants,
not defaults**, and that a refusal read as "you hold no such capability" rather than as a permission
error. Both halves are proved, and the refusal turns out to have two forms:

- An empty capability table slot. A client that was not granted an input endpoint has *nothing* in
  slot 2. Its `RECEIVE` there returns `abi::Error::NoSuchSlot` (-1), whose doc comment has said the
  right thing since milestone 7 (user mode): "The slot is empty. Not permission denied: there is
  nothing there." The test asserts on exactly that value, because `NotPermitted` would mean the
  authority existed and was withheld, which is a different and weaker world.
- An unmapped address. A client that was not granted the screen has no mapping where the screen
  would be. Its read faults, the kernel kills it, and the test observes the fault (and, on aarch64,
  the exact faulting address). "There is nothing there" again, in the address space's dialect
  instead of the capability table's.

A capture client holds a read-only mapping of the screen and the window list, so it can screenshot
and enumerate with no verb to call, and its attempt to write the screen faults. It is also the
screen-sharing case: one grant with three uses, revocable through `PageFrame::REVOKE`
([compositor/screen-sharing.md](compositor/screen-sharing.md)).

## How the isolation, the damage and the picture are proved

Each claim is proved rather than asserted, and the full account is in
[compositor/proofs.md](compositor/proofs.md). Isolation is proved from four directions in one kernel
test: an attacker handed its neighbor's real, adjacent address faults on the write, never reports,
and leaves the victim's pixels unchanged by both the kernel's digest and the victim's own. The
damage rectangle is proved on the host and in the guest, where the kernel plays the display and
checks that the flush was exactly the damage and that poison outside it survived. The picture has
four witnesses, the last being the host reading QEMU's `screendump`, because nothing in the guest
can see the device's own surface; its negative control rejects a z-order inversion and a missing
window.

## The primitive this rung wants and does not have

The most useful finding of this milestone is a limit, and it shaped everything above.

A process here has exactly one blocking wait point. A thread can be parked in one `RECEIVE`. There
is no wait-any and no non-blocking receive (DECISIONS §24 (interrupting the foreground process)
records the same gap from the shell's side). And two threads cannot share an address space
(`ThreadControlBlock::CONFIGURE` *consumes* the address-space capability, and the address space dies with the thread). A
compositor has three classes of sender: its clients, an input source, and whoever may read the
screen. Distinguishing classes of sender is what rendezvouses are for, and one rendezvous per class needs
one wait point per class.

So the constraint is structural, not stylistic: **a component that must distinguish more than one
class of sender must either be more than one process, or route everything through one rendezvous and
carry authority somewhere else.** This design took the second road, and it turned out well (the
memory-carried authority is stronger than a per-class rendezvous would have been, and it removed
authorization code rather than adding it). But the road was not chosen freely, and the honest record
says so.

Either form of the primitive, a wait-any or threads sharing an address space, would change three
things. The compositor could hold one rendezvous per client and gain sender identity, a screenshot
could be a consistent served copy, and input delivery could stop being a blocking `CALL`
([compositor/missing-primitive.md](compositor/missing-primitive.md)).

Both forms are real design work with real consequences (a shared address space raises lifetime and
revocation questions; a wait-any primitive widens the §4 (kernel shape) syscall surface), so neither
is something this milestone gets to decide. It is recorded in DECISIONS §33 (the compositor's
authority is memory, not messages) as the fork it is.

## Who is trusted with what, stated exactly

The claim this rung proves is client-to-client isolation.

The compositor sees every client's pixels. It has to: compositing is reading them. So a client's
confidentiality is against *other clients*, not against the compositor, and `compositor` is in every
client's trusted computing base for the contents of its window. That is true of every compositor,
Wayland included, and it is the reason the interesting question was never "can the compositor be
prevented from reading a surface" but "can a client be". What the kernel does buy here is that the
compositor's authority is enumerated in one spawn literal and cannot grow. It holds no device, no
interrupt, no DMA authority, no physical address, and no way to name a frame it was not handed. A
compromised compositor can lie about the screen and read the windows it composites. It cannot reach
the disk, the network, another process's memory, or the GPU's command stream (that last one is rung
one's confinement, and it is why the driver is a separate process).

The display driver sees the composed screen and nothing else of the clients: it never maps a client
surface. The kernel is trusted absolutely, as always here, and in the tests it also plays the roles
a full system would give to separate components (the input driver, and the display server in three
of the four tests). That is worth saying so that "the kernel checked it" is not mistaken for "a
distrusted component was checked".

## What this does not do that a real compositor would

- No window management. The scene is a compile-time constant (`compositor::SCENE`): three windows,
  fixed sizes, fixed positions, fixed stacking order. A real compositor learns its windows from
  clients that ask for surfaces and from a user who moves, resizes, raises, and closes them. Nothing
  here negotiates a surface; the kernel grants three at spawn. That is also what makes the composed
  screen a value a test can predict, which is why rung two is built this way and rung three would
  have to change it.
- No alpha, no transforms, no scaling. Windows are opaque and composition is a copy. Blending is
  arithmetic the crate could grow; it would not change any authority question.
- One damage rectangle per frame, as a bounding box. Two small changes far apart cost the rectangle
  that contains both. A real compositor keeps a region (a list of rectangles) and pays only for the
  parts. The union is a few extra pixels of copying here and the wrong trade at desktop resolution.
- Software composition only, which at 128x64 is nothing and at 4K would be the whole cost. Rung
  four (milestone 34) is where a GPU does this, and this milestone deliberately does not start it.
- A screenshot can tear. The capture grant is a live read-only mapping, so a reader that looked
  during a composite would see a half-composed screen. The tests read it at a quiet moment. The fix
  is the served-copy path in the section above, which wants the missing primitive.
- A window can tear too, and the reason is sharper than the screenshot's (milestone 43's audit,
  notes/shared-page-audit.md finding 5). The compositor composites every committed window, not only
  the one that rang, so a client that did not ring can be writing its damage rectangle while it is
  read. For reads it is bounded to tearing, never a pixel outside a surface, because every slice
  length and clip comes from `compositor::SCENE`. A guarantee means per-client double buffering, a
  design decision this note does not take. The full account is in
  [compositor/window-tearing.md](compositor/window-tearing.md).
- The compositor holds every client surface read-write and never writes one. Read-only there
  would make "the compositor cannot deface a client's window" a fact about the mapping rather than
  about the code, exactly as `ROLE_CAPTURE`'s read-only screen already is. Recorded by the same
  audit and not taken in it, because flipping a mapping wants a test that proves the fault.
- No defense against denial of service. A client can spam the doorbell, never answer an input
  `CALL`, or never reply, and the compositor's single thread will slow or stall. Confidentiality and
  integrity are what this rung proves; availability against a hostile client needs the same missing
  primitive plus a policy, and Wayland does not solve it either.
- No vsync, no frame pacing, no cursor. There is no display interrupt to wake on (virtio-gpu's
  cursor queue is untouched, as rung one left it), so every frame is driven by a client's commit.
- The window list is the scene, not live state. Enumeration returns the fixed geometry plus the
  live focus. With window management would come a list that changes, and the page layout already has
  room for it.

## Where the pieces are

| piece | file |
|---|---|
| the contract and the pixel arithmetic, host-tested | `crates/compositor/src/lib.rs` |
| the compositor | `components/src/compositor.rs` |
| a client, with its roles and its attacks | `fixtures/src/window.rs` |
| the wiring (frames, endpoints, grants) | `kernel/src/user/compositor_service.rs` |
| the tests | `system_tests/src/user/compositor_tests.rs` |
| the host-side scanout check and its negative control | `xtask/src/scanout.rs` |
| the display driver it flushes to, unchanged | `components/src/gpu_driver.rs` |
