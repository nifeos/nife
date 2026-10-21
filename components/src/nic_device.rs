//! **The NIC `net_stack` drives**, as smoltcp's `Device`: the one enum over every transport, its
//! receive and transmit tokens, the MAC, and smoltcp's clock.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Our MAC. Locally administered; slirp routes DHCP regardless.
pub(crate) const MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

/// smoltcp's clock, from the monotonic counter, in milliseconds.
pub(crate) fn instant() -> Instant {
    let ms = (now() as u128 * 1000 / cntfrq() as u128) as i64;
    Instant::from_millis(ms)
}

/// **The NIC this server drives**: virtio-net (milestone 30 (the network stack as a confined
/// component)) or the `e1000e` family (milestone 494). One enum rather than a generic parameter,
/// so the dispatch below is compiled once.
pub(crate) enum Nic {
    Virtio(virtio_net_transport::VirtioNet),
    Gigabit(e1000e_transport::GigabitNic),
    DesignWare(designware_ethernet_transport::DesignWareNic),
}

impl Nic {
    pub(crate) fn mac(&self) -> [u8; 6] {
        match self {
            Nic::Virtio(_) => MAC,
            Nic::Gigabit(n) => n.mac(),
            Nic::DesignWare(n) => n.mac(),
        }
    }
    pub(crate) fn ack_irq(&self) {
        if let Nic::Virtio(v) = self {
            v.ack_irq();
        }
    }
    fn rx_take(&mut self) -> Option<alloc::vec::Vec<u8>> {
        let frame = match self {
            // virtio's wait is its interrupt; only a polled NIC's wait reads the stamp below.
            Nic::Virtio(v) => return v.rx_take(),
            Nic::Gigabit(n) => n.rx_take(),
            Nic::DesignWare(n) => n.rx_take(),
        };
        if frame.is_some() {
            nic_wait::frame_taken();
        }
        frame
    }
    fn tx_send(&mut self, frame: &[u8]) {
        match self {
            Nic::Virtio(v) => v.tx_send(frame),
            Nic::Gigabit(n) => n.tx_send(frame),
            Nic::DesignWare(n) => n.tx_send(frame),
        }
    }
}

impl smoltcp::phy::Device for Nic {
    type RxToken<'a> = NicRxToken;
    type TxToken<'a> = NicTxToken;

    fn receive(&mut self, _timestamp: Instant) -> Option<(NicRxToken, NicTxToken)> {
        let frame = self.rx_take()?;
        Some((NicRxToken { frame }, NicTxToken { dev: self }))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<NicTxToken> {
        Some(NicTxToken { dev: self })
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = match self {
            Nic::Virtio(_) => virtio_net_transport::MTU,
            Nic::Gigabit(_) => e1000e::MAX_FRAME,
            Nic::DesignWare(_) => designware_ethernet::MAX_FRAME,
        };
        caps
    }
}

pub(crate) struct NicRxToken {
    frame: alloc::vec::Vec<u8>,
}

impl smoltcp::phy::RxToken for NicRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.frame)
    }
}

pub(crate) struct NicTxToken {
    dev: *mut Nic,
}

impl smoltcp::phy::TxToken for NicTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = vec![0u8; len];
        let r = f(&mut buf);
        // SAFETY: single-threaded; the device outlives this token, and the `&mut self` borrow that
        // produced the token has ended (the token is an owned value). `virtio_net_transport`'s own
        // token made the same argument before this enum took its place.
        unsafe {
            (*self.dev).tx_send(&buf);
        }
        r
    }
}
