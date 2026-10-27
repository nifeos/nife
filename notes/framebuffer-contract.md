# The framebuffer contract

Milestone 29, rung one of the display ladder. The contract a client speaks to get pixels on a
screen, written down so rung two (the compositor, milestone 33) implements against a contract
instead of inventing one. The code half is `crates/graphics_contract`; this is the prose half, the same
split `notes/fs-server.md` makes for the filesystem and `notes/terminal-contract.md` for the
terminal. The evidence behind it is in [framebuffer-contract/](framebuffer-contract/README.md).

## The shape

```text
  virtio-gpu ──virtio (PCIe, behind the IOMMU)──► display driver ──display IPC──► client
       │                                              │  (display)                  (painter)
       └──── DMA: the whole region ──────────────────► │
                                   the surface ────── shared frames ──────────────┘
```

Three parties. The device, on the PCIe bus. The display driver (`components/src/gpu_driver.rs`), which
owns the device and serves the contract. The client (`fixtures/src/painter.rs`), which owns the
pixels and has never heard of virtio-gpu.

What each holds is the whole design:

| | display driver | client |
|---|---|---|
| slot 0 | report endpoint (status to its spawner) | report endpoint |
| slot 1 | `Interrupt`, the device's completion interrupt | the display endpoint, WRITE (it CALLs) |
| slot 2 | `Virtio`, the confined transport | |
| slot 3 | the display endpoint, READ (it serves) | |
| mapped | the whole DMA region (rings + surface) | the surface frames only |
| knows | its DMA region's physical base | no physical address at all |

The client cannot program a queue, cannot ring a doorbell, cannot see a descriptor ring (they are in
a page it is not mapped), and cannot name a physical address. So the worst a hostile client can do
is draw nonsense, which is exactly the authority a thing that draws should have.

That separation, not the picture, is why this increment exists. A compositor is the canonical
multiplexer of one device among mutually distrusting clients, and it can only be that if the thing
that draws is provably not the thing that talks to the hardware. Rung two takes the client's place
here unchanged.

## Control by message, pixels by shared frame

DECISIONS §10's division, third instance (after blk IPC and file IPC). A request is an endpoint
`CALL` carrying an opcode and a rectangle; the pixels never ride in a message. They live in frames
the kernel maps into both address spaces at spawn, which the client may write at any time without
asking anyone.

Two verbs, and that is the whole surface today:

- `INFO` -> `(0, width | height << 32)`. Ask the surface's geometry rather than assuming it. The
  crate's constants are the compile-time contract; this is the runtime one, and it is the question
  that still makes sense at rung two, where the compositor hands a client a surface whose size the
  client did not choose.
- `FLUSH(rect)` -> `(0, 0)` or a negative errno. "The pixels in this rectangle changed; get them
  onto the screen." The driver does the two device commands that means and does not reply until the
  device has completed both. The opcode and the rectangle both ride in the first word (`gfx::req`
  packs the rectangle into its low 56 bits), so a flush is one message with no second word to keep in
  step.

A rectangle outside the surface is refused, not clamped. A clamp hides a client's coordinate bug,
and the client is the only party that can tell the difference. `EINVAL` (-22), the same
negative-is-an-error convention `filesystem_contract` sets.

Why two device commands per flush, in this order: `TRANSFER_TO_HOST_2D` is what reads the
guest pixels (the device DMAs them out of the backing into its own host-side image), and
`RESOURCE_FLUSH` is what makes the host show that image. A driver that only flushed would put stale
pixels on the screen.

## The memory story: a framebuffer does not fit in a page

Every other driver here gets one 4 KiB DMA page. A 128x64 surface at 4 bytes a pixel is 32 KiB, so
the GPU needs eight more. The tempting shortcut is to let the framebuffer live outside the DMA
region: it is "pixels", after all. That would put the one device that reads bulk memory
outside the confinement everything else is inside, which is exactly backwards.

So the region is wider, not special:

```text
  page 0        rings (queue 0 at 0x000) + control request (0x400) + response (0x600)   driver-private
  pages 1..8    the surface, 32 KiB of pixels                                           shared with the client
```

