//! A virtio-net transport for smoltcp, at EL0, behind the multi-queue DMA confinement.
//!
//! The driver half of the net server (milestone 30 (the network stack as a confined component),
//! piece 3): it brings the NIC up through the confined `Virtio` capability, manages the receive and
//! transmit rings, and moves Ethernet frames across them for `net_stack`'s `Nic`, which presents
//! smoltcp's `phy::Device` over this NIC and the `e1000e` one. smoltcp does the TCP/IP. Everything
//! the device DMAs stays inside the one page the spawn service maps, which the kernel's validator
//! confines as it does the disk's; so the MTU is small (`MTU`), and full 1514-byte buffers cannot
//! be posted: a recorded caveat, not a bug (notes/net.md, notes/net/the-driver-and-dhcp.md).
//!
//! Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-24
//! (UTC), option D, on milestone 784 (reuse what the tree wrote)'s items (c1) to (c4).
//! `virtio-drivers` assumes a driver lays out its own virtqueues, and nife's confinement does not
//! allow that: the kernel builds every ring itself, eight entries at fixed offsets, Kani-proved in
//! `direct_memory_access_validator`, so no driver can aim device DMA. See
//! design/roadmap/0784-reuse-what-the-tree-wrote/the-virtio-drivers-stay-ours.md.
//!
//! Name: ratified 2026-10-06 (calef, "Yes", in conversation), replacing `net_transport`, itself
//! ratified 2026-08-01 (calef, milestone 63 (one spelling per thing)) replacing `vnet`. A network
//! card transport is `<device family>_transport`, as `e1000e_transport` beside it. Refused
//! `net_transport` (right while it was the only one; beside a second card's transport it no longer
//! says which), `vnet` (an abbreviation, milestone 63) and `virtio_net` (milestone 63: `crates/virtio`
//! also drives net, so the device-class name alone would collide). This file is a single-consumer
//! `#[path]` module rather than a `[[bin]]`, which is why rule 7's check leaves it alone.

//! # BUGS
//!
//! - The DMA-page layout below (`RX_DESC`..`TX_USED`, stride 0x200) restates
//!   `direct_memory_access_validator`'s ring offsets as literals, and nothing checks the copies
//!   (notes/net/the-driver-and-dhcp.md has why the constants are not consumed).

use alloc::vec;
use alloc::vec::Vec;

use user_mode_runtime::interrupt_acknowledge;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::virtio::virtio_ring_barrier;

/// The DMA page's virtual address, matching the kernel's `net_server` mapping.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

// SAFETY: the spawn service maps one page read/write at DMA_VA before this program runs
// (milestone 139; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled
// r8/r16/r32/w8/w16 and `write_desc` below -- including the stray comment milestone 112 found
// pasted onto one of them, describing `invoke` and capability validation over a plain DMA write).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(DMA_VA, PAGE) };

/// Capability slots the kernel granted, by convention.
const IRQ: u64 = 1;
const VIRTIO: u64 = 2;

// virtio-mmio register offsets (the transport seam speaks this vocabulary on both buses).
const MAGIC: u64 = 0x000;
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

/// Queue size and the two queues, matching the kernel's contract (receive = 0, transmit = 1).
const QSIZE: u16 = 8;
const RX_Q: u64 = 0;
const TX_Q: u64 = 1;

/// The 12-byte modern virtio-net header in front of every frame (`VIRTIO_F_VERSION_1` negotiated).
const NET_HDR_LEN: u64 = 12;

// The DMA-page layout: each queue's rings at the kernel's per-queue stride (RING_BLOCK = 0x200,
// desc/avail/used at +0x000/+0x080/+0x100), then the frame buffers. All within the one 4 KiB page.
const RX_DESC: u64 = 0x000;
const RX_AVAIL: u64 = 0x080;
const RX_USED: u64 = 0x100;
const TX_DESC: u64 = 0x200;
const TX_AVAIL: u64 = 0x280;
const TX_USED: u64 = 0x300;

/// How many receive and transmit buffers fit, and how big each is. Two of each keeps a spare in
/// flight; 0x2C0 bytes holds a small frame plus the virtio header.
const RX_BUFS: usize = 2;
const TX_BUFS: usize = 2;
const BUF: u64 = 0x2C0;
const BUF_BASE: u64 = 0x400;

/// The interface MTU. Small because the buffers are small (single-page DMA region); slirp's DHCP,
/// DNS, and small TCP segments fit. A recorded demonstrator caveat, not a protocol limit.
pub(crate) const MTU: usize = 576;

