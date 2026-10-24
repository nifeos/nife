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
//! The device is driven through `vendor/virtio-drivers` (milestone 947): the crate's `Transport`
//! status dance and its `VirtQueue`, over the kernel's confined transport and the one DMA page,
//! by way of `user_mode_runtime::confined_virtio`. What stays here is nife's: the keymap, the two
//! delivery modes, the bring-up codes and the check on a device-written buffer index.
//!
//! Reuse: `virtio-drivers` 0.13.0 plus rcore-os/virtio-drivers#232 for the queue and the status
//! dance (calef, #2586, 2026-10-22 UTC). Its `VirtIOInput` is not used: it fixes 32-entry queues,
//! four times what the confinement drives, and boxes its buffers in a heap this program does not
//! have. See design/roadmap/0947-virtio-input-on-virtio-drivers.md.
//!
//! # BUGS
//!
//! - A used-ring entry naming a buffer this driver never posted ends it with `E_COMPLETION`. The
//!   crate cannot step past a used entry it will not pop, so dropping the entry and going on is not
//!   available; a lying device stops the keyboard, which it could do anyway by staying silent.

//! Name: ratified 2026-08-27 (calef), renamed from `kbd` (never ratified) to match the
//! `<device>_driver` shape of `block_driver` and `gpu_driver`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::proto;
use user_mode_runtime::confined_virtio::virtio_drivers::device::common::Feature;
use user_mode_runtime::confined_virtio::virtio_drivers::queue::VirtQueue;
use user_mode_runtime::confined_virtio::virtio_drivers::transport::{
    DeviceStatus, DeviceType, Transport,
};
use user_mode_runtime::confined_virtio::{self, ConfinedTransport, QUEUE_SIZE, Refusal, WindowHal};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
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

/// The event queue. virtio-input has a second (status) queue for LEDs, which this driver never sets
/// up, so it stays inside the two-queue confinement ceiling DECISIONS §23 records.
const EVENT_Q: u16 = 0;

/// The event queue, as the crate drives it: its rings at the kernel's offsets in the DMA page.
type EventQueue = VirtQueue<WindowHal, QUEUE_SIZE>;

/// Where the event buffers start in the DMA page, past queue 0's ring block (which
/// `confined_virtio::WindowHal` fills) and below the physical-base word at the page's end. Inside
/// the one 4 KiB page the kernel granted, so the shadow-ring validator confines every address the
/// device is handed.
const EVENT_BASE: u64 = 0x400;

/// `struct virtio_input_event { le16 type; le16 code; le32 value; }`. Eight bytes, and the device
/// writes exactly one per buffer.
const EVENT_LEN: u64 = 8;
/// How many events can be in flight: one buffer per queue entry. A keyboard produces two or three
/// events per keystroke and this drains on every interrupt, so the ring never has to be deep.
const EVENTS: usize = QUEUE_SIZE;

/// Bring-up failures, in a `0xDEAD_...` word so a failure names its step instead of hanging.
const E_MAGIC: u64 = 0x01;
const E_DEVICE_ID: u64 = 0x02;
const E_FEATURES: u64 = 0x03;
const E_QUEUE: u64 = 0x04;
const E_MODE: u64 = 0x05;
/// The device completed a buffer this driver never posted (see the module's BUGS).
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

/// Event buffer `i` as the bytes the crate hands the device.
fn event_slot(i: usize) -> &'static mut [u8] {
    // SAFETY: `event_buf(i)` for `i < EVENTS` is inside the DMA page mapped read/write at `DMA_VA`
    // (`EVENTS * EVENT_LEN` bytes from 0x400 end below the physical-base word), this program is one
    // thread, and each slice lives only for the one `add` or `pop_used` call it is passed to.
    unsafe { MappedWindow::new(DMA_VA + event_buf(i), EVENT_LEN).as_mut_slice() }
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

// Through the runtime's entry, because `VirtQueue` holds 16-byte-aligned descriptors and a plain
// `_start` runs eight bytes off on x86_64.
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
    confined_virtio::install_window(DMA_VA, direct_memory_access_phys, PAGE);
    let (transport, queue) = bring_up_device();

    send(
        REPORT,
        video_terminal::status::KEYBOARD_UP,
        EVENTS as u64,
        0,
    );

    let mut events = Events {
        mode,
        keys: video_terminal::keymap::Keyboard::new(),
        transport,
        queue,
        tail: 0,
        direct_buf: [0u8; 8],
        direct_n: 0,
    };
    loop {
        interrupt_wait(IRQ);

        let typed = events.drain();

        // Quiet the device, then re-enable the line at the controller. In that order: the disk
        // driver's discipline, and the reason an interrupt does not immediately re-fire.
        events.transport.ack_interrupt();
        interrupt_acknowledge(IRQ); // re-enable the interrupt the kernel masked when it fired

        events.flush(typed);
    }
}

