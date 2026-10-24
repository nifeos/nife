//! The keyboard driver (the input of milestone 29 (a display terminal: framebuffer, virtio-gpu, and
//! a foreign component)): a confined userspace virtio-input driver that turns key events into the
//! bytes a terminal understands.
//!
//! `arg0` picks where the bytes go. In [`MODE_RING`] they go into the input ring it shares with the
//! compositor, and that mapping is its power to type (DECISIONS §33 (the compositor's authority is
//! memory, not messages)): the doorbell it rings is content-free, and focus is the compositor's
//! decision. In [`MODE_DIRECT`], for a boot with one terminal, it holds one fixed `CALL` capability
//! to `line_editor` and sends the UART driver's [`line_editor::proto::OPERATION_BYTES`] framing:
//! narrower authority, not looser. See notes/line-discipline.md and notes/glyphs.md.
//!
//! # BUGS
//!
//! - A used-ring entry naming a buffer this driver never posted ends it with `E_COMPLETION`, so a
//!   lying device stops the keyboard. It could do that anyway by staying silent.

//! Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-24
//! (UTC), option D, on milestone 784 (reuse what the tree wrote)'s items (c1) to (c4).
//! `virtio-drivers` assumes a driver lays out its own virtqueues, and nife's confinement does not
//! allow that: the kernel builds every ring itself, eight entries at fixed offsets, Kani-proved in
//! `direct_memory_access_validator`, so no driver can aim device DMA. See
//! design/roadmap/0784-reuse-what-the-tree-wrote/the-virtio-drivers-stay-ours.md.
//!
//! Name: ratified 2026-08-27 (calef), renamed from `kbd` (never ratified) to match the
//! `<device>_driver` shape of `block_driver` and `gpu_driver`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::proto;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::virtio::virtio_ring_barrier;
use user_mode_runtime::{call, interrupt_acknowledge, interrupt_wait, send};

/// Capability slots, by convention with `kernel/src/user/keyboard_service.rs`. Slot 0, the report
/// endpoint: how this driver tells its spawner it came up.
const REPORT: u64 = 0;
/// Slot 1, an `Irq`: the device's event interrupt.
const IRQ: u64 = 1;
/// Slot 2, a `Virtio`: the confined transport, and the only way this driver can reach the device.
const VIRTIO: u64 = 2;
/// Slot 3, held `WRITE`: the compositor's doorbell in [`MODE_RING`], `line_editor`'s served
/// endpoint in [`MODE_DIRECT`], chosen by `arg0` as `display_terminal`'s `PRESENT` slot is.
const OUT: u64 = 3;

/// `arg0`: the compositor-mediated wiring (rung two), ring-and-doorbell. The default, and the only
/// mode this driver had before milestone 177.
const MODE_RING: u64 = 0;
/// `arg0`: milestone 177's direct wiring, a fixed `CALL` to `line_editor`. See the module note.
const MODE_DIRECT: u64 = 1;

/// Where the kernel maps this driver's DMA page and the compositor's input ring.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);
/// The input ring, mapped in [`MODE_RING`] only and shared with the compositor and nobody else.
const RING_VA: u64 = address_space_map::pair_page(0x0000_0000_0082_0000);

// SAFETY: the wiring maps one page read/write at DMA_VA before this program runs (milestone 139).
const DMA: MappedWindow = unsafe { MappedWindow::new(DMA_VA, PAGE) };
// SAFETY: the wiring maps one page read/write at RING_VA, shared with the compositor, before this
// program runs.
const RING: MappedWindow = unsafe { MappedWindow::new(RING_VA, PAGE) };

// virtio-mmio register offsets. The §18 transport seam speaks this vocabulary on both buses, so a
// driver written against it runs over PCIe and over mmio without knowing which it got.
const MAGIC: u64 = 0x000;
const DEVICE_ID: u64 = 0x008;
const DEVICE_FEATURES: u64 = 0x010;
const DEVICE_FEATURES_SEL: u64 = 0x014;
const DRIVER_FEATURES: u64 = 0x020;
const DRIVER_FEATURES_SEL: u64 = 0x024;
const INTERRUPT_STATUS: u64 = 0x060;
const INTERRUPT_ACK: u64 = 0x064;
const STATUS: u64 = 0x070;

const S_ACKNOWLEDGE: u32 = 1;
const S_DRIVER: u32 = 2;
const S_DRIVER_OK: u32 = 4;
const S_FEATURES_OK: u32 = 8;
const F_VERSION_1_HI: u32 = 1; // feature bit 32
const F_ACCESS_PLATFORM_HI: u32 = 1 << 1; // feature bit 33 (behind an IOMMU)