fn rx_buf(i: usize) -> u64 {
    BUF_BASE + i as u64 * BUF
}
fn tx_buf(i: usize) -> u64 {
    BUF_BASE + (RX_BUFS as u64 + i as u64) * BUF
}

// Raw DMA-page and device-register access. The DMA page is one contiguous physical frame mapped at
// DMA_VA, so a physical address the device needs is `direct_memory_access_phys + offset`.
// Bounds-checked against the page by `WINDOW` (milestone 139 (drive the unsafe count down))
// instead of trusted by hand at every call site.
fn r8(off: u64) -> u8 {
    WINDOW.r8(off)
}
fn r16(off: u64) -> u16 {
    WINDOW.r16(off)
}
fn r32(off: u64) -> u32 {
    WINDOW.r32(off)
}
fn w8(off: u64, v: u8) {
    WINDOW.w8(off, v);
}
fn w16(off: u64, v: u16) {
    WINDOW.w16(off, v);
}

fn mr(off: u64) -> u32 {
    user_mode_runtime::virtio::read_register(VIRTIO, off) as u32
}
fn mw(off: u64, v: u32) {
    user_mode_runtime::virtio::write_register(VIRTIO, off, v as u64);
}

fn write_desc(desc_base: u64, i: u64, addr: u64, len: u32, flags: u16, next: u16) {
    let b = desc_base + i * 16;
    WINDOW.write(b, addr);
    WINDOW.write(b + 8, len);
    WINDOW.write(b + 12, flags);
    WINDOW.write(b + 14, next);
}

/// The virtio-net device, behind `net_stack`'s `Nic`. Holds the ring bookkeeping; the buffers
/// and descriptor tables live in the DMA page at fixed offsets.
pub(crate) struct VirtioNet {
    direct_memory_access_phys: u64,
    rx_avail: u16,
    rx_seen: u16, // receive used-ring index already drained
    tx_avail: u16,
    tx_next: usize,
}

impl VirtioNet {
    /// Bring the NIC up: the modern handshake, both queues through the kernel, and every receive
    /// buffer posted so the device has somewhere to put inbound frames from the first instant.
    pub fn bring_up(direct_memory_access_phys: u64) -> VirtioNet {
        assert_eq!(mr(MAGIC), 0x7472_6976, "not a virtio device");

        mw(STATUS, 0);
        mw(STATUS, S_ACKNOWLEDGE);
        mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

        mw(DRIVER_FEATURES_SEL, 0);
        mw(DRIVER_FEATURES, 0); // no low-word net features (no MRG_RXBUF, no offloads)

        mw(DEVICE_FEATURES_SEL, 1);
        let dev_hi = mr(DEVICE_FEATURES);
        let mut ack_hi = F_VERSION_1_HI;
        if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
            ack_hi |= F_ACCESS_PLATFORM_HI; // behind an IOMMU
        }
        mw(DRIVER_FEATURES_SEL, 1);
        mw(DRIVER_FEATURES, ack_hi);

        mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
        assert!(mr(STATUS) & S_FEATURES_OK != 0, "FEATURES_OK stuck off");

        // Set up both queues through the kernel; it programs each queue's ring addresses.
        assert_eq!(
            user_mode_runtime::virtio::setup_queue(VIRTIO, QSIZE as u64, RX_Q),
            0
        );
        assert_eq!(
            user_mode_runtime::virtio::setup_queue(VIRTIO, QSIZE as u64, TX_Q),
            0
        );

        mw(
            STATUS,
            S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
        );

        let mut dev = VirtioNet {
            direct_memory_access_phys,
            rx_avail: 0,
            rx_seen: 0,
            tx_avail: 0,
            tx_next: 0,
        };

