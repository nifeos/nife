# The kernel's display wiring

This appendix to [`notes/framebuffer-contract.md`](../framebuffer-contract.md) carries the design
behind `kernel/src/user/display_service.rs`: how the kernel wires the GPU driver, its clients and
the firmware-screen terminal, and why each one holds what it holds.

## One run, one capability

The driver's DMA region is one contiguous, zeroed allocation: a page of rings and control buffers,
then the surface. It is registered with `crate::virtio` as one region and granted as one
`PageFrame` capability, because a frame capability names a run of pages (§102 (a Frame names a run
of pages)). At 1120x624 the surface is 683 pages. That many one-page capabilities would not fit a
sixteen-slot capability table.

Contiguity is also what the device needs. The surface must be one run of physical frames for the
device's backing to be a single memory entry and for the IOMMU domain to cover it as one range.
Zeroing means no stale descriptor or pixel ever reaches the device or the client.

The region records its own physical base in the tail of its first page, at
`abi::virtio::DMA_PHYS_OFFSET`. A driver the progenitor builds has no other way to learn it, since
the progenitor holds the run as a capability and knows no address either. The kernel's own test
wiring passes `arg1: 0` for the same reason, so the harness and the boot hand the driver one world.

## The fault slot

The supervisor writes each process's fault endpoint at `abi::fault::FAULT_EP_SLOT`, the last slot
of the table. A grant placed there would overwrite it, and nothing would complain: `grant_at` on an
empty slot succeeds. The failure would surface later as a faulting process that hangs instead of
being reaped, far from its cause. So each grant list's highest slot is checked against the fault
slot by a `const` assertion, in run terms.

## The escape test's victim frame

`start_backing_escape` spawns a driver that asks the device to read a frame outside its grant. It
gets the honest driver's world exactly: the same confined transport, region and interrupt. That
makes it a test of the barrier rather than of a missing capability.

The kernel picks the victim frame and passes it in `arg2`. The caller must know the exact address
to look for in the IOMMU's fault queue. A driver that guessed "the frame past my region" would guess
wrong, because the shadow page is allocated right after the region and is in the domain. The frame
is never freed. Returning it to the allocator while a device has been told to read it is the
use-after-free by hardware that [`notes/dma.md`](../dma.md) warns about.
[`iommu-escape-test.md`](iommu-escape-test.md) has why the test reads the fault queue.

## The terminal's authority

`display_terminal` takes `painter`'s place at the display seam with exactly `painter`'s authority:
a report endpoint, the display endpoint and the surface. It holds no device, interrupt or physical
address, and `gpu_driver` cannot tell it from the client that draws a test pattern. So the
framebuffer contract carries text unchanged, and the spawn literal is the evidence.

What the terminal adds belongs to the terminal contract: an endpoint it serves, and a page an
application writes bytes into (§10's control by message, bulk by shared page).

A caller of `start_terminal` or `start_driver` must take every blocking report. The driver sends
`UP`, then `FLUSHED` from inside its serving loop once the first flush is served. A caller that
stops after `UP` and `TERM_UP` leaves the driver parked and every later flush unanswered.
[`flushed-hang.md`](flushed-hang.md) has the hang this caused. The function does not swallow
`FLUSHED`, because the tests read its digest as the driver-side witness. The boot does not call it:
the progenitor's `build_graphical_stack` in `system_initializer` builds the same processes and
takes the same reports.

## The firmware-screen terminal

`start_screen_terminal` runs `framebuffer_driver` over a screen the firmware already set up. The
driver holds no interrupt, DMA, transport or physical address. Its aperture covers only the rows its
surface can reach (`screen_console::Aperture::span`), mapped at spawn, so it holds no name for them.

The surface is the screen divided by `ScreenConsole::scale_for_cell`'s scale, one rule shared by
both consoles, capped at what one terminal grid can lay out. The driver draws each surface pixel as
a scale-by-scale block. The terminal's cell is twice the boot console's width, so it draws at half
the boot console's scale and both show the same columns: 137 on xenon's 1920x1080, at scale one.

The aperture is capped at 2,048 pages, because its page tables come out of the driver's own
address-space budget. A larger screen, such as 2560x1440, gets the terminal over the rows that fit.

The surface is RAM shared by the driver and the terminal, contiguous like the virtio path's but
without the ring page, since the driver copies it with the CPU. The endpoints come out of a memory
region of their own, the shape `virtio_service::wire_net_server` uses. Both programs park in
`RECEIVE` for good, and reclaiming the region their endpoints live in is the only thing that wakes
them to die. The boot never tears this down. The suite does, because a service a test leaves
standing holds frames and region slots a later test needs (`user::holding`).