const VIRTQ_DESC_F_WRITE: u16 = 2;

/// The virtio device type this driver will talk to. Checked, not assumed: the PCI transport
/// synthesizes the device id, and a driver that looks cannot program another device's queues.
const VIRTIO_ID_INPUT: u32 = 18;

/// The event queue. virtio-input has a second (status) queue for LEDs, which this driver never sets
/// up, so it stays inside the two-queue confinement ceiling DECISIONS §23 records.
const EVENT_Q: u64 = 0;
const QSIZE: u16 = 8;

/// The DMA page's layout: the event queue's rings at the kernel's per-queue offsets, then the event
/// buffers. All inside the one 4 KiB page the kernel granted, so the shadow-ring validator confines
/// every address the device is handed.
const EQ_DESC: u64 = 0x000;
const EQ_AVAIL: u64 = 0x080;
const EQ_USED: u64 = 0x100;
const EVENT_BASE: u64 = 0x400;

/// `struct virtio_input_event { le16 type; le16 code; le32 value; }`. Eight bytes, and the device
/// writes exactly one per buffer.
const EVENT_LEN: u64 = 8;
/// How many events can be in flight. Eight is the queue size; a keyboard produces two or three
/// events per keystroke and this drains on every interrupt, so the ring never has to be deep.
const EVENTS: usize = 8;

/// Bring-up failures, in a `0xDEAD_...` word so a failure names its step instead of hanging.
const E_MAGIC: u64 = 0x01;
const E_DEVICE_ID: u64 = 0x02;
const E_FEATURES: u64 = 0x03;
const E_QUEUE: u64 = 0x04;
const E_MODE: u64 = 0x05;
/// The device completed a buffer this driver never posted.
const E_COMPLETION: u64 = 0x06;

fn event_buf(i: usize) -> u64 {
    EVENT_BASE + i as u64 * EVENT_LEN
}

fn r16(off: u64) -> u16 {
    DMA.r16(off)
}

fn r32(off: u64) -> u32 {
    DMA.r32(off)
}

fn w16(off: u64, v: u16) {
    DMA.w16(off, v);
}

fn mr(off: u64) -> u32 {
    user_mode_runtime::virtio::read_register(VIRTIO, off) as u32
}

fn mw(off: u64, v: u32) {
    user_mode_runtime::virtio::write_register(VIRTIO, off, v as u64);
}

fn write_desc(i: u64, addr: u64, len: u32, flags: u16) {
    let b = EQ_DESC + i * 16;
    // Inside the DMA page, at the descriptor table the kernel programmed the queue with; all four
    // writes are bounds-checked by `DMA` rather than trusted by hand.
    DMA.write(b, addr);
    DMA.write(b + 8, len);
    DMA.write(b + 12, flags);
    DMA.write::<u16>(b + 14, 0);
}

fn die(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    user_mode_runtime::exit();
}

/// **Put a byte in the compositor's input ring.**
///
/// The ring is `compositor::proto::ring`: a byte buffer with a head the compositor advances and a tail
/// this driver advances. Writing the bytes *before* the tail is the whole synchronization, and the
/// fence between them is what makes it true on a weakly ordered machine (DECISIONS rule 4).
fn ring_push(tail: &mut u32, byte: u8) {
    use compositor::proto::ring;
    let at = ring::BYTES + (*tail % ring::CAPACITY) as u64;
    RING.w8(at, byte);
    *tail = tail.wrapping_add(1);
}

fn ring_publish(tail: u32) {
    use compositor::proto::ring;
    // The bytes must be visible before the tail that advertises them.
    //
    // PAIR: two readers of one contract. `take_typed` in kernel/src/user/keyboard_service.rs has the
    // matching `fence(SeqCst)`; `drain_input` in components/src/compositor.rs is the half milestone 43's
    // audit found missing (finding 7). The `call(OUT, ...)` this program makes immediately
    // after `ring_publish` orders it against the compositor anyway, because the compositor is
    // blocked in `receive_cap` on that doorbell. See notes/memory-ordering.md.
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    RING.w32(ring::TAIL, tail);
}