        // A permanent descriptor per receive buffer, then publish them all as available.
        for i in 0..RX_BUFS {
            write_desc(
                RX_DESC,
                i as u64,
                dev.direct_memory_access_phys + rx_buf(i),
                BUF as u32,
                VIRTQ_DESC_F_WRITE,
                0,
            );
        }
        for i in 0..RX_BUFS {
            let slot = (dev.rx_avail % QSIZE) as u64;
            w16(RX_AVAIL + 4 + slot * 2, i as u16);
            dev.rx_avail = dev.rx_avail.wrapping_add(1);
        }
        virtio_ring_barrier();
        w16(RX_AVAIL + 2, dev.rx_avail);
        virtio_ring_barrier();
        dev.notify(RX_Q);
        dev
    }

    fn notify(&self, q: u64) {
        user_mode_runtime::virtio::notify(VIRTIO, q);
    }

    /// Quiet the device's interrupt and re-enable the line at the controller, after a `WAIT`
    /// returned. The server calls this each wakeup, the disk driver's discipline.
    pub(crate) fn ack_irq(&self) {
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ); // re-enable the interrupt the kernel masked when it fired
    }

    /// Take one received frame, if the receive used ring has advanced. Copies the frame out (minus
    /// the virtio header) and re-posts the buffer so the device can fill it again.
    pub(crate) fn rx_take(&mut self) -> Option<Vec<u8>> {
        let used_idx = r16(RX_USED + 2);
        if used_idx == self.rx_seen {
            return None;
        }
        // used ring element: { u32 id; u32 len } at RX_USED + 4 + slot*8.
        let slot = (self.rx_seen % QSIZE) as u64;
        let id = r32(RX_USED + 4 + slot * 8) as usize; // descriptor head = buffer index
        let total = r32(RX_USED + 4 + slot * 8 + 4) as u64; // bytes written incl the virtio header

        // `id` and `total` are device-written and untrusted (notes/shared-page-audit.md, finding
        // 6): an unchecked `id` walks `rx_buf(id)` out of the DMA page, an unchecked `total` reads
        // past the buffer. Fail closed: drop a completion naming a buffer never posted, without
        // re-posting it (a bogus `id` does not say which), and truncate a length to the buffer.
        // notes/net/the-driver-and-dhcp.md has the worked case.
        if id >= RX_BUFS {
            self.rx_seen = self.rx_seen.wrapping_add(1);
            return None;
        }
        let frame_len = total.saturating_sub(NET_HDR_LEN).min(BUF - NET_HDR_LEN);

        let base = rx_buf(id) + NET_HDR_LEN;
        let mut v = vec![0u8; frame_len as usize];
        for (k, byte) in v.iter_mut().enumerate() {
            *byte = r8(base + k as u64);
        }

        self.rx_seen = self.rx_seen.wrapping_add(1);
        // Re-post this buffer (its descriptor is permanent) so the device reuses it.
        let aslot = (self.rx_avail % QSIZE) as u64;
        w16(RX_AVAIL + 4 + aslot * 2, id as u16);
        virtio_ring_barrier();
        self.rx_avail = self.rx_avail.wrapping_add(1);
        w16(RX_AVAIL + 2, self.rx_avail);
        virtio_ring_barrier();
        self.notify(RX_Q);
        Some(v)
    }

    /// Transmit one frame: prepend the zero virtio header, post the next transmit buffer's
    /// descriptor, and ring the device. Waits briefly for a buffer to free if all are in flight.
    pub(crate) fn tx_send(&mut self, frame: &[u8]) {
        let i = self.tx_next;
        // Do not overwrite a buffer the device has not finished sending. Under slirp this never
        // actually spins, but a bounded wait keeps a slow completion from corrupting an in-flight
        // frame. The transmit used ring advances once per consumed descriptor.
        if self.tx_avail.wrapping_sub(r16(TX_USED + 2)) >= TX_BUFS as u16 {
            for _ in 0..100_000 {
                if self.tx_avail.wrapping_sub(r16(TX_USED + 2)) < TX_BUFS as u16 {
                    break;
                }
                core::hint::spin_loop();
            }
        }

        let buf = tx_buf(i);
        for k in 0..NET_HDR_LEN {
            w8(buf + k, 0);
        }
        for (k, &byte) in frame.iter().enumerate() {
            w8(buf + NET_HDR_LEN + k as u64, byte);
        }
        write_desc(
            TX_DESC,
            i as u64,
            self.direct_memory_access_phys + buf,
            (NET_HDR_LEN + frame.len() as u64) as u32,
            0, // device reads the buffer (transmit)
            0,
        );
        let slot = (self.tx_avail % QSIZE) as u64;
        w16(TX_AVAIL + 4 + slot * 2, i as u16);
        virtio_ring_barrier();
        self.tx_avail = self.tx_avail.wrapping_add(1);
        w16(TX_AVAIL + 2, self.tx_avail);
        virtio_ring_barrier();
        self.notify(TX_Q);
        self.tx_next = (self.tx_next + 1) % TX_BUFS;
    }
}
