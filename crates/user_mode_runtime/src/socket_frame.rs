//! **The socket contract's shared frame, as a value** (milestone 410 (six copies of the
//! shared-frame accessors)): one mapped page laid out by [`socket_contract`], its header fields
//! and payload reached as methods over offsets, each bounds-checked through
//! [`MappedWindow`]. Each program maps its own frame wherever
//! its image leaves room, so the type is constructed with its base. It lives here rather than in
//! `crates/socket_contract` because a contract crate never links the runtime: the dependency runs
//! from this crate to that one. See notes/user-mode-runtime.md.

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
//! FRAME.set_destination([10, 0, 2, 2], 69);
//! FRAME.payload_write(b"nife");
//! // The length of a send rides in the CALL's argument word, never in the frame.
//! ```

//! Name: provisional (milestone 410's lane, 2026-10-16 UTC), module and type. `Frame` over
//! `SharedFrame`, which doubles a word `socket_contract`'s docs already put on "frame"; over
//! `PageFrame`, which names the kernel object a frame is minted from rather than the layout read
//! through it; and over holding a second `base: u64` field beside the window, which would
//! duplicate the base [`MappedWindow`] already carries and
//! give the two copies a way to disagree. The module takes its name from the contract it reads,
//! `socket_frame`, so `socket_contract::` stays the one place a reader looks for the wire.

use socket_contract::{
    DESTINATION_IP_ADDRESS_OFFSET_BYTES, DESTINATION_PORT_OFFSET_BYTES, MAXIMUM_PAYLOAD_BYTES,
    PAYLOAD_LENGTH_OFFSET_BYTES, PAYLOAD_OFFSET_BYTES,
};

use crate::mapped_window::{MappedWindow, PAGE_BYTES};

/// **The shared frame as a value**: one mapped page carrying `socket_contract`'s layout.
///
/// # Safety of `Frame::new`
///
/// `MappedWindow::new`'s contract, forwarded once: `base .. base + PAGE_BYTES` must be a range the
/// kernel has mapped read/write-accessible into this process for as long as the `Frame` is used.
/// One page, which is what [`socket_contract::MAXIMUM_PAYLOAD_BYTES`] already assumes.
#[derive(Clone, Copy)]
pub struct Frame {
    window: MappedWindow,
}

impl Frame {
    /// The frame mapped at `base` in this program's address space.
    ///
    /// # Safety
    /// `base .. base + PAGE_BYTES` must be a range the kernel has mapped read/write-accessible into
    /// this process for as long as the returned `Frame` is used: the program's own
    /// `PageFrame::MAP` succeeded first (the mint-and-map behind
    /// `socket_contract::OPERATION_ATTACH_SHARED_PAGE`), or the wiring mapped it before `_start`
    /// ran. `MappedWindow::new`'s contract, forwarded.
    pub const unsafe fn new(base: u64) -> Self {
        Self {
            // SAFETY: forwarded from this function's own contract, verbatim: `base .. base +
            // PAGE_BYTES` is mapped for as long as the `Frame` is used, and one page is what
            // `socket_contract::MAXIMUM_PAYLOAD_BYTES` assumes.
            window: unsafe { MappedWindow::new(base, PAGE_BYTES) },
        }
    }

    /// Write the destination header: the address and port the next `SENDTO` or `CONNECT` uses,
    /// and the fields a UDP `RECEIVE` reply carries the sender's endpoint back in.
    ///
    /// Name: provisional, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
    pub fn set_destination(&self, ip: [u8; 4], port: u16) {
        for (i, &b) in ip.iter().enumerate() {
            self.window
                .write_u8(DESTINATION_IP_ADDRESS_OFFSET_BYTES + i as u64, b);
        }
        self.window.write_u16(DESTINATION_PORT_OFFSET_BYTES, port);
    }