/// **[`MODE_DIRECT`]: send whatever is buffered as one `OPERATION_BYTES` `CALL`.** Byte for byte the
/// framing `components/src/input.rs`'s `drain` already uses to feed the same endpoint from the UART side;
/// a keyboard and a serial line are both "one input source" to `line_editor`, and neither contract
/// had to grow anything to carry the other's bytes. A no-op if nothing is buffered, so a caller need
/// not check first.
fn direct_send(buf: &[u8], n: &mut usize) {
    if *n == 0 {
        return;
    }
    let mut word: u64 = 0;
    for (i, &b) in buf[..*n].iter().enumerate() {
        word |= (b as u64) << (8 * i);
    }
    call(OUT, proto::req(proto::OPERATION_BYTES, *n as u64), word);
    *n = 0;
}

user_mode_runtime::program_entry!(start);

fn start(mode: u64, _arg1: u64, _arg2: u64) -> ! {
    if mode != MODE_RING && mode != MODE_DIRECT {
        die(E_MODE);
    }
    // Where the page is, read out of the page (milestone 600 (provisional)): the progenitor holds
    // the page as a capability and knows no physical address, so whoever built this process wrote
    // the base into its tail (`abi::virtio::DMA_PHYS_OFFSET`, past `EVENT_BASE`'s eight records).
    // SAFETY: the page is mapped read/write at `DMA_VA` before `_start` runs (the spawner's `maps`),
    // and the offset is inside it.
    let direct_memory_access_phys =
        unsafe { core::ptr::read_volatile((DMA_VA + abi::virtio::DMA_PHYS_OFFSET) as *const u64) };
    bring_up_device();
    let avail = post_event_buffers(direct_memory_access_phys);

    send(
        REPORT,
        video_terminal::status::KEYBOARD_UP,
        EVENTS as u64,
        0,
    );

    let mut events = Events {
        mode,
        keys: video_terminal::keymap::Keyboard::new(),
        seen: 0,
        avail,
        tail: 0,
        direct_buf: [0u8; 8],
        direct_n: 0,
    };
    loop {
        interrupt_wait(IRQ);

        let typed = events.drain();

        // Quiet the device, then re-enable the line at the controller. In that order: the disk
        // driver's discipline, and the reason an interrupt does not immediately re-fire.
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ); // re-enable the interrupt the kernel masked when it fired

        events.flush(typed);
    }
}

/// Check the device is a virtio-input device, negotiate features, and set up the event queue,
/// ending with `DRIVER_OK`.
fn bring_up_device() {
    if mr(MAGIC) != 0x7472_6976 {
        die(E_MAGIC);
    }
    // Check what we are talking to: a keyboard driver that skipped this would happily program a
    // disk's queues.
    if mr(DEVICE_ID) != VIRTIO_ID_INPUT {
        die(E_DEVICE_ID);
    }

    mw(STATUS, 0);
    mw(STATUS, S_ACKNOWLEDGE);
    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

    mw(DRIVER_FEATURES_SEL, 0);
    mw(DRIVER_FEATURES, 0); // virtio-input has no low-word features
    mw(DEVICE_FEATURES_SEL, 1);
    let dev_hi = mr(DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI; // behind an IOMMU, which on this bus it is
    }
    mw(DRIVER_FEATURES_SEL, 1);
    mw(DRIVER_FEATURES, ack_hi);

    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
    if mr(STATUS) & S_FEATURES_OK == 0 {
        die(E_FEATURES);
    }

    // The kernel programs the queue's ring addresses; this driver never writes a queue address
    // register, which is the §18 seam and the reason the shadow ring can be trusted.
    if user_mode_runtime::virtio::setup_queue(VIRTIO, QSIZE as u64, EVENT_Q) != 0 {
        die(E_QUEUE);
    }
    mw(
        STATUS,
        S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
    );
}

/// Post every event buffer to the device and notify it; returns the avail index after them.
fn post_event_buffers(direct_memory_access_phys: u64) -> u16 {
    // **Every buffer is device-writable, and posted before the first key can arrive.** This is the
    // receive direction: the device writes into our memory. It is the direction the DMA validator
    // grew for virtio-net (DECISIONS §23) and the direction where confinement matters most, because
    // an unbounded address here would let a device overwrite whatever it liked.
    let mut avail: u16 = 0;
    for i in 0..EVENTS {
        write_desc(
            i as u64,
            direct_memory_access_phys + event_buf(i),
            EVENT_LEN as u32,
            VIRTQ_DESC_F_WRITE,
        );
        let slot = (avail % QSIZE) as u64;
        w16(EQ_AVAIL + 4 + slot * 2, i as u16);
        avail = avail.wrapping_add(1);
    }
    virtio_ring_barrier();
    w16(EQ_AVAIL + 2, avail);
    virtio_ring_barrier();
    // The kernel validates the newly published descriptors and copies them into the shadow ring
    // the device actually reads.
    user_mode_runtime::virtio::notify(VIRTIO, EVENT_Q);
    avail
}