`1 + graphics_contract::SURFACE_FRAMES` contiguous frames, allocated with `alloc_contiguous` and
registered whole as the driver's DMA region. Three things follow, and they are the reason this is the
right shape:

1. The shadow-ring validator bounds every descriptor to
   `[direct_memory_access_base, direct_memory_access_base + direct_memory_access_size)`, and the
   surface is inside that. `crates/direct_memory_access_validator` needed no change at all,
   because it bounds addresses against a region whose size is a parameter; the region got nine times
   bigger and the proof still covers it. (This was the one place the increment could have tempted a
   change to a proved crate, and it did not.)
2. `iommu::confine` maps exactly that region plus the kernel's shadow page, frame-granular. The
   device can reach the pixels and nothing else.
3. The client maps only pages 1..8, so it cannot touch a descriptor ring even though it shares
   memory with a driver.

The block server already took a two-page region this way for whole-block reads (milestone 32). This
is the same move, wider, and it is worth stating as a general rule: a device that needs more memory
gets a bigger grant, never an exemption.

The backing is one virtio-gpu memory entry rather than eight, because the frames are contiguous. That
is not required (the command takes a list) but it keeps the domain a single range and the entry list
trivial.

## The confinement hazard a GPU adds, and what actually stops it

This is the part worth reading twice, because it is the first device here whose DMA addresses do not
all arrive in descriptors.

Everywhere else, every address a device will touch arrives in a virtqueue descriptor. The kernel
validates each one and copies it into a shadow ring the driver physically cannot write, so the bytes
the device acts on are the bytes the kernel checked (notes/dma.md). A virtio-gpu's *backing*
addresses arrive somewhere else: inside a `RESOURCE_ATTACH_BACKING` command payload. The kernel
bounds the descriptor that carries that command, but the addresses inside it are bytes it does not
parse.

And it should not start parsing them. Teaching the transport to read virtio-gpu commands would put
device knowledge in the one place DECISIONS §18 (the PCIe transport) keeps device-neutral. It would be a per-device arms
race: the next device class with addresses in a payload would need its own parser in the kernel.

So the IOMMU is the barrier for this class of address, and that is proven, not assumed.
`the_iommu_refuses_the_gpu_a_framebuffer_outside_the_drivers_grant` gives a driver exactly the
honest driver's authority. It points the resource's backing at a frame the kernel deliberately left
out of its domain, asks the device to transfer from it, and asserts the IOMMU recorded a fault at
that frame.
Both ISAs.

Two consequences follow, both stated plainly because they matter:

- `iommu_platform=on` carries more weight for the GPU than for the disk. Drop it from the runner
  and the disk still has the shadow ring; the GPU has nothing bounding its backing. The runners say
  so at the device line.
- On a board with no IOMMU, this hazard is open. The VisionFive 2 has no IOMMU (notes/dma.md, the
  standing default), so a display driver on first silicon would be able to point the GPU at any
  physical address. On that board the display driver has to be trusted, or the transport has to grow
  a virtio-gpu-aware check, and the record should say which was chosen rather than discovering it
  later. Under HVF, likewise: PCIe DMA runs unconfined there by standing default.

Two things the escape test found on the way. The device's "OK" is not evidence: QEMU hands a
device a bounce buffer on a translation failure, so the command succeeds while the confinement holds.
The test therefore asserts on the IOMMU's fault queue. And the RISC-V IOMMU's fault queue (128
records, `FQ_LOG2 = 7`) overflows silently: the driver never clears the overflow bit, so a flood of
faults stops all later fault recording. The escape now attaches four bytes and drains the queue
after. Clearing the bit and routing faults is left for a fault-handling milestone (DECISIONS §20
(IOMMU-backed DMA isolation)).
Both are in [framebuffer-contract/iommu-escape-test.md](framebuffer-contract/iommu-escape-test.md).
## What the test proves, and the one thing it does not

`a_confined_userspace_driver_puts_a_known_pattern_in_a_framebuffer`, one arch-neutral test that runs
on both ISAs (everything under it is portable: same two binaries in both archives, same PCIe seam on
both boards, one host-tested contract crate).

Proven:

