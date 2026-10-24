# How the compositor is proved rather than asserted

*Name: provisional.*

An appendix to [`notes/compositor.md`](../compositor.md), which states the conclusion.

## How the isolation is proved rather than asserted

This is the thesis content of the rung, so it is proved from four directions at once, in
`kernel::user::compositor_tests::a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen`.

The attacker is given every advantage short of a capability:

- It is the same binary as the honest client, with the same grants. It paints its own window and
  reports correctly first, because an attack that failed for its own reasons would prove nothing.
- The kernel hands it the exact virtual address at which its neighbor's pixels sit, the way
  milestone 29 (a display terminal) and its escape test is handed its victim frame.
- That address is real. Every client maps its surface at the same virtual address, so this is the
  number the neighbor itself uses. And the kernel deliberately allocates every client's frames from
  one contiguous run, so the page just past the attacker's grant genuinely is its neighbor's memory.
  The test asserts that adjacency before believing anything else, because an attack on an empty
  hole would "pass" while proving nothing.

Then:

1. The write faults (both ISAs), and on aarch64 the faulting address is exactly the one it was
   handed.
2. The attacker's own report endpoint is silent afterwards. It would have sent `WIN_ESCAPED` with
   the value it read back had the access succeeded. So this is the negative half stated as an
   observation (`endpoint_waiting_senders`, the same trick milestone 22 (trusted init) uses to say
   "and then nothing happened" without hanging).
3. The victim's witness pattern is unchanged, digested by the kernel through the direct map, from a
   value the kernel computed itself out of the contract.
4. And the victim re-reads its own surface and reports the digest again. It is held in a `CALL`
   across the whole attack, so "after" really is after the attacker is dead. The second witness
   lives in the victim's own address space rather than in the kernel's account of it.

A read fault proves the page is not mapped *at all*. That is why the two probes in this test are a
write (integrity, at a neighbor's pixels) and a read (confidentiality, at the screen). Both
directions are exercised against real hardware behavior rather than one being argued from the other.

## The damage rectangle, proved end to end

Rung one put a damage rectangle in the contract, on the argument that a compositor redrawing one
window should not pay for the whole screen. Rung two is where that becomes checkable, and it is
checked twice.

On the host, in microseconds: `crates/compositor`'s tests poison a screen buffer and composite one
window's damage. Then they assert that every pixel outside the rectangle still holds the poison
while every pixel inside holds the composed picture.

In the guest, end to end: the kernel plays the display server, so the flush rectangle is a value it
can compare. The test poisons the real scanout frames between two frames of one client. After the
second commit it asserts three things: the flush was exactly `damage_to_screen(window,
SMALL_DAMAGE)`, one commit produced exactly one flush, and the poison outside that rectangle
survived. A compositor that quietly repainted the screen every frame would erase the poison and
fail, rather than merely being slow in a way no test could see.

That is also the reason three of the four tests use a kernel stand-in for the display rather than
the GPU. It does save a bring-up, but that is not the reason. A real driver honors the rectangle and
says nothing about it, so the flush has to be observed somewhere it can be read.

## Proving the picture, and why the host has to be involved

Four witnesses, because a compositor's output is exactly the thing a guest-side digest cannot
confirm:

1. The display driver, digesting the frames it handed the device after the device reported the
   transfer complete. Its one status report covers the compositor's *startup* frame, which is the
   background alone (no client has committed yet). So it doubles as the check that an empty screen
   is a defined picture rather than whatever was in RAM.
2. The kernel, reading the scanout frames through the direct map and comparing every pixel against
   `compositor::expected_screen_pixel`, which it computed from the contract.
3. A capture client in its own address space, reading the screen through the read-only mapping that
   is its screenshot capability, and digesting it.
4. The host, through QEMU's monitor. `cargo xtask` dumps the scanout with `screendump` beside the
   running suite and compares the PPM against the same per-pixel definition.

The fourth is not decoration. `-display none` means nothing in the guest can see the device's own
surface. A wrong pixel format, a wrong scanout rectangle, or a compositor writing its picture
somewhere other than the scanout would satisfy all three in-guest witnesses and show garbage on a
screen. Milestone 29 built that check, and this milestone made it prove two pictures over one boot,
in order. The composed screen comes first (the compositor test holds it up for three seconds so a
250 ms poll cannot miss it), then rung one's pattern, which stays up until QEMU exits. Both must be
seen or the run fails. The ordering is part of the check, so a reordering of the suite fails loudly
instead of being waved through.

The composed check has its own negative control (`cargo test -p xtask`), because rung two's failure
modes are not rung one's. It must reject a z-order inversion and a missing window. Both are pictures
made entirely of correct pixels in almost the right places. That is what a compositor actually gets
wrong, and what an "is it not black?" checker would happily accept. It must also reject rung one's
pattern, and rung one's checker must reject the composed screen, or the poll loop's ordering would
mean nothing.