/// The interrupt loop's state: the keymap, both ring indices, and whichever delivery buffer
/// `mode` uses.
struct Events {
    mode: u64,
    keys: video_terminal::keymap::Keyboard,
    seen: u16,           // used-ring index already drained
    avail: u16,          // our avail-ring index
    tail: u32,           // our end of the compositor's input ring (MODE_RING)
    direct_buf: [u8; 8], // buffered bytes awaiting one OPERATION_BYTES CALL (MODE_DIRECT)
    direct_n: usize,
}

impl Events {
    /// Drain every completed event, deliver what it types, and re-post its buffer; `true` if
    /// anything was typed. Per scancode, so inlined.
    #[inline]
    fn drain(&mut self) -> bool {
        let mut typed = false;
        loop {
            let used_idx = r16(EQ_USED + 2);
            if used_idx == self.seen {
                break;
            }
            let slot = (self.seen % QSIZE) as u64;
            // used-ring element: { u32 id; u32 len }.
            let id = r32(EQ_USED + 4 + slot * 8) as usize;
            // `id` is a 32-bit value the device wrote (notes/shared-page-audit.md, finding 6): the
            // IOMMU confines where the device may touch, not what it may say, and unchecked
            // `event_buf(id)` leaves the page at `id = 462`. Every buffer below `EVENTS` is always
            // posted, so anything else names a buffer this driver never posted: the device is lying
            // about the ring, and nothing it says after that can be trusted, so the driver stops.
            if id >= EVENTS {
                die(E_COMPLETION);
            }
            let at = event_buf(id);
            let (kind, code, value) = (r16(at), r16(at + 2), r32(at + 4));
            self.seen = self.seen.wrapping_add(1);

            if let Some(bytes) = self.keys.event(kind, code, value) {
                self.deliver(bytes.as_slice());
                typed = true;
            }

            self.repost(id);
        }
        typed
    }

    /// Hand one key's bytes to whichever delivery `mode` uses. Per scancode, so inlined.
    #[inline]
    fn deliver(&mut self, bytes: &[u8]) {
        // Most keys send one byte; a cursor or navigation key sends its `CSI` sequence,
        // up to six bytes (video_terminal::keymap::Bytes). Either way both delivery modes take them one at
        // a time, in order, which is what makes a multi-byte key indistinguishable
        // downstream from several single-byte keys typed fast -- true of the ring
        // (MODE_RING) and just as true of the direct-batch buffer (MODE_DIRECT).
        for &b in bytes {
            if self.mode == MODE_DIRECT {
                self.direct_buf[self.direct_n] = b;
                self.direct_n += 1;
                if self.direct_n == self.direct_buf.len() {
                    direct_send(&self.direct_buf, &mut self.direct_n);
                }
            } else {
                ring_push(&mut self.tail, b);
            }
        }
    }

    /// Re-post buffer `id`. Its descriptor is permanent, so this is one index write. Per scancode,
    /// so inlined.
    #[inline]
    fn repost(&mut self, id: usize) {
        let aslot = (self.avail % QSIZE) as u64;
        w16(EQ_AVAIL + 4 + aslot * 2, id as u16);
        virtio_ring_barrier();
        self.avail = self.avail.wrapping_add(1);
        w16(EQ_AVAIL + 2, self.avail);
        virtio_ring_barrier();
        user_mode_runtime::virtio::notify(VIRTIO, EVENT_Q);
    }

    /// After a drain: send the direct batch, or publish the ring and ring the compositor's
    /// doorbell if anything was typed. Per interrupt, so inlined.
    #[inline]
    fn flush(&mut self, typed: bool) {
        if self.mode == MODE_DIRECT {
            // Flush whatever this drain accumulated that did not already fill a whole word: a
            // partial batch is still worth delivering promptly, the same "send what you have"
            // rule `components/src/input.rs`'s own `drain` follows for the UART side.
            direct_send(&self.direct_buf, &mut self.direct_n);
        } else if typed {
            // Publish the tail, then ring. **The doorbell carries nothing**, and that is the design:
            // what was typed is in a page the compositor maps and no client does. This is also the
            // frame that will show the keystroke, because the compositor drains the ring and then
            // rescans every client's control page before it composites (see components/src/display_terminal.rs).
            ring_publish(self.tail);
            let _ = call(OUT, compositor::proto::req(compositor::proto::COMMIT, 0), 0);
        }
    }
}

user_mode_runtime::panic_handler!();