- The pattern is a per-coordinate function, not a fill: red rises with `x` five times faster than
  with `y`, green rises with `y`, blue is `x xor 2y` times an odd number. A blank buffer, a solid
  fill, a loop that never advanced, a transposed surface, a one-row stride error, and a one-pixel
  shift all fail it. `crates/graphics_contract`'s host tests assert those properties of the pattern itself,
  so the pattern's fitness is checked in milliseconds rather than trusted.
- The digest is position sensitive (FNV-1a over the pixel words in row-major order), so the same
  pixels in the wrong order is a different answer.
- Two independent witnesses in two address spaces. The client digests the surface after the
  flush through its own mapping; the driver digests it through a different mapping after the device
  reported the transfer complete. The kernel compares both against a value it computed itself from
  the contract, so neither process grades its own homework, and the two are also compared against
  each other.
- The device could reach exactly those frames and no others: the successful
  `RESOURCE_ATTACH_BACKING` happened while translation was in force, and the escape test shows an
  address outside the grant faults.
- The one-shot client is reaped, not leaked, before the test returns.

Not proven by the in-guest test: the scanout. The suite runs `-display none`, and nothing inside
the guest can read QEMU's host-side surface back, so "the bytes we handed the device are the bytes it
read out of our frames" is as far as an in-guest test reaches. A wrong pixel format or a wrong
scanout rectangle would pass it and show garbage on a real screen.

## Proving the scanout, from the host

That gap is closed, and by the host rather than the guest, because only the host can see the pixels.

QEMU's monitor works headlessly: `screendump FILE` writes a PPM of the scanout even with
`-display none` (verified against QEMU 11.0.2). So the runners take a monitor socket
(`NIFE_GPU_MON`), and `cargo xtask`'s `cargo_test_with_scanout_check` drives it while the ordinary
test run is happening. It spawns the suite, and beside it polls the monitor every 100 ms, dumps the
scanout, and compares the PPM against `graphics_contract::pixel`, the same definition the client
painted from. The first match ends the polling. Both ISAs. Every command goes over one monitor
connection and waits for QEMU's prompt, so a dump is whole when it is read. On success it prints:

```text
scanout check (aarch64): the 128x64 pattern reached the DEVICE's scanout, verified pixel for pixel
against graphics_contract::pixel (target/gpu-scanout-aarch64.ppm)
```

So the pixels are proven all the way to the device, not just to our own frames. The guest's two
witnesses agree about the framebuffer, and the host independently confirms that what QEMU is
scanning out is the pattern, pixel for pixel, in the right format, at the right geometry.

The check runs inside the existing test run, the 128x64 geometry is part of the assertion, and the
checker has a negative control that must reject a red/blue swap, a one-row shift and a single wrong
pixel. Test order is load-bearing: the confinement test resets the device, so it runs first, and a
changed order fails loudly. Those details and two practical constraints are in
[framebuffer-contract/scanout-referee.md](framebuffer-contract/scanout-referee.md).

What is still not proven, to be exact about it: that a physical panel would show this. QEMU's
scanout is the last thing we can observe, and on real hardware there is a display controller past it.
That is a silicon question (notes/target-hardware.md), not a QEMU one.

### A false lead, resolved

On 2026-08-26 a grown 1280x720 scanout looked like a QEMU resize bug. It was a guest hang: three
programs still looped over per-page capability slots that DECISIONS §102 (a Frame names a run of
pages) had removed, so the driver
died before its "UP" report. The investigation, a false lead, is in
[framebuffer-contract/scanout-referee.md](framebuffer-contract/scanout-referee.md).

## What the next clients did with this contract

Rung two's compositor (milestone 33 (a compositor)) and the display terminal (milestone 29 (a display
terminal)'s text increment) each
took `painter`'s place at this seam with exactly `painter`'s authority. `crates/graphics_contract` and
`gpu_driver` stayed byte-for-byte the same both times. A kernel stand-in for the driver made the
damage rectangle observable. The scanout check grew to three pictures over one boot. How each
prediction below came out is in [framebuffer-contract/later-rungs.md](framebuffer-contract/later-rungs.md).
## The seams left open

Deliberately not in rung one, each with the seam it will use:

