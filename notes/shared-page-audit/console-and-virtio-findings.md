# Shared-page audit: findings 3 and 6, the console and two virtio drivers

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/shared-page-audit.md](../shared-page-audit.md), the tree's second security
audit (2026-08-04). The parent states the lens and its four questions: (a) the window, (b) what can
land in it, (c) the corrupted state, (d) whether it is reachable. Its honest summary has every
finding's disposition.

## 3. The console server takes an unbounded byte count from its client

(a) The window. `components/src/console.rs`:

```rust
let (len, _, _) = receive(REQUEST);
let shared = SHARED_VA as *const u8;
for i in 0..len {
    let byte = unsafe { core::ptr::read_volatile(shared.add(i as usize)) };
    uart_put(byte);
}
```

`len` is the first register word with no clamp. The shared mapping is exactly one frame
(`kernel/src/user/console_service.rs`, one `alloc()`, mapped `Flags::user_rodata()`).

(b) What can land in it. Nothing needs to: this is not a race. Any holder of `WRITE` on the
request endpoint sends `len = u64::MAX`.

(c) The corrupted state. The server reads off the end of its own mapping and is killed by the
fault. The console is a shared service, so its death takes out every other client's output too.

(d) Reachable? Yes, trivially, by any client of this contract. What bounds the damage is that
this wiring is milestone 19f's test console; the interactive system's terminal is `line_editor`,
which does clamp (`.min(PAGE)` at every one of its four length sites).

The finding worth keeping is not the missing clamp but the reason recorded next to it. The
`SAFETY` comment says:

> A malicious length is a read out of our OWN mapping, which faults us, not the kernel: a driver bug
> is a crashed process.

The length is not the driver's; it is the client's. The comment classifies a client-triggered
kill of a shared server as a driver bug, and a reader who trusted it would carry that reasoning to a
contract where it is not merely a test program. This is the same failure the arch audit's finding 1
was about: the code was defensible and the record was wrong.

Disposition: fixed in this lane. The count is clamped to the page, matching `line_editor`, and
the comment now says who supplies the length and what the clamp is for.

## 6. Two userspace virtio drivers trust the index the device writes into the used ring

Not a double fetch. It is what the enumeration the lens required turned up: to ask "is this value
checked twice" you must first list every value read from a page a hostile party writes, and two of
those values are not checked at all.

(a) The window. `components/src/virtio_net_transport.rs`'s `rx_take` and `components/src/keyboard_driver.rs`'s drain loop both
take a used-ring element and use its 32-bit `id` as a buffer index:

```rust
let id = r32(RX_USED + 4 + slot * 8) as usize;    // descriptor head = buffer index
let total = r32(RX_USED + 4 + slot * 8 + 4) as u64;
let base = rx_buf(id) + NET_HDR_LEN;              // rx_buf(i) = 0x400 + i * 0x2C0
```

Neither `id` nor `total` is bounded by anything.

(b) What can land in it. The device, and it needs no race: it simply writes a number. The
used ring lives inside the driver's own single-page DMA region, which the device is entitled to
write; that is what a used ring is for.

This is not covered by the DMA confinement, and the reason is worth stating because it is easy
to assume otherwise. `crates/dma_validator` validates the driver to device direction: at
`NOTIFY` it checks that every descriptor the device could follow lies inside the granted region, and
copies the validated descriptors into a kernel-private shadow the driver cannot touch afterwards.
Its own module comment says the shadow is what makes the check hold under time-of-check to
time-of-use. All of that is about where the device may touch. Nothing anywhere says what the
device may say on the way back, and the driver believes it.

(c) The corrupted state. `id = 4` is already past the one-page region. `id` near 1.5 million puts
`base` at this process's heap (`memory_region_heap`'s `DEFAULT_BASE`), which is an ordinary `u32`, so the
network driver copies its own heap into a frame and hands it to smoltcp, which may put it on the
wire. `total` unbounded reads past the buffer and asks a 96-page heap for up to 4 GiB. In `kbd.rs`
the same index leaves the region at `id = 462` and returns process memory as a keystroke.

(d) Reachable? By a device, yes, and by nothing else. The threat model is the question, and this
project's answer is already on the record: DECISIONS §20, §23 and §30 exist because the device is
not trusted, which is why there is an IOMMU and a validator at all. A driver that trusts the
device's own accounting contradicts the thesis those milestones establish. Under QEMU with slirp
nothing lies; on the VisionFive 2, or behind any device the host does not fully own, the assumption
is doing real work and is written down nowhere.

The other two drivers show this is an omission rather than a policy: `entropy.rs` clamps its length
(`.min(POOL_LEN)`), and `display.rs` and `crates/virtio` read only `used.idx` and never the element,
because they use one fixed buffer.

Disposition: fixed in this lane, both drivers, failing closed: a completion naming a buffer that
was never posted is consumed and dropped, and a length larger than a buffer is truncated to it. What
that costs is one receive buffer per lie, which is the right trade against a device that has stopped
being a network card.

And what is not proven, said plainly: nothing in the suite exercises a lying device, so the
existing tests show the honest path is unchanged and nothing shows the hostile path is now safe. A
harness that can make a virtio device misbehave is a piece of work in its own right and is proposed
in the parent's "What wants a lane" (case B).
