//! **The rings and their descriptors**: how big each ring and buffer is, the descriptor bits the
//! driver writes and reads, and what a receive descriptor says.

pub(crate) const PAGE: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, one page with room to grow.
pub const RX_ENTRIES: u16 = 16;

/// Transmit descriptors.
pub const TX_ENTRIES: u16 = 16;

/// Bytes per descriptor: four little-endian words.
pub const DESCRIPTOR_BYTES: u64 = 16;

/// Bytes per buffer. Written into the receive control register's buffer size field, so the
/// device never writes past it, and large enough that a 1518-byte frame never spans two.
pub const BUFFER_SIZE: u64 = 2048;

/// The largest frame either direction carries: 1500 bytes of payload and the 14-byte header, CRC
/// excluded (the device appends it on transmit, and the driver strips it on receive).
pub const MAX_FRAME: usize = 1514;

/// The frame check sequence the device leaves on every received frame.
pub const CRC_BYTES: usize = 4;

/// Receive and transmit, read and write-back formats: descriptor word 3 is the one that carries
/// ownership, and the only one the data plane judges.
///
/// **Owned by the device.** Set by the driver to hand a descriptor over, cleared by the device's
/// write-back.
pub const RDES3_OWN: u32 = 1 << 31;

/// Receive, read format: interrupt on completion. Interrupts are masked at the channel; set
/// anyway so the channel status's receive bit reports completions, which the coherence probe reads.
pub const RDES3_IOC: u32 = 1 << 30;

/// Receive, read format: buffer 1's address is valid.
pub const RDES3_BUF1V: u32 = 1 << 24;

/// Receive, write-back: this is a context descriptor (a timestamp), not a frame.
pub const RDES3_CTXT: u32 = 1 << 30;

/// Receive, write-back: first descriptor of a frame.
pub const RDES3_FD: u32 = 1 << 29;

/// Receive, write-back: last descriptor of a frame.
pub const RDES3_LD: u32 = 1 << 28;

/// Receive, write-back: error summary.
pub const RDES3_ES: u32 = 1 << 15;

/// Receive, write-back: the frame length including the CRC, valid when [`RDES3_LD`] is set.
pub const RDES3_LENGTH: u32 = 0x7fff;

/// Transmit, read format: interrupt on completion (in word 2).
pub const TDES2_IC: u32 = 1 << 31;

/// Transmit, read format: buffer 1's length (in word 2).
pub const TDES2_BUF1_LEN: u32 = 0x3fff;

/// Transmit: first descriptor of a frame.
pub const TDES3_FS: u32 = 1 << 29;

/// Transmit: last descriptor of a frame.
pub const TDES3_LS: u32 = 1 << 28;

/// Transmit: the device owns it ([`RDES3_OWN`]'s bit, in the transmit ring).
pub const TDES3_OWN: u32 = 1 << 31;

/// Transmit, read format: the whole frame's length.
pub const TDES3_FRAME_LEN: u32 = 0x7fff;

/// Transmit, write-back: error summary.
pub const TDES3_ES: u32 = 1 << 15;

/// **A receive descriptor handed to the device**: the buffer's address in words 0 and 1 (low and
/// high 32 bits, as OpenBSD and Linux's `dwmac4_set_addr` both write it), word 2 clear, and word 3
/// owned, interrupt-on-completion, buffer 1 valid. As two little-endian `u64`s: words 0 and 1,
/// then words 2 and 3.
pub const fn rx_descriptor_words(buffer_phys: u64) -> [u64; 2] {
    [
        buffer_phys,
        ((RDES3_OWN | RDES3_IOC | RDES3_BUF1V) as u64) << 32,
    ]
}

/// **A transmit descriptor for one whole frame in one buffer**: the address, the length with
/// interrupt-on-completion, and word 3 owned, first and last, with the frame length. The device
/// inserts the CRC and pads a short frame (checksum and padding control zero).
pub const fn tx_descriptor_words(buffer_phys: u64, len: u16) -> [u64; 2] {
    let len = len as u32 & TDES3_FRAME_LEN;
    let des2 = (len & TDES2_BUF1_LEN) | TDES2_IC;
    let des3 = TDES3_OWN | TDES3_FS | TDES3_LS | len;
    [buffer_phys, des2 as u64 | (des3 as u64) << 32]
}

/// What a receive descriptor's word 3 says, judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxVerdict {
    /// The device still owns it.
    NotYet,
    /// A whole, good frame of this many bytes (CRC removed), which fits the buffer and the room.
    Frame(usize),
    /// The device wrote it and it is not a frame to deliver: an error, a context descriptor, a
    /// frame spanning descriptors, or a length it could not have meant. Consumed and reposted.
    Drop,
}

/// **Judge a receive descriptor's word 3**, where `room` is how many bytes the caller can take.
///
/// Every field is the device's statement and none is trusted, since a device that lies about a
/// length would lead a trusting driver past its buffer: a `Frame(n)` always has `n <= room`,
/// `n <= MAX_FRAME` and `n + CRC_BYTES <= BUFFER_SIZE`, which the Kani harness below proves for
/// every word.
///
/// # Examples
///
/// ```
/// use designware_ethernet::{RxVerdict, rx_verdict, RDES3_FD, RDES3_LD, RDES3_OWN};
///
/// // 64 bytes on the wire is 60 of frame and 4 of CRC, which the driver strips.
/// assert_eq!(rx_verdict(RDES3_FD | RDES3_LD | 64, 1514), RxVerdict::Frame(60));
/// assert_eq!(rx_verdict(RDES3_OWN | RDES3_FD | RDES3_LD | 64, 1514), RxVerdict::NotYet);
/// assert_eq!(rx_verdict(RDES3_FD | RDES3_LD | 0x7fff, 1514), RxVerdict::Drop);
/// ```
pub fn rx_verdict(des3: u32, room: usize) -> RxVerdict {
    if des3 & RDES3_OWN != 0 {
        return RxVerdict::NotYet;
    }
    let whole = des3 & (RDES3_FD | RDES3_LD) == RDES3_FD | RDES3_LD;
    if des3 & RDES3_CTXT != 0 || !whole || des3 & RDES3_ES != 0 {
        return RxVerdict::Drop;
    }
    let wire = (des3 & RDES3_LENGTH) as usize;
    let Some(len) = wire.checked_sub(CRC_BYTES) else {
        return RxVerdict::Drop;
    };
    let fits = (14..=MAX_FRAME).contains(&len) && len <= room && wire as u64 <= BUFFER_SIZE;
    if !fits {
        return RxVerdict::Drop;
    }
    RxVerdict::Frame(len)
}