- **Font rendering and a VT state engine.** ~~They arrive as a *client* of this contract~~ Done,
  2026-07-30, and as predicted: a terminal draws glyphs into a surface and calls `FLUSH`, with no
  change here. The VT engine is Rust (`crates/video_terminal`); libghostty-vt is still an open choice and
  notes/glyphs.md prices it now that there is a built engine to compare against.
- **Input.** Done, 2026-07-30. A keyboard is a second device with its own driver and its own
  capability (`components/src/keyboard_driver.rs`, virtio-input over this same PCIe transport and behind the same IOMMU
  domain), and the routing question turned out to be the compositor's exactly as predicted. What this
  note did not predict is where the *typing* authority lives: it is the input ring's mapping, not the
  driver's device, and the doorbell it rings carries nothing at all. See notes/glyphs.md.
- **Several surfaces.** One resource id today, hardwired. The contract routes by *endpoint*, so a
  compositor holding one endpoint per client surface needs a driver change and not a contract change.
- **Damage tracking.** `FLUSH` already takes a rectangle and the driver honors it (the transfer's
  offset is computed from it), so a compositor that redraws one window does not pay for the screen.
  The client today flushes the whole surface because it changed the whole surface.
- **The cursor queue.** virtio-gpu has a second virtqueue for a hardware cursor. The driver never
  sets it up, which is why the multi-queue confinement's two-queue ceiling (DECISIONS §23) is
  untouched by this milestone.

## BUGS

**Every spawner of `gpu_driver` must receive `FLUSHED`, or the driver serves exactly one flush.**
`FLUSHED` is a blocking `SEND` the driver makes from inside its serving loop, right after replying to
its first flush, and while it waits there nothing is in `RECEIVE` on the display endpoint. The three
test spawners read it because they want the digest; the boot now reads it because it must. Nothing
enforces this: `display_service::start_terminal`'s doc says so and names it a foot gun, because the
tests need the digest and the wiring function therefore cannot swallow it for them.

RESOLVED 2026-10-12 (lane/scanout-referee-flake): **the referee missed pictures because QEMU
refused its dumps.** The monitor socket serves one client with a listen backlog of one, and the
referee opened a connection for `sendkey` and another for `screendump` microseconds apart without
reading either reply. On macOS the second connect was refused while the first sat in the backlog,
so most polls took no dump; one loaded riscv64 run went 25 polls (2.6 s) without one, against a
picture held for 3 s. That is the failure seen on 2026-10-12 (HVF) and 2026-10-13 (riscv64, which
PR #2047 recorded as an unconfirmed reading). The referee now holds one connection and waits for
QEMU's prompt after each command, and an unmatched picture says whether it was ever looked for.
[framebuffer-contract/scanout-referee.md](framebuffer-contract/scanout-referee.md) has the runs.

The red report still opens "the compositor test passed" for every unmatched picture, though the
referee cannot know that: when the guest panicked first, that test may never have run. Read the
guest's verdict above the scanout lines before the scanout lines.

RESOLVED 2026-09-19 (milestone 177): the second `FLUSH` on the real boot never returned because
nobody received the driver's `FLUSHED` report. The terminal's first flush came before its `TERM_UP`,
so the driver was already blocked sending `FLUSHED` when the boot stopped listening. The keyboard had
the identical hang one step later, and `keyboard_service::start_direct` now takes its report itself.
The boot evidence is in [framebuffer-contract/flushed-hang.md](framebuffer-contract/flushed-hang.md).

## Where the pieces are

| piece | file |
|---|---|
| the contract, host-tested | `crates/graphics_contract/src/lib.rs` |
| the display driver | `components/src/gpu_driver.rs` |
| the second driver: a screen the firmware already set up (milestone 400) | `components/src/framebuffer_driver.rs`, its arithmetic in `screen_console::Aperture` |
| the client that draws | `fixtures/src/painter.rs` |
| enumeration | `kernel/src/pci.rs` (`find_gpu_device`) |
| the spawn wiring | `kernel/src/user/display_service.rs` |
| the tests | `system_tests/src/user/display_tests.rs` |
| the device lines | `helpers/qemu-runner-aarch64.sh`, `helpers/qemu-runner-riscv64.sh` (`NIFE_GPU`) |
