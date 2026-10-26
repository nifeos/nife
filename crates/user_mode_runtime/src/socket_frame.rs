//! **The socket contract's shared frame, as a value** (milestone 410 (six copies of the
//! shared-frame accessors)): one mapped page laid out by [`socket_contract`], its header fields
//! and payload reached as methods over offsets, each bounds-checked through
//! [`MappedWindow`](crate::mapped_window::MappedWindow). Each program maps its own frame wherever
//! its image leaves room, so the type is constructed with its base. It lives here rather than in
//! `crates/socket_contract` because `MappedWindow` cannot compile for the host and that crate's
//! wire format is host-tested. See notes/user-mode-runtime.md.

//! ```no_run
//! use user_mode_runtime::socket_frame::Frame;
//!
//! // Where THIS program mapped the frame it minted and delegated. Another program's is its own.
//! const PAGE_FRAME_VA: u64 = 0x0000_0000_0140_0000;
//!
//! // SAFETY: this program's own PageFrame::MAP (or its wiring's) mapped one page read/write at
//! // PAGE_FRAME_VA before any access through this value.
//! static FRAME: Frame = unsafe { Frame::new(PAGE_FRAME_VA) };
//!
//! // Nothing below is unsafe: the bounds check is the window's.
//! FRAME.set_dst([10, 0, 2, 2], 69);
//! FRAME.payload_write(b"nife");
//! FRAME.set_len(4);
//! ```

//! Name: provisional (milestone 410's lane, 2026-10-16 UTC), module and type. `Frame` over
//! `SharedFrame`, which doubles a word `socket_contract`'s docs already put on "frame"; over
//! `PageFrame`, which names the kernel object a frame is minted from rather than the layout read
//! through it; and over holding a second `base: u64` field beside the window, which would
//! duplicate the base [`MappedWindow`](crate::mapped_window::MappedWindow) already carries and
//! give the two copies a way to disagree. The module takes its name from the contract it reads,
//! `socket_frame`, so `socket_contract::` stays the one place a reader looks for the wire.

use socket_contract::{OFF_DST_IP, OFF_DST_PORT, OFF_LEN, OFF_PAYLOAD};

use crate::mapped_window::{MappedWindow, PAGE};

/// **The shared frame as a value**: one mapped page carrying `socket_contract`'s layout.
///
/// # Safety of `Frame::new`
///
/// `MappedWindow::new`'s contract, forwarded once: `base .. base + PAGE` must be a range the
/// kernel has mapped read/write-accessible into this process for as long as the `Frame` is used.
/// One page, which is what [`socket_contract::DATA_MAX`] already assumes.
#[derive(Clone, Copy)]
pub struct Frame {
    window: MappedWindow,
}

impl Frame {
    /// The frame mapped at `base` in this program's address space.
    ///
    /// # Safety
    /// `base .. base + PAGE` must be a range the kernel has mapped read/write-accessible into
    /// this process for as long as the returned `Frame` is used: the program's own
    /// `PageFrame::MAP` succeeded first (the mint-and-map behind
    /// `socket_contract::OPERATION_ATTACH_PAGE_FRAME`), or the wiring mapped it before `_start`
    /// ran. `MappedWindow::new`'s contract, forwarded.
    pub const unsafe fn new(base: u64) -> Self {
        Self {
            // SAFETY: forwarded from this function's own contract, verbatim: `base .. base +
            // PAGE` is mapped for as long as the `Frame` is used, and one page is what
            // `socket_contract::DATA_MAX` assumes.
            window: unsafe { MappedWindow::new(base, PAGE) },
        }
    }

    /// Write the destination header: the address and port the next `SENDTO` or `CONNECT` uses,
    /// and the fields a UDP `RECEIVE` reply carries the sender's endpoint back in.
    pub fn set_dst(&self, ip: [u8; 4], port: u16) {
        for (i, &b) in ip.iter().enumerate() {
            self.window.w8(OFF_DST_IP + i as u64, b);
        }
        self.window.w16(OFF_DST_PORT, port);
    }

    /// The destination address, or the sender's after a UDP `RECEIVE` (the layout note on
    /// `OFF_DST_IP` in `socket_contract`).
    pub fn dst_ip(&self) -> [u8; 4] {
        let mut ip = [0u8; 4];
        for (i, b) in ip.iter_mut().enumerate() {
            *b = self.window.r8(OFF_DST_IP + i as u64);
        }
        ip
    }

    /// The destination port, or the sender's after a UDP `RECEIVE` (the layout note on
    /// `OFF_DST_PORT` in `socket_contract`).
    pub fn dst_port(&self) -> u16 {
        self.window.r16(OFF_DST_PORT)
    }

    /// Write the payload length: what the next `SEND*` sends, or what a `RECEIVE` reply wrote.
    pub fn set_len(&self, len: u16) {
        self.window.w16(OFF_LEN, len);
    }

    /// Write `bytes` at the start of the payload. An overlong slice panics at the window's
    /// bounds check rather than running past the frame.
    pub fn payload_write(&self, bytes: &[u8]) {
        self.window.copy_from_slice(OFF_PAYLOAD, bytes);
    }

    /// Read up to `n` bytes of payload into `out`, returning how many were read. `n` is usually
    /// a reply word's length, and a peer's idea of a length is data, so the clamp against
    /// `out.len()` is the bounds check's first line rather than the caller's job.
    pub fn payload_read(&self, n: usize, out: &mut [u8]) -> usize {
        let n = n.min(out.len());
        self.window.copy_to_slice(OFF_PAYLOAD, &mut out[..n]);
        n
    }
}