    /// The destination address, or the sender's after a UDP `RECEIVE` (the layout note on
    /// `DESTINATION_IP_ADDRESS_OFFSET_BYTES` in `socket_contract`).
    ///
    /// Name: provisional, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
    pub fn destination_network_address(&self) -> [u8; 4] {
        let mut ip = [0u8; 4];
        for (i, b) in ip.iter_mut().enumerate() {
            *b = self
                .window
                .read_u8(DESTINATION_IP_ADDRESS_OFFSET_BYTES + i as u64);
        }
        ip
    }

    /// The destination port, or the sender's after a UDP `RECEIVE` (the layout note on
    /// `DESTINATION_PORT_OFFSET_BYTES` in `socket_contract`).
    ///
    /// Name: provisional, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
    pub fn destination_port(&self) -> u16 {
        self.window.read_u16(DESTINATION_PORT_OFFSET_BYTES)
    }

    /// Read the length field at `PAYLOAD_LENGTH_OFFSET_BYTES`: what the last `RECEIVE` wrote, or what [`set_len`](Self::set_len)
    /// cleared it to, so a client can watch for the reply.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes, for
    /// std's `len`.
    #[allow(
        clippy::len_without_is_empty,
        reason = "a header field, not a collection"
    )]
    pub fn len(&self) -> u16 {
        self.window.read_u16(PAYLOAD_LENGTH_OFFSET_BYTES)
    }

    /// Write the length field at `PAYLOAD_LENGTH_OFFSET_BYTES`. The stack never reads it: a `SEND*` takes its length
    /// from the `CALL` argument word, and only `RECEIVE` writes the field, so this is for a client
    /// that clears it before a `RECEIVE` to watch for the reply.
    pub fn set_len(&self, len: u16) {
        self.window.write_u16(PAYLOAD_LENGTH_OFFSET_BYTES, len);
    }

    /// Write `bytes` at the start of the payload. An overlong slice panics at the window's
    /// bounds check rather than running past the frame.
    pub fn payload_write(&self, bytes: &[u8]) {
        self.window.copy_from_slice(PAYLOAD_OFFSET_BYTES, bytes);
    }

    /// Read up to `n` bytes of payload into `out`, returning how many were read. `n` is usually
    /// a reply word's length, and a peer's idea of a length is data, so the clamp against
    /// `out.len()` is the bounds check's first line rather than the caller's job.
    pub fn payload_read(&self, n: usize, out: &mut [u8]) -> usize {
        let n = n.min(out.len()).min(MAXIMUM_PAYLOAD_BYTES);
        self.window
            .copy_to_slice(PAYLOAD_OFFSET_BYTES, &mut out[..n]);
        n
    }
}

#[cfg(test)]
mod tests {
    use socket_contract::MAXIMUM_PAYLOAD_BYTES;

    use super::Frame;

    #[repr(align(4096))]
    struct Page([u8; 4096]);

    fn frame(page: &mut Page) -> Frame {
        // SAFETY: `page` is live for the test and is one page of writable memory.
        unsafe { Frame::new(page.0.as_mut_ptr() as u64) }
    }

    #[test]
    fn the_destination_and_length_round_trip() {
        let mut page = Page([0; 4096]);
        let f = frame(&mut page);
        f.set_destination([10, 0, 2, 2], 69);
        f.set_len(512);
        assert_eq!(f.destination_network_address(), [10, 0, 2, 2]);
        assert_eq!(f.destination_port(), 69);
        assert_eq!(f.len(), 512);
    }

    #[test]
    fn a_payload_read_is_clamped_to_the_buffer_and_the_frame() {
        let mut page = Page([0; 4096]);
        let f = frame(&mut page);
        f.payload_write(&[9, 8, 7]);
        let mut small = [0u8; 2];
        assert_eq!(f.payload_read(100, &mut small), 2);
        assert_eq!(small, [9, 8]);
        let mut big = [0u8; 8192];
        assert_eq!(f.payload_read(usize::MAX, &mut big), MAXIMUM_PAYLOAD_BYTES);
    }
}