/// Check the device is a virtio-input device, negotiate features, set up the event queue, end
/// with `DRIVER_OK`, then post every event buffer.
fn bring_up_device() -> (ConfinedTransport, EventQueue) {
    // Check what we are talking to: a keyboard driver that skipped this would happily program a
    // disk's queues. The PCI transport synthesizes the device id, so it is checked, not assumed.
    let mut transport = match ConfinedTransport::new(VIRTIO) {
        Ok(t) => t,
        Err(Refusal::NotVirtio) => die(E_MAGIC),
        Err(_) => die(E_DEVICE_ID),
    };
    if transport.device_type() != DeviceType::Input {
        die(E_DEVICE_ID);
    }

    // virtio-input has no feature of its own to accept. ACCESS_PLATFORM is offered behind an
    // IOMMU, which on this bus it is, and a device that offers it refuses FEATURES_OK without it.
    let features = transport.begin_init(Feature::VERSION_1 | Feature::ACCESS_PLATFORM);
    // The crate writes FEATURES_OK and never reads it back (rcore-os/virtio-drivers at 0.13.0).
    if !transport.get_status().contains(DeviceStatus::FEATURES_OK) {
        die(E_FEATURES);
    }

    // The kernel programs the queue's ring addresses; `ConfinedTransport::queue_set` only checks
    // that the crate put its rings where the kernel will point the device.
    let Ok(mut queue) = EventQueue::new(
        &mut transport,
        EVENT_Q,
        false,
        false,
        features.contains(Feature::ACCESS_PLATFORM),
    ) else {
        die(E_QUEUE);
    };
    if transport.refusal().is_some() {
        die(E_QUEUE);
    }
    transport.finish_init();

    // **Every buffer is device-writable, and posted before the first key can arrive.** This is the
    // receive direction: the device writes into our memory. It is the direction the DMA validator
    // grew for virtio-net (DECISIONS §23) and the direction where confinement matters most, because
    // an unbounded address here would let a device overwrite whatever it liked. Buffer `i` gets
    // token `i`, because the crate's free list starts in order.
    for i in 0..EVENTS {
        // SAFETY: the buffer is in the DMA page and stays there, untouched by this program, until
        // the device returns it under this token and `drain` pops it.
        if unsafe { queue.add(&[], &mut [event_slot(i)]) } != Ok(i as u16) {
            die(E_QUEUE);
        }
    }
    // The kernel validates the newly published descriptors and copies them into the shadow ring
    // the device actually reads.
    if queue.should_notify() {
        transport.notify(EVENT_Q);
    }
    if transport.refusal().is_some() {
        die(E_QUEUE);
    }
    (transport, queue)
}

/// The interrupt loop's state: the keymap, both ring indices, and whichever delivery buffer
/// `mode` uses.
struct Events {
    mode: u64,
    keys: video_terminal::keymap::Keyboard,
    transport: ConfinedTransport,
    queue: EventQueue,
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
        while let Some(token) = self.queue.peek_used() {
            // `token` is a value the device wrote (notes/shared-page-audit.md, finding 6): the
            // IOMMU confines where the device may touch, not what it may say. Every buffer is
            // always posted, so any token below `EVENTS` names one; anything else is a lie the
            // crate cannot step past.
            let id = token as usize;
            if id >= EVENTS {
                die(E_COMPLETION);
            }
            // SAFETY: `event_slot(id)` is the buffer `add` posted under this token.
            if unsafe { self.queue.pop_used(token, &[], &mut [event_slot(id)]) }.is_err() {
                die(E_COMPLETION);
            }
            let at = event_buf(id);
            let (kind, code, value) = (r16(at), r16(at + 2), r32(at + 4));

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

    /// Re-post buffer `id` and notify. Its descriptor was just freed by `pop_used`, and the
    /// crate's free list hands back the head it was given, so the token stays `id`. Per scancode,
    /// so inlined.
    #[inline]
    fn repost(&mut self, id: usize) {
        // SAFETY: as in `bring_up_device`: the buffer is in the DMA page and untouched until the
        // device returns it.
        if unsafe { self.queue.add(&[], &mut [event_slot(id)]) } != Ok(id as u16) {
            die(E_QUEUE);
        }
        if self.queue.should_notify() {
            self.transport.notify(EVENT_Q);
        }
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
