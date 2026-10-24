# The compositor tests, in detail

An appendix to [notes/system-test-modules.md](../system-test-modules.md). The doc comments in
`system_tests/src/user/compositor_tests.rs` say what each test proves. This page holds why each is
shaped the way it is. The compositor itself is in [notes/compositor.md](../compositor.md), and which
falsification patch reaches which part of claim 25 is in
[notes/confinement-claims.md](../confinement-claims.md).

## The kernel's display stand-in

Every test but one takes a display the kernel serves itself, with no device behind it. That is not a
shortcut. It keeps many device bring-ups down to one, and it makes the flush rectangles observable.
A real driver honors the rectangle and says nothing about it. The stand-in records it, which is how
"a one-window redraw does not cost a whole screen" becomes an assertion instead of a claim. It is
also the swappable-component property for free: the compositor cannot tell whether the endpoint it
flushes to is a virtio-gpu driver or the kernel.

## No capability for a neighbor's pixels or the screen

The first claim-25 test proves isolation from four directions. The attacker gets every advantage
short of a capability. It is the same binary as the honest client, with the same grants. The kernel
hands it the exact virtual address of its neighbor's pixels, one page past its own surface, where
the contiguous allocation really put them. The test asserts that adjacency, so the attack cannot
quietly become a poke at nothing. Every client maps its surface at the same virtual address, so this
is also where the neighbor sees its own pixels. The attacker still cannot touch them, because the
boundary is the mapping and not the layout.

1. The refusal that needs no attack. Receiving on the ungranted input slot answers `NoSuchSlot`,
   "there is nothing there", not a permission error from a server that consulted a list. Slot 2 is
   empty because the spawn literal left it empty, and that emptiness is the whole difference between
   this client and a focusable one.
2. The write faults, at exactly the address it was handed.
3. Nothing was written. The victim's pattern digests identically before and after, from two
   independent readers: the kernel through the direct map, and the victim through its own mapping
   after the attacker is dead. The victim is held in a `CALL` across the attack, so "after" really
   is after.
4. No ambient display. A third client, which painted into this very screen, reads the address where
   the screen is mapped in the compositor and the capture client, and faults. It holds no mapping of
   the screen, so there is nothing to read and no verb to ask with.

The two probes are a write (integrity) and a read (confidentiality), so both directions are
exercised on real hardware behavior rather than argued from one.

## Claim 25, part by part

The four-direction test proves four things in one body, and a patch recorded against it reaches only
one. The others sit behind the fault wait, behind a write probe that cannot see a read-only
exposure, or behind an address the fixture never maps. So each part has its own test and its own
replayable falsification.

Part 2 reads rather than writes, because a spawner that exposed the neighbor's pixels read-only
would leave a write probe green while every pixel leaked.

Part 3 reaches the witness digests without waiting on a fault. The compositor has write access to
every client's memory: it publishes each control page and writes `STATUS` and `ACKED` back. Feeding
it lying damage rectangles tests that the address it writes is never a function of what a client
said.

Part 4 asks for the screen mapping exactly as a capture client does, and then reads. A grant alone
maps nothing, so a probe that never tried to map would stay green under a spawner that leaked the
screen capability to every client. With nothing granted the map is refused and the read faults; with
the screen leaked read-only the map succeeds, the read returns a pixel, and the fault wait fails.

The forged-control-page test attacks everything in the control page besides the damage fields. It
forges `ID`, `WIDTH`, `HEIGHT`, `STRIDE` (a zero, and `stride = 1` with a width near `i32::MAX`),
`STATUS`, `ACKED`, and a `SEQ` that jumps back to zero and then forward to `u32::MAX`. The page is
the client's own granted memory, so writing it is not an escape. Re-reading its geometry for
clipping, or steering the `ACKED` and `SEQ` bookkeeping with a forged value, would be: a confused
deputy writing through someone else's words. An error answer to a commit takes the client down at
`E_COMMIT`, which the test reads as a `0xDEAD` word.

## Damage, input and focus

The redraw test's poison is the same technique the compositor crate's host test uses in
microseconds. Finding the poison intact outside the expected rectangle is the proof that nothing
repainted it.

The input test separates three things Unix conflates. Any client may ring the doorbell, but the
keystroke arrives in a ring page shared with the input source alone, so no client can inject one.
There is no "grab the keyboard" verb to guard, because nothing a message could say would do it. A
client without an input endpoint cannot be sent a keystroke by anyone, however the compositor feels
about it. Focus moves on a byte, in userspace, in the compositor; the kernel routes the message and
knows nothing about focus.

The terminal focus test makes the same routing visible in the picture, a stronger statement than the
endpoint-level one and a different kind of evidence. Terminal 1 receives the `B` because it holds an
input endpoint, not because it asked and not because the compositor consulted a list (§33 (the
compositor's authority is memory, not messages)). The compositor's whole part is choosing which of
its capabilities to use. `compositor` cannot tell a display terminal from the `window` client: same
grants, same control page, same doorbell, same `COMMIT`. Neither `compositor` nor
`graphics_contract` changes to carry text, which is the seam claim made once at each rung. The test
asserts that the two terminals' pictures differ, because if the scripts ever printed the same text
the comparison would pass while proving nothing.

## Four witnesses to one scanout

The end-to-end test runs rung one's driver unchanged: the compositor takes `painter`'s place at that
seam, and `gpu_driver` cannot tell. Its four witnesses are the driver's digest of the frames the
device took and the kernel's pixel-by-pixel comparison. Then come a capture client's digest through
its read-only mapping (that mapping is the screenshot capability), and the host's `screendump`. The
host witness is not optional. Under `-display none` nothing in the guest can see the device's own
surface, so a wrong pixel format or scanout rectangle would pass all three in-guest witnesses and
show garbage on a screen.

The capture client proves the grant's shape twice. It enumerates the windows out of the read-only
page the compositor publishes, since there is no enumerate verb to call. Its write to the screen
faults, because a thing that may look at the screen may not draw on it.
